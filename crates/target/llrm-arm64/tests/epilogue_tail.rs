//! Two exits that restore the same registers share the epilogue
//! (`BranchFolder` tail merge): `paths` wrote `ldp x29, x30, [sp], #48` twice,
//! one for the loop's exit and one for the early one (rectwo -O2: 4
//! instructions and 16 bytes over the pin).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn two_exits_share_one_epilogue() {
    let asm = driver::assembly(include_str!("data/epilogue_tail.ll"), Platform::Linux, Level::O2).expect("lowers");
    let paths = asm.split("\npaths:").nth(1).expect("paths").split("\nmain:").next().expect("a body");
    assert_eq!(paths.matches("ldp x29, x30, [sp], #").count(), 1, "{asm}");
}
