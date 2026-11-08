//! `llrm-6502-llc [-S] [-O<n>] [--platform=NAME] [--cpu=6502|65c02]
//! [-f[no-]PASS] [-o OUT] FILE.ll`: LLVM IR from clang's front end alone
//! through the mid-end and the 6502 backend to ca65 text. `-S` is the only
//! output; the platform is sim65 unless named.

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};

/// The `-f` switches the backend takes besides the shared ones.
const OWN_SWITCHES: &[&str] = &["-fno-second-inline", "-fno-static-locals"];

/// Whether `switch` is one the shared flag parser knows or the backend's own;
/// an unknown `-f` was taken silently before (2026-11-08: `-fno-bogus`).
fn known_switch(switch: &str) -> Result<(), String> {
    if OWN_SWITCHES.contains(&switch) {
        return Ok(());
    }
    let mut at = 0;
    match llrm_codegen::driver::flags::Flags::default().take(&[switch.to_owned()], &mut at)? {
        true => Ok(()),
        false => Err(format!("unrecognized argument {switch}")),
    }
}

fn main() {
    // The mid-end recurses along dependency chains: run on a thread with room.
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(run).expect("a thread");
    std::process::exit(worker.join().unwrap_or(101));
}

fn run() -> i32 {
    let mut request = Request::new(Level::O2);
    let mut output = None;
    let mut input = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-S" => {}
            "-o" => output = arguments.next(),
            "--cpu=6502" => request.cpu = Cpu::Mos6502,
            "--cpu=65c02" => request.cpu = Cpu::W65c02,
            other if other.starts_with("--platform=") => match driver::platform_named(&other["--platform=".len()..]) {
                Ok(platform) => request.platform = platform,
                Err(why) => {
                    eprintln!("llrm-6502-llc: {why}");
                    return 2;
                }
            },
            other if other.starts_with("-O") => match Level::parse(&other[2..]) {
                Ok(level) => request.level = level,
                Err(why) => {
                    eprintln!("llrm-6502-llc: {why}");
                    return 2;
                }
            },
            other if other.starts_with("-f") => match known_switch(other) {
                Ok(()) => request.passes.push(other.to_owned()),
                Err(why) => {
                    eprintln!("llrm-6502-llc: {why}");
                    return 2;
                }
            },
            other if input.is_none() && !other.starts_with('-') => input = Some(other.to_owned()),
            other if llrm_debug::is_debug_flag(other) => request.debug = true,
            other => {
                eprintln!("llrm-6502-llc: unrecognized argument {other}");
                return 2;
            }
        }
    }
    // for measuring what the platform's zero page is worth (clamped by the
    // allocator to 16..=26)
    if let Some(bytes) = std::env::var("LLRM_6502_ZERO_PAGE").ok().and_then(|n| n.parse().ok()) {
        request.platform.zero_page = bytes;
    }
    // the test profile: check every spill-slot sharing, panic on a wrong one
    if std::env::var_os("LLRM_6502_VERIFY_SLOTS").is_some() {
        llrm_spike_mf::spill::VERIFY_SLOTS.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let Some(input) = input else {
        eprintln!("usage: llrm-6502-llc [-S] [-O<n>] [--platform=NAME] [--cpu=6502|65c02] [-o OUT] FILE.ll");
        return 2;
    };
    let text = match std::fs::read_to_string(&input) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("llrm-6502-llc: {input}: {error}");
            return 1;
        }
    };
    match driver::assembly(&text, &request) {
        Ok(out) => {
            let path = output
                .unwrap_or_else(|| std::path::Path::new(&input).with_extension("s").to_string_lossy().into_owned());
            match std::fs::write(&path, out) {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("llrm-6502-llc: {path}: {error}");
                    1
                }
            }
        }
        Err(why) => {
            eprintln!("llrm-6502-llc: {why}");
            1
        }
    }
}
