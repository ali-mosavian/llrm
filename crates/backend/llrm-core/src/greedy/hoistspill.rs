//! `HoistSpillHelper::runHoistSpills`: where the stores of one spilled value
//! go.
//!
//! # How it works
//! Translated from InlineSpiller.cpp at d1106deb71cc (`rmRedundantSpills`,
//! `getVisitOrders`, `runHoistSpills`). The spills of one value into one slot
//! are equal. Per block only the first stays. Then, walking up the dominator
//! tree from each block that holds one towards the block of the value's
//! definition, a spill dominated by another is redundant, and the blocks on the
//! way are the places a hoisted store may go. Those blocks are visited bottom
//! up; where the cost of the stores below (their blocks' frequencies) exceeds
//! the block's own frequency (times 9/10 when more than one store would merge),
//! the stores below become one in this block, if the block can hold one (a
//! sibling of the value is live at its last insert point).
//!
//! This part is the decision alone, over block numbers; the caller owns the
//! instructions (`hoistAllSpills`).
//!
//! DEVIATE (no number): spills are visited in slot-index order, where LLVM
//! visits a pointer set; which of two equal-cost choices is made may differ.
//! `BlockFrequency * BranchProbability(9, 10)` is compared exactly
//! (`cost * 10 > freq * 9`), not truncated.
//!
//! # Example
//! A value defined in block 0 and spilled in both arms 1 and 2 of a diamond
//! (frequencies 12, 5, 5, 12): the arms cost 10 against 9/10 of block 0's 12
//! (10.8), so the stores stay. With block 0 at 8 they become one store in
//! block 0.
//!
//! # Used by
//! Nothing yet.

use std::collections::{BTreeMap, BTreeSet};

/// One spill of the value: its identity, the block it is in, and its slot
/// index (its place in that block's order).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spill {
    pub id: u64,
    pub block: usize,
    pub order: u32,
}

/// The decision: the spills to remove, and the blocks that get a store of the
/// register named.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Hoisted {
    pub remove: Vec<u64>,
    pub insert: BTreeMap<usize, u32>,
}

pub struct Tree<'a> {
    /// The immediate dominator; none for the root of the function.
    pub idom: &'a dyn Fn(usize) -> Option<usize>,
    pub blocks: usize,
    /// `MBFI.getBlockFreq`.
    pub freq: &'a dyn Fn(usize) -> u128,
}

