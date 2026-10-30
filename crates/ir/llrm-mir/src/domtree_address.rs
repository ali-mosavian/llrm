//! Dominance of a graph whose blocks are named by address (`i64`), as the
//! walks over the front end's and the back end's blocks read it.
//!
//! # How it works
//! The blocks are put in order of address and numbered; the numbers are the
//! blocks of a [`DomGraph`], and the tree is [`DomTree`] over them. Nothing
//! dominates a block the entry does not reach, where `DomTree::dominates`
//! says everything does: that is the difference this type makes, and the only
//! one.
//!
//! # Example
//! A diamond `0 -> {1, 2} -> 3` has 0 as the immediate dominator of 3.
//!
//! # Used by
//! `llrm-hir`'s `dominance::check`, `llrm-codegen`'s `analysis::loops`.

use std::collections::{BTreeMap, BTreeSet};

use crate::blocklist::BlockList;
use crate::domtree::{DomGraph, DomTree};
use crate::module::BlockId;

/// The blocks as slots: a block's slot is its place among the sorted
/// addresses.
struct Slots {
    entry: Option<BlockId>,
    blocks: Vec<BlockId>,
    successors: Vec<Vec<BlockId>>,
    predecessors: Vec<Vec<BlockId>>,
}

impl DomGraph for Slots {
    fn entry(&self) -> Option<BlockId> {
        self.entry
    }

    fn max_number(&self) -> usize {
        self.blocks.len()
    }

    fn nodes(&self) -> &[BlockId] {
        &self.blocks
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.successors[block.0 as usize].iter().copied().collect()
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.predecessors[block.0 as usize].iter().copied().collect()
    }
}

/// Dominance as the walks over blocks read it: nothing dominates a block the
/// entry does not reach.
pub trait Dominates {
    fn reachable(
        &self,
        at: i64,
    ) -> bool;
    fn dominates(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool;
}

impl Dominates for AddressDominance {
    fn reachable(
        &self,
        at: i64,
    ) -> bool {
        AddressDominance::reachable(self, at)
    }

    fn dominates(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool {
        AddressDominance::dominates(self, dominator, at)
    }
}

/// The dominator tree over the sorted block addresses.
#[derive(Debug)]
pub struct AddressDominance {
    ats: Vec<i64>,
    tree: DomTree,
}

impl AddressDominance {
    /// The dominance of `blocks`, each an address and the addresses it goes
    /// to, from `entry` (the first block's address by default). A successor
    /// that names no block is dropped, and a later block of one address
    /// replaces an earlier one's successors.
    pub fn of<'a>(
        blocks: impl IntoIterator<Item = (i64, &'a [i64])>,
        entry: Option<i64>,
    ) -> Self {
        let blocks: Vec<(i64, &[i64])> = blocks.into_iter().collect();
        let mut ats: Vec<i64> = blocks.iter().map(|one| one.0).collect();
        ats.sort_unstable();
        ats.dedup();
        let slot = |at: i64| ats.binary_search(&at).ok();
        let mut graph = Slots {
            entry: None,
            blocks: (0..ats.len() as u32).map(BlockId).collect(),
            successors: vec![Vec::new(); ats.len()],
            predecessors: vec![Vec::new(); ats.len()],
        };
        let mut tree = DomTree::new();
        if let Some(&(first, _)) = blocks.first()
            && let Some(start) = slot(entry.unwrap_or(first))
        {
            for &(at, successors) in &blocks {
                let from = slot(at).expect("a block address");
                graph.successors[from] =
                    successors.iter().filter_map(|to| slot(*to)).map(|to| BlockId(to as u32)).collect();
            }
            for from in 0..ats.len() {
                for &to in &graph.successors[from].clone() {
                    graph.predecessors[to.0 as usize].push(BlockId(from as u32));
                }
            }
            graph.entry = Some(BlockId(start as u32));
            tree.recalculate(&graph);
        }
        Self { ats, tree }
    }

    fn slot(
        &self,
        at: i64,
    ) -> Option<BlockId> {
        self.ats.binary_search(&at).ok().map(|slot| BlockId(slot as u32))
    }

    /// The nearest block that strictly dominates `at`; none for the entry and
    /// for a block the entry does not reach.
    pub fn immediate(
        &self,
        at: i64,
    ) -> Option<i64> {
        self.tree.immediate_dominator(self.slot(at)?).map(|slot| self.ats[slot.0 as usize])
    }

    /// Whether the entry reaches `at`: only then does anything dominate it.
    pub fn reachable(
        &self,
        at: i64,
    ) -> bool {
        self.slot(at).is_some_and(|slot| self.tree.contains(slot))
    }

    /// `at`'s place in a preorder of the dominator tree: a block dominated by
    /// another comes after it.
    pub fn preorder(
        &self,
        at: i64,
    ) -> Option<u32> {
        self.tree.span(self.slot(at)?).map(|(enter, _)| enter)
    }

    pub fn dominates(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool {
        match (self.slot(dominator), self.slot(at)) {
            (Some(dominator), Some(at)) => self.tree.contains(at) && self.tree.dominates(dominator, at),
            _ => false,
        }
    }

    /// Every block's dominators, itself among them; none for a block the entry
    /// does not reach.
    pub fn named(&self) -> BTreeMap<i64, BTreeSet<i64>> {
        self.ats
            .iter()
            .map(|&at| {
                let mut above = BTreeSet::new();
                let mut next = self.reachable(at).then_some(at);
                while let Some(one) = next {
                    above.insert(one);
                    next = self.immediate(one);
                }
                (at, above)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::AddressDominance;

    fn of(
        blocks: &[(i64, &[i64])],
        entry: Option<i64>,
    ) -> AddressDominance {
        AddressDominance::of(blocks.iter().copied(), entry)
    }

    /// Addresses a sparse and unsorted, one block named twice and an edge to
    /// nowhere: the chain 40 -> 7 -> 90 with a diamond, a dead block and a
    /// repeated one.
    #[test]
    fn blocks_by_address_dominate_as_their_edges_say() {
        let found = of(&[(40, &[7, 5]), (7, &[90]), (5, &[90, 1000]), (90, &[]), (13, &[90]), (7, &[90])], Some(40));
        assert_eq!(found.immediate(90), Some(40), "the join is under the fork");
        assert_eq!(found.immediate(40), None);
        assert!(found.dominates(40, 90) && !found.dominates(7, 90) && found.dominates(7, 7));
        assert!(
            !found.reachable(13) && !found.dominates(40, 13) && !found.dominates(13, 90),
            "a block no path reaches"
        );
        assert!(found.preorder(40) < found.preorder(90) && found.preorder(13).is_none());
        assert_eq!(found.named()[&90], [40, 90].into_iter().collect());
        assert!(found.named()[&13].is_empty());
    }

    #[test]
    fn a_graph_without_the_entry_or_blocks_has_no_dominance() {
        assert!(!of(&[], None).reachable(0));
        assert!(!of(&[(1, &[2]), (2, &[])], Some(9)).reachable(1));
    }
}
