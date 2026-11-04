use super::*;

/// A target node of type `Other` whose second operand is `undef` (wasm's
/// `RETURN` of an undefined value, from the `ON ERROR` handler dispatch of
/// resume0.bas) asked `Other` for its size: llrm-qb --wasm32 panicked with
/// "Other has no size" on 20 programs of tests/run/qb.
#[test]
fn a_target_node_with_an_undef_operand_does_not_ask_for_a_size() {
    let mut dag = dag();
    let (chain, undef) = (dag.entry_node(), dag.get_undef(EVT::I32));
    let node = dag.get_node(ISD::Target(1), EVT::OTHER, &[chain, undef]);
    assert_eq!(dag.opcode(node), ISD::Target(1));
}
