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

/// A branch on a constrained `fp128` compare (`#pragma STDC FENV_ACCESS ON`)
/// was `cmp; cset; tbz`: the legalizer met the branch before the compare, which
/// it softened afterwards, so the branch read a materialised bit. Pin: `bl
/// __gttf2; cmp w0, #0; b.le` (QuickrBasic `src/plat/host/pfpu.c`, +104 B).
#[test]
fn a_branch_on_a_constrained_fp128_compare_reads_the_flags_of_the_routines_result() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/fp128_strict_compare_branch.ll")).unwrap();
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

/// pfpu.c `checked` (reduced): four branches on constrained `fp128` compares,
/// some through the `xor c, 1` that inverts a branch. Each was `cmp; cset;
/// tbz w9`, a bit made and tested.
#[test]
fn branches_on_constrained_fp128_compares_test_no_materialised_bit() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/fp128_strict_compare_branches.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(!asm.contains("tbz w9") && !asm.contains("tbnz w9"), "a compare was materialised:\n{asm}");
}
