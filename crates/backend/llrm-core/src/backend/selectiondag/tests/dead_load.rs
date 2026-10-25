//! `visitLOAD`: a load nothing reads stayed in the chain, so the stores
//! after it waited for it.

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::super::{MemBase, MemInfo};
use super::*;

fn info(slot: i32) -> MemInfo {
    MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0 }
}

fn chain_after_load(volatile: bool) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let entry = dag.entry_node();
    let ptr = dag.get_register(EVT::I32, 7);
    let load = dag.get_load(EVT::I32, entry, ptr, MemInfo { volatile, ..info(1) });
    let value = dag.get_register(EVT::I32, 5);
    let store = dag.get_store(SDValue { node: load.node, res: 1 }, value, ptr, info(2));
    dag.set_root(store);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    (dag, root)
}

/// The store after a load nobody reads is chained to the load's input.
#[test]
fn a_load_nobody_reads_is_bypassed_in_the_chain() {
    let (dag, root) = chain_after_load(false);
    assert_eq!(dag.opcode(root), ISD::Store);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::EntryToken);
}

/// A volatile load stays: the store still follows it.
#[test]
fn a_volatile_load_nobody_reads_stays_in_the_chain() {
    let (dag, root) = chain_after_load(true);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Load);
}
