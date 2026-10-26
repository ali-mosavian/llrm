//! `mergeConsecutiveStores`: the 14 `i16` stores of -1 to `cache+20..47` of
//! bench/lru stayed 14 stores in 4+ pieces (`word +32`, `dword +28 ..`)
//! instead of the pin's seven aligned `dword` stores: +4..+12 B at every
//! level once the old `merge` pass is gone.

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::super::{MemBase, MemInfo};
use super::*;

fn info(
    vt: EVT,
    volatile: bool,
) -> MemInfo {
    MemInfo { mem_vt: vt, align: 2, volatile, base: MemBase::Global(0), ext: None, space: 0 }
}

/// Stores of `value` of type `vt` to `_g+offset` for each offset, chained one
/// after another off the entry; returns the stores reachable from the root.
fn merged(
    offsets: &[i64],
    vt: EVT,
    volatile: bool,
) -> Vec<(i64, u128, EVT)> {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = dag();
    let entry = dag.entry_node();
    let mut stores = Vec::new();
    for (i, &offset) in offsets.iter().enumerate() {
        let ptr = dag.get_global_address(EVT::I16, 0, offset);
        let value = dag.get_constant(0x10 + i as u128, vt);
        stores.push(dag.get_store(entry, value, ptr, info(vt, volatile)));
    }
    let root = dag.get_token_factor(&stores);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let mut out = Vec::new();
    let mut seen = vec![dag.get_root()];
    while let Some(node) = seen.pop() {
        match dag.opcode(node) {
            ISD::TokenFactor => seen.extend((0..dag.node(node).ops.len()).map(|i| dag.operand(node, i))),
            ISD::Store => {
                let ptr = dag.operand(node, 2);
                let NodeData::Global { offset, .. } = dag.node(ptr).data else { panic!("address") };
                let NodeData::Mem(info) = dag.node(node).data else { panic!("mem") };
                out.push((offset, dag.constant(dag.operand(node, 1)).unwrap(), info.mem_vt));
            }
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Two `i16` stores at consecutive offsets are one `i32` store of the joined
/// constant, the lower address in the low half.
#[test]
fn two_consecutive_word_stores_are_one_dword_store() {
    assert_eq!(merged(&[20, 22], EVT::I16, false), vec![(20, 0x0011_0010, EVT::I32)]);
}

/// Fourteen words are seven dwords.
#[test]
fn fourteen_consecutive_word_stores_are_seven_dword_stores() {
    let offsets: Vec<i64> = (0..14).map(|i| 20 + 2 * i).collect();
    let out = merged(&offsets, EVT::I16, false);
    assert_eq!(
        out.iter().map(|s| (s.0, s.2)).collect::<Vec<_>>(),
        (0..7).map(|i| (20 + 4 * i, EVT::I32)).collect::<Vec<_>>()
    );
}

/// A gap, a volatile store and a store as wide as the widest legal type stay.
#[test]
fn stores_with_a_gap_or_volatile_or_full_width_are_not_merged() {
    assert_eq!(merged(&[20, 24], EVT::I16, false).len(), 2);
    assert_eq!(merged(&[20, 22], EVT::I16, true).len(), 2);
    assert_eq!(merged(&[20, 24], EVT::I32, false).len(), 2);
}

/// Three words: a dword and the word after it.
#[test]
fn three_word_stores_are_a_dword_and_a_word() {
    assert_eq!(merged(&[20, 22, 24], EVT::I16, false), vec![(20, 0x0011_0010, EVT::I32), (24, 0x12, EVT::I16)]);
}

/// The argument stores of a call sequence stay separate: they become pushes,
/// and `pushw 63; pushw 4; pushw 257` merged into `pushd 63; pushd 262401` grew
/// queens.bas by 3 bytes and jpeg_fixed.bas by 23 at m16 -Os.
#[test]
fn argument_stores_of_a_call_sequence_are_not_merged() {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = dag();
    let entry = dag.entry_node();
    let start = dag.get_callseq_start(entry, 4);
    let chain = SDValue { node: start.node, res: 0 };
    let mut stores = Vec::new();
    for (i, offset) in [0i64, 2].into_iter().enumerate() {
        let ptr = dag.get_global_address(EVT::I16, 0, offset);
        let value = dag.get_constant(1 + i as u128, EVT::I16);
        stores.push(dag.get_store(chain, value, ptr, info(EVT::I16, false)));
    }
    let root = dag.get_token_factor(&stores);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let mut seen = vec![dag.get_root()];
    let mut widths = Vec::new();
    while let Some(node) = seen.pop() {
        match dag.opcode(node) {
            ISD::TokenFactor => seen.extend((0..dag.node(node).ops.len()).map(|i| dag.operand(node, i))),
            ISD::Store => {
                let NodeData::Mem(info) = dag.node(node).data else { panic!("mem") };
                widths.push(info.mem_vt);
            }
            _ => {}
        }
    }
    assert_eq!(widths, vec![EVT::I16, EVT::I16]);
}
