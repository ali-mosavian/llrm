//! `getConstant` is the hottest intern of a compile: memexp -O2 made 1.49 M
//! of them (334 M Ir, 224 each, the pin's 74) through a six-field `Key` and a
//! `dyn` compare. A constant is now found by value.

use super::{Ap, ScalarEvolution};

#[test]
fn a_constant_asked_again_is_the_same_node_found_by_value() {
    let mut se = ScalarEvolution::new();
    let first: Vec<_> = [(32, 7), (32, 8), (16, 7)].iter().map(|&(w, v)| se.get_constant(Ap::new(w, v))).collect();
    let nodes = se.nodes.len();
    for _ in 0..1000 {
        for (&(w, v), &node) in [(32, 7), (32, 8), (16, 7)].iter().zip(&first) {
            assert_eq!(se.get_constant(Ap::new(w, v)), node);
        }
    }
    assert_eq!(se.nodes.len(), nodes);
    assert_eq!(se.constants.len(), 3, "one entry for each width and value");
}
