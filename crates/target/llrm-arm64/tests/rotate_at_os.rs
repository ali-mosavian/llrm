//! At -Os a loop is rotated only when the copied exit test folds in the preheader.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// sieve -Os: the outer loop was rotated with its unfoldable `i*i <= n` test
/// copied into the preheader, one instruction (4 B) over the unrotated loop;
/// the same on jpeg_single (-84 B), jpeg_fixed, quicksort, shellsort.
#[test]
fn a_loop_whose_copied_exit_test_does_not_fold_is_not_rotated_at_os() {
    let asm = driver::assembly(include_str!("data/sieve_os.ll"), Platform::Linux, Level::Os).expect("lowers");
    let insts = asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(insts >= 40, "premise: the function is lowered:\n{asm}");
    assert!(insts <= 51, "{insts} instructions (rotated: 52):\n{asm}");
}
