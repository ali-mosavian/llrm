//! SwitchCG with the IR switch\'s own edge probabilities (SD101 closed): a case
//! whose block is `unreachable` or ends in a `noreturn` call is nearly never
//! taken, which changes how the clusters are split and ordered. Uniform
//! probabilities (1/(cases+1)) make other blocks.

use super::bundle_tests::{ours, pins};

#[test]
fn switches_with_cold_cases_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/switchbpi.ll")), pins(include_str!("golden/switchbpi.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
