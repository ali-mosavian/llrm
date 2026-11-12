//! DEVIATE INL-SPEED-OS: at -Os an inline trial the byte count refuses is kept where its sites run in a hot
//! loop and the copies save clocks. jpeg_single -Os: `clamp` (8 instructions) called 3 times per pixel of
//! `convert`, 1.18 M calls, 16,886,488 instructions against clang's 16,273,546; inlined, 16,297,779 for +24 B.

use std::process::Command;

#[test]
fn a_small_callee_in_a_hot_loop_is_inlined_at_os() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/jpeg_single_unopt.ll"))
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(asm.contains("convert:") || asm.contains("bench_jpeg_single:"), "premise: lowered");
    let calls = asm.lines().filter(|line| line.trim_start().starts_with("bl") && line.contains("clamp")).count();
    assert_eq!(calls, 0, "clamp is called {calls} times");
}
