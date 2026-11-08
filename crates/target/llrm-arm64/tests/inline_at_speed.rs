//! At -O2 an inline that grows the code is taken when the clocks it saves pay for the bytes.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// A byte of growth was priced at 250 clocks (the trade point gcc and clang sit
/// at over whole runs), not at the 0.3 clock a byte costs: a 12-instruction
/// helper called from two loops stayed a function (3 calls, 66 instructions)
/// where the copies are +20 instructions for the call, the frame and the
/// argument moves of each site in the loops.
#[test]
fn a_helper_called_from_loops_is_inlined_where_the_clocks_pay_for_the_bytes() {
    let asm = driver::assembly(include_str!("data/speed_inline_sites.ll"), Platform::Linux, Level::O2).expect("lowers");
    let calls = asm.lines().filter(|line| line.trim().starts_with("bl ") && line.contains("mix")).count();
    assert!(asm.lines().any(|line| line.starts_with("f:")), "premise: lowered:\n{asm}");
    assert_eq!(calls, 0, "{calls} calls of mix remain:\n{asm}");
}
