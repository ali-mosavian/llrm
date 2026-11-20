//! `and (trunc (lshr x, 52)), 2047` of an `i32` is one `ubfm x0, x0, #52, #62`
//! (the pin's `isBitfieldExtractOpFromAnd`, `TRUNCATE` arm). It was `ubfm` and
//! `and w0, w9, #0x7ff`: QuickrBasic `src/core/num/double.c` `round_whole`
//! (the exponent of a double), 4 B over the pin.

#[test]
fn a_masked_truncated_shift_is_one_bitfield_extract() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/bitfield_extract_through_truncate.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    let exp = asm.split("exp64:").next().unwrap();
    assert!(exp.contains("ubfm x0, x0, #52, #62"), "no single extract:\n{exp}");
    assert!(!exp.contains("and w"), "the mask stayed:\n{exp}");
}
