//! Items placed in blocks, asked for the ones that dominate a block, nearest
//! first: what a load's provider is looked for among (`avail`, `loadjoins`),
//! without a scan of every item of the function for each load.
//!
//! The blocks that hold items of one key are sorted by their number in the
//! dominator tree's preorder, each linked to the nearest block above it that
//! holds one too (a stack over the sorted blocks). The blocks that dominate a
//! point are then found by the last block at or before it, and the links up
//! from there: the length of that chain, not of the function.

use std::hash::Hash;

use llrm_mir::dominators::DominatorTree;
use llrm_mir::module::BlockId;
use llrm_support::hash::HashMap;

struct Entry {
    enter: u32,
    last: u32,
    /// The nearest entry above this one, if any.
    up: Option<usize>,
    /// The items in this block, ascending.
    items: Vec<usize>,
}

pub struct Nearest<K> {
    chains: HashMap<K, Vec<Entry>>,
}

impl<K: Hash + Eq> Nearest<K> {
    /// The items `placed` as (key, block, item); one in a block the entry does
    /// not reach dominates nothing that is reachable, and is left out.
    pub fn of(
        tree: &DominatorTree,
        placed: impl IntoIterator<Item = (K, BlockId, usize)>,
    ) -> Self {
        let mut raw: HashMap<K, Vec<(u32, u32, usize)>> = HashMap::default();
        for (key, block, item) in placed {
            if let Some((enter, last)) = tree.span(block) {
                raw.entry(key).or_default().push((enter, last, item));
            }
        }
        let chains = raw
            .into_iter()
            .map(|(key, mut found)| {
                found.sort_unstable();
                let mut entries: Vec<Entry> = Vec::new();
                let mut above: Vec<usize> = Vec::new();
                for (enter, last, item) in found {
                    if let Some(entry) = entries.last_mut().filter(|one| one.enter == enter) {
                        entry.items.push(item);
                        continue;
                    }
                    while above.last().is_some_and(|&top| entries[top].last < enter) {
                        above.pop();
                    }
                    entries.push(Entry { enter, last, up: above.last().copied(), items: vec![item] });
                    above.push(entries.len() - 1);
                }
                (key, entries)
            })
            .collect();
        Self { chains }
    }

    /// The items of `keys` in blocks that dominate `at`, each with its block's
    /// number, the nearest block first and in a block the last first, each
    /// once. Nothing for a block the entry does not reach (everything
    /// dominates it: the caller decides).
    pub fn dominating<'a>(
        &'a self,
        tree: &DominatorTree,
        keys: &[&K],
        at: BlockId,
    ) -> Walk<'a> {
        let Some((point, _)) = tree.span(at) else { return Walk { cursors: Vec::new(), last: None } };
        let cursors = keys
            .iter()
            .filter_map(|key| self.chains.get(*key))
            .filter_map(|chain| {
                let after = chain.partition_point(|entry| entry.enter <= point);
                let mut at = after.checked_sub(1);
                while let Some(one) = at {
                    if chain[one].last >= point {
                        break;
                    }
                    at = chain[one].up;
                }
                at.map(|entry| Cursor { chain, entry: Some(entry), within: chain[entry].items.len() })
            })
            .collect();
        Walk { cursors, last: None }
    }
}

struct Cursor<'a> {
    chain: &'a [Entry],
    entry: Option<usize>,
    /// Items of the entry not yet given, counted from the front.
    within: usize,
}

impl Cursor<'_> {
    /// The next item and its block's number, without taking it.
    fn peek(&self) -> Option<(u32, usize)> {
        let entry = &self.chain[self.entry?];
        Some((entry.enter, entry.items[self.within - 1]))
    }

    fn take(&mut self) {
        self.within -= 1;
        if self.within == 0 {
            self.entry = self.chain[self.entry.expect("an entry")].up;
            self.within = self.entry.map_or(0, |one| self.chain[one].items.len());
        }
    }
}

pub struct Walk<'a> {
    cursors: Vec<Cursor<'a>>,
    last: Option<usize>,
}

impl Iterator for Walk<'_> {
    type Item = (u32, usize);

    fn next(&mut self) -> Option<(u32, usize)> {
        loop {
            let best = (0..self.cursors.len())
                .filter_map(|at| self.cursors[at].peek().map(|key| (key, at)))
                .max_by_key(|(key, _)| *key)?;
            self.cursors[best.1].take();
            if self.last != Some(best.0.1) {
                self.last = Some(best.0.1);
                return Some(best.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{function, parsed};

    /// A chain of N diamonds: each join dominates the next.
    fn diamonds(count: usize) -> String {
        let mut text = String::from("define void @f(i1 %c) {\nentry:\n  br i1 %c, label %a0, label %b0\n");
        for at in 0..count {
            let next = if at + 1 == count { "end".to_owned() } else { format!("a{}", at + 1) };
            let other = if at + 1 == count { "end".to_owned() } else { format!("b{}", at + 1) };
            text += &format!(
                "a{at}:\n  br label %j{at}\nb{at}:\n  br label %j{at}\nj{at}:\n  br i1 %c, label %{next}, label %{other}\n"
            );
        }
        text + "end:\n  ret void\n}\n"
    }

    /// The items that dominate a block, nearest first, are what a scan of every
    /// item finds, sorted: the items of two keys, in all the blocks of 8
    /// diamonds.
    #[test]
    fn test_the_items_that_dominate_a_block_are_found_nearest_first() {
        let module = parsed(&diamonds(8));
        let function = function(&module, "f");
        let tree = llrm_mir::dominators::DominatorTree::new(function);
        let blocks: Vec<BlockId> = function.layout().to_vec();
        // Item i in block (i * 7) % n, under key i % 3 (key 2 is on every
        // item).
        let mut placed: Vec<(u32, BlockId, usize)> =
            (0..40).map(|item| ((item % 3) as u32, blocks[(item * 7) % blocks.len()], item)).collect();
        // Every fourth item has a second key, so is found through two.
        placed
            .extend((0..40).step_by(4).map(|item| (((item + 1) % 3) as u32, blocks[(item * 7) % blocks.len()], item)));
        let nearest = Nearest::of(&tree, placed.iter().copied());
        for &at in &blocks {
            for keys in [vec![0u32], vec![1], vec![0, 1], vec![0, 1, 2]] {
                let found: Vec<usize> =
                    nearest.dominating(&tree, &keys.iter().collect::<Vec<_>>(), at).map(|(_, item)| item).collect();
                let mut every: Vec<(u32, usize)> = placed
                    .iter()
                    .filter(|(key, block, _)| keys.contains(key) && tree.dominates(*block, at))
                    .map(|(_, block, item)| (tree.span(*block).expect("reachable").0, *item))
                    .collect();
                every.sort_unstable();
                every.dedup();
                let expected: Vec<usize> = every.into_iter().rev().map(|(_, item)| item).collect();
                assert_eq!(found, expected, "{keys:?} at {at:?}");
            }
        }
    }
}
