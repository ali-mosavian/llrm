//! DEVIATE FR-LOW: a real unwinder (libgcc's `_Unwind_Backtrace`, run under
//! qemu) walks through frames whose record is lowest.

use std::path::PathBuf;
use std::process::Command;

use llrm_arm64::machine::Platform;

fn has(tool: &str) -> bool {
    Command::new(tool).arg("--version").output().is_ok_and(|said| said.status.success())
}

/// `main` and `leaf` each keep locals and make a call: their frames are
/// `stp x29, x30, [sp, #-N]!` with the locals above. The backtrace from
/// inside the callback must reach both and the C runtime start: three frames
/// or more (a wrong rule stops it at one).
#[test]
fn a_backtrace_walks_through_frames_with_the_record_lowest() {
    if !has("aarch64-linux-gnu-gcc") || !has("qemu-aarch64") {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let ir = directory.path().join("t.ll");
    let source = format!(
        "target datalayout = \"{}\"\ntarget triple = \"aarch64-unknown-linux-gnu\"\n{}",
        Platform::Linux.datalayout(),
        include_str!("data/unwind_backtrace.ll")
    );
    std::fs::write(&ir, source).unwrap();
    let object = directory.path().join("t.o");
    let llc = PathBuf::from(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    let said = Command::new(llc).arg("-o").arg(&object).arg(&ir).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let elf = directory.path().join("t.elf");
    let said = Command::new("aarch64-linux-gnu-gcc").arg("-static").arg(&object).arg("-o").arg(&elf).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let ran = Command::new("qemu-aarch64").arg(&elf).status().unwrap();
    assert!(ran.success(), "the backtrace stopped early: {ran}");
}
