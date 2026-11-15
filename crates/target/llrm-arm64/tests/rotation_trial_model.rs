//! DEVIATE ROTATION-TRIAL prices the finished code, laid out.

fn instructions(trial: bool) -> usize {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/rotation_trial_recsum.ll")).unwrap();
    let mut llc = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    llc.args(["-S", "-Os", "-o"]).arg(&output).arg(&input);
    if !trial {
        llc.env("LLRM_ARM64_NO_TRIAL", "1");
    }
    let said = llc.output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    let insts = asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(insts >= 10, "premise: the function is lowered:\n{asm}");
    insts
}

/// recsum -Os: the unrotated loop is 8 B smaller and runs the same 10005
/// instructions, but the trial priced the blocks before layout (a `b` to the
/// next block a trip, 293 against 136.5) and the guard's zero heuristic made
/// the rotated loop 20 trips against 31, so it always put the rotated build
/// back: 21 instructions with the trial, as without it.
#[test]
fn a_loop_unrotated_is_kept_when_smaller_and_laid_out_to_run_the_same_at_os() {
    assert_eq!((instructions(true), instructions(false)), (19, 21));
}
