//! `sext (icmp ne (and a, b), 0)` was `ands; cset; sbfm`, the pin's is `tst;
//! csetm`: +4 B in each of 19 QuickrBasic operands.c compare functions.

#[test]
fn a_sign_extended_compare_is_one_conditional_select() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/sext_of_compare.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(!asm.contains("sbfm"), "the flag was extended in a register:\n{asm}");
}
