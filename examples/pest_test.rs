#![recursion_limit="2000"]
#![feature(drop_types_in_const)]

#[macro_use]
extern crate pest;
extern crate getopts;
extern crate pbrt;

use pbrt::{Float, Matrix4x4, Point3f, RenderOptions, Transform, TransformSet, Vector3f};
// parser
use pest::prelude::*;
// getopts
use getopts::Options;
// std
use std::str::FromStr;
use std::collections::LinkedList;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

static mut CUR_TRANSFORM: TransformSet = TransformSet {
    t: [Transform {
        m: Matrix4x4 {
            m: [[1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0]],
        },
        m_inv: Matrix4x4 {
            m: [[1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0]],
        },
    }; 2],
};
static mut RENDER_OPTIONS: Option<Box<RenderOptions>> = None;

#[derive(Debug, PartialEq)]
pub enum Node {
    Statement(LinkedList<Node>),
    LastStatement,
}

impl_rdp! {
    grammar! {
        pbrt = _{ statement* ~ last_statement }
        statement = { look_at | rotate | named_statement | keyword }
        named_statement = { camera |
                            pixel_filter |
                            sampler |
                            film |
                            coord_sys_transform |
                            light_source |
                            texture |
                            material |
                            shape }
        parameter = { float_param |
                      string_param |
                      integer_param |
                      point_param |
                      rgb_param |
                      spectrum_param |
                      texture_param }
        float_param = { ["\"float"] ~ ident ~ ["\""] ~ lbrack ~ number ~ rbrack }
        string_param = { ["\"string"] ~ ident ~ ["\""] ~ lbrack ~ string ~ rbrack }
        integer_param = { ["\"integer"] ~ ident ~ ["\""] ~ lbrack ~ integer ~ rbrack }
        point_param = { ["\"point"] ~ ident ~ ["\""] ~ lbrack ~ number ~ number ~ number ~ rbrack }
        rgb_param = { ["\"rgb"] ~ ident ~ ["\""] ~ lbrack ~ number ~ number ~ number ~ rbrack }
        spectrum_param = { ["\"spectrum\""] ~ string }
        texture_param = { ["\"texture"] ~ ident ~ ["\""] ~ string }
        // Rotate angle x y z
        rotate = { ["Rotate"] ~
                   // followed by 4 numbers:
                   number ~ number ~ number ~ number
        }
        // LookAt eye_x eye_y eye_z look_x look_y look_z up_x up_y up_z
        look_at = { ["LookAt"] ~
                    // followed by 9 numbers:

                    // eye_x eye_y eye_z
                    number ~ number ~ number ~
                    // look_x look_y look_z
                    number ~ number ~ number ~
                    // up_x up_y up_z
                    number ~ number ~ number
        }
        // Camera "perspective" "float fov" [90] ...
        camera = { ["Camera"] ~ string ~ parameter* }
        // PixelFilter "mitchell" "float xwidth" [2] "float ywidth" [2]
        pixel_filter = { ["PixelFilter"] ~ string ~ parameter* }
        // Sampler "halton"
        sampler = { ["Sampler"] ~ string ~ parameter* }
        // Film "image" "string filename" ["..."] ...
        film = { ["Film"] ~ string ~ parameter* }
        // CoordSysTransform "camera"
        coord_sys_transform = { ["CoordSysTransform"] ~ string }
        // LightSource "point" "rgb I" [ .5 .5 .5 ]
        light_source = { ["LightSource"] ~ string ~ parameter* }
        // Texture "mydiffuse" "spectrum" "imagemap" "string filename" "image.tga"
        texture = { ["Texture"] ~ string ~ parameter* }
        // Material "matte" "texture Kd" "mydiffuse"
        material = { ["Material"] ~ string ~ parameter* }
        // Shape "sphere" "float radius" [0.25]
        shape = { ["Shape"] ~ string ~ parameter* }
        // keywords
        keyword = {
            (["Accelerator"] |
             ["ActiveTransform"] |
             ["All"] |
             ["AreaLightSource"] |
             attribute_begin |
             attribute_end |
             ["ConcatTransform"] |
             ["CoordinateSystem"] |
             ["CoordSysTransform"] |
             ["EndTime"] |
             ["Identity"] |
             ["Include"] |
             ["MakeNamedMedium"] |
             ["MakeNamedMaterial"] |
             ["MediumInterface"] |
             ["NamedMaterial"] |
             ["ObjectBegin"] |
             ["ObjectEnd"] |
             ["ObjectInstance"] |
             ["ReverseOrientation"] |
             ["Scale"] |
             ["StartTime"] |
             ["Integrator"] |
             ["TransformBegin"] |
             ["TransformEnd"] |
             ["TransformTimes"] |
             ["Transform"] |
             ["Translate"] |
             world_begin
            )
        }
        attribute_begin = { ["AttributeBegin"] }
        attribute_end = { ["AttributeEnd"] }
        world_begin = { ["WorldBegin"] }
        // IDENT [a-zA-Z_][a-zA-Z_0-9]*
        ident =  { (['a'..'z'] | ['A'..'Z'] | ["_"]) ~
                   (['a'..'z'] | ['A'..'Z'] | ["_"] | ['0'..'9'])* }
        string = { (["\""] ~ ident ~ ["\""]) | (["\""] ~ filename ~ ["\""]) }
        filename = { (['a'..'z'] | ['A'..'Z'] | ["_"]) ~ // TODO: can be a full path
                     (['a'..'z'] | ['A'..'Z'] | ["_"] | ["."] | ['0'..'9'])* }
        // "[" { return LBRACK; }
        lbrack = { ["["] }
        // "]" { return RBRACK; }
        rbrack = { ["]"] }
        // NUMBER [-+]?([0-9]+|(([0-9]+\.[0-9]*)|(\.[0-9]+)))([eE][-+]?[0-9]+)?
        number = @{
            (["-"] | ["+"])? ~ // optional sign, followed by
            (
                (
                    (["."] ~ ['0'..'9']+) // dot and digits
                        | // or
                    (['0'..'9']+ ~ ["."] ~ ['0'..'9']*) // digits, dot, and (optional digits)
                )
                    | // or
                ['0'..'9']+ // just digits
            ) ~ ( // followed by (optional)
                (["e"] | ["E"]) ~ // 'e' or 'E', followed by
                (["-"] | ["+"])? ~ // optional sign, followed by
                ['0'..'9']+ // digits
            )?
        }
        integer = @{
            (["-"] | ["+"])? ~ // optional sign, followed by
            ['1'..'9'] ~ // at least one non-zero digit, followed by
            ['0'..'9']* // just digits
        }
        last_statement = { ["WorldEnd"] ~ whitespace? }
        whitespace = _{ ([" "] | ["\t"] | ["\r"] | ["\n"]) }
    }
    process! {
        main(&self) -> () {
            (_list: _pbrt()) => {
            }
        }
        _pbrt(&self) -> () {
            (_head: statement, _tail: _statement()) => {},
            (_l: last_statement) => { println!("WorldEnd"); },
        }
        // statements
        _statement(&self) -> () {
            (_head: look_at, _tail: _look_at()) => {},
            (_head: rotate, _tail: _rotate()) => {},
            (_head: named_statement, _tail: _named_statement()) => {},
            (_head: keyword, _tail: _keyword()) => {},
        }
        _look_at(&self) -> () {
            (eye_x: _number(), eye_y: _number(), eye_z: _number(),
             look_x: _number(), look_y: _number(), look_z: _number(),
             up_x: _number(), up_y: _number(), up_z: _number()) => {
                println!("LookAt {} {} {} {} {} {} {} {} {}",
                         eye_x, eye_y, eye_z,
                         look_x, look_y, look_z,
                         up_x, up_y, up_z,);
                let pos: Point3f = Point3f { x: eye_x, y: eye_y, z: eye_z, };
                let look: Point3f = Point3f { x: look_x, y: look_y, z: look_z, };
                let up: Vector3f = Vector3f { x: up_x, y: up_y, z: up_z, };
                let look_at: Transform = Transform::look_at(pos, look, up);
                unsafe {
                    CUR_TRANSFORM.t[0] = CUR_TRANSFORM.t[0] * look_at;
                    CUR_TRANSFORM.t[1] = CUR_TRANSFORM.t[1] * look_at;
                    println!("CUR_TRANSFORM: {:?}", CUR_TRANSFORM);
                }
                self._pbrt();
            }
        }
        _rotate(&self) -> () {
            (angle: _number(), x: _number(), y: _number(), z: _number()) => {
                println!("Rotate {} {} {} {}",
                         angle, x, y, z);
                self._pbrt();
            }
        }
        // named statements
        _named_statement(&self) -> () {
            (_head: camera, _tail: _camera()) => {},
            (_head: pixel_filter, _tail: _pixel_filter()) => {},
            (_head: sampler, _tail: _sampler()) => {},
            (_head: film, _tail: _film()) => {},
            (_head: coord_sys_transform, _tail: _coord_sys_transform()) => {},
            (_head: light_source, _tail: _light_source()) => {},
            (_head: texture, _tail: _texture()) => {},
            (_head: material, _tail: _material()) => {},
            (_head: shape, _tail: _shape()) => {},
        }
        _camera(&self) -> () {
            (name: _string(), optional_parameters) => {
                unsafe {
                    if let Some(ref mut render_options) = RENDER_OPTIONS {
                        // TODO: renderOptions->CameraName = name;
                        render_options.camera_name = name;
                        print!("Camera \"{}\" ", render_options.camera_name);
                        // TODO: renderOptions->CameraParams = params;
                        render_options.camera_to_world.t[0] = Transform::inverse(CUR_TRANSFORM.t[0]);
                        render_options.camera_to_world.t[1] = Transform::inverse(CUR_TRANSFORM.t[1]);
                        println!("render_options.camera_to_world: {:?}", render_options.camera_to_world);
                    }
                }
                if optional_parameters.rule == Rule::parameter {
                    self._parameter();
                } else {
                    println!("ERROR: parameter expected, {:?} found ...", optional_parameters);
                }
            },
        }
        _pixel_filter(&self) -> () {
            (name: _string(), optional_parameters) => {
                print!("PixelFilter \"{}\" ", name);
                if optional_parameters.rule == Rule::parameter {
                    self._parameter();
                } else {
                    println!("ERROR: parameter expected, {:?} found ...", optional_parameters);
                }
            },
        }
        _sampler(&self) -> () {
            (name: _string(), optional_parameters) => {
                print!("Sampler \"{}\" ", name);
                if optional_parameters.rule == Rule::parameter {
                    self._parameter();
                } else if optional_parameters.rule == Rule::statement {
                    println!("");
                    self._statement();
                } else if optional_parameters.rule == Rule::last_statement {
                    println!("");
                    println!("WorldEnd");
                } else {
                    println!("ERROR: parameter expected, {:?} found ...", optional_parameters);
                }
            },
        }
        _film(&self) -> () {
            (name: _string(), optional_parameters) => {
                print!("Film \"{}\" ", name);
                if optional_parameters.rule == Rule::parameter {
                    self._parameter();
                } else {
                    println!("ERROR: parameter expected, {:?} found ...", optional_parameters);
                }
            },
        }
        _coord_sys_transform(&self) -> () {
            (name: _string()) => {
                println!("CoordSysTransform \"{}\" ", name);
                self._pbrt();
            },
        }
        _light_source(&self) -> () {
            (name: _string(), optional_parameters) => {
                print!("LightSource \"{}\" ", name);
                if optional_parameters.rule == Rule::parameter {
                    self._parameter();
                } else {
                    println!("ERROR: parameter expected, {:?} found ...", optional_parameters);
                }
            },
        }
        _texture(&self) -> () {
            (name: _string(), optional_parameters) => {
                print!("Texture \"{}\" ", name);
                if optional_parameters.rule == Rule::parameter {
                    self._parameter();
                } else {
                    println!("ERROR: parameter expected, {:?} found ...", optional_parameters);
                }
            },
        }
        _material(&self) -> () {
            (name: _string(), optional_parameters) => {
                print!("Material \"{}\" ", name);
                if optional_parameters.rule == Rule::parameter {
                    self._parameter();
                } else {
                    println!("ERROR: parameter expected, {:?} found ...", optional_parameters);
                }
            },
        }
        _shape(&self) -> () {
            (name: _string(), optional_parameters) => {
                print!("Shape \"{}\" ", name);
                if optional_parameters.rule == Rule::parameter {
                    self._parameter();
                } else {
                    println!("ERROR: parameter expected, {:?} found ...", optional_parameters);
                }
            },
        }
        // parameters
        _parameter(&self) -> () {
            (_head: float_param, tail: _float_param()) => {
                let (string, number) = tail;
                print!("\"float {}\" [{}] ", string, number);
                self._parameter();
            },
            (_head: string_param, tail: _string_param()) => {
                let (string1, string2) = tail;
                print!("\"string {}\" [{}] ", string1, string2);
                self._parameter();
            },
            (_head: integer_param, tail: _integer_param()) => {
                let (string, number) = tail;
                print!("\"integer {}\" [{}] ", string, number);
                self._parameter();
            },
            (_head: point_param, tail: _point_param()) => {
                let (string, number1, number2, number3) = tail;
                print!("\"point {}\" [ {} {} {} ] ", string, number1, number2, number3);
                self._parameter();
            },
            (_head: rgb_param, tail: _rgb_param()) => {
                let (string, number1, number2, number3) = tail;
                print!("\"rgb {}\" [ {} {} {} ] ", string, number1, number2, number3);
                self._parameter();
            },
            (_head: spectrum_param, tail: _spectrum_param()) => {
                let string = tail;
                print!("\"spectrum\" {} ", string);
                self._parameter();
            },
            (_head: texture_param, tail: _texture_param()) => {
                let (string1, string2) = tail;
                print!("\"texture {}\" {} ", string1, string2);
                self._parameter();
            },
            (optional_parameters) => {
                if optional_parameters.rule == Rule::statement {
                    println!("");
                    self._statement();
                } else if optional_parameters.rule == Rule::last_statement {
                    println!("");
                    println!("WorldEnd");
                } else if optional_parameters.rule == Rule::parameter {
                    self._parameter();
                } else {
                    println!("ERROR: statement or parameter expected, {:?} found ...",
                             optional_parameters);
                }
            }
        }
        _float_param(&self) -> (String, Float) {
            (&i: ident, _l: lbrack, &n: number, _r: rbrack) => {
                let string: String = String::from_str(i).unwrap();
                let number: Float = f32::from_str(n).unwrap();
                (string, number)
            },
        }
        _string_param(&self) -> (String, String) {
            (&i: ident, _l: lbrack, &s: string, _r: rbrack) => {
                let string1: String = String::from_str(i).unwrap();
                let string2: String = String::from_str(s).unwrap();
                (string1, string2)
            },
            (&i: ident, _l: lbrack, _s: string, &f: filename, _r: rbrack) => {
                let string1: String = String::from_str(i).unwrap();
                let string2: String = String::from_str(f).unwrap();
                (string1, string2)
            },
        }
        _integer_param(&self) -> (String, i32) {
            (&i: ident, _l: lbrack, &n: integer, _r: rbrack) => {
                let string: String = String::from_str(i).unwrap();
                let number: i32 = i32::from_str(n).unwrap();
                (string, number)
            },
        }
        _point_param(&self) -> (String, Float, Float, Float) {
            (&i: ident, _l: lbrack, &n1: number, &n2: number, &n3: number, _r: rbrack) => {
                let string: String = String::from_str(i).unwrap();
                let number1: Float = f32::from_str(n1).unwrap();
                let number2: Float = f32::from_str(n2).unwrap();
                let number3: Float = f32::from_str(n3).unwrap();
                (string, number1, number2, number3)
            },
        }
        _rgb_param(&self) -> (String, Float, Float, Float) {
            (&i: ident, _l: lbrack, &n1: number, &n2: number, &n3: number, _r: rbrack) => {
                let string: String = String::from_str(i).unwrap();
                let number1: Float = f32::from_str(n1).unwrap();
                let number2: Float = f32::from_str(n2).unwrap();
                let number3: Float = f32::from_str(n3).unwrap();
                (string, number1, number2, number3)
            },
        }
        _spectrum_param(&self) -> String {
            (&s: string, _i: ident) => {
                let string: String = String::from_str(s).unwrap();
                string
            },
        }
        _texture_param(&self) -> (String, String) {
            (&i: ident, &s: string, _i: ident) => {
                let string1: String = String::from_str(i).unwrap();
                let string2: String = String::from_str(s).unwrap();
                (string1, string2)
            },
        }
        // keywords
        _keyword(&self) -> () {
            (_ab: attribute_begin) => {
                println!("AttributeBegin");
                self._pbrt();
            },
            (_ae: attribute_end) => {
                println!("AttributeEnd");
                self._pbrt();
            },
            (_wb: world_begin) => {
                println!("WorldBegin");
                self._pbrt();
            },
            (t) => { println!("TODO: {:?}", t); },
        }
        // numbers
        _number(&self) -> Float {
            (&n: number) => {
                let number: Float = f32::from_str(n).unwrap();
                number
            },
        }
        // strings
        _string(&self) -> String {
            (_s: string) => {
                self._ident()
            },
        }
        // identifiers
        _ident(&self) -> String {
            (&i: ident) => {
                let string: String = String::from_str(i).unwrap();
                string
            },
        }
    }
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

fn print_version(program: &str) {
    println!("{} {}", program, VERSION);
}

fn main() {
    // handle command line options
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optopt("i", "", "parse an input file", "FILE");
    opts.optflag("v", "version", "print version number");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    } else if matches.opt_present("i") {
        let infile = matches.opt_str("i");
        match infile {
            Some(x) => {
                println!("FILE = {}", x);
                let f = File::open(x).unwrap();
                let mut reader = BufReader::new(f);
                let mut str_buf: String = String::default();
                reader.read_to_string(&mut str_buf);
                unsafe {
                    // render options
                    RENDER_OPTIONS = Some(Box::new(RenderOptions::default()));
                    // parser
                    let mut parser = Rdp::new(StringInput::new(&str_buf));
                    assert!(parser.pbrt());
                    assert!(parser.end());
                    println!("{:?}", parser.queue());
                    println!("do something with created tokens ...");
                    parser.main();
                    println!("done.");
                }
            }
            None => panic!("no input file name"),
        }
        return;
    } else if matches.opt_present("v") {
        print_version(&program);
        return;
    } else {
        print_usage(&program, opts);
        return;
    }
}