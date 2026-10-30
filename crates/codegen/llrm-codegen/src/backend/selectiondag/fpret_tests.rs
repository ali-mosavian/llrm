//! A call's float result is `X86ISD::POP_FROM_X87_REG chain, $fp0, glue` on
//! the call's chain, as the pin's initial DAGs show. Before, a call of a
//! function returning `float` or `double` stopped the builder at
//! "floating-point result of a call" (6 corpus blocks at m32).

use super::bundle_tests::{ours, pins};

#[test]
fn float_call_results_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/fpret.ll")), pins(include_str!("golden/fpret.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
