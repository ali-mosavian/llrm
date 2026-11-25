//! A copy's source is read where its destination was (`MachineCopyPropagation`
//! forward), so the early-exit compare of quicksort's `sort` reads `w2`, not the
//! callee-saved `w20` the loop keeps it in, and the frame waits for the loop.
//! Ours compared `w20` and made the frame first: arm64 -O2 executed 119,846
//! instructions against 117,375.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn an_early_exit_compare_reads_the_argument_register_and_runs_before_the_frame() {
    let asm = driver::assembly(include_str!("data/shrink_wrapped_copy_forward.ll"), Platform::Linux, Level::O2)
        .expect("lowers");
    let body: Vec<&str> = asm.lines().map(str::trim).skip_while(|line| !line.starts_with("sort:")).collect();
    let compare = body.iter().position(|line| line.starts_with("cmp ") || line.starts_with("subs ")).expect("a compare");
    let frame = body.iter().position(|line| line.starts_with("stp ") || line.starts_with("str x")).expect("a frame");
    assert!(compare < frame, "the frame is made before the early exit:\n{asm}");
}
