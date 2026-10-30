//! LICM's promoted loads at a loop's entry read the stores just before them
//! (InstCombine's `visitLoadInst` forwards them): without that the trip count
//! stayed unknown to the full unroll, and the two summing loops of split.bas
//! stayed two loops where the pin folds them to `R% = 275` (-Os m16, +32 B).

use std::process::Command;

#[test]
fn test_two_counted_summing_loops_over_basic_variables_fold_to_their_sum_at_os() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("split.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .arg(format!("{}/tests/run/qb/split.bas", env!("CARGO_MANIFEST_DIR")))
        .args(["--dialect", "qb45", "--runtime", "qb45", "-Os", "-S", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(out).unwrap();
    assert!(text.contains("275"), "the sum is not a constant: {text}");
    assert!(!text.lines().any(|line| line.trim().starts_with("jne ")), "a counted loop is left: {text}");
}
