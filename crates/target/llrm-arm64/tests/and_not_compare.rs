//! `(x & C) == C` is `(~x & C) == 0`, one `bics` (the pin's
//! `TargetLowering::SimplifySetCC` with AArch64's `hasAndNotCompare`). It was
//! `and`, `mov`, `cmp` (3 instructions for 2): QuickrBasic
//! `src/core/num/double.c` tests an infinity this way in `fmt` and `dubl_fn`.
//! Its register is dead, so it is the zero register
//! (`AArch64DeadRegisterDefinitions`), as `tst` is: `bics w9, w9, w0` kept a
//! register live (QuickrBasic `known_loads_walks` O2 +8 B).

#[test]
fn a_masked_compare_with_its_mask_is_one_bics() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/and_not_compare.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(asm.contains("bics xzr,"), "no bics of the 64-bit compare, or its result is kept:\n{asm}");
    assert!(asm.contains("bics wzr,"), "no bics of the 32-bit compare, or its result is kept:\n{asm}");
    assert!(!asm.contains("cmp ") && !asm.contains("subs "), "a compare stayed:\n{asm}");
}
