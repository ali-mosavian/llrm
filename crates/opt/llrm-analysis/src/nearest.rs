//! The items placed in blocks that dominate a given block, nearest first.
//!
//! # How it works
//! Loads look for a provider among the stores and loads that dominate them.
//! Scanning every item of the function per load is quadratic. `Nearest::of`
//! builds, per key, the blocks that hold an item of that key, sorted by their
//! preorder number in the dominator tree, and links each to the nearest block
//! above it in the tree that also holds one. `Nearest::dominating` binary
//! searches for the last block at or before the query block, walks the links up
//! until a block's subtree span contains the query, and yields the items from
//! there up. The work is the length of that chain, not of the function.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   entry["<b>entry</b>"]:::mid
//!   a["<b>A (K)</b>"]:::mid
//!   b["<b>B</b>"]:::mid
//!   c["<b>C (K)</b>"]:::mid
//!   e["<b>E (K)</b>"]:::mid
//!   f["<b>F</b><br/><small>query</small>"]:::mid
//!   entry --> a
//!   a --> b
//!   a --> c
//!   c --> e
//!   e --> f
//! ```
//!
//! *Key K holds items in A, C, E; sorted by preorder A C E with up links - A C.
//! A query at F finds E, the last block <= F, and walks the chain E -> C -> A*
//!
//! Items of several keys are merged by a cursor per key: the walk takes the
//! largest preorder number first, and within a block the last item first. An
//! item reached through two keys is returned once. A block the dominator tree
//! does not reach is left out of the chains and, as the query, yields nothing:
//! everything dominates it, so the caller decides.
//!
//! # Example
//! None: the module has no MIR input of its own. Its test,
//! `test_the_items_that_dominate_a_block_are_found_nearest_first` in
//! `nearest.rs`, places items in a generated chain of diamonds and compares
//! `dominating` with a scan of every item. The figure above is the worked case.
//!
//! # Provides
//! The `Nearest` index and its `Walk` iterator. Consumers: [`crate::avail`] and
//! `loadjoins` in llrm-transforms.
//!
//! # Needs
//! `llrm_mir::domtree::DomTree` (`span`, the preorder interval of a
//! block).

use std::hash::Hash;

use llrm_mir::domtree::DomTree;
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
        tree: &DomTree,
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
        tree: &DomTree,
        keys: &[&K],
        at: BlockId,
    ) -> Walk<'a> {
        self.dominating_below(tree, keys, at, usize::MAX)
    }

    /// `dominating`, but of the items in `at` itself only those below `below`:
    /// a caller that places its items in block order asks for those before one
    /// of them, and is not given the N-1 after it (N loads of one cell in a
    /// block: N^2/2 items walked for the nearest of each).
    pub fn dominating_below<'a>(
        &'a self,
        tree: &DomTree,
        keys: &[&K],
        at: BlockId,
        below: usize,
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
                let entry = at?;
                let within = if chain[entry].enter == point {
                    chain[entry].items.partition_point(|item| *item < below)
                } else {
                    chain[entry].items.len()
                };
                let mut cursor = Cursor { chain, entry: Some(entry), within };
                cursor.settle();
                cursor.entry.is_some().then_some(cursor)
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
        self.settle();
    }

    /// On to the entry above while this one has no item left to give.
    fn settle(&mut self) {
        while self.within == 0 {
            let Some(entry) = self.entry else { return };
            self.entry = self.chain[entry].up;
            self.within = self.entry.map_or(0, |one| self.chain[one].items.len());
        }
    }
}

thread_local! {
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many items this thread's walks have gone through, for a test that the
/// loads of a block are not each asked of all the others.
pub fn steps() -> usize {
    STEPS.with(std::cell::Cell::get)
}

pub struct Walk<'a> {
    cursors: Vec<Cursor<'a>>,
    last: Option<usize>,
}

impl Iterator for Walk<'_> {
    type Item = (u32, usize);

    fn next(&mut self) -> Option<(u32, usize)> {
        loop {
            STEPS.with(|steps| steps.set(steps.get() + 1));
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
        let tree = llrm_mir::domtree::DomTree::of(function);
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
