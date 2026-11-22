//! Blocks that end alike share their end after the blocks below them merged.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// QuickrBasic `resolve` -Os: two arms each called `own`, loaded, and
/// branched to a copy of the same two blocks. The copies merged, but the
/// arms kept `b copy` against `b merged` and so never matched: two `bl own`
/// and a second run of the loads (+60 B against the pin, which retargets the
/// branch of a merged block between rounds, `OptimizeBranches`).
#[test]
fn arms_whose_successors_merged_merge_in_the_next_round() {
    let asm = driver::assembly(include_str!("data/tail_merge_rounds.ll"), Platform::Linux, Level::O0).expect("lowers");
    assert_eq!(asm.matches("bl own").count(), 1, "{asm}");
}
