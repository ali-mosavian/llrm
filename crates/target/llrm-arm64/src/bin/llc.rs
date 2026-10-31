//! `llrm-arm64-llc [-S] [--no-opt] [-o OUT] FILE.ll`: LLVM IR from clang's
//! front end alone (`-O2 -Xclang -disable-llvm-passes -Xclang
//! -disable-lifetime-markers`; `-debug-only=inline-cost`: each inlining
//! decision and every callee instruction's cost on stderr, as LLVM's option of
//! that name) through the mid-end and the arm64 backend to an object (`-S`: its
//! instructions as text; `-O<n>`: the mid-end's level, -O2 by default, as
//! clang's `-O` of the same IR; `--no-opt`: no mid-end, for IR that is already
//! optimized; `-f[no-]PASS`: a mid-end pass as gcc names it, `-fno-unroll-loops`;
//! `--isel=hand|dag|shadow`: the hand selector, the DAG path from the
//! pin's AArch64 table, or the DAG path with the hand selector for what it
//! refuses, counted on stderr). The platform is the module's `target triple`.

use llrm_arm64::driver;

fn main() {
    // The mid-end recurses along dependency chains (a 3,000-line function
    // overflowed the main thread's 8 MB): the compile runs on a thread with
    // room.
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(run).expect("a thread");
    std::process::exit(worker.join().unwrap_or(101));
}

fn run() -> i32 {
    let mut assembly = false;
    let mut debug = false;
    let mut level = driver::Level::O2;
    let mut output = None;
    let mut input = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => assembly = true,
            "-g" => debug = true,
            "-debug-only=inline-cost" => {
                llrm_env::preset("LLRM_INLINE_REMARKS");
                llrm_env::preset("LLRM_EXP_INLINETRACE");
            }
            "--no-opt" => level = driver::Level::O0,
            other if other.starts_with("--isel=") => match driver::IselMode::parse(&other["--isel=".len()..]) {
                Some(mode) => driver::set_isel_mode(mode),
                None => {
                    eprintln!("llrm-arm64-llc: {other}: choose hand, dag or shadow");
                    return 2;
                }
            },
            other if other.starts_with("-f") && driver::pass_flag(other) => {}
            other if other.starts_with("-O") => match driver::Level::parse(&other[2..]) {
                Ok(parsed) => level = parsed,
                Err(why) => {
                    eprintln!("llrm-arm64-llc: {why}");
                    return 2;
                }
            },
            "-o" => output = arguments.next(),
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other => {
                eprintln!("llrm-arm64-llc: unrecognized argument {other}");
                return 2;
            }
        }
    }
    let Some(input) = input else {
        eprintln!("usage: llrm-arm64-llc [-S] [-g] [-O0|-O1|-O2|-O3|-Os|-Oz] [--no-opt] [-o OUT] FILE.ll");
        return 2;
    };
    let text = match std::fs::read_to_string(&input) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("llrm-arm64-llc: {input}: {error}");
            return 1;
        }
    };
    let (triple, _) = driver::without_triple(&text);
    let Some(platform) = triple.as_deref().and_then(driver::platform_of) else {
        eprintln!("llrm-arm64-llc: no arm64 platform for the triple {triple:?}");
        return 1;
    };
    let name = std::path::Path::new(&input).file_stem().and_then(|one| one.to_str()).unwrap_or("module").to_owned();
    let result = if assembly {
        driver::assembly(&text, platform, level).map(String::into_bytes)
    } else {
        driver::object_file_with(&text, platform, &name, level, debug)
    };
    match result {
        Ok(bytes) => {
            let path = output.unwrap_or_else(|| {
                std::path::Path::new(&input)
                    .with_extension(if assembly { "s" } else { "o" })
                    .to_string_lossy()
                    .into_owned()
            });
            if let Err(error) = std::fs::write(&path, bytes) {
                eprintln!("llrm-arm64-llc: {path}: {error}");
                return 1;
            }
            0
        }
        Err(why) => {
            eprintln!("llrm-arm64-llc: {why}");
            return 1;
        }
    }
}
