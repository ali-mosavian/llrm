//! A float stored through its bits (`sngl_bits`, a union pun) went through a
//! GPR: `fmov w9, s16; str w9, [x0]`, where the pin stores `s16`. QuickrBasic
//! operands.c -Os: +4 B in each of 40 float ops.

#[test]
fn a_float_stored_as_bits_is_stored_from_its_float_register() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/float_bits_store.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(!asm.contains("fmov w"), "the float went through a GPR:\n{asm}");
}
