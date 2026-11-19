//! A branch on one `fcmp` is `fcmp` and `b.cc` (the pin's `LowerBR_CC`). The
//! compare was lowered to a `csinc` first and the branch tested its bit:
//! `fcmp; csinc w9, wzr, wzr, le; tbz w9, #0` (8 B more per branch; QuickrBasic
//! `src/core/num/double.c` `dubl_add`).

#[test]
fn a_branch_on_a_float_compare_reads_the_flags() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/fp_compare_branch.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(
        !asm.contains("csinc") && !asm.contains("cset") && !asm.contains("tbz"),
        "the compare was materialised:\n{asm}"
    );
    assert!(asm.contains("b.le") || asm.contains("b.gt"), "no conditional branch on the flags:\n{asm}");
}

/// The same fold on an `fp128` compare reached `emit_fcmp` and failed
/// (`CannotSelect`): the compare is a routine call, then an integer branch.
#[test]
fn a_branch_on_an_fp128_compare_calls_the_routine() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/fp128_compare_branch.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    assert!(std::fs::read_to_string(&output).unwrap().contains("bl __gttf2"));
}
