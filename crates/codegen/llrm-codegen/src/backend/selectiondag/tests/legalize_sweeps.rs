use super::super::legalize_dag::{DagLegalizer, NoHooks};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

/// A DAG that was already legal was swept twice, the second sweep only to
/// find no node it had not seen: a topological sort and a hash set over the
/// whole DAG per block (2.3 M Ir of 119 on jpeg_fixed -O2).
#[test]
fn a_legal_dag_is_swept_once() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (a, b) = (dag.get_register(EVT::I32, 8), dag.get_register(EVT::I32, 9));
    let sum = dag.get_node(ISD::Add, EVT::I32, &[a, b]);
    let (chain, ptr) = (dag.entry_node(), dag.get_register(EVT::I32, 7));
    let info = MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(1), ext: None, space: 0 , ordering: None };
    let store = dag.get_store(chain, sum, ptr, info);
    dag.set_root(store);
    let mut hooks = NoHooks;
    let mut legalizer = DagLegalizer::new(&mut dag, &tli, &mut hooks);
    legalizer.run().unwrap();
    assert_eq!(legalizer.sweeps, 1);
}
