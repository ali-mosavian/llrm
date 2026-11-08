//! x86-64: the call-frame pair writes the flags and reads none (`implicit-def dead $eflags`).
//! Taken for an instruction with no effect table, it read them: the flags of the `sub` before a
//! call were live, and `fib(n - 1)` was `mov edi, ebp; dec edi` where the pin makes
//! `lea edi, [rbp-1]` (jpeg_fixed -O2 8324 B against 8131 B, fib -Os 77 B against 72 B).
//! Needs `LLRM_LLC`.

use std::process::Command;

#[test]
fn a_decrement_before_a_call_is_a_lea_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o", "-"])
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/fib_calls.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let asm = String::from_utf8_lossy(&run.stdout);
    assert!(asm.contains("lea edi, [rbp-1]") && !asm.contains("dec edi"), "n - 1 is not a lea:\n{asm}");
}
