//! The bits of `uitofp` are those of a non-negative float, so their sign bit is
//! clear: `bitcast (uitofp x) < 0` stayed a conversion, a `fmov` and a test
//! (known bits of a bitcast from a float were not computed). QuickrBasic
//! operands.c -Os: +12 B in each float compare with an unsigned operand.

#[test]
fn the_sign_bit_of_the_bits_of_an_unsigned_float_is_clear() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/bits_of_unsigned_float.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(!asm.contains("ucvtf"), "the compare was not folded:\n{asm}");
}
