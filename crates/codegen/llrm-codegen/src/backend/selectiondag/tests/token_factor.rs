//! `visitTokenFactor`: before, the combiner left a token factor as the
//! builder made it, with entry tokens, repeats and operands another operand
//! already orders after.

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::super::{MemBase, MemInfo};
use super::*;

fn store(
    dag: &mut SelectionDAG,
    chain: SDValue,
    slot: i32,
) -> SDValue {
    let value = dag.get_register(EVT::I32, 5);
    let ptr = dag.get_register(EVT::I32, 7);
    let info = MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0 , ordering: None };
    dag.get_store(chain, value, ptr, info)
}

/// The operands of the root token factor after combining.
fn combined_roots(
    build: impl FnOnce(&mut SelectionDAG, SDValue) -> (SDValue, Vec<SDValue>)
) -> (Vec<SDValue>, Vec<SDValue>) {
    let mut dag = dag();
    let entry = dag.entry_node();
    let (root, expected) = build(&mut dag, entry);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let found = if dag.opcode(root) == ISD::TokenFactor { dag.node(root).ops.clone() } else { vec![root] };
    (found, expected)
}

fn same_set(
    mut a: Vec<SDValue>,
    mut b: Vec<SDValue>,
) -> bool {
    a.sort_by_key(|v| v.node);
    b.sort_by_key(|v| v.node);
    a == b
}

/// `TF(entry, s1, s2)` kept the entry token: a dependence on nothing.
#[test]
fn a_token_factor_drops_its_entry_token() {
    let (found, expected) = combined_roots(|dag, entry| {
        let (s1, s2) = (store(dag, entry, 1), store(dag, entry, 2));
        (dag.get_token_factor(&[entry, s1, s2]), vec![s1, s2])
    });
    assert!(same_set(found, expected));
}

/// `TF(s1, s2, s1)` kept the second `s1`.
#[test]
fn a_token_factor_drops_a_repeated_operand() {
    let (found, expected) = combined_roots(|dag, entry| {
        let (s1, s2) = (store(dag, entry, 1), store(dag, entry, 2));
        (dag.get_token_factor(&[s1, s2, s1]), vec![s1, s2])
    });
    assert!(same_set(found, expected));
}

/// `TF(TF(s1, s2), s3)` with the inner read only here stayed two nodes.
#[test]
fn a_token_factor_takes_in_a_token_factor_only_it_reads() {
    let (found, expected) = combined_roots(|dag, entry| {
        let (s1, s2, s3) = (store(dag, entry, 1), store(dag, entry, 2), store(dag, entry, 3));
        let inner = dag.get_token_factor(&[s1, s2]);
        (dag.get_token_factor(&[inner, s3]), vec![s1, s2, s3])
    });
    assert!(same_set(found, expected));
}

/// `TF(s1, s3, s2)` where `s3` is chained after `s1`: `s1` is ordered
/// already, and was kept as an operand.
#[test]
fn a_token_factor_drops_an_operand_another_operand_is_chained_after() {
    let (found, expected) = combined_roots(|dag, entry| {
        let (s1, s2) = (store(dag, entry, 1), store(dag, entry, 2));
        let s3 = store(dag, s1, 3);
        (dag.get_token_factor(&[s1, s3, s2]), vec![s3, s2])
    });
    assert!(same_set(found, expected));
}

/// `TF(a, l, b, c)` where `c` follows `TF(b, a, l)`: the walk met three
/// operands in one token factor with two left to consider and panicked
/// `attempt to subtract with overflow` (`fconst` at m64). The pin's unsigned
/// count wraps.
#[test]
fn a_token_factor_meeting_more_operands_than_are_left_does_not_underflow() {
    let (found, _) = combined_roots(|dag, entry| {
        let (a, l, b) = (store(dag, entry, 1), store(dag, entry, 2), store(dag, entry, 3));
        let behind = dag.get_token_factor(&[b, a, l]);
        let c = store(dag, behind, 5);
        (dag.get_token_factor(&[a, l, b, c]), vec![])
    });
    assert_eq!(found.len(), 1, "everything but `c` is ordered before it");
}
