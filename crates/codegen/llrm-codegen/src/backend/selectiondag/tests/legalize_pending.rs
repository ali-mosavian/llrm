use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

/// A node reading two results of one producer (a store of a loaded value
/// on the load's own chain) waited for a second release that never came:
/// it and everything after it was never looked at, so the `i1` condition of
/// the branch below it stayed unpromoted and reached selection as
/// `X86ISD::CMP` on an `i1` (SB_BUILD, CannotSelect on the m32 shadow).
#[test]
fn a_node_reading_two_results_of_one_producer_is_still_legalized() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
    let info = MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(1), ext: None, space: 0 , ordering: None };
    let load = dag.get_load(EVT::I32, chain, ptr, info);
    let store = dag.get_store(SDValue { node: load.node, res: 1 }, load, ptr, info);
    let (a, b) = (dag.get_register(EVT::I32, 8), dag.get_register(EVT::I32, 9));
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
    let dest = dag.get_basic_block(1);
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[store, test, dest]);
    dag.set_root(branch);
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
}

/// `add x, x` reads one value in two operand slots: also released once.
#[test]
fn a_node_reading_one_value_twice_is_still_legalized() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 8);
    let y = dag.get_node(ISD::Mul, EVT::I32, &[x, x]);
    let test = dag.get_setcc(EVT::I1, y, x, CondCode::SetLt);
    let (chain, dest) = (dag.entry_node(), dag.get_basic_block(1));
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, test, dest]);
    dag.set_root(branch);
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
}
