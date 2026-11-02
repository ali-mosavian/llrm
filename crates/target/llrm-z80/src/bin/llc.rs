//! `llrm-z80-llc [-S] [-O<n>] [-o OUT] FILE.ll`: LLVM IR from clang's front end
//! alone through the mid-end and the Z80 backend to sdasz80 text. `-S` is the
//! only output. A function the backend refuses is named on stderr and the exit
//! status is 1; the others are still written.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request, selections};
use llrm_z80::inline_rt::inline_sole_uses;

fn main() {
    // The mid-end recurses along dependency chains: run on a thread with room.
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(run).expect("a thread");
    std::process::exit(worker.join().unwrap_or(101));
}

fn run() -> i32 {
    let mut level = Level::O2;
    let mut output = None;
    let mut input = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => {}
            "-o" => output = arguments.next(),
            other if other.starts_with("-O") => match Level::parse(&other[2..]) {
                Ok(parsed) => level = parsed,
                Err(why) => {
                    eprintln!("llrm-z80-llc: {why}");
                    return 2;
                }
            },
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other => {
                eprintln!("llrm-z80-llc: unrecognized argument {other}");
                return 2;
            }
        }
    }
    let Some(input) = input else {
        eprintln!("usage: llrm-z80-llc [-S] [-O<n>] [-o OUT] FILE.ll");
        return 2;
    };
    let ir = match std::fs::read_to_string(&input) {
        Ok(ir) => ir,
        Err(error) => {
            eprintln!("llrm-z80-llc: {input}: {error}");
            return 1;
        }
    };
    let request = Request::new(level);
    let names: Vec<String> = match selections(&ir, &request) {
        Ok(selected) => selected.into_iter().map(|(name, _)| name).collect(),
        Err(why) => {
            eprintln!("llrm-z80-llc: {why}");
            return 1;
        }
    };
    let mut status = 0;
    let mut module = String::from("\t.area _CODE\n");
    for (number, name) in names.iter().enumerate() {
        module.push_str(&format!("\t.globl _{name}\n"));
        match text(&ir, name, number, &request) {
            Ok(text) => module.push_str(&text),
            Err(why) => {
                eprintln!("llrm-z80-llc: REFUSED @{name}: {why}");
                status = 1;
            }
        }
    }
    let out = inline_sole_uses(&module, llrm_z80::RUNTIME);
    let path =
        output.unwrap_or_else(|| std::path::Path::new(&input).with_extension("s").to_string_lossy().into_owned());
    match std::fs::write(&path, out) {
        Ok(()) => status,
        Err(error) => {
            eprintln!("llrm-z80-llc: {path}: {error}");
            1
        }
    }
}
