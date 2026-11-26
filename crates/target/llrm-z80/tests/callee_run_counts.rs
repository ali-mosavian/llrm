//! The choice between the divide's size and speed routines counted a
//! function's clocks once however often it was called: a divide run 10,000
//! times took the routine of a bit a trip for a few hundred bytes.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_divide_in_a_function_a_loop_nest_calls_keeps_the_speed_routines_at_size() {
    let ir = include_str!("data/divide_in_loop_nest.ll");
    let divide = text(ir, "divide", 1, &Request::new(Level::Os)).expect("compiles");
    assert!(!divide.contains("__s_"), "the size routines are called:\n{divide}");
    assert!(divide.contains("__divdi3") && divide.contains("__udivdi3"), "{divide}");
}
