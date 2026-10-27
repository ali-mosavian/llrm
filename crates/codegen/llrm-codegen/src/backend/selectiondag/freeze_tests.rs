//! `freeze` builds `FREEZE` and folds away for a constant, as the pin's
//! initial DAGs show. Before, it stopped the builder at "instruction freeze"
//! (6 corpus blocks).

use super::bundle_tests::{ours, pins};

#[test]
fn freeze_matches_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/freeze.ll")), pins(include_str!("golden/freeze.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
