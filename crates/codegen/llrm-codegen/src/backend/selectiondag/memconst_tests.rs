//! `memcpy` from a constant array of integers, floats or bytes stores
//! immediates on the root (`getMemsetStringVal`); from a struct or a
//! variable it loads. Before, any constant source stopped the builder at
//! "memcpy from constant data".

use super::bundle_tests::{ours, pins};

#[test]
fn memcpy_from_constant_data_matches_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/memconst.ll")), pins(include_str!("golden/memconst.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
