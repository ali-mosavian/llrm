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

/// The stores of a far call's cell come before `callseq_start`, not after the
/// argument copies glued to the call: the call then needed what came after it
/// (a cycle), the scheduler left the whole sequence out, and a call through a
/// function table vanished with its loads (function_table at m16).
#[test]
fn the_cell_is_stored_before_the_call_sequence_starts() {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let (offset, selector) = (dag.get_register(EVT::I16, 7), dag.get_register(EVT::I16, 8));
    let far = dag.get_node(ISD::BuildPair, MVT::Far.into(), &[offset, selector]);
    let (entry, mask) = (dag.entry_node(), dag.get_register_mask());
    let start = dag.get_callseq_start(entry, 0);
    let copy = dag.get_copy_to_reg(SDValue { node: start.node, res: 0 }, 5, offset);
    let call = dag.get_node_multi(own::CALL, &[EVT::OTHER, EVT::GLUE], &[copy, far, mask]);
    dag.set_root(SDValue { node: call.node, res: 0 });
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), own::LLRM_FARCALL);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::CopyToReg, "the call follows the copies directly");
    let before = dag.operand(start, 0);
    assert_eq!(dag.opcode(before), ISD::TokenFactor, "the stores are joined before the sequence starts");
    let stores = (0..dag.node(before).ops.len()).filter(|i| dag.opcode(dag.operand(before, *i)) == ISD::Store).count();
    assert_eq!(stores, 2);
}

/// The call sequence of a far call whose arguments are stored to the stack
/// (`callseq_start`, the stores joined by a `TokenFactor`, the register copies,
/// the call): the cell's stores are found behind that join too. Missing the
/// start, they were chained after the copies, the scheduler's graph had a
/// cycle and the compile ran out of memory (regparm3_matrix_22 `_main`).
#[test]
fn the_start_is_found_behind_the_argument_stores() {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let (offset, selector) = (dag.get_register(EVT::I16, 7), dag.get_register(EVT::I16, 8));
    let far = dag.get_node(ISD::BuildPair, MVT::Far.into(), &[offset, selector]);
    let (entry, mask) = (dag.entry_node(), dag.get_register_mask());
    let start = dag.get_callseq_start(entry, 0);
    let slot = dag.get_frame_index(EVT::I16, 0);
    let info = super::super::dag::MemInfo {
        mem_vt: EVT::I16,
        align: 2,
        volatile: false,
        base: MemBase::Stack(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let argument = dag.get_store(SDValue { node: start.node, res: 0 }, offset, slot, info);
    let joined = dag.get_token_factor(&[argument]);
    let copy = dag.get_copy_to_reg(joined, 5, offset);
    let call = dag.get_node_multi(own::CALL, &[EVT::OTHER, EVT::GLUE], &[copy, far, mask]);
    dag.set_root(SDValue { node: call.node, res: 0 });
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), own::LLRM_FARCALL);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::CopyToReg, "the call follows the copies directly");
    assert_eq!(dag.opcode(dag.operand(start, 0)), ISD::TokenFactor, "the cell's stores precede the sequence");
}
