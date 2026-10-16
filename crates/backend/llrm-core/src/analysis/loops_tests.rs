//! Port of `tests/test_loops.py`.
//!
//! An empty `LirBlock` stands in for a block: the walks read only its
//! address and successors.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::model::lir::{Block, LirBlock};

fn block(
    at: i64,
    succ: &[i64],
) -> LirBlock {
    LirBlock { succ: succ.to_vec().into(), ..LirBlock::new(at, vec![]) }
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

/// The blocks of `graph` the entry reaches without passing through `removed`.
fn reached(
    graph: &[Block],
    removed: Option<i64>,
) -> BTreeSet<i64> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![0];
    while let Some(at) = pending.pop() {
        if Some(at) == removed || !seen.insert(at) {
            continue;
        }
        pending.extend(graph.iter().find(|one| one.at == at).map(|one| one.succ.to_vec()).unwrap_or_default());
    }
    seen
}

/// Dominators by the definition: `d` dominates `a` where `a` is unreachable
/// once `d` is removed.
#[test]
fn test_dominators_are_what_the_definition_says_on_random_graphs() {
    let mut seed = 12345_u64;
    let mut next = |modulus: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % modulus
    };
    for _ in 0..300 {
        let size = 1 + next(12) as i64;
        let graph: Vec<Block> = (0..size)
            .map(|at| block(at, &(0..next(4)).map(|_| next(size as u64) as i64).collect::<Vec<_>>()))
            .collect();
        let found = dominators(&graph, Some(0));
        let reachable = reached(&graph, None);
        for a in 0..size {
            let want: BTreeSet<i64> = if reachable.contains(&a) {
                (0..size).filter(|d| !reached(&graph, Some(*d)).contains(&a) || *d == a).collect()
            } else {
                BTreeSet::new()
            };
            assert_eq!(
                found[&a],
                want,
                "block {a} of {:?}",
                graph.iter().map(|one| (one.at, one.succ.clone())).collect::<Vec<_>>()
            );
        }
    }
}

/// The dominators were bit sets, iterated to a fixed point in layout order: a
/// body laid out against its flow was a round of the body for each block. 4,000
/// blocks took 0.66 s, in `irreducible`, which `lir jumps` runs for every tail
/// copy it tries (#560).
/// Whether `work` at four times the size does under 6 times the steps (linear
/// is 4, quadratic 16). A count: load does not move it, and the wall clock did
/// (164 ms for 4,000 blocks at load 200, against a 100 ms bound).
fn grows_linearly(work: impl Fn(usize) -> u64) -> bool {
    work(8000) < 6 * work(2000)
}

fn reversed_body(n: i64) -> Vec<LirBlock> {
    (0..n).rev().map(|at| block(at, &[(at + 1).min(n)])).chain([block(n, &[])]).collect()
}

#[test]
fn test_dominance_of_a_body_laid_out_against_its_flow_is_not_quadratic() {
    let found = dominance(&reversed_body(4000), Some(0));
    assert!(found.dominates(0, 4000) && found.dominates(1500, 2000) && !found.dominates(2000, 1500));
    let steps = |n: usize| {
        let blocks = reversed_body(n as i64);
        WORK.with(|work| work.set(0));
        drop(_dominance(&blocks, Some(0)));
        WORK.with(Cell::get)
    };
    assert!(grows_linearly(steps), "{} steps for 2,000 blocks, {} for 8,000", steps(2000), steps(8000));
}

/// The bound must fail what it is for: a quadratic count.
#[test]
fn test_the_slope_check_fails_a_quadratic_and_passes_a_linear() {
    assert!(!grows_linearly(|n| (n * n) as u64));
    assert!(grows_linearly(|n| (3 * n) as u64));
}
