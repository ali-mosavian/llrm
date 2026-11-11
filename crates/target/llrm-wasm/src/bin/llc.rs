//! `llrm-wasm-llc (-S | -filetype=obj) [-mtriple=wasm32|wasm64] [-o OUT]
//! FILE.ll`: LLVM IR through the wasm backend to assembly text or a relocatable
//! object (`-o -` is standard output), as the pin's `llc -O0 -fast-isel=false`
//! does. `-no-stackify` drops `RegStackify`, which LLVM 20's `llc -O0` does not
//! run.

use llrm_wasm::dag::{ExceptionModel, Options};
use llrm_wasm::driver;

#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() {
    // The mid-end recurses along dependency chains: run on a thread with room.
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(run).expect("a thread");
    std::process::exit(worker.join().unwrap_or(101));
}

fn run() -> i32 {
    let mut assembly = false;
    let mut object = false;
    let mut triple = "wasm32".to_owned();
    // The pin's `llc` defaults to -O2; the library default (-O0) is for the tests that pin it.
    let mut options = Options { level: driver::Level::O2, ..Options::default() };
    let mut handling = false;
    let mut debug = false;
    let mut output = None;
    let mut input = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => assembly = true,
            "-filetype=obj" => object = true,
            "-o" => output = arguments.next(),
            "-no-stackify" => options.stackify = false,
            "-g" => debug = true,
            other if other.starts_with("-O") => match driver::Level::parse(&other[2..]) {
                Ok(level) => options.level = level,
                Err(why) => {
                    eprintln!("llrm-wasm-llc: {why}");
                    return 2;
                }
            },
            "-exception-model=wasm" => options.exception_model = ExceptionModel::Wasm,
            "-exception-model=none" => options.exception_model = ExceptionModel::None,
            "-mattr=+exception-handling" => handling = true,
            other if other.starts_with("-mtriple=") => triple = other["-mtriple=".len()..].to_owned(),
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other if other.starts_with("-f") && driver::pass_flag(other) => {}
            // `bounds` checks nothing in IR (llrm-cc); `stack` needs a C
            // runtime this target lacks.
            "-fsanitize=bounds" | "-fno-sanitize=bounds" | "-fno-sanitize=stack" => {}
            "-fsanitize=stack" => {
                eprintln!("llrm-wasm-llc: -fsanitize=stack: wasm has no C runtime stack check yet");
                return 2;
            }
            other if llrm_debug::is_debug_flag(other) => {
                eprintln!("{}", llrm_debug::no_writer("llrm-wasm-llc", other, "wasm"));
                return 2;
            }
            other => {
                eprintln!("llrm-wasm-llc: unrecognized argument {other}");
                return 2;
            }
        }
    }
    let Some(input) = input.filter(|_| assembly || object) else {
        eprintln!(
            "usage: llrm-wasm-llc (-S | -filetype=obj) [-mtriple=wasm32|wasm64] [-no-stackify] [-g] [-exception-model=wasm -mattr=+exception-handling] [-o OUT] FILE.ll"
        );
        return 2;
    };
    // The pin refuses one without the other.
    if (options.exception_model == ExceptionModel::Wasm) != handling {
        eprintln!("llrm-wasm-llc: -exception-model=wasm and -mattr=+exception-handling go together");
        return 2;
    }
    let text = match std::fs::read_to_string(&input) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("llrm-wasm-llc: {input}: {error}");
            return 1;
        }
    };
    let built = driver::width_of(&triple).and_then(|width| {
        if object && debug {
            driver::object_g(&text, width, options, &input)
        } else if object {
            driver::object(&text, width, options)
        } else {
            driver::assembly(&text, width, options).map(String::into_bytes)
        }
    });
    match built {
        Ok(bytes) => {
            let extension = if object { "o" } else { "s" };
            let path = output.unwrap_or_else(|| {
                std::path::Path::new(&input).with_extension(extension).to_string_lossy().into_owned()
            });
            let written = if path == "-" {
                std::io::Write::write_all(&mut std::io::stdout(), &bytes)
            } else {
                std::fs::write(&path, bytes)
            };
            if let Err(error) = written {
                eprintln!("llrm-wasm-llc: {path}: {error}");
                return 1;
            }
            0
        }
        Err(why) => {
            eprintln!("llrm-wasm-llc: {why}");
            1
        }
    }
}
