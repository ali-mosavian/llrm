//! Graphs of blocks that are `Node`s: who reaches whom, in what order, and
//! what dominates what. Moved from `llrm-analysis`'s `graph::loops` so that
//! an IR crate can check dominance without depending on the optimiser.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

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
/// Blocks in reverse postorder from `entry`, then those it cannot reach in body
/// order: the order a forward dataflow worklist drains in, predecessors before
/// successors but for back edges.
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

/// The blocks that branch to each block, sorted, each list once: three
/// allocations whatever the blocks, where a map of sets made one for every
/// block and a node for every eleven predecessors (`predecessors` was 20 M of
/// a d_faces -O2 compile, 16 M of it in the trees).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Predecessors {
    /// Every block's `at`, ascending.
    ats: Vec<i64>,
    /// `from[start[k]..start[k + 1]]` are the predecessors of `ats[k]`.
    start: Vec<u32>,
    from: Vec<i64>,
}

impl Predecessors {
    fn place(
        &self,
        at: &i64,
    ) -> Option<usize> {
        self.ats.binary_search(at).ok()
    }

    /// The predecessors of block `at`, ascending, or none for an unknown block.
    pub fn get(
        &self,
        at: &i64,
    ) -> Option<&[i64]> {
        let k = self.place(at)?;
        Some(&self.from[self.start[k] as usize..self.start[k + 1] as usize])
    }

    pub fn contains_key(
        &self,
        at: &i64,
    ) -> bool {
        self.place(at).is_some()
    }

    /// Whether the predecessors of `at` are exactly the blocks `expected`
    /// names, in any order.
    pub fn are(
        &self,
        at: &i64,
        expected: &[i64],
    ) -> bool {
        let mut want = expected.to_vec();
        want.sort_unstable();
        want.dedup();
        self.get(at) == Some(&want[..])
    }

    /// How many blocks there are.
    pub fn len(&self) -> usize {
        self.ats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ats.is_empty()
    }

    /// Every block, ascending.
    pub fn keys(&self) -> impl Iterator<Item = &i64> {
        self.ats.iter()
    }

    /// Every block with its predecessors, ascending by block.
    pub fn iter(&self) -> impl Iterator<Item = (&i64, &[i64])> {
        self.ats.iter().enumerate().map(|(k, at)| (at, &self.from[self.start[k] as usize..self.start[k + 1] as usize]))
    }

    /// Every block's predecessors, ascending by block.
    pub fn values(&self) -> impl Iterator<Item = &[i64]> {
        self.iter().map(|(_, from)| from)
    }

    /// As the map of sets it was.
    pub fn named(&self) -> BTreeMap<i64, BTreeSet<i64>> {
        self.iter().map(|(at, from)| (*at, from.iter().copied().collect())).collect()
    }
}

impl std::ops::Index<&i64> for Predecessors {
    type Output = [i64];

    fn index(
        &self,
        at: &i64,
    ) -> &[i64] {
        self.get(at).expect("a block of the graph")
    }
}