/// `runHoistSpills` for the spills `spills` of a value defined in `root`.
/// `candidate` answers `isSpillCandBB`: the register to store from, if the
/// block can hold a store.
pub fn run_hoist_spills(
    tree: &Tree<'_>,
    root: usize,
    spills: &[Spill],
    candidate: &mut dyn FnMut(usize) -> Option<u32>,
) -> Hoisted {
    let mut out = Hoisted::default();
    // rmRedundantSpills: the earliest of a block stays.
    let mut sorted = spills.to_vec();
    sorted.sort_by_key(|s| (s.order, s.id));
    let mut in_block: BTreeMap<usize, Spill> = BTreeMap::new();
    for spill in &sorted {
        match in_block.get(&spill.block) {
            Some(first) if first.order <= spill.order => out.remove.push(spill.id),
            Some(first) => {
                out.remove.push(first.id);
                in_block.insert(spill.block, *spill);
            }
            None => {
                in_block.insert(spill.block, *spill);
            }
        }
    }
    // getVisitOrders.
    let root_idom = (tree.idom)(root);
    let mut work: BTreeSet<usize> = BTreeSet::new();
    let mut keep: BTreeMap<usize, Option<u32>> = BTreeMap::new();
    let kept: Vec<Spill> = {
        let mut kept: Vec<Spill> = in_block.values().copied().collect();
        kept.sort_by_key(|s| (s.order, s.id));
        kept
    };
    for spill in &kept {
        let mut on_path: BTreeSet<usize> = BTreeSet::new();
        let mut redundant = false;
        let mut node = Some(spill.block);
        while node != root_idom {
            let here = node.expect("the walk ends at the root's dominator");
            if here != spill.block && in_block.contains_key(&here) {
                redundant = true;
                break;
            } else if work.contains(&here) {
                break;
            }
            on_path.insert(here);
            node = (tree.idom)(here);
        }
        if redundant {
            out.remove.push(spill.id);
        } else {
            keep.insert(spill.block, None);
            work.extend(on_path);
        }
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); tree.blocks];
    for block in 0..tree.blocks {
        if let Some(parent) = (tree.idom)(block) {
            children[parent].push(block);
        }
    }
    let mut orders = vec![root];
    let mut at = 0;
    while at < orders.len() {
        let node = orders[at];
        at += 1;
        orders.extend(children[node].iter().copied().filter(|child| work.contains(child)));
    }
    // runHoistSpills: bottom up.
    let mut below: BTreeMap<usize, (BTreeSet<usize>, u128)> = BTreeMap::new();
    for &block in orders.iter().rev() {
        if keep.get(&block) == Some(&None) {
            let entry = below.entry(block).or_default();
            entry.0.insert(block);
            entry.1 = (tree.freq)(block);
            continue;
        }
        for &child in &children[block] {
            if let Some((set, cost)) = below.remove(&child) {
                let entry = below.entry(block).or_default();
                entry.1 += cost;
                entry.0.extend(set);
            }
        }
        let Some((set, cost)) = below.get(&block).cloned() else { continue };
        if set.is_empty() {
            continue;
        }
        let Some(live) = candidate(block) else { continue };
        let (num, den) = if set.len() > 1 { (9, 10) } else { (1, 1) };
        if cost * den > (tree.freq)(block) * num {
            for spill_block in &set {
                if keep.get(spill_block) == Some(&None) {
                    out.remove.push(in_block[spill_block].id);
                }
                keep.remove(spill_block);
            }
            keep.insert(block, Some(live));
            below.insert(block, (BTreeSet::from([block]), (tree.freq)(block)));
        }
    }
    for (block, live) in keep {
        if let Some(live) = live {
            out.insert.insert(block, live);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A diamond 0 -> {1, 2} -> 3 with the value defined in 0.
    fn diamond(
        freq: [u128; 4],
        spills: &[Spill],
    ) -> Hoisted {
        let idom = |block: usize| match block {
            0 => None,
            _ => Some(0),
        };
        let freqs = freq;
        let tree = Tree { idom: &idom, blocks: 4, freq: &move |b| freqs[b] };
        run_hoist_spills(&tree, 0, spills, &mut |_| Some(7))
    }

    const ARMS: [Spill; 2] = [Spill { id: 1, block: 1, order: 10 }, Spill { id: 2, block: 2, order: 20 }];

    /// Stores in both arms of a diamond whose head is hotter than the arms
    /// together stay where they are.
    #[test]
    fn test_spills_in_the_arms_stay_when_the_head_is_hotter() {
        let out = diamond([12, 5, 5, 12], &ARMS);
        assert_eq!(out, Hoisted::default());
    }

    /// With the head colder than 9/10 of the arms together the two stores
    /// become one in the head, from the sibling that is live there.
    #[test]
    fn test_spills_in_both_arms_hoist_to_a_colder_head() {
        let out = diamond([8, 5, 5, 10], &ARMS);
        assert_eq!(out.insert.get(&0), Some(&7), "{out:?}");
        let mut removed = out.remove.clone();
        removed.sort();
        assert_eq!(removed, vec![1, 2]);
    }

    /// Two stores of one block: the later is redundant.
    #[test]
    fn test_the_later_of_two_spills_in_a_block_is_redundant() {
        let spills = [Spill { id: 1, block: 1, order: 10 }, Spill { id: 2, block: 1, order: 14 }];
        let out = diamond([12, 5, 5, 12], &spills);
        assert_eq!(out.remove, vec![2]);
        assert!(out.insert.is_empty());
    }
}
