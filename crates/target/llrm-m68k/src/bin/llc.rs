//! `llrm-m68k-llc -S [-mcpu=68000|..|68060] [-O<n>] [--dump DIR] [-o OUT]
//! FILE.ll`: LLVM IR from clang's front end alone through the mid-end and the
//! m68k backend to assembly text with `-S`, else to an ELF32 relocatable
//! object (`-c` is the same).

use llrm_m68k::driver::{self, Level};

fn main() {
    // The mid-end recurses along dependency chains: run on a thread with room.
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(run).expect("a thread");
    std::process::exit(worker.join().unwrap_or(101));
}

fn run() -> i32 {
    let mut assembly = false;
    let mut debug = false;
    let mut level = Level::O2;
    let mut cpu = "68000".to_owned();
    let mut output = None;
    let mut input = None;
    let mut dump = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => assembly = true,
            "-c" => assembly = false,
            "-g" => debug = true,
            "-o" => output = arguments.next(),
            "--dump" => dump = arguments.next().map(std::path::PathBuf::from),
            other if other.starts_with("--dump=") => dump = Some(other["--dump=".len()..].into()),
            other if other.starts_with("-mcpu=") => cpu = other["-mcpu=".len()..].to_owned(),
            other if other.starts_with("-O") => match Level::parse(&other[2..]) {
                Ok(parsed) => level = parsed,
                Err(why) => {
                    eprintln!("llrm-m68k-llc: {why}");
                    return 2;
                }
            },
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other if llrm_debug::is_debug_flag(other) => {
                eprintln!("{}", llrm_debug::no_writer("llrm-m68k-llc", other, "m68k"));
                return 2;
            }
            other => {
                eprintln!("llrm-m68k-llc: unrecognized argument {other}");
                return 2;
            }
        }
    }
    let Some(input) = input else {
        eprintln!(
            "usage: llrm-m68k-llc [-S|-c] [-g] [-mcpu=68000|..|68060] [-O0|-O1|-O2|-O3|-Os|-Oz] [--dump DIR] [-o OUT] FILE.ll"
        );
        return 2;
    };
    let text = match std::fs::read_to_string(&input) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("llrm-m68k-llc: {input}: {error}");
            return 1;
        }
    };
    let env = llrm_support::env_var("LLRM_MIR_STAGES").map(std::path::PathBuf::from);
    let result: Result<Vec<u8>, String> = driver::cpu_of(&cpu).and_then(|cpu| {
        if assembly {
            driver::assembly_with(&text, cpu, level, debug, dump.as_deref().or(env.as_deref())).map(String::into_bytes)
        } else {
            driver::object(&text, cpu, level, &input)
        }
    });
    match result {
        Ok(bytes) => {
            let extension = if assembly { "s" } else { "o" };
            let path = output.unwrap_or_else(|| {
                std::path::Path::new(&input).with_extension(extension).to_string_lossy().into_owned()
            });
            if let Err(error) = std::fs::write(&path, bytes) {
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
