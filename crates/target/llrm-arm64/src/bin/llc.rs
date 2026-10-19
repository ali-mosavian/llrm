//! `llrm-arm64-llc [-S] [--no-opt] [-o OUT] FILE.ll`: LLVM IR from clang's
//! front end alone (`-O2 -Xclang -disable-llvm-passes -Xclang
//! -disable-lifetime-markers`) through the mid-end and the arm64 backend to an
//! object (`-S`: its instructions as text; `--no-opt`: no mid-end, for IR that
//! is already optimized). The platform is the module's `target triple`.

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
    let mut optimize = true;
    let mut output = None;
    let mut input = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => assembly = true,
            "--no-opt" => optimize = false,
            "-o" => output = arguments.next(),
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other => {
                eprintln!("llrm-arm64-llc: unrecognized argument {other}");
                return 2;
            }
        }
    }
    let Some(input) = input else {
        eprintln!("usage: llrm-arm64-llc [-S] [--no-opt] [-o OUT] FILE.ll");
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
        driver::assembly(&text, platform, optimize).map(String::into_bytes)
    } else {
        driver::object_file(&text, platform, &name, optimize)
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
