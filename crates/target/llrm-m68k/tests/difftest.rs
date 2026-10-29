//! The reduced programs of `tests/difftest/`: each once failed in the differential driver
//! (`examples/difftest`) and must now agree with m68k gcc. Needs the pin's clang, the m68k gcc
//! toolchain and qemu-m68k, so it is not part of the default run: `cargo test --test difftest --
//! --ignored`.

use std::process::Command;

#[test]
#[ignore = "needs the pin's clang, m68k gcc and qemu-m68k; run with --ignored"]
fn the_reduced_programs_that_failed_once_agree_with_gcc() {
    let status = Command::new(env!("CARGO"))
        .args(["run", "-q", "--example", "difftest", "--"])
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/difftest"))
        .status()
        .expect("cargo runs");
    assert!(status.success(), "a reduced program no longer agrees: see the lines above");
}
