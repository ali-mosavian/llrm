use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

/// `br (icmp slt a, b), then, else`: the condition is an `i1`. 82 of 160
/// functions of the select shadow stopped here, at "BrCond: promote
/// operand", and everything below it was hidden. After: the branch reads
/// an `i8` condition and nothing illegal is reachable.
#[test]
fn a_branch_on_an_i1_compare_reads_an_i8_condition() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let load = |dag: &mut SelectionDAG, slot: i32| {
        let info =
            MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0, ordering: None };
        dag.get_load(EVT::I32, chain, ptr, info)
    };
    let (a, b) = (load(&mut dag, 1), load(&mut dag, 2));
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
    let dest = dag.get_register(EVT::OTHER, 9);
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, test, dest]);
    dag.set_root(branch);
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    assert!(!legalizer.all_legal());
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::BrCond);
    assert_eq!(dag.value_type(dag.operand(root, 1)), EVT::I8);
    assert_eq!((dag.operand(root, 0), dag.operand(root, 2)), (chain, dest));
}
