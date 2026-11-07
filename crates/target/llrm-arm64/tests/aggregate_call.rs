//! A call with an aggregate argument passes every leaf: `by_value(struct S)`
//! at -O0 passed `[2 x i64]` as `x0` only, the next argument in `x1` instead of
//! `x2`, and tests/run/c/ia16_abi printed 60 where clang prints 50.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

#[test]
fn an_array_argument_is_passed_in_both_registers() {
    let asm = driver::assembly(include_str!("data/aggregate_call.ll"), Platform::Linux, Level::O0).expect("lowers");
    let caller = asm.split("caller:").nth(1).expect("caller");
    assert!(caller.contains("ldr x1, [") && caller.contains("movz w2, #9"), "{asm}");
}
