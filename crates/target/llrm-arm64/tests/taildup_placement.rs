//! Tail duplication in block placement (`TailDupPlacement`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn asm() -> String {
    driver::assembly(include_str!("data/taildup_tre_loop.ll"), Platform::Linux, Level::O1).expect("lowers")
}

/// bench/frames `fib` (clang's IR, no passes run) becomes a loop whose header
/// is `subs; b.le` and whose latch jumps back to it. Placement copies the
/// header's test into the latch (`maybeTailDuplicateBlock`), so the loop
/// ends in the conditional branch back (`bl; sub; add; subs; b.gt`: 5
/// instructions an iteration). Without the copy every iteration also takes an
/// unconditional `b` (6): frames -O1 +3.3% instructions.
#[test]
fn a_loop_latch_takes_the_headers_test_instead_of_jumping_to_it() {
    let asm = asm();
    assert!(asm.contains("bl fib"), "the function is not there:\n{asm}");
    let jumps: Vec<&str> = asm.lines().filter(|line| line.trim_start().starts_with("b ")).collect();
    assert!(jumps.is_empty(), "an unconditional jump is left: {jumps:?}\n{asm}");
}

/// The premise: the loop is there, one `bl` to `fib` and a conditional branch.
#[test]
fn the_loop_calls_fib_and_branches_back_conditionally() {
    let asm = asm();
    assert_eq!(asm.matches("bl fib").count(), 1, "{asm}");
    assert!(asm.lines().any(|line| line.trim_start().starts_with("b.")), "{asm}");
}
