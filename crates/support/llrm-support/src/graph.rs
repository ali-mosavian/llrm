//! Graphs of blocks that are `Node`s: who reaches whom, in what order, and
//! what dominates what. Moved from `llrm-analysis`'s `graph::loops` so that
//! an IR crate can check dominance without depending on the optimiser.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::bits::Bits;

/// All these walks read of a block: where it is and where it goes.
pub trait Node {
    fn at(&self) -> i64;
    fn succ(&self) -> &[i64];
}

impl<T: Node> Node for &T {
    fn at(&self) -> i64 {
        (*self).at()
    }

    fn succ(&self) -> &[i64] {
        (*self).succ()
    }
}

/// Who can reach each block, inverted from its own successors.
/// Blocks in reverse postorder from `entry`, then those it cannot reach in body order:
/// the order a forward dataflow worklist drains in, predecessors before successors
/// but for back edges.
pub fn reverse_postorder<N: Node>(
    blocks: &[N],
    entry: i64,
) -> Vec<i64> {
    let known = blocks.iter().map(|block| (block.at(), block)).collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::from([entry]);
    let mut post = Vec::with_capacity(blocks.len());
    let mut stack = known.get(&entry).map(|block| vec![(*block, 0usize)]).unwrap_or_default();
    while let Some((block, next)) = stack.last_mut() {
        let block = *block;
        if let Some(successor) = block.succ().get(*next) {
            *next += 1;
            if let Some(child) = known.get(successor).filter(|_| seen.insert(*successor)) {
                stack.push((*child, 0));
            }
        } else {
            post.push(block.at());
            stack.pop();
        }
    }
    post.reverse();
    post.extend(blocks.iter().map(Node::at).filter(|at| !seen.contains(at)));
    post
}

pub fn predecessors<N: Node>(blocks: &[N]) -> BTreeMap<i64, BTreeSet<i64>> {
    let known = blocks.iter().map(Node::at).collect::<BTreeSet<_>>();
    let mut found = blocks.iter().map(|block| (block.at(), BTreeSet::new())).collect::<BTreeMap<_, _>>();
    for block in blocks {
        for successor in block.succ() {
            if known.contains(successor) {
                found.get_mut(successor).expect("known").insert(block.at());
            }
        }
    }
    found
}

/// Every block that must have run before each one, to a fixed point.
///
/// A block unreachable from the entry gets the empty set rather than "every
/// block".
pub fn dominators<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> BTreeMap<i64, BTreeSet<i64>> {
    dominance(blocks, entry).named()
}

/// Dominance as the walks here read it: nothing dominates a block the entry
/// does not reach.
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

impl Dominates for Dominance {
    fn reachable(
        &self,
        at: i64,
    ) -> bool {
        Dominance::reachable(self, at)
    }

    fn dominates(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool {
        Dominance::dominates(self, dominator, at)
    }
}

/// Each block's dominators as bits over the sorted block addresses; naming
/// them as sets costs more than finding them.
pub struct Dominance {
    ats: Vec<i64>,
    doms: Vec<Bits>,
}

impl Dominance {
    fn slot(
        &self,
        at: i64,
    ) -> Option<usize> {
        self.ats.binary_search(&at).ok()
    }

    /// Whether the entry reaches `at`: only then does anything dominate it.
    pub fn reachable(
        &self,
        at: i64,
    ) -> bool {
        self.slot(at).is_some_and(|slot| !self.doms[slot].is_empty())
    }

    pub fn dominates(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool {
        match (self.slot(dominator), self.slot(at)) {
            (Some(dominator), Some(at)) => self.doms[at].contains(dominator),
            _ => false,
        }
    }

    fn named(&self) -> BTreeMap<i64, BTreeSet<i64>> {
        let ats = &self.ats;
        ats.iter().zip(&self.doms).map(|(at, set)| (*at, set.iter().map(|one| ats[one]).collect())).collect()
    }
}

pub fn dominance<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Rc<Dominance> {
    Rc::new(_dominance(blocks, entry))
}

fn _dominance<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Dominance {
    if blocks.is_empty() {
        return Dominance { ats: Vec::new(), doms: Vec::new() };
    }
    let start = entry.unwrap_or_else(|| blocks[0].at());
    let indexed = blocks.iter().map(|block| (block.at(), block)).collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(at) = pending.pop() {
        if !indexed.contains_key(&at) || reachable.contains(&at) {
            continue;
        }
        reachable.insert(at);
        pending.extend(indexed[&at].succ().iter().copied());
    }
    let every = reachable.clone();
    let preds = predecessors(&blocks.iter().filter(|block| reachable.contains(&block.at())).collect::<Vec<_>>());

    // The fixed point runs on bit sets over the distinct block addresses.
    let ats = blocks.iter().map(Node::at).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let slot = |at: &i64| ats.binary_search(at).expect("a block address");
    let mut all = Bits::new(ats.len());
    for at in &every {
        all.insert(slot(at));
    }
    let empty = Bits::new(ats.len());
    let mut doms = vec![empty.clone(); ats.len()];
    for block in blocks {
        doms[slot(&block.at())] = if reachable.contains(&block.at()) { all.clone() } else { empty.clone() };
    }
    if indexed.contains_key(&start) {
        let mut own = empty.clone();
        own.insert(slot(&start));
        doms[slot(&start)] = own;
    }
    let reaching = |at: i64| preds[&at].iter().map(slot).collect::<Vec<_>>();
    let reaching =
        blocks
            .iter()
            .map(|block| {
                if block.at() == start || !reachable.contains(&block.at()) { Vec::new() } else { reaching(block.at()) }
            })
            .collect::<Vec<_>>();

    let mut changing = true;
    while changing {
        changing = false;
        for (block, reaching) in blocks.iter().zip(&reaching) {
            if block.at() == start || !reachable.contains(&block.at()) {
                continue;
            }
            let mut now = match reaching.split_first() {
                Some((first, rest)) => rest.iter().fold(doms[*first].clone(), |mut shared, one| {
                    shared.intersect_with(&doms[*one]);
                    shared
                }),
                None => empty.clone(),
            };
            let at = slot(&block.at());
            now.insert(at);
            if now != doms[at] {
                doms[at] = now;
                changing = true;
            }
        }
    }
    Dominance { ats, doms }
}
