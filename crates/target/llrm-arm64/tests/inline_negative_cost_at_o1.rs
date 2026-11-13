//! SZ1's byte rule stands down at -O1 where the pin's own inline cost is below zero (a callee that folds
//! away: `clamp`, `extend`). jpeg_single -O1 left both as calls: 17.04 M instructions against clang's
//! 16.45 M; inlined, 16.41 M for +184 B (4,344 B, gcc 5,627).

use std::process::Command;

#[test]
fn a_callee_the_pin_prices_below_zero_is_inlined_at_o1() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O1", "-o"])
        .arg(&output)
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/jpeg_single_unopt.ll"))
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(asm.contains("bench_jpeg_single:"), "premise: lowered");
    let calls = |name: &str| asm.lines().filter(|line| line.trim_start().starts_with("bl") && line.contains(name)).count();
    assert!(calls("next") > 0, "premise: a callee over the byte rule stays a call");
    assert_eq!(calls("clamp"), 0, "clamp is called {} times", calls("clamp"));
    assert_eq!(calls("extend"), 0, "extend is called {} times", calls("extend"));
}
