//! The speed inline rate binds at -O1 on arm64 (`speed_inline_rate_at_o1`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// jpeg -O1 after #8148 (the rate off at -O1 for every target): the pin's
/// threshold alone inlined 5,440 B of kernel into 6,556 (jpeg_single 5,376 ->
/// 6,676) for 3% fewer instructions. A 14-operation callee called twice from a
/// loop is the shape: the rate keeps the calls at -O1 as at -O3.
#[test]
fn a_medium_callee_called_twice_in_a_loop_stays_a_call_at_o1() {
    let text = include_str!("data/inline_o1_rate.ll");
    let calls = |level| driver::assembly(text, Platform::Linux, level).expect("lowers").matches("\tbl ").count();
    assert!(calls(Level::O3) > 0, "premise: the rate keeps the calls at -O3");
    assert_eq!(calls(Level::O1), calls(Level::O3), "-O1 inlined past the rate");
}
