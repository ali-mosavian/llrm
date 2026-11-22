//! DEVIATE ROTATION-TRIAL prices the finished code, laid out.

/// recsum -Os: the unrotated loop is 8 B smaller and runs the same 10005
/// instructions, but the trial priced the blocks before layout (a `b` to the
/// next block a trip, 293 against 136.5) and the guard's zero heuristic made
/// the rotated loop 20 trips against 31, so it always put the rotated build
/// back. Read from the trial's note, not an instruction count: the no-trial
/// build also expands the `srem` by 11 (the pin rule, SD535), which is not
/// what this measures.
#[test]
fn a_loop_unrotated_is_kept_when_smaller_and_laid_out_to_run_the_same_at_os() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/rotation_trial_recsum.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .env("LLRM_DEBUG", "trial")
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let log = String::from_utf8_lossy(&said.stderr);
    let unrotated = log.lines().find(|line| line.starts_with("trial unrotated:")).expect("premise: the trial ran");
    assert!(unrotated.ends_with(": kept"), "{log}");
}

/// fib -Os: the unrotated loop is 8 B smaller (64 -> 56) and the model reads it
/// 0.7% slower laid out (260.1 -> 262), which the strict "not slower" rule
/// read as a loss and put back: QB-bench fib was 88 B against gcc's 80.
/// DEVIATE OS-TRIAL-CLOCK-CAP keeps it within the 1% cap.
#[test]
fn a_loop_unrotated_is_kept_for_bytes_when_laid_out_within_the_clock_cap_at_os() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/rotation_trial_fib.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .env("LLRM_DEBUG", "trial")
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let log = String::from_utf8_lossy(&said.stderr);
    let unrotated = log.lines().find(|line| line.starts_with("trial unrotated:")).expect("premise: the trial ran");
    assert!(unrotated.ends_with(": kept"), "{log}");
}
