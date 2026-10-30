//! `llrm-wasm-llc -S [-mtriple=wasm32|wasm64] [-o OUT] FILE.ll`: LLVM IR through the wasm
//! backend to assembly text, as the pin's `llc -O0 -fast-isel=false` does. Stops at `-S`.
//! `-no-stackify` drops `RegStackify`, which LLVM 20's `llc -O0` does not run.

use llrm_wasm::dag::Options;
use llrm_wasm::driver;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let mut assembly = false;
    let mut triple = "wasm32".to_owned();
    let mut options = Options::default();
    let mut output = None;
    let mut input = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => assembly = true,
            "-o" => output = arguments.next(),
            "-no-stackify" => options.stackify = false,
            other if other.starts_with("-mtriple=") => triple = other["-mtriple=".len()..].to_owned(),
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other => {
                eprintln!("llrm-wasm-llc: unrecognized argument {other}");
                return 2;
            }
        }
    }
    let Some(input) = input.filter(|_| assembly) else {
        eprintln!("usage: llrm-wasm-llc -S [-mtriple=wasm32|wasm64] [-no-stackify] [-o OUT] FILE.ll");
        return 2;
    };
    let text = match std::fs::read_to_string(&input) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("llrm-wasm-llc: {input}: {error}");
            return 1;
        }
    };
    match driver::width_of(&triple).and_then(|width| driver::assembly(&text, width, options)) {
        Ok(text) => {
            let path = output.unwrap_or_else(|| std::path::Path::new(&input).with_extension("s").to_string_lossy().into_owned());
            if let Err(error) = std::fs::write(&path, text) {
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
