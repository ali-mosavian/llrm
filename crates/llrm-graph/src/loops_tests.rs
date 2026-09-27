//! Copied from llrm-core's `analysis/loops_tests.rs`, the port of
//! `tests/test_loops.py`. Its corpus tests read the rich-MIR corpus in
//! llrm-analysis's `corpus_tests.rs`.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

/// A block: where it is and where it goes.
struct Block {
    at: i64,
    succ: Vec<i64>,
}

impl Node for Block {
    fn at(&self) -> i64 {
        self.at
    }

    fn succ(&self) -> &[i64] {
        &self.succ
    }
}

fn block(at: i64, succ: &[i64]) -> Block {
    Block { at, succ: succ.to_vec() }
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
fn test_loops_are_remembered_by_shape_not_by_addresses() {
    // Loops are kept per CFG shape across passes; keyed too loosely, a pass
    // that cut a back edge would still be handed the loop it removed.
    let looped = [block(0, &[1]), block(1, &[1, 2]), block(2, &[])];
    let cut = [block(0, &[1]), block(1, &[2]), block(2, &[])];
    assert_eq!(loops(&looped, None).len(), 1);
    assert_eq!(loops(&cut, None), vec![]);
    assert_eq!(loops(&looped, Some(2)), vec![], "from another entry the loop is unreachable");
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

#[test]
fn an_empty_graph_has_no_loops_dominators_or_frontiers() {
    let none: [Block; 0] = [];
    assert_eq!(loops(&none, None), vec![]);
    assert_eq!(dominators(&none, None), BTreeMap::new());
    assert_eq!(frontiers(&none, None), BTreeMap::new());
    assert_eq!(irreducible(&none, None), set(&[]));
    assert_eq!(reverse_postorder(&none, 0), Vec::<i64>::new());
}

#[test]
fn a_self_loop_at_the_entry_is_a_loop_headed_by_the_entry() {
    let chain = [block(0, &[0, 1]), block(1, &[])];
    assert_eq!(loops(&chain, None), vec![Loop { header: 0, latches: set(&[0]), body: set(&[0]) }]);
    assert_eq!(depth(&chain, None), BTreeMap::from([(0, 1), (1, 0)]));
}

#[test]
fn two_latches_to_one_header_are_one_loop() {
    let chain = [block(0, &[1]), block(1, &[2, 3]), block(2, &[1]), block(3, &[1, 4]), block(4, &[])];
    assert_eq!(loops(&chain, None), vec![Loop { header: 1, latches: set(&[2, 3]), body: set(&[1, 2, 3]) }]);
    assert_eq!(depth(&chain, None)[&2], 1, "one loop, not two");
}

#[test]
fn sibling_loops_do_not_contain_each_other() {
    let chain = [block(0, &[1]), block(1, &[1, 2]), block(2, &[3]), block(3, &[3, 4]), block(4, &[])];
    let found = loops(&chain, None);
    assert_eq!(found.iter().map(|one| (one.header, one.body.clone())).collect::<Vec<_>>(), [(1, set(&[1])), (3, set(&[3]))]);
    assert_eq!(depth(&chain, None), BTreeMap::from([(0, 0), (1, 1), (2, 0), (3, 1), (4, 0)]));
}

#[test]
fn nested_loops_come_innermost_first_and_the_outer_body_holds_the_inner() {
    let chain = [block(0, &[1]), block(1, &[2]), block(2, &[3]), block(3, &[2, 4]), block(4, &[1, 5]), block(5, &[])];
    let found = loops(&chain, None);
    assert_eq!(found.iter().map(|one| one.header).collect::<Vec<_>>(), [2, 1]);
    assert_eq!(found[0].body, set(&[2, 3]));
    assert_eq!(found[1].body, set(&[1, 2, 3, 4]));
    assert_eq!(depth(&chain, None), BTreeMap::from([(0, 0), (1, 1), (2, 2), (3, 2), (4, 1), (5, 0)]));
    assert_eq!(irreducible(&chain, None), set(&[]));
}

#[test]
fn a_loop_with_two_exits_keeps_neither_exit_in_its_body() {
    let chain = [block(0, &[1]), block(1, &[2, 4]), block(2, &[1, 3]), block(3, &[]), block(4, &[])];
    assert_eq!(loops(&chain, None), vec![Loop { header: 1, latches: set(&[2]), body: set(&[1, 2]) }]);
}

#[test]
fn a_branch_with_both_arms_to_one_block_is_one_edge() {
    let chain = [block(0, &[1]), block(1, &[1, 1]), block(2, &[])];
    assert_eq!(predecessors(&chain)[&1], set(&[0, 1]));
    assert_eq!(loops(&chain, None), vec![Loop { header: 1, latches: set(&[1]), body: set(&[1]) }]);
}

#[test]
fn an_entry_other_than_the_first_block_roots_dominance() {
    let chain = [block(0, &[1]), block(1, &[]), block(5, &[0])];
    assert_eq!(dominators(&chain, Some(5)), BTreeMap::from([(0, set(&[0, 5])), (1, set(&[0, 1, 5])), (5, set(&[5]))]));
    assert_eq!(immediate_dominators(&chain, Some(5))[&0], Some(5));
}

#[test]
fn only_the_branch_point_dominates_a_diamonds_join() {
    let diamond = [block(0, &[1, 2]), block(1, &[3]), block(2, &[3]), block(3, &[])];
    assert_eq!(dominators(&diamond, None)[&3], set(&[0, 3]));
    assert_eq!(immediate_dominators(&diamond, None)[&3], Some(0));
}

#[test]
fn an_unreachable_block_has_no_immediate_dominator() {
    let chain = [block(0, &[1]), block(1, &[]), block(9, &[1])];
    assert_eq!(immediate_dominators(&chain, None)[&9], None);
    assert_eq!(immediate_dominators(&chain, None)[&1], Some(0));
}

#[test]
fn a_self_loop_is_on_its_own_frontier() {
    let chain = [block(0, &[1]), block(1, &[1, 2]), block(2, &[])];
    assert_eq!(frontiers(&chain, None)[&1], set(&[1]));
}

#[test]
fn reverse_postorder_puts_predecessors_first_and_unreachable_blocks_last() {
    let diamond = [block(9, &[3]), block(0, &[2, 1]), block(1, &[3]), block(2, &[3]), block(3, &[])];
    let order = reverse_postorder(&diamond, 0);
    let position = |at| order.iter().position(|&one| one == at).unwrap();
    assert_eq!(order[0], 0);
    assert!(position(1) < position(3) && position(2) < position(3));
    assert_eq!(*order.last().unwrap(), 9);
    assert_eq!(order.len(), 5);
}

#[test]
fn an_irreducible_cycle_yields_no_natural_loop() {
    let chain = [block(0, &[1, 2]), block(1, &[2]), block(2, &[1, 3]), block(3, &[])];
    assert_eq!(loops(&chain, None), vec![]);
    assert!(!irreducible(&chain, None).is_empty());
}

#[test]
fn a_reducible_loop_beside_an_irreducible_cycle_is_still_found() {
    let chain = [block(0, &[1, 2, 4]), block(1, &[2]), block(2, &[1, 3]), block(3, &[]), block(4, &[4, 3])];
    assert_eq!(loops(&chain, None), vec![Loop { header: 4, latches: set(&[4]), body: set(&[4]) }]);
    assert!(!irreducible(&chain, None).contains(&4));
}
