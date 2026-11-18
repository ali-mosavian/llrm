//! `ComputeNumSignBits` took no depth: it walked every path of the expression
//! down to the leaves, so a chain whose steps each read the one before twice
//! (`a' = a ^ b; b' = a & b`) cost 2^n visits. A straight-line function of 400
//! such statements spent 99.9% of its compile in `dag legalize` (cubic in the
//! statements). The pin stops at `MaxRecursionDepth` (6).

use super::super::known_bits::{SIGN_BITS_VISITS, num_sign_bits};
use super::*;

#[test]
fn the_sign_bits_of_a_deep_chain_are_found_in_a_bounded_number_of_visits() {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I8, 5), dag.get_register(EVT::I8, 6));
    let mut a = dag.get_node(ISD::SignExtend, EVT::I32, &[x]);
    let mut b = dag.get_node(ISD::SignExtend, EVT::I32, &[y]);
    for _ in 0..24 {
        let (xor, and) = (dag.get_node(ISD::Xor, EVT::I32, &[a, b]), dag.get_node(ISD::And, EVT::I32, &[a, b]));
        (a, b) = (xor, and);
    }
    let before = SIGN_BITS_VISITS.with(std::cell::Cell::get);
    let bits = num_sign_bits(&dag, a);
    let visits = SIGN_BITS_VISITS.with(std::cell::Cell::get) - before;
    assert!(bits >= 1);
    assert!(visits < 200, "{visits} values looked at for a chain of 24 steps");
}
