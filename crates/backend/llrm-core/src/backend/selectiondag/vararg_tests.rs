//! `va_start`, `va_end` and `va_copy` build `vastart`, `vaend` and `vacopy`
//! with a `SrcValue` per list, on the root, as the pin's initial DAGs do.
//! Before, each stopped the builder at "intrinsic call".

use super::bundle_tests::{ours, pins};

#[test]
fn va_intrinsics_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/vararg.ll")), pins(include_str!("golden/vararg.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
