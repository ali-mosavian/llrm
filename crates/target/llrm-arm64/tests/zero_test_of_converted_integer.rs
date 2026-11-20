//! The bits of a converted integer are zero (or negative) exactly when the
//! integer is: `icmp eq (bitcast (uitofp x)), 0` stayed a `ucvtf`, a `fmov`
//! and a `cbz` on the float's bits (the pin's `foldICmpBitCast`). QuickrBasic
//! operands.c -Os: +12 B in each float divide with a byte divisor.

#[test]
fn a_zero_or_sign_test_of_the_bits_of_a_converted_integer_tests_the_integer() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/zero_test_of_converted_integer.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(!asm.contains("fmov"), "the test went through the float's bits:\n{asm}");
}
