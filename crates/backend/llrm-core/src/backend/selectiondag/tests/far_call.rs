use super::super::legalize_types::TypeLegalizer;
use super::super::x86isd as own;
use super::*;
use crate::backend::selectiondag::MemBase;

/// A call through a far pointer (a function table in far code) stopped the
/// type legalizer: "Target(1): expand operand" (6 programs of the m16
/// execution run). The offset and the selector go to a 4-byte stack cell and
/// the call reads the cell (SD208).
#[test]
fn a_call_through_a_far_pointer_calls_through_a_cell_holding_both_words() {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let (offset, selector) = (dag.get_register(EVT::I16, 7), dag.get_register(EVT::I16, 8));
    let far = dag.get_node(ISD::BuildPair, MVT::Far.into(), &[offset, selector]);
    let (entry, mask) = (dag.entry_node(), dag.get_register_mask());
    let call = dag.get_node_multi(own::CALL, &[EVT::OTHER, EVT::GLUE], &[entry, far, mask]);
    dag.set_root(SDValue { node: call.node, res: 0 });
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), own::LLRM_FARCALL);
    let cell = dag.operand(root, 1);
    assert_eq!(dag.opcode(cell), ISD::TargetFrameIndex);
    assert_eq!(dag.operand(root, 2), mask, "the rest of the call is as it was");
    let stores = dag.operand(root, 0);
    assert_eq!(dag.opcode(stores), ISD::TokenFactor);
    let (low, high) = (dag.operand(stores, 0), dag.operand(stores, 1));
    for (store, value, at) in [(low, offset, 0), (high, selector, 2)] {
        assert_eq!(dag.opcode(store), ISD::Store);
        assert_eq!(dag.operand(store, 1), value);
        let NodeData::Mem(info) = dag.node(store).data else { panic!("a store") };
        assert!(matches!(info.base, MemBase::Stack(_)));
        let address = dag.operand(store, 2);
        let displacement = if at == 0 { 0 } else { dag.constant(dag.operand(address, 1)).unwrap() };
        assert_eq!(displacement, at);
    }
}
