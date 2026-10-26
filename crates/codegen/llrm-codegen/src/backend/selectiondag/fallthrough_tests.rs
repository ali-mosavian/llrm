//! A conditional branch keeps its `br` to the false block even when that
//! block follows (`visitSwitchCase`: "do this even if it's a fall through
//! branch"), while an unconditional `br` to the next block is dropped
//! (`visitBr`). The pin's `finalize-isel` MIR has the `JMP_1` for the first.

use super::bundle_tests::{ours, pins};

#[test]
fn a_conditional_branch_keeps_its_br_to_the_next_block() {
    let (ours, pins) = (ours(include_str!("golden/fallthrough.ll")), pins(include_str!("golden/fallthrough.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
