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

/// rectwo -Os: `n - 1`, invariant in the loop that calls `paths` again, was
/// hoisted into the entry block (LICM with speculation, the loop being
/// unrotated); it lived in x19 across the call, so the frame was built before
/// the `n == 0` test and every base-case call (most of 970,000 instructions
/// against clang's 778,000) paid the prologue and epilogue.
#[test]
fn the_base_case_of_a_recursive_loop_returns_before_the_frame_is_built_at_os() {
    let asm = driver::assembly(include_str!("data/frame_late_os.ll"), Platform::Linux, Level::Os).expect("lowers");
    let body: Vec<&str> =
        asm.lines().skip_while(|line| *line != "paths:").skip(1).take_while(|line| !line.ends_with(':') || line.starts_with(".L")).collect();
    let first_frame = body.iter().position(|line| line.trim().starts_with("stp x29, x30"));
    let first_test = body.iter().position(|line| line.trim().starts_with("cbz"));
    assert!(first_frame.is_some() && first_test.is_some(), "premise: a framed function:\n{asm}");
    assert!(first_test < first_frame, "the frame is built before the n == 0 test:\n{asm}");
}
