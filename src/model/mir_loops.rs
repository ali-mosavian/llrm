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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::model::mir::MirBlock;

    use super::{dominators, predecessors};

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
}
