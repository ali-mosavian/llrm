//! Direct graph helpers used by the Python MIR port.
//!
//! This is the required subset of `qbopt/analysis/loops.py`.  It remains
//! separate from the portable-IR analyses because Python MIR's graph accepts
//! source-side shapes that the portable IR intentionally does not represent.

use std::collections::{BTreeMap, BTreeSet};

use super::mir::MirBlock;

/// Direct port of `qbopt.analysis.loops:predecessors` for MIR blocks.
///
/// Only successor addresses belonging to a supplied block are predecessors.
/// Unknown targets are intentionally ignored rather than rejected.
pub(crate) fn predecessors(blocks: &[MirBlock]) -> BTreeMap<i64, BTreeSet<i64>> {
    let known = blocks.iter().map(|block| block.at).collect::<BTreeSet<_>>();
    let mut found = blocks
        .iter()
        .map(|block| (block.at, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();

    for block in blocks {
        for successor in &block.succ {
            if known.contains(successor) {
                found
                    .get_mut(successor)
                    .expect("known successor has a predecessor entry")
                    .insert(block.at);
            }
        }
    }
    found
}

/// Direct port of `qbopt.analysis.loops:dominators` for MIR blocks.
///
/// This intentionally returns an empty dominator set for an unreachable block
/// and accepts a missing entry without diagnosing it.  `mir::verify` checks
/// only SSA promises and must retain that narrow contract.
pub(crate) fn dominators(blocks: &[MirBlock], entry: i64) -> BTreeMap<i64, BTreeSet<i64>> {
    if blocks.is_empty() {
        return BTreeMap::new();
    }

    let indexed = blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![entry];
    while let Some(at) = pending.pop() {
        let Some(block) = indexed.get(&at) else {
            continue;
        };
        if !reachable.insert(at) {
            continue;
        }
        pending.extend(block.succ.iter().copied());
    }

    let every = reachable.clone();
    let reachable_blocks = blocks
        .iter()
        .filter(|block| reachable.contains(&block.at))
        .cloned()
        .collect::<Vec<_>>();
    let predecessors = predecessors(&reachable_blocks);
    let mut dominators = blocks
        .iter()
        .map(|block| {
            (
                block.at,
                if reachable.contains(&block.at) {
                    every.clone()
                } else {
                    BTreeSet::new()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    if indexed.contains_key(&entry) {
        dominators.insert(entry, BTreeSet::from([entry]));
    }

    let mut changing = true;
    while changing {
        changing = false;
        for block in blocks {
            if block.at == entry || !reachable.contains(&block.at) {
                continue;
            }
            let reaching = predecessors
                .get(&block.at)
                .expect("reachable block has a predecessor entry")
                .iter()
                .filter_map(|predecessor| dominators.get(predecessor))
                .collect::<Vec<_>>();
            let mut now = if let Some((first, rest)) = reaching.split_first() {
                rest.iter().fold((*first).clone(), |shared, predecessor| {
                    shared.intersection(predecessor).copied().collect()
                })
            } else {
                BTreeSet::new()
            };
            now.insert(block.at);
            let previous = dominators
                .get(&block.at)
                .expect("every supplied block has a dominator entry");
            if now != *previous {
                dominators.insert(block.at, now);
                changing = true;
            }
        }
    }
    dominators
}

/// One natural loop: where control comes back to, and what is inside.
///
/// Direct port of `qbopt.analysis.loops:Loop`.  `latches` is keyed by the
/// header rather than by an individual back edge: Python deliberately treats
/// several edges returning to one header as one loop and unions their bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Loop {
    pub header: i64,
    pub latches: BTreeSet<i64>,
    pub body: BTreeSet<i64>,
}

/// Direct port of `qbopt.analysis.loops:back_edges` for MIR blocks.
///
/// The supplied block and successor order is retained.  Python exposes that
/// order through its list result, and `loops` relies on it when equal-sized
/// loops retain their discovery order after the stable size sort.
pub(crate) fn back_edges(
    blocks: &[MirBlock],
    dominators: &BTreeMap<i64, BTreeSet<i64>>,
) -> Vec<(i64, i64)> {
    let known = blocks.iter().map(|block| block.at).collect::<BTreeSet<_>>();
    blocks
        .iter()
        .flat_map(|block| {
            block.succ.iter().filter_map(|successor| {
                (known.contains(successor)
                    && dominators
                        .get(&block.at)
                        .is_some_and(|dominated_by| dominated_by.contains(successor)))
                .then_some((block.at, *successor))
            })
        })
        .collect()
}

/// Direct port of `qbopt.analysis.loops:_body` for one natural-loop edge.
///
/// The header enters the body before walking backwards.  In particular, a
/// self edge returns without expanding the header's predecessors, exactly as
/// Python does.
fn body(latch: i64, header: i64, predecessors: &BTreeMap<i64, BTreeSet<i64>>) -> BTreeSet<i64> {
    let mut body = BTreeSet::from([header]);
    if latch == header {
        return body;
    }
    body.insert(latch);
    let mut pending = vec![latch];
    while let Some(at) = pending.pop() {
        for predecessor in predecessors.get(&at).into_iter().flatten() {
            if body.insert(*predecessor) {
                pending.push(*predecessor);
            }
        }
    }
    body
}

/// Direct port of `qbopt.analysis.loops:loops` for MIR blocks.
///
/// Only reachable blocks are handed to the predecessor walk.  This preserves
/// Python's refusal to turn disconnected cycles or dead edges into natural
/// loops.  Header groups use a `Vec`, rather than a map, to retain Python's
/// first-back-edge insertion order where its stable final sort ties.
pub(crate) fn loops(blocks: &[MirBlock], entry: Option<i64>) -> Vec<Loop> {
    let Some(entry) = entry.or_else(|| blocks.first().map(|block| block.at)) else {
        return Vec::new();
    };
    let dominators = dominators(blocks, entry);
    let reachable_blocks = blocks
        .iter()
        .filter(|block| !dominators[&block.at].is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let predecessors = predecessors(&reachable_blocks);

    let mut found = Vec::<Loop>::new();
    for (latch, header) in back_edges(blocks, &dominators) {
        let loop_body = body(latch, header, &predecessors);
        if let Some(found_loop) = found
            .iter_mut()
            .find(|found_loop| found_loop.header == header)
        {
            found_loop.latches.insert(latch);
            found_loop.body.extend(loop_body);
        } else {
            found.push(Loop {
                header,
                latches: BTreeSet::from([latch]),
                body: loop_body,
            });
        }
    }
    found.sort_by_key(|found_loop| found_loop.body.len());
    found
}

/// Direct port of `qbopt.analysis.loops:depth` for MIR blocks.
pub(crate) fn depth(blocks: &[MirBlock], entry: Option<i64>) -> BTreeMap<i64, usize> {
    let mut found = blocks
        .iter()
        .map(|block| (block.at, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for found_loop in loops(blocks, entry) {
        for at in found_loop.body {
            if let Some(value) = found.get_mut(&at) {
                *value += 1;
            }
        }
    }
    found
}

/// Direct port of `qbopt.analysis.loops:irreducible` for MIR blocks.
///
/// This cuts precisely the dominator-defined back edges, then reports the
/// targets of DFS back edges that remain.  It does not use address ordering:
/// source addresses say nothing about CFG reducibility.
pub(crate) fn irreducible(blocks: &[MirBlock], entry: i64) -> BTreeSet<i64> {
    let dominators = dominators(blocks, entry);
    let known = blocks
        .iter()
        .filter(|block| !dominators[&block.at].is_empty())
        .map(|block| block.at)
        .collect::<BTreeSet<_>>();
    let back_edges = back_edges(blocks, &dominators)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let forward = blocks
        .iter()
        .filter(|block| known.contains(&block.at))
        .map(|block| {
            (
                block.at,
                block
                    .succ
                    .iter()
                    .copied()
                    .filter(|successor| {
                        known.contains(successor) && !back_edges.contains(&(block.at, *successor))
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    // Python's three-colour DFS records an edge into its active stack.  Keep
    // an explicit iterator index so successor visitation remains the block's
    // own `succ` order, rather than an incidental sorted container order.
    const WHITE: u8 = 0;
    const GREY: u8 = 1;
    const BLACK: u8 = 2;
    let mut colour = forward
        .keys()
        .copied()
        .map(|at| (at, WHITE))
        .collect::<BTreeMap<_, _>>();
    let mut found = BTreeSet::new();

    for start in forward.keys().copied().collect::<Vec<_>>() {
        if colour[&start] != WHITE {
            continue;
        }
        let mut stack = vec![(start, 0_usize)];
        colour.insert(start, GREY);
        while let Some((at, next)) = stack.last_mut() {
            let successors = &forward[at];
            if *next == successors.len() {
                colour.insert(*at, BLACK);
                stack.pop();
                continue;
            }
            let successor = successors[*next];
            *next += 1;
            match colour[&successor] {
                GREY => {
                    found.insert(successor);
                }
                WHITE => {
                    colour.insert(successor, GREY);
                    stack.push((successor, 0));
                }
                BLACK => {}
                _ => unreachable!("the Python DFS has exactly three colours"),
            }
        }
    }
    found
}

/// Direct port of `qbopt.analysis.loops:immediate_dominators` for MIR
/// blocks.  The entry and every unreachable block have no immediate
/// dominator.
pub(crate) fn immediate_dominators(blocks: &[MirBlock], entry: i64) -> BTreeMap<i64, Option<i64>> {
    let dominators = dominators(blocks, entry);
    blocks
        .iter()
        .map(|block| {
            let strict = dominators[&block.at]
                .iter()
                .copied()
                .filter(|one| *one != block.at)
                .collect::<Vec<_>>();
            // Python's `max(strict, key=lambda one: len(doms[one]))`; the
            // strict dominators of a reachable node form a chain, so there is
            // one greatest cardinality.  Keep the first maximum just as
            // Python's `max` does if a malformed graph violates that fact.
            let nearest = strict.into_iter().fold(None, |nearest, one| match nearest {
                None => Some(one),
                Some(previous) if dominators[&one].len() > dominators[&previous].len() => Some(one),
                Some(previous) => Some(previous),
            });
            (block.at, nearest)
        })
        .collect()
}

/// Direct port of `qbopt.analysis.loops:frontiers` for MIR blocks.
pub(crate) fn frontiers(blocks: &[MirBlock], entry: i64) -> BTreeMap<i64, BTreeSet<i64>> {
    let dominators = dominators(blocks, entry);
    let live = blocks
        .iter()
        .filter(|block| !dominators[&block.at].is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let immediate = immediate_dominators(&live, entry);
    let predecessors = predecessors(&live);
    let mut found = blocks
        .iter()
        .map(|block| (block.at, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();

    for block in &live {
        if predecessors[&block.at].len() < 2 {
            continue;
        }
        for predecessor in &predecessors[&block.at] {
            let mut runner = Some(*predecessor);
            while runner.is_some() && runner != immediate[&block.at] {
                let at = runner.expect("checked above");
                found
                    .get_mut(&at)
                    .expect("a live predecessor belongs to the supplied blocks")
                    .insert(block.at);
                runner = immediate[&at];
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::model::mir::MirBlock;

    use super::{
        Loop, depth, dominators, frontiers, immediate_dominators, irreducible, loops, predecessors,
    };

    fn block(at: i64, successors: &[i64]) -> MirBlock {
        MirBlock::new(at, vec![], vec![], successors.to_vec())
    }

    #[test]
    fn direct_mir_loops_straight_line_has_no_loops() {
        // Direct port of tests/test_loops.py:test_a_straight_line_has_no_loops.
        let chain = vec![block(0, &[1]), block(1, &[2]), block(2, &[])];
        assert_eq!(loops(&chain, Some(0)), Vec::<Loop>::new());
        assert_eq!(
            loops(&chain, None),
            loops(&chain, Some(0)),
            "None selects Python's first supplied block as the entry"
        );
        assert_eq!(
            depth(&chain, Some(0)),
            std::collections::BTreeMap::from([(0, 0), (1, 0), (2, 0)])
        );
        let empty = Vec::<MirBlock>::new();
        assert_eq!(loops(&empty, None), Vec::<Loop>::new());
        assert_eq!(depth(&empty, None), std::collections::BTreeMap::new());
    }

    #[test]
    fn direct_mir_loops_self_loop_is_its_own_body() {
        // Direct port of tests/test_loops.py:test_a_self_loop_is_its_own_body.
        let chain = vec![block(0, &[1]), block(1, &[1, 2]), block(2, &[])];
        assert_eq!(
            loops(&chain, Some(0)),
            vec![Loop {
                header: 1,
                latches: BTreeSet::from([1]),
                body: BTreeSet::from([1]),
            }],
            "block 0 is not inside the loop"
        );
    }

    #[test]
    fn direct_mir_loops_bottom_tested_loop_is_reducible() {
        // Direct port of tests/test_loops.py:test_a_test_at_the_bottom_loop_is_reducible.
        let chain = vec![
            block(0, &[2]),
            block(1, &[2]),
            block(2, &[1, 3]),
            block(3, &[]),
        ];
        assert_eq!(irreducible(&chain, 0), BTreeSet::new());
        assert_eq!(
            loops(&chain, Some(0)),
            vec![Loop {
                header: 2,
                latches: BTreeSet::from([1]),
                body: BTreeSet::from([1, 2]),
            }],
            "the test dominates the body, so the test is the header"
        );
    }

    #[test]
    fn direct_mir_loops_nesting_counts_every_enclosing_loop() {
        // Direct port of tests/test_loops.py:test_nesting_counts_every_enclosing_loop.
        let outer_only = vec![
            block(0, &[1]),
            block(1, &[2, 4]),
            block(2, &[3]),
            block(3, &[2, 1]),
            block(4, &[]),
        ];
        let found = depth(&outer_only, Some(0));
        assert_eq!(found[&0], 0);
        assert!(
            found[&2] > found[&1],
            "the inner block sits inside both loops"
        );
    }

    #[test]
    fn direct_mir_loops_disconnected_cycle_has_no_natural_loop() {
        // Direct port of tests/test_loops.py:test_disconnected_cycle_has_no_dominators_or_natural_loops.
        let chain = vec![
            block(0, &[1]),
            block(1, &[]),
            block(8, &[9]),
            block(9, &[8]),
        ];
        assert_eq!(dominators(&chain, 0)[&8], BTreeSet::new());
        assert_eq!(dominators(&chain, 0)[&9], BTreeSet::new());
        assert_eq!(loops(&chain, Some(0)), Vec::<Loop>::new());
        assert_eq!(irreducible(&chain, 0), BTreeSet::new());
    }

    #[test]
    fn direct_mir_loops_dead_edge_into_latch_is_excluded() {
        // Direct port of tests/test_loops.py:test_dead_edge_into_latch_is_not_part_of_live_loop.
        let chain = vec![
            block(0, &[1]),
            block(1, &[2, 3]),
            block(2, &[1]),
            block(3, &[]),
            block(9, &[2]),
        ];
        assert_eq!(
            loops(&chain, Some(0)),
            vec![Loop {
                header: 1,
                latches: BTreeSet::from([2]),
                body: BTreeSet::from([1, 2]),
            }]
        );
    }

    #[test]
    fn direct_mir_loops_shared_header_unions_multiple_latches() {
        // Direct port of qbopt.analysis.loops:Loop and loops' shared-header
        // contract.  The Python corpus regression describes this exact case:
        // several back edges returning to one header are one loop, not nested
        // loops counted once per latch.
        let chain = vec![
            block(0, &[1]),
            block(1, &[2, 3, 5]),
            block(2, &[4]),
            block(3, &[4]),
            block(4, &[1]),
            block(5, &[1]),
        ];
        assert_eq!(
            loops(&chain, Some(0)),
            vec![Loop {
                header: 1,
                latches: BTreeSet::from([4, 5]),
                body: BTreeSet::from([1, 2, 3, 4, 5]),
            }]
        );
    }

    #[test]
    fn direct_mir_verify_graph_helpers_ignore_unknown_targets_and_empty_unreachable_dominators() {
        let blocks = vec![
            MirBlock::new(0, vec![], vec![], vec![1, 99]),
            MirBlock::new(1, vec![], vec![], vec![]),
            MirBlock::new(2, vec![], vec![], vec![]),
        ];

        assert_eq!(
            predecessors(&blocks),
            std::collections::BTreeMap::from([
                (0, BTreeSet::new()),
                (1, BTreeSet::from([0])),
                (2, BTreeSet::new()),
            ])
        );
        assert_eq!(
            dominators(&blocks, 0),
            std::collections::BTreeMap::from([
                (0, BTreeSet::from([0])),
                (1, BTreeSet::from([0, 1])),
                (2, BTreeSet::new()),
            ])
        );
    }

    #[test]
    fn direct_mir_resolved_loop_helpers_keep_python_live_graph_rules() {
        // Direct ports of tests/test_loops.py::{test_a_cycle_entered_at_two_blocks_is_irreducible,
        // test_the_entry_dominates_everything_and_nothing_dominates_it,
        // test_a_frontier_is_where_two_definitions_could_meet}.
        let diamond = vec![
            MirBlock::new(0, vec![], vec![], vec![1, 2]),
            MirBlock::new(1, vec![], vec![], vec![3]),
            MirBlock::new(2, vec![], vec![], vec![3]),
            MirBlock::new(3, vec![], vec![], vec![]),
            MirBlock::new(9, vec![], vec![], vec![3]),
        ];
        assert_eq!(immediate_dominators(&diamond, 0)[&0], None);
        assert_eq!(immediate_dominators(&diamond, 0)[&3], Some(0));
        let found = frontiers(&diamond, 0);
        assert_eq!(found[&1], BTreeSet::from([3]));
        assert_eq!(found[&2], BTreeSet::from([3]));
        assert_eq!(found[&9], BTreeSet::new(), "dead edges create no frontier");

        let irreducible_graph = vec![
            MirBlock::new(0, vec![], vec![], vec![1, 2]),
            MirBlock::new(1, vec![], vec![], vec![2]),
            MirBlock::new(2, vec![], vec![], vec![1]),
        ];
        assert!(!irreducible(&irreducible_graph, 0).is_empty());
    }

    #[test]
    fn direct_mir_resolved_reducibility_matches_python_loop_regressions() {
        // Direct ports of tests/test_loops.py::{test_a_test_at_the_bottom_loop_is_reducible,
        // test_disconnected_cycle_has_no_dominators_or_natural_loops}.
        let bottom_tested = vec![
            MirBlock::new(0, vec![], vec![], vec![2]),
            MirBlock::new(1, vec![], vec![], vec![2]),
            MirBlock::new(2, vec![], vec![], vec![1, 3]),
            MirBlock::new(3, vec![], vec![], vec![]),
        ];
        assert!(irreducible(&bottom_tested, 0).is_empty());

        let disconnected = vec![
            MirBlock::new(0, vec![], vec![], vec![1]),
            MirBlock::new(1, vec![], vec![], vec![]),
            MirBlock::new(8, vec![], vec![], vec![9]),
            MirBlock::new(9, vec![], vec![], vec![8]),
        ];
        assert!(dominators(&disconnected, 0)[&8].is_empty());
        assert!(dominators(&disconnected, 0)[&9].is_empty());
        assert!(irreducible(&disconnected, 0).is_empty());
    }

    #[test]
    fn direct_mir_resolved_frontiers_match_python_loop_regressions() {
        // Direct ports of tests/test_loops.py::{test_a_loop_header_is_on_its_own_frontier,
        // test_a_block_with_one_predecessor_is_never_a_frontier}.
        let looped = vec![
            MirBlock::new(0, vec![], vec![], vec![1]),
            MirBlock::new(1, vec![], vec![], vec![2, 3]),
            MirBlock::new(2, vec![], vec![], vec![1]),
            MirBlock::new(3, vec![], vec![], vec![]),
        ];
        assert!(frontiers(&looped, 0)[&2].contains(&1));

        let chain = vec![
            MirBlock::new(0, vec![], vec![], vec![1]),
            MirBlock::new(1, vec![], vec![], vec![2]),
            MirBlock::new(2, vec![], vec![], vec![]),
        ];
        assert!(
            frontiers(&chain, 0)
                .values()
                .all(|where_| !where_.contains(&1) && !where_.contains(&2))
        );
    }
}
