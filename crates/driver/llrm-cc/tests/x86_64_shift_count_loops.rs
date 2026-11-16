//! x86-64: `while (x >>= 1) n++` is a `bsr`, as the pin's
//! `recognizeShiftUntilZero` forms on every target. It stayed a loop (`shr;
//! inc; test; jne` a trip): bitcount_idioms +2.1% over clang's `bsr; cmovae` at
//! -O2. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_loop_that_shifts_until_zero_is_a_bit_scan_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("shift-count-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/shift_count_loops.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let body =
        text.split("log2_of proc near").nth(1).and_then(|rest| rest.split("log2_of endp").next()).expect("a body");
    assert!(body.contains("bsr") || body.contains("lzcnt"), "no bit scan:\n{body}");
    let loops: Vec<&str> =
        body.lines().map(str::trim).filter(|line| line.starts_with("jne") || line.starts_with("jnz")).collect();
    assert!(loops.is_empty(), "a loop is left: {loops:?}\n{body}");
}
