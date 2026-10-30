//! `OptLevel == None`: the pin's `-O0` combiner skips the load, store and
//! chain combines. Without the switch wasm's `-O0` output forwarded a stored
//! value into its load (the pin's `llc -O0` keeps the load).

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::super::{MemBase, MemInfo};
use super::*;

fn info() -> MemInfo {
    MemInfo {
        mem_vt: EVT::I32,
        align: 4,
        volatile: false,
        base: MemBase::Stack(1),
        ext: None,
        space: 0,
        ordering: None,
    }
}

/// `store v -> [slot]; x = load [slot]; store x -> [out]`: the opcode of the
/// value the final store writes, combined with or without `-O0`.
fn written(opt_none: bool) -> ISD {
    let mut dag = dag();
    let entry = dag.entry_node();
    let (ptr, out) = (dag.get_register(EVT::I32, 7), dag.get_register(EVT::I32, 8));
    let value = dag.get_register(EVT::I32, 5);
    let store = dag.get_store(entry, value, ptr, info());
    let loaded = dag.get_load(EVT::I32, store, ptr, info());
    let last = dag.get_store(SDValue { node: loaded.node, res: 1 }, loaded, out, info());
    dag.set_root(last);
    let mut hooks = NoCombines;
    let combiner = DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes);
    if opt_none { combiner.with_opt_none() } else { combiner }.run();
    let root = dag.get_root();
    dag.opcode(dag.operand(root, 1))
}

#[test]
fn the_o0_combiner_keeps_a_load_after_its_store() {
    assert_eq!(written(false), ISD::Register, "the control: the optimizing combiner forwards the stored value");
    assert_eq!(written(true), ISD::Load);
}