impl<'a> IntoIterator for &'a Predecessors {
    type Item = (&'a i64, &'a [i64]);
    type IntoIter = Box<dyn Iterator<Item = (&'a i64, &'a [i64])> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

pub fn predecessors<N: Node>(blocks: &[N]) -> Predecessors {
    let mut ats: Vec<i64> = blocks.iter().map(Node::at).collect();
    ats.sort_unstable();
    ats.dedup();
    let mut edges: Vec<(u32, i64)> = Vec::with_capacity(blocks.iter().map(|block| block.succ().len()).sum());
    for block in blocks {
        for successor in block.succ() {
            if let Ok(to) = ats.binary_search(successor) {
                edges.push((to as u32, block.at()));
            }
        }
    }
    edges.sort_unstable();
    edges.dedup();
    let mut start = vec![0_u32; ats.len() + 1];
    for (to, _) in &edges {
        start[*to as usize + 1] += 1;
    }
    for k in 0..ats.len() {
        start[k + 1] += start[k];
    }
    Predecessors { ats, start, from: edges.into_iter().map(|(_, from)| from).collect() }
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

/// The dominator tree over the sorted block addresses, with a preorder interval
/// for each block: `a` dominates `b` where `b` is in `a`'s interval. Naming
/// every block's dominators as bits is quadratic.
pub struct Dominance {
    ats: Vec<i64>,
    /// The immediate dominator's slot; the entry's own, and unreachable
    /// blocks', is `NONE`.
    idom: Vec<u32>,
    /// Preorder numbers over the dominator tree; `NONE` for a block the entry
    /// does not reach.
    enter: Vec<u32>,
    /// The last preorder number inside the block's subtree.
    last: Vec<u32>,
}

const NONE: u32 = u32::MAX;

impl Dominance {
    fn slot(
        &self,
        at: i64,
    ) -> Option<usize> {
        self.ats.binary_search(&at).ok()
    }

    /// The nearest block that strictly dominates `at`; none for the entry and
    /// for a block the entry does not reach.
    pub fn immediate(
        &self,
        at: i64,
    ) -> Option<i64> {
        let slot = self.slot(at)?;
        (self.enter[slot] != NONE && self.idom[slot] != NONE).then(|| self.ats[self.idom[slot] as usize])
    }

    /// Whether the entry reaches `at`: only then does anything dominate it.
    pub fn reachable(
        &self,
        at: i64,
    ) -> bool {
        self.slot(at).is_some_and(|slot| self.enter[slot] != NONE)
    }

    /// `at`'s place in a preorder of the dominator tree: a block dominated by
    /// another comes after it.
    pub fn preorder(
        &self,
        at: i64,
    ) -> Option<u32> {
        self.slot(at).map(|slot| self.enter[slot]).filter(|&enter| enter != NONE)
    }

    pub fn dominates(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool {
        match (self.slot(dominator), self.slot(at)) {
            (Some(dominator), Some(at)) => {
                self.enter[at] != NONE
                    && self.enter[dominator] <= self.enter[at]
                    && self.enter[at] <= self.last[dominator]
            }
            _ => false,
        }
    }

    pub fn named(&self) -> BTreeMap<i64, BTreeSet<i64>> {
        (0..self.ats.len())
            .map(|slot| {
                let mut above = BTreeSet::new();
                if self.enter[slot] != NONE {
                    let mut at = slot as u32;
                    loop {
                        above.insert(self.ats[at as usize]);
                        if self.idom[at as usize] == NONE {
                            break;
                        }
                        at = self.idom[at as usize];
                    }
                }
                (self.ats[slot], above)
            })
            .collect()
    }
}

pub fn dominance<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Rc<Dominance> {
    Rc::new(_dominance(blocks, entry))
}

// Blocks visited and dominator-tree steps climbed by `_dominance`: the work a
// test counts, as the wall clock moves with the host's load.
thread_local! {
    static WORK: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn counted() {
    WORK.with(|work| work.set(work.get() + 1));
}

/// The steps `dominance` has taken on this thread.
pub fn dominance_work() -> u64 {
    WORK.with(std::cell::Cell::get)
}

/// Cooper, Harvey and Kennedy's iteration over reverse postorder, on the blocks
/// the entry reaches.
fn _dominance<N: Node>(
    blocks: &[N],
    entry: Option<i64>,
) -> Dominance {
    let ats = blocks.iter().map(Node::at).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let count = ats.len();
    let mut found = Dominance { ats, idom: vec![NONE; count], enter: vec![NONE; count], last: vec![NONE; count] };
    if blocks.is_empty() {
        return found;
    }
    let start = entry.unwrap_or_else(|| blocks[0].at());
    let Some(start) = found.slot(start) else { return found };
    // Each address's successors, as slots; a later block of one address
    // replaces an earlier one's.
    let mut successors: Vec<Vec<u32>> = vec![Vec::new(); count];
    for block in blocks {
        let from = found.slot(block.at()).expect("a block address");
        successors[from] = block.succ().iter().filter_map(|to| found.slot(*to)).map(|to| to as u32).collect();
    }

    // Postorder by an explicit stack.
    let mut order = Vec::new();
    let mut seen = vec![false; count];
    let mut stack = vec![(start, 0_usize)];
    seen[start] = true;
    while let Some(&(at, next)) = stack.last() {
        if let Some(&to) = successors[at].get(next) {
            stack.last_mut().expect("nonempty").1 += 1;
            if !seen[to as usize] {
                seen[to as usize] = true;
                stack.push((to as usize, 0));
            }
        } else {
            order.push(at);
            stack.pop();
        }
    }
    let mut number = vec![NONE; count];
    for (index, &at) in order.iter().enumerate() {
        number[at] = index as u32;
    }
    let mut predecessors: Vec<Vec<u32>> = vec![Vec::new(); count];
    for &at in &order {
        for &to in &successors[at] {
            predecessors[to as usize].push(at as u32);
        }
    }

    let mut idom = vec![NONE; count];
    idom[start] = start as u32;
    let mut changing = true;
    while changing {
        changing = false;
        for &at in order.iter().rev() {
            if at == start {
                continue;
            }
            counted();
            let mut now = NONE;
            for &from in &predecessors[at] {
                if idom[from as usize] == NONE {
                    continue;
                }
                now = if now == NONE {
                    from
                } else {
                    let (mut one, mut other) = (now, from);
                    while one != other {
                        while number[one as usize] < number[other as usize] {
                            counted();
                            one = idom[one as usize];
                        }
                        while number[other as usize] < number[one as usize] {
                            counted();
                            other = idom[other as usize];
                        }
                    }
                    one
                };
            }
            if now != idom[at] {
                idom[at] = now;
                changing = true;
            }
        }
    }
    idom[start] = NONE;

    // Preorder intervals of the tree.
    let mut children: Vec<Vec<u32>> = vec![Vec::new(); count];
    for &at in order.iter().rev() {
        if idom[at] != NONE {
            children[idom[at] as usize].push(at as u32);
        }
    }
    let mut clock = 0;
    let mut walk = vec![(start, 0_usize)];
    found.enter[start] = 0;
    clock += 1;
    while let Some(&(at, next)) = walk.last() {
        if let Some(&child) = children[at].get(next) {
            walk.last_mut().expect("nonempty").1 += 1;
            found.enter[child as usize] = clock;
            clock += 1;
            walk.push((child as usize, 0));
        } else {
            found.last[at] = clock - 1;
            walk.pop();
        }
    }
    found.idom = idom;
    found
}
