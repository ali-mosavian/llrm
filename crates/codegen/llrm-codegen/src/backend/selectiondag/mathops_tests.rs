//! The arithmetic intrinsics (`smin`..`umax`, bit counts, `bswap`, `abs`, the
//! float functions, `lrint`) build the pin's one node each, with its
//! zero-is-poison variants. Before, each stopped the builder at "intrinsic
//! call" (about 35 corpus blocks at m32).

use super::bundle_tests::{ours, pins};

#[test]
fn arithmetic_intrinsics_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/mathops.ll")), pins(include_str!("golden/mathops.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
