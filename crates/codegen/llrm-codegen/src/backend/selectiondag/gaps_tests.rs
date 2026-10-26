//! `scmp`/`ucmp`, `fmuladd`, `smul.fix`/`sdiv.fix` (widened by `expandDivFix`),
//! `assume`/`expect`, `stacksave`/`stackrestore` build what the pin's initial
//! DAGs show. Before, each stopped the builder at "intrinsic call".

use super::bundle_tests::{ours, pins};

#[test]
fn comparison_fused_fixed_hint_and_stack_intrinsics_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/gaps.ll")), pins(include_str!("golden/gaps.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}
