//! `llrm-wasm-llc (-S | -filetype=obj) [-mtriple=wasm32|wasm64] [-o OUT]
//! FILE.ll`: LLVM IR through the wasm backend to assembly text or a relocatable
//! object (`-o -` is standard output), as the pin's `llc -O0 -fast-isel=false`
//! does. `-no-stackify` drops `RegStackify`, which LLVM 20's `llc -O0` does not
//! run.

use llrm_wasm::dag::{ExceptionModel, Options};
use llrm_wasm::driver;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let mut assembly = false;
    let mut object = false;
    let mut triple = "wasm32".to_owned();
    let mut options = Options::default();
    let mut handling = false;
    let mut output = None;
    let mut input = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => assembly = true,
            "-filetype=obj" => object = true,
            "-o" => output = arguments.next(),
            "-no-stackify" => options.stackify = false,
            "-exception-model=wasm" => options.exception_model = ExceptionModel::Wasm,
            "-exception-model=none" => options.exception_model = ExceptionModel::None,
            "-mattr=+exception-handling" => handling = true,
            other if other.starts_with("-mtriple=") => triple = other["-mtriple=".len()..].to_owned(),
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other => {
                eprintln!("llrm-wasm-llc: unrecognized argument {other}");
                return 2;
            }
        }
    }
    let Some(input) = input.filter(|_| assembly || object) else {
        eprintln!(
            "usage: llrm-wasm-llc (-S | -filetype=obj) [-mtriple=wasm32|wasm64] [-no-stackify] [-exception-model=wasm -mattr=+exception-handling] [-o OUT] FILE.ll"
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
        if object {
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
