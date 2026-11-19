//! A signed divide by a power of two above 2 is `lea; test; cmovns; sar` where
//! the target has `cmov`.

use std::process::Command;

mod common;

const DIVIDES: &str = "long q(long x) { return x / 1024; }\n\
                       int d(int x) { return x / 64; }\n\
                       long h(long x) { return x / 2; }\n\
                       long n(long x) { return x / -8; }\n";

/// jpeg_fixed at -O2 m64 divided by 1024 as `mov; sar 63; shr 54; add; sar 10`
/// (5 instructions) where the pin's X86 `BuildSDIVPow2` makes `lea; test;
/// cmovns; sar` (4): 737,280 of the 2.02M instructions it runs over the best.
#[test]
fn a_signed_divide_by_a_power_of_two_is_a_cmov_at_m64() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), DIVIDES).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .current_dir(scratch.path())
        .args(["-m64", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let cmovs = text.lines().filter(|line| line.trim().starts_with("cmovns")).count();
    // q, d and n take the cmov form; h (/2) keeps the default expansion.
    assert_eq!(cmovs, 3, "{text}");
    // The bias is a three-address `lea` (`ADD64ri32` to `LEA64r`), not `mov;
    // add`.
    assert!(text.contains("lea rax, [rdi+1023]"), "{text}");
}

/// The i486 has no `cmov`: the shift sequence stays.
#[test]
fn a_signed_divide_by_a_power_of_two_stays_shifts_without_cmov() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), DIVIDES).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    assert!(!text.contains("cmov"), "{text}");
}
