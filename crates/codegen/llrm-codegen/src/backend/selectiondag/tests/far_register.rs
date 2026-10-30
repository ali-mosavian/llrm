use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

/// A far pointer a call returns comes back in the register pair a dword
/// stands for: `CopyFromReg` of a far value stopped the type legalizer
/// ("CopyFromReg: expand result", wc16_mixed `main`). It is read as the dword
/// and split: offset low, selector high.
#[test]
fn a_far_pointer_returned_in_a_register_pair_is_read_as_the_dword() {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let (entry, eax) = (dag.entry_node(), dag.get_phys_register(MVT::Far.into(), llrm_lir::registers::RegId::MM0));
    let read = dag.get_node_multi(ISD::CopyFromReg, &[MVT::Far.into(), EVT::OTHER], &[entry, eax]);
    let offset = dag.get_node(ISD::Truncate, EVT::I16, &[SDValue { node: read.node, res: 0 }]);
    let store_chain = SDValue { node: read.node, res: 1 };
    let sum = dag.get_node(ISD::Add, EVT::I16, &[offset, offset]);
    let ptr = dag.get_register(EVT::I16, 7);
    let info = MemInfo { mem_vt: EVT::I16, align: 2, volatile: false, base: MemBase::Unknown, ext: None, space: 0, ordering: None };
    let store = dag.get_store(store_chain, sum, ptr, info);
    dag.set_root(store);
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
}

/// A far pointer returned in the register pair (`CopyToReg $eax, far`, the
/// `mk8` of the regparm3 matrix) stopped the type legalizer: "CopyToReg:
/// expand operand". The halves are joined into the dword the pair names.
#[test]
fn a_far_pointer_copied_to_a_register_pair_is_the_dword_of_its_halves() {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let (offset, selector) = (dag.get_register(EVT::I16, 7), dag.get_register(EVT::I16, 8));
    let far = dag.get_node(ISD::BuildPair, MVT::Far.into(), &[offset, selector]);
    let (entry, eax) = (dag.entry_node(), dag.get_phys_register(EVT::I32, llrm_lir::registers::RegId::MM0));
    let copy = dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER, EVT::GLUE], &[entry, eax, far]);
    dag.set_root(SDValue { node: copy.node, res: 0 });
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
    let root = dag.get_root();
    let dword = dag.operand(root, 2);
    assert_eq!((dag.opcode(dword), dag.value_type(dword)), (ISD::BuildPair, EVT::I32));
    assert_eq!((dag.operand(dword, 0), dag.operand(dword, 1)), (offset, selector));
}

/// A far pointer loaded through a far pointer (`les` at `es:[bx]`) stopped
/// the type legalizer at the `les` itself: "Target(24576): expand operand"
/// (c/fsgs_call.c). `ES` takes the selector first, the `les` is at the offset.
#[test]
fn a_les_through_a_far_pointer_sets_es_first() {
    use super::super::x86_lowering::{ES, x86isd};
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let (offset, selector) = (dag.get_register(EVT::I16, 7), dag.get_register(EVT::I16, 8));
    let far = dag.get_node(ISD::BuildPair, MVT::Far.into(), &[offset, selector]);
    let entry = dag.entry_node();
    let les = dag.get_node_multi(x86isd::LES, &[EVT::I16, EVT::OTHER], &[entry, far]);
    dag.set_root(SDValue { node: les.node, res: 1 });
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
    let les = SDValue { node: dag.get_root().node, res: 0 };
    assert_eq!(dag.operand(les, 1), offset);
    let set_es = dag.operand(les, 0);
    assert_eq!(dag.opcode(set_es), ISD::CopyToReg);
    assert_eq!((dag.node(dag.operand(set_es, 1)).data, dag.operand(set_es, 2)), (NodeData::Register(ES), selector));
}
