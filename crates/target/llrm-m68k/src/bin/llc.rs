//! `llrm-m68k-llc -S [-mcpu=68000|68020] [-O<n>] [-o OUT] FILE.ll`: LLVM IR from
//! clang's front end alone through the mid-end and the m68k backend to
//! assembly text. Stops at `-S`: there is no object writer behind this driver.

use llrm_m68k::driver::{self, Level};

fn main() {
    // The mid-end recurses along dependency chains: run on a thread with room.
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(run).expect("a thread");
    std::process::exit(worker.join().unwrap_or(101));
}

fn run() -> i32 {
    let mut assembly = false;
    let mut level = Level::O2;
    let mut cpu = "68000".to_owned();
    let mut output = None;
    let mut input = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => assembly = true,
            "-o" => output = arguments.next(),
            other if other.starts_with("-mcpu=") => cpu = other["-mcpu=".len()..].to_owned(),
            other if other.starts_with("-O") => match Level::parse(&other[2..]) {
                Ok(parsed) => level = parsed,
                Err(why) => {
                    eprintln!("llrm-m68k-llc: {why}");
                    return 2;
                }
            },
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other => {
                eprintln!("llrm-m68k-llc: unrecognized argument {other}");
                return 2;
            }
        }
    }
    let Some(input) = input.filter(|_| assembly) else {
        eprintln!("usage: llrm-m68k-llc -S [-mcpu=68000|68020] [-O0|-O1|-O2|-O3|-Os|-Oz] [-o OUT] FILE.ll");
        return 2;
    };
    let text = match std::fs::read_to_string(&input) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("llrm-m68k-llc: {input}: {error}");
            return 1;
        }
    };
    let result = driver::cpu_of(&cpu).and_then(|cpu| driver::assembly(&text, cpu, level));
    match result {
        Ok(text) => {
            let path = output.unwrap_or_else(|| {
                std::path::Path::new(&input).with_extension("s").to_string_lossy().into_owned()
            });
            if let Err(error) = std::fs::write(&path, text) {
                eprintln!("llrm-m68k-llc: {path}: {error}");
                return 1;
            }
            0
        }
        Err(why) => {
            eprintln!("llrm-m68k-llc: {why}");
            1
        }
    }
}
