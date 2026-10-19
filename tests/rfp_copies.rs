//! The copies between x87 registers are propagated before the stack form.

use std::process::Command;

/// nbody at -O2 under `LLRM_GREEDY_RFP` kept every `fp := fp` RegAlloc made, and
/// the stackifier turned each into a `fld st(i)` and a pop: 101 floating
/// instructions where copy propagation leaves 95 (the 200-trip position block
/// is 34 instructions, clang's 36).
#[test]
fn test_nbody_keeps_no_copy_between_x87_registers() {
    let scratch = tempfile::tempdir().unwrap();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/bench/nbody/nbody.c");
    let listing = scratch.path().join("nbody.s");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .env("LLRM_GREEDY_RFP", "1")
        .args(["-m32", "-O2", "-march=i486", "-S", "-o"])
        .arg(&listing)
        .arg(source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(listing).unwrap();
    let floating = listing.lines().map(str::trim).filter(|line| line.starts_with('f')).count();
    assert!(floating <= 95, "{floating} floating instructions: {listing}");
}
