//! Two paths that end in the same calls and then meet share those calls
//! (`BranchFolder` tail merge): each wrote `g(11); h(22, 33); k(44)` out in
//! full (jpeg -Os: 64 bytes over what the pin's merge leaves).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn two_paths_that_end_in_the_same_calls_share_them() {
    let asm = driver::assembly(include_str!("data/tail_merge_calls.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert_eq!(asm.matches("bl h").count(), 1, "{asm}");
}
