//! The loop of x_nbody_f at `-m32 -O2` kept six float values in x87 registers
//! over its back edge where the old selector kept four, and the latch paid the
//! difference: a run of 7 `fxch` to restore the header's order, 4 before. A
//! register-held loop value costs the exchanges it adds to the latch; the spill
//! choice prices them against the spill.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn a_loop_carried_float_does_not_cost_the_latch_more_exchanges_than_its_spill() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW names nothing");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-m32", "-mabi=sysv", "-O2", "-march=i486", "-S", "-o", "-"])
        .arg(root.join("crates/target/llrm-x86-m32/vsgcc/kernels/x_nbody_f/x_nbody_f.c"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let listing = String::from_utf8_lossy(&out.stdout);
    // The exchanges right before a conditional jump are a latch's (the entry
    // edge's, before a `jmp`, run once).
    let (mut run, mut longest) = (0, 0);
    for line in listing.lines().map(str::trim) {
        if line.starts_with("fxch") {
            run += 1;
        } else {
            if line.starts_with('j') && !line.starts_with("jmp") {
                longest = longest.max(run);
            }
            run = 0;
        }
    }
    // The old selector's latch: four exchanges.
    assert!(longest <= 4, "{longest} exchanges before a conditional jump");
}
