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
    MemInfo { mem_vt: vt, align: 2, volatile, base: MemBase::Global(0), ext: None, space: 0, ordering: None }
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

/// Copies of `vt` words `from+offset` to `to+offset`, each store chained off
/// its own load, or all off the entry once the chains are improved; what the
/// combiner leaves, as `(load type, load offset, store type, store offset)` per
/// store.
fn copied(
    offsets: &[i64],
    vt: EVT,
    volatile: bool,
    extra_use: bool,
    from_entry: bool,
) -> Vec<(EVT, i64, EVT, i64)> {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = dag();
    let entry = dag.entry_node();
    let mut stores = Vec::new();
    let mut loads = Vec::new();
    for &offset in offsets {
        let from = dag.get_global_address(EVT::I16, 1, offset);
        loads.push(dag.get_load(vt, entry, from, info(vt, volatile)));
    }
    for (&offset, &load) in offsets.iter().zip(&loads) {
        let to = dag.get_global_address(EVT::I16, 0, offset);
        let chain = if from_entry { entry } else { SDValue { node: load.node, res: 1 } };
        stores.push(dag.get_store(chain, load, to, info(vt, false)));
        if extra_use {
            let sum = dag.get_node(ISD::Add, vt, &[load, load]);
            let other = dag.get_global_address(EVT::I16, 2, offset);
            stores.push(dag.get_store(chain, sum, other, info(vt, false)));
        }
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
                let NodeData::Global { symbol, offset } = dag.node(dag.operand(node, 2)).data else {
                    panic!("address")
                };
                let NodeData::Mem(store) = dag.node(node).data else { panic!("mem") };
                let value = dag.operand(node, 1);
                if symbol != 0 || dag.opcode(value) != ISD::Load {
                    continue;
                }
                let NodeData::Global { offset: from, .. } = dag.node(dag.operand(value, 1)).data else {
                    panic!("address")
                };
                let NodeData::Mem(load) = dag.node(value).data else { panic!("mem") };
                out.push((load.mem_vt, from, store.mem_vt, offset));
            }
            _ => {}
        }
    }
    out.sort_by_key(|copy| copy.3);
    out.dedup();
    out
}

/// Two word copies stayed two loads and two stores (`mov cx, [si]; mov [di],
/// cx` twice) where the pin makes one dword copy.
#[test]
fn two_consecutive_word_copies_are_one_dword_copy() {
    assert_eq!(copied(&[0, 2], EVT::I16, false, false, false), vec![(EVT::I32, 0, EVT::I32, 0)]);
}

/// Three words: a dword copy and the word after it.
#[test]
fn three_word_copies_are_a_dword_and_a_word_copy() {
    assert_eq!(
        copied(&[0, 2, 4], EVT::I16, false, false, true),
        vec![(EVT::I32, 0, EVT::I32, 0), (EVT::I16, 4, EVT::I16, 4)]
    );
}

/// A volatile load, a load that something else reads and a gap stay copies.
#[test]
fn copies_of_a_volatile_or_shared_load_or_with_a_gap_stay() {
    assert_eq!(copied(&[0, 2], EVT::I16, true, false, false).len(), 2);
    assert_eq!(copied(&[0, 2], EVT::I16, false, true, false).len(), 2);
    assert_eq!(copied(&[0, 4], EVT::I16, false, false, false).len(), 2);
}

/// The argument stores of a call sequence copy two words of a struct as two
/// word loads and two word stores; they are not pushes when the area is
/// addressed through a register, and a merged copy is a dword load and store
/// (`_pl_grenade_explode`, `-m16 -Os`: eleven word moves through `bx`;
/// weapons.c grew 76 bytes over the pin's dword copies).
#[test]
fn word_copies_into_a_call_sequence_are_one_dword_copy() {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = dag();
    let entry = dag.entry_node();
    let loads: Vec<SDValue> = [0i64, 2]
        .into_iter()
        .map(|offset| {
            let from = dag.get_global_address(EVT::I16, 1, offset);
            dag.get_load(EVT::I16, entry, from, info(EVT::I16, false))
        })
        .collect();
    let start = dag.get_callseq_start(entry, 4);
    let chain = SDValue { node: start.node, res: 0 };
    let stores: Vec<SDValue> = [0i64, 2]
        .into_iter()
        .zip(&loads)
        .map(|(offset, &load)| {
            let to = dag.get_global_address(EVT::I16, 0, offset);
            dag.get_store(chain, load, to, info(EVT::I16, false))
        })
        .collect();
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
    assert_eq!(widths, vec![EVT::I32]);
}
