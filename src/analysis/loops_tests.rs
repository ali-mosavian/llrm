//! Port of `tests/test_loops.py`.
//!
//! `frontend.blocks.Block` is not ported; `MirBlock` stands in, as both are
//! a `Node`. Skipped, needing the corpus and `frontend.blocks`:
//! test_every_fixture_is_reducible,
//! test_a_loop_body_always_contains_its_own_header_and_latch,
//! test_nothing_in_the_corpus_nests_past_two_loops.

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::model::mir::MirBlock;

fn block(at: i64, succ: &[i64]) -> MirBlock {
    MirBlock::new(at, vec![], vec![], succ.to_vec())
}

fn set(items: &[i64]) -> BTreeSet<i64> {
    items.iter().copied().collect()
}

#[test]
fn test_a_straight_line_has_no_loops() {
    let chain = [block(0, &[1]), block(1, &[2]), block(2, &[])];
    assert_eq!(loops(&chain, None), vec![]);
    assert_eq!(depth(&chain, None), BTreeMap::from([(0, 0), (1, 0), (2, 0)]));
}

#[test]
fn test_a_self_loop_is_its_own_body() {
    let chain = [block(0, &[1]), block(1, &[1, 2]), block(2, &[])];
    let found = loops(&chain, None);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].header, 1);
    assert_eq!(found[0].latches, set(&[1]));
    assert_eq!(found[0].body, set(&[1]), "block 0 is not inside the loop");
}

#[test]
fn test_a_test_at_the_bottom_loop_is_reducible() {
    let chain = [block(0, &[2]), block(1, &[2]), block(2, &[1, 3]), block(3, &[])];
    assert_eq!(irreducible(&chain, None), set(&[]));
    let found = loops(&chain, None);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].header, 2, "the test dominates the body, so the test is the header");
    assert_eq!(found[0].body, set(&[1, 2]));
}

#[test]
fn test_a_cycle_entered_at_two_blocks_is_irreducible() {
    let chain = [block(0, &[1, 2]), block(1, &[2]), block(2, &[1]), block(3, &[])];
    assert_ne!(irreducible(&chain, None), set(&[]));
}

#[test]
fn test_nesting_counts_every_enclosing_loop() {
    let outer_only = [block(0, &[1]), block(1, &[2, 4]), block(2, &[3]), block(3, &[2, 1]), block(4, &[])];
    let found = depth(&outer_only, None);
    assert_eq!(found[&0], 0);
    assert!(found[&2] > found[&1], "the inner block sits inside both loops");
}

#[test]
fn test_an_unreachable_block_dominates_nothing() {
    let chain = [block(0, &[1]), block(1, &[]), block(9, &[1])];
    assert_eq!(dominators(&chain, None)[&9], set(&[]));
}

#[test]
fn test_dead_predecessor_does_not_erase_live_dominance() {
    let chain = [block(0, &[1]), block(1, &[2]), block(2, &[]), block(9, &[2])];
    assert_eq!(
        dominators(&chain, None),
        BTreeMap::from([(0, set(&[0])), (1, set(&[0, 1])), (2, set(&[0, 1, 2])), (9, set(&[]))])
    );
}

#[test]
fn test_dead_predecessor_has_no_dominance_frontier() {
    let chain = [block(0, &[1]), block(1, &[2]), block(2, &[]), block(9, &[2])];
    assert_eq!(frontiers(&chain, None), [0, 1, 2, 9].into_iter().map(|at| (at, set(&[]))).collect());
}

#[test]
fn test_disconnected_cycle_has_no_dominators_or_natural_loops() {
    let chain = [block(0, &[1]), block(1, &[]), block(8, &[9]), block(9, &[8])];
    assert_eq!(dominators(&chain, None)[&8], set(&[]));
    assert_eq!(dominators(&chain, None)[&9], set(&[]));
    assert_eq!(loops(&chain, None), vec![]);
    assert_eq!(irreducible(&chain, None), set(&[]));
}

#[test]
fn test_dead_edge_into_latch_is_not_part_of_live_loop() {
    let chain = [block(0, &[1]), block(1, &[2, 3]), block(2, &[1]), block(3, &[]), block(9, &[2])];
    assert_eq!(loops(&chain, None), vec![Loop { header: 1, latches: set(&[2]), body: set(&[1, 2]) }]);
}

#[test]
fn test_the_entry_dominates_everything_and_nothing_dominates_it() {
    let chain = [block(0, &[1]), block(1, &[2]), block(2, &[])];
    assert_eq!(immediate_dominators(&chain, None)[&0], None);
    assert_eq!(immediate_dominators(&chain, None)[&2], Some(1));
}

#[test]
fn test_a_frontier_is_where_two_definitions_could_meet() {
    let diamond = [block(0, &[1, 2]), block(1, &[3]), block(2, &[3]), block(3, &[])];
    let found = frontiers(&diamond, None);
    assert_eq!(found[&1], set(&[3]));
    assert_eq!(found[&2], set(&[3]));
    assert_eq!(found[&0], set(&[]), "the head dominates the join");
}

#[test]
fn test_a_loop_header_is_on_its_own_frontier() {
    let chain = [block(0, &[1]), block(1, &[2, 3]), block(2, &[1]), block(3, &[])];
    assert!(frontiers(&chain, None)[&2].contains(&1), "the latch reaches the header around the entry path");
}

#[test]
fn test_a_block_with_one_predecessor_is_never_a_frontier() {
    let chain = [block(0, &[1]), block(1, &[2]), block(2, &[])];
    assert!(frontiers(&chain, None).values().all(|where_| !where_.contains(&1) && !where_.contains(&2)));
}
