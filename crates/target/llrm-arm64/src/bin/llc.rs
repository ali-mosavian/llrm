//! `llrm-arm64-llc [-S] [-o OUT] FILE.ll`: LLVM IR from clang's front end alone
//! (`-O2 -Xclang -disable-llvm-passes -Xclang -disable-lifetime-markers`)
//! through the mid-end and the arm64 backend to an object (`-S`: its
//! instructions as text). The platform is the module's `target triple`.

use llrm_arm64::driver;

fn main() {
    let mut assembly = false;
    let mut output = None;
    let mut input = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => assembly = true,
            "-o" => output = arguments.next(),
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other => {
                eprintln!("llrm-arm64-llc: unrecognized argument {other}");
                std::process::exit(2);
            }
        }
    }
    let Some(input) = input else {
        eprintln!("usage: llrm-arm64-llc [-S] [-o OUT] FILE.ll");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(&input).unwrap_or_else(|error| {
        eprintln!("llrm-arm64-llc: {input}: {error}");
        std::process::exit(1);
    });
    let (triple, _) = driver::without_triple(&text);
    let Some(platform) = triple.as_deref().and_then(driver::platform_of) else {
        eprintln!("llrm-arm64-llc: no arm64 platform for the triple {triple:?}");
        std::process::exit(1);
    };
    let name = std::path::Path::new(&input).file_stem().and_then(|one| one.to_str()).unwrap_or("module").to_owned();
    let result = if assembly {
        driver::assembly(&text, platform).map(String::into_bytes)
    } else {
        driver::object_file(&text, platform, &name)
    };
    match result {
        Ok(bytes) => {
            let path = output.unwrap_or_else(|| {
                std::path::Path::new(&input)
                    .with_extension(if assembly { "s" } else { "o" })
                    .to_string_lossy()
                    .into_owned()
            });
            std::fs::write(&path, bytes).unwrap_or_else(|error| {
                eprintln!("llrm-arm64-llc: {path}: {error}");
                std::process::exit(1);
            });
        }
        Err(why) => {
            eprintln!("llrm-arm64-llc: {why}");
            std::process::exit(1);
        }
    }
}
