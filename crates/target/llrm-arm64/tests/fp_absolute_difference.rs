//! `fabs (fsub a, b)` is one `fabd` (the pin's `AArch64InstrInfo.td` pattern).
//! The DAG selected `FABD64` and the emitter refused it: "machine opcode FABD64
//! has no arm64 instruction yet", QuickrBasic `src/core/num/double.c`
//! (`dubl_sub`) did not compile.

#[test]
fn an_absolute_difference_is_one_fabd() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/fp_absolute_difference.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(asm.contains("fabd d0, d0, d1"), "no fabd on the double:\n{asm}");
    assert!(asm.contains("fabd s0, s0, s1"), "no fabd on the float:\n{asm}");
    assert!(!asm.contains("fabs"), "the absolute value stayed apart:\n{asm}");
}
