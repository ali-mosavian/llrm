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
    let info = MemInfo { mem_vt: EVT::I16, align: 2, volatile: false, base: MemBase::Unknown, ext: None, space: 0 };
    let store = dag.get_store(store_chain, sum, ptr, info);
    dag.set_root(store);
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
}
