//! A dynamic `alloca` (a variable size, or any outside the entry block) is
//! `DYNAMIC_STACKALLOC` on the size scaled, rounded to the stack alignment
//! and masked, as the pin's initial DAGs show. Before it stopped the builder
//! at "instruction alloca".

use super::bundle_tests::{ours, pins};

#[test]
fn dynamic_allocas_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/alloca.ll")), pins(include_str!("golden/alloca.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
