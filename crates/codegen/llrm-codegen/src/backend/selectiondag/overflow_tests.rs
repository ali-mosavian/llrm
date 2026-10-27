//! `*.with.overflow` is the pin's two-result node (`saddo`..`umulo`) and
//! `extractvalue` picks one result of it. Before, the intrinsic stopped the
//! builder at "intrinsic call" and the extract at "instruction extractvalue".

use super::bundle_tests::{ours, pins};

#[test]
fn overflow_intrinsics_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/overflow.ll")), pins(include_str!("golden/overflow.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
