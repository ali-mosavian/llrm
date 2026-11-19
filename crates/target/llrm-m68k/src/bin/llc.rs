//! `llrm-m68k-llc -S [-mcpu=68000|..|68060] [-mcmodel=small|medium|large] [-O<n>] [-f[no-]PASS] [-fno-inline=NAME] [--dump DIR] [--size-estimate=FILE] [-o OUT]
//! FILE.ll`: LLVM IR from clang's front end alone through the mid-end and the
//! m68k backend to assembly text with `-S`, else to an ELF32 relocatable
//! object (`-c` is the same).

use std::io::Write;

use llrm_m68k::compile::CodeModel;
use llrm_m68k::driver::{self, Level, Policy};

#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() {
    // The mid-end recurses along dependency chains: run on a thread with room.
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(|| llrm_support::debug::run_main(run)).expect("a thread");
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
    let mut model = CodeModel::Small;
    let mut policy = Policy::default();
    let mut estimates = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => assembly = true,
            other if other.starts_with("--size-estimate=") => estimates = Some(other["--size-estimate=".len()..].to_owned()),
            "-c" => assembly = false,
            "-g" => debug = true,
            "-o" => output = arguments.next(),
            "--dump" => dump = arguments.next().map(std::path::PathBuf::from),
            other if other.starts_with("--dump=") => dump = Some(other["--dump=".len()..].into()),
            other if other.starts_with("-mcpu=") => cpu = other["-mcpu=".len()..].to_owned(),
            other if other.starts_with("-mcmodel=") => match CodeModel::parse(&other["-mcmodel=".len()..]) {
                Ok(parsed) => model = parsed,
                Err(why) => {
                    eprintln!("llrm-m68k-llc: {why}");
                    return 2;
                }
            },
            other if other.starts_with("-O") => match Level::parse(&other[2..]) {
                Ok(parsed) => level = parsed,
                Err(why) => {
                    eprintln!("llrm-m68k-llc: {why}");
                    return 2;
                }
            },
            // DEBT M68K-SANITIZE, owner target-m68k, exit 2026-11-12: the run-time
            // checks are not built; refused by name, never dropped. The pin's
            // clang `-fsanitize` stays a reference column (it needs a ubsan
            // handler stub to link).
            other if other.starts_with("-fsanitize") || other.starts_with("-fno-sanitize") => {
                eprintln!("llrm-m68k-llc: {other}: sanitizer checks are not implemented at m68k (M68K-SANITIZE)");
                return 2;
            }
            other if other.starts_with("-f") && policy.take(other) => {}
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
            "usage: llrm-m68k-llc [-S|-c] [-g] [-mcpu=68000|..|68060] [-mcmodel=small|medium|large] [-O0|-O1|-O2|-O3|-Os|-Oz] [-f[no-]PASS] [-fno-inline=NAME] [--dump DIR] [--size-estimate=FILE] [-o OUT] FILE.ll"
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
    if let Some(path) = &estimates {
        let lines = driver::cpu_of(&cpu)
            .and_then(|cpu| driver::size_estimates(&text, cpu, level))
            .map(|rows| rows.iter().map(|(name, bytes)| format!("{name}\t{bytes}\n")).collect::<String>());
        if let Err(why) = lines.and_then(|lines| std::fs::write(path, lines).map_err(|error| format!("{path}: {error}"))) {
            eprintln!("llrm-m68k-llc: {why}");
            return 1;
        }
    }
    let env = llrm_support::env_var("LLRM_MIR_STAGES").map(std::path::PathBuf::from);
    let result: Result<Vec<u8>, String> = driver::cpu_of(&cpu).and_then(|cpu| {
        if assembly {
            driver::assembly_flagged(&text, cpu, level, debug, dump.as_deref().or(env.as_deref()), model, &policy).map(String::into_bytes)
        } else {
            driver::object_flagged(&text, cpu, level, &input, model, &policy, dump.as_deref().or(env.as_deref()))
        }
    });
    match result {
        Ok(bytes) => {
            let extension = if assembly { "s" } else { "o" };
            let path = output.unwrap_or_else(|| {
                std::path::Path::new(&input).with_extension(extension).to_string_lossy().into_owned()
            });
            // `-o -` is stdout, as in llc and clang; it wrote a file named `-`.
            let written = if path == "-" { std::io::stdout().write_all(&bytes) } else { std::fs::write(&path, bytes) };
            if let Err(error) = written {
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
