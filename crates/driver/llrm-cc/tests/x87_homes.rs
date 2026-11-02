//! A float the program also holds in a cell (`px[b] = px[b] + vx[b]` with the
//! array read by index elsewhere) was spilled to a slot of its own, so the
//! unrolled update of fpbench at `-m32 -O3` stored each result twice, `fst
//! slot` then `fstp cell`. The spill reads the cell the program holds the value
//! in, and the second store is not made.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn listing(extra: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-m32", "-mabi=sysv", "-O3", "-march=i486", "-S", "-o", "-"])
        .args(extra)
        .arg(root.join("bench/fpbench/fpbench.c"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Stores of one value to two cells in a row: `fst [a]` then `fstp [b]`.
fn double_stores(listing: &str) -> usize {
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    lines.windows(2).filter(|pair| pair[0].starts_with("fst ") && pair[1].starts_with("fstp ")).count()
}

#[test]
fn a_float_held_in_a_cell_is_not_also_spilled_to_a_slot() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW names nothing");
        return;
    }
    let without = double_stores(&listing(&["-fno-llrm-homes"]));
    if without == 0 {
        // The shape needs the full unroll of the 6-trip update (x86-m32's
        // -O3 bar); a build without it has no spill to avoid.
        eprintln!("skipped: no double store without the homes pass in this build");
        return;
    }
    // Pairs the program stores itself remain in both listings; the spill
    // pairs are the difference.
    let with = double_stores(&listing(&[]));
    assert!(with < without, "fst slot; fstp cell: {with} pairs with homes, {without} without");
}
