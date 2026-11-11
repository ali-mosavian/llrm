//! At -Os x86-64 laid blocks out in `MachineBlockPlacement`'s chain order,
//! which left 37 conditional jumps of jpeg_single too far for a short
//! encoding (6 bytes each, 3467 B of text); `-apply-ext-tsp-for-size` keeps
//! 24 (3402 B).

mod common;

use std::path::Path;
use std::process::Command;

#[test]
fn os_layout_keeps_most_conditional_jumps_short_on_x86_64() {
    if Command::new("objdump").arg("--version").output().is_err() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("jpeg.o");
    let made = common::cc::llrm_c()
        .args(["-m64", "-Os", "-c", "-Dmain=bench_main_", "-I"])
        .arg(root.join("runtime/shared/posix"))
        .arg("-I")
        .arg(root.join("bench"))
        .arg(root.join("bench/jpeg_single/jpeg_single.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let listing = Command::new("objdump").arg("-d").arg(&object).output().unwrap();
    let rel32 = String::from_utf8_lossy(&listing.stdout)
        .lines()
        .filter(|line| {
            let mut bytes = line.split('\t').nth(1).unwrap_or("").split_whitespace();
            bytes.next() == Some("0f") && bytes.next().is_some_and(|op| op.starts_with('8')) && bytes.count() == 4
        })
        .count();
    assert!(rel32 <= 30, "{rel32} conditional jumps use a 32-bit displacement");
}
