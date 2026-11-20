//! `llvm.is.fpclass` had no lowering: the call stayed a call of the symbol
//! `llvm.is.fpclass.f32` (`isnan`, `isinf` and `isfinite` of clang's output),
//! which no library defines. The class test is compares and integer
//! operations (`expandIS_FPCLASS`).

fn assembly() -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/is_fpclass.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

#[test]
fn a_class_test_is_not_a_call_of_the_intrinsic() {
    let asm = assembly();
    assert!(!asm.contains("llvm.is.fpclass"), "the intrinsic stayed a symbol:\n{asm}");
}

#[test]
fn isnan_and_iszero_are_float_compares() {
    let asm = assembly();
    assert!(asm.contains("fcmp s0, s0"), "isnan is x != x:\n{asm}");
    assert!(asm.contains("fcmp d0, #0.0"), "iszero is x == 0:\n{asm}");
}
