//! `ForwardStoreValueToDirectLoad`: a load straight after a store to the same
//! address read memory the stored value was already in.

use super::super::actions::LoadExt;
use super::super::dag_combiner::{CombineLevel, NoCombines, combine};
use super::super::mvt::MVT;
use super::super::{MemBase, MemInfo};
use super::*;

const F64: EVT = EVT::Simple(MVT::F64);

fn info(
    vt: EVT,
    slot: i32,
) -> MemInfo {
    MemInfo { mem_vt: vt, align: 4, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0, ordering: None }
}

/// `store value -> [slot_store]; x = load [slot_load]; store x -> [out]`: the
/// value the final store writes, after the combiner ran.
fn written(
    store_vt: EVT,
    store_slot: i32,
    load: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue,
) -> (SelectionDAG, SDValue, SDValue) {
    let mut dag = dag();
    let entry = dag.entry_node();
    let ptr = dag.get_register(EVT::I32, 7);
    let out = dag.get_register(EVT::I32, 8);
    let value = dag.get_register(store_vt, 5);
    let store = dag.get_store(entry, value, ptr, info(store_vt, store_slot));
    let loaded = load(&mut dag, store, ptr);
    let last = dag.get_store(SDValue { node: loaded.node, res: 1 }, loaded, out, info(EVT::I32, 9));
    dag.set_root(last);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    let root = dag.get_root();
    let stored = dag.operand(root, 1);
    (dag, stored, value)
}

/// `x = 5; ... read x`: the load kept reading memory the store just wrote.
#[test]
fn a_load_of_the_stored_slot_is_the_stored_value() {
    let (_, stored, value) =
        written(EVT::I32, 1, |dag, chain, ptr| dag.get_load(EVT::I32, chain, ptr, info(EVT::I32, 1)));
    assert_eq!(stored, value);
}

/// A load of another slot is not the stored value.
#[test]
fn a_load_of_another_slot_stays_a_load() {
    let (dag, stored, value) = written(EVT::I32, 1, |dag, chain, _| {
        let other = dag.get_register(EVT::I32, 6);
        dag.get_load(EVT::I32, chain, other, info(EVT::I32, 2))
    });
    assert_ne!(stored, value);
    assert_eq!(dag.opcode(stored), ISD::Load);
}

/// A volatile load must read memory.
#[test]
fn a_volatile_load_is_not_forwarded() {
    let (dag, stored, _) = written(EVT::I32, 1, |dag, chain, ptr| {
        dag.get_load(EVT::I32, chain, ptr, MemInfo { volatile: true, ..info(EVT::I32, 1) })
    });
    assert_eq!(dag.opcode(stored), ISD::Load);
}

/// A narrower zero-extending load at the store's address reads the low half
/// of the value: `zext (trunc v)` is `v & 0xffff`.
#[test]
fn a_narrow_load_of_a_wide_store_is_the_low_bits() {
    let (dag, stored, value) = written(EVT::I32, 1, |dag, chain, ptr| {
        dag.get_ext_load(LoadExt::ZExtLoad, EVT::I32, chain, ptr, info(EVT::I16, 1))
    });
    assert_eq!(dag.opcode(stored), ISD::And);
    assert_eq!(dag.operand(stored, 0), value);
    assert_eq!(dag.constant(dag.operand(stored, 1)), Some(0xffff));
}

/// A stored double read back stayed a load: forwarding the x87 value to its
/// second reader made deedlines.bas fail to compile ("floating stack input is
/// unavailable" in CREATEOBJECT).
#[test]
fn a_stored_double_is_not_forwarded() {
    let mut dag = dag();
    let entry = dag.entry_node();
    let ptr = dag.get_register(EVT::I32, 7);
    let out = dag.get_register(EVT::I32, 8);
    let value = dag.get_register(F64, 5);
    let store = dag.get_store(entry, value, ptr, info(F64, 1));
    let loaded = dag.get_load(F64, store, ptr, info(F64, 1));
    let last = dag.get_store(SDValue { node: loaded.node, res: 1 }, loaded, out, info(F64, 9));
    dag.set_root(last);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    let root = dag.get_root();
    assert_eq!(dag.opcode(dag.operand(root, 1)), ISD::Load);
}
