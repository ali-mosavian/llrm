//! `SimplifyDemandedBits` asked `computeKnownBits` for each operand from depth
//! 0, where the pin passes its own depth: a walk of `MaxRecursionDepth` levels
//! under each of the `MaxRecursionDepth` levels it was already down: 13,628
//! values looked at for a chain of 12 steps, 5,230 at the depth it is at.

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::super::known_bits::KNOWN_VISITS;
use super::*;

#[test]
fn simplifying_demanded_bits_asks_known_bits_at_its_own_depth() {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let (mut a, mut b) = (x, y);
    for step in 0..12 {
        let mask = dag.get_constant(0x0F0F_0F0F >> (step % 4), EVT::I32);
        let masked = dag.get_node(ISD::And, EVT::I32, &[a, mask]);
        let (xor, or) = (dag.get_node(ISD::Xor, EVT::I32, &[masked, b]), dag.get_node(ISD::Or, EVT::I32, &[a, b]));
        (a, b) = (xor, or);
    }
    let root = dag.get_node(ISD::Xor, EVT::I32, &[a, b]);
    dag.set_root(root);
    let before = KNOWN_VISITS.with(std::cell::Cell::get);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let visits = KNOWN_VISITS.with(std::cell::Cell::get) - before;
    assert!(visits < LIMIT, "{visits} values looked at by known bits for a chain of 12 steps");
}

const LIMIT: usize = 8000;
