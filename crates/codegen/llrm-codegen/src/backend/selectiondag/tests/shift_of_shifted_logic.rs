use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

/// `srl (and (srl x, 15), 0xc000), 14` stayed three operations (`lsr; and;
/// add ..., lsr #14` for `sext(sdiv i16 x, 4)`, stft_fixed -O1); the pin
/// makes `and (srl x, 29), 3` (`combineShiftOfShiftedLogic`).
#[test]
fn a_shift_of_a_masked_shift_is_one_shift_and_a_shifted_mask() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let by = |dag: &mut SelectionDAG, n| dag.get_constant(n, EVT::I8);
    let (c15, c14) = (by(&mut dag, 15), by(&mut dag, 14));
    let inner = dag.get_node(ISD::Srl, EVT::I32, &[x, c15]);
    let mask = dag.get_constant(0xc000, EVT::I32);
    let masked = dag.get_node(ISD::And, EVT::I32, &[inner, mask]);
    let root = dag.get_node(ISD::Srl, EVT::I32, &[masked, c14]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::And);
    let (shifted, low) = (dag.operand(root, 0), dag.operand(root, 1));
    assert_eq!(dag.constant(low), Some(3));
    assert_eq!(dag.opcode(shifted), ISD::Srl);
    assert_eq!(dag.constant(dag.operand(shifted, 1)), Some(29));
}
