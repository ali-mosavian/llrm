use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

fn combined(carry: impl FnOnce(&mut SelectionDAG) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I16, 5), dag.get_register(EVT::I16, 6));
    let (carry, cc) = (carry(&mut dag), dag.get_cond_code(CondCode::SetGe));
    let root = dag.get_node(ISD::SetCCCarry, EVT::I1, &[x, y, carry, cc]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    (dag, root)
}

/// A compare of a long with a constant whose low words are zero has `usubo x,
/// 0` as its low borrow, which folds to no borrow: the node stayed a
/// `SetCCCarry` of a constant zero, which the 6502 and z80 backends refused
/// ("convert node to libcall"). The pin's `visitSETCCCARRY` makes it a `setcc`.
#[test]
fn a_setcc_carry_with_no_borrow_in_is_a_setcc() {
    let (dag, root) = combined(|dag| dag.get_constant(0, EVT::I1));
    assert_eq!(dag.opcode(root), ISD::SetCC);
    assert_eq!(dag.node(dag.operand(root, 2)).data, NodeData::CondCode(CondCode::SetGe));
}

#[test]
fn a_setcc_carry_with_a_borrow_in_stays() {
    let (dag, root) = combined(|dag| dag.get_register(EVT::I1, 7));
    assert_eq!(dag.opcode(root), ISD::SetCCCarry);
}
