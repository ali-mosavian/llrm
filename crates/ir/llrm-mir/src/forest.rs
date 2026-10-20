//! The natural loops of a CFG as a forest, LLVM's `LoopInfo`: each block knows
//! the innermost loop holding it, each loop its parent, and a loop's body is
//! what its subtree holds. Found in the time of the blocks and edges, the
//! walk out from each header, innermost first, taking a loop it meets whole
//! and going on from its header; naming every loop's body is the blocks times
//! how deep they nest, which is what a caller that only asks where a block
//! stands, or how deep, does not pay.
//!
//! One owner for the MIR, the LIR and the graph walks: they ask this, and name
//! a body only where one is read whole. Over block addresses (`i64`), so a CFG
//! of any kind is one.

use crate::loopinfo::{BlockNumber, Dominance, LoopGraph, LoopInfo};

/// One loop of a [`Forest`].
#[derive(Debug)]
pub struct TreeLoop {
    pub header: i64,
    /// Every block whose own edge goes back to the header, in the order met.
    pub latches: Vec<i64>,
    /// The loop holding this one.
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// The blocks whose innermost loop this is, the header among them.
    pub own: Vec<i64>,
    /// Blocks in the loop, those of the loops inside it too.
    pub size: usize,
    /// Loops holding this one, itself counted.
    pub depth: u32,
    enter: u32,
    last: u32,
}

/// The loops of a CFG: in the order the first back edge of each header was met.
#[derive(Debug)]
pub struct Forest {
    pub loops: Vec<TreeLoop>,
    ats: Vec<i64>,
    /// Each block's innermost loop by place in `ats`; `NONE` outside any.
    innermost: Vec<u32>,
    /// Where an address stands in `ats`, by its distance from the lowest, when
    /// the addresses lie close (a block's number in its function does): a
    /// lookup is then an index, not a search.
    table: Vec<u32>,
}

const NONE: u32 = u32::MAX;

impl Forest {
    /// The innermost loop holding `at`.
    pub fn innermost(
        &self,
        at: i64,
    ) -> Option<usize> {
        let slot = if self.table.is_empty() {
            self.ats.binary_search(&at).ok()?
        } else {
            let place = *self.table.get(usize::try_from(at.checked_sub(self.ats[0])?).ok()?)?;
            if place == NONE {
                return None;
            }
            place as usize
        };
        (self.innermost[slot] != NONE).then(|| self.innermost[slot] as usize)
    }

    /// How many loops hold `at`.
    pub fn depth(
        &self,
        at: i64,
    ) -> u32 {
        self.innermost(at).map_or(0, |one| self.loops[one].depth)
    }

    /// Whether loop `index` holds `at`.
    pub fn contains(
        &self,
        index: usize,
        at: i64,
    ) -> bool {
        let one = &self.loops[index];
        self.innermost(at).is_some_and(|inner| (one.enter..=one.last).contains(&self.loops[inner].enter))
    }

    /// The blocks of loop `index`, loops inside it included, in no order.
    pub fn blocks(
        &self,
        index: usize,
    ) -> impl Iterator<Item = i64> + '_ {
        let mut pending = vec![index];
        let mut now: std::slice::Iter<'_, i64> = [].iter();
        std::iter::from_fn(move || {
            loop {
                if let Some(&at) = now.next() {
                    return Some(at);
                }
                let one = pending.pop()?;
                pending.extend(self.loops[one].children.iter().copied());
                now = self.loops[one].own.iter();
            }
        })
    }
}

/// The blocks of one loop, those of the loops inside it too, as the forest
/// holds them: whether a block is in it is a lookup, and they are named, in
/// ascending order, the first time they are walked.
#[derive(Clone)]
pub struct Body {
    forest: std::rc::Rc<Forest>,
    index: usize,
    /// Blocks put in since, by a pass that keeps the loops up to date as it
    /// edits (a block put on an edge); none in a loop as found.
    added: std::collections::BTreeSet<i64>,
    named: std::rc::Rc<std::cell::OnceCell<Vec<i64>>>,
}

impl Body {
    pub fn new(
        forest: &std::rc::Rc<Forest>,
        index: usize,
    ) -> Self {
        Self {
            forest: std::rc::Rc::clone(forest),
            index,
            added: std::collections::BTreeSet::new(),
            named: std::rc::Rc::default(),
        }
    }

    /// A body of the blocks named, in no function: a loop made for a caller
    /// that has the blocks and not a forest.
    pub fn from_blocks(blocks: impl IntoIterator<Item = i64>) -> Self {
        let mut ats: Vec<i64> = blocks.into_iter().collect();
        ats.sort_unstable();
        ats.dedup();
        let header = ats.first().copied().unwrap_or(0);
        let forest = Forest {
            loops: vec![TreeLoop {
                header,
                latches: Vec::new(),
                parent: None,
                children: Vec::new(),
                own: ats.clone(),
                size: ats.len(),
                depth: 1,
                enter: 0,
                last: 0,
            }],
            innermost: vec![0; ats.len()],
            table: lookup(&ats),
            ats,
        };
        Self::new(&std::rc::Rc::new(forest), 0)
    }

    pub fn contains(
        &self,
        at: &i64,
    ) -> bool {
        self.forest.contains(self.index, *at) || self.added.contains(at)
    }

    /// This loop's place in its forest's `loops`.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The forest this loop is in.
    pub fn forest(&self) -> &std::rc::Rc<Forest> {
        &self.forest
    }

    /// Puts `at` in; whether it was not.
    pub fn insert(
        &mut self,
        at: i64,
    ) -> bool {
        if self.contains(&at) {
            return false;
        }
        self.added.insert(at);
        self.named = std::rc::Rc::default();
        true
    }

    pub fn len(&self) -> usize {
        self.forest.loops[self.index].size + self.added.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether every block of `other` is in this one: for loops of one forest,
    /// whether `other` is this loop or inside it.
    pub fn is_superset(
        &self,
        other: &Body,
    ) -> bool {
        if std::rc::Rc::ptr_eq(&self.forest, &other.forest) && self.added.is_empty() && other.added.is_empty() {
            let (outer, inner) = (&self.forest.loops[self.index], &self.forest.loops[other.index]);
            return outer.enter <= inner.enter && inner.enter <= outer.last;
        }
        other.len() <= self.len() && other.iter().all(|at| self.contains(at))
    }

    /// How many loops hold this one, itself counted.
    pub fn depth(&self) -> u32 {
        self.forest.loops[self.index].depth
    }

    /// The header of the loop holding this one, the nearest.
    pub fn parent_header(&self) -> Option<i64> {
        self.forest.loops[self.index].parent.map(|parent| self.forest.loops[parent].header)
    }

    /// Whether every block of this one is in `other`.
    pub fn is_subset(
        &self,
        other: &Body,
    ) -> bool {
        other.is_superset(self)
    }

    /// The blocks as a set, for a caller that changes them.
    pub fn to_set(&self) -> std::collections::BTreeSet<i64> {
        self.iter().copied().collect()
    }

    /// The blocks in ascending order.
    pub fn iter(&self) -> std::slice::Iter<'_, i64> {
        self.named
            .get_or_init(|| {
                let mut found: Vec<i64> = self.forest.blocks(self.index).chain(self.added.iter().copied()).collect();
                found.sort_unstable();
                found
            })
            .iter()
    }
}

impl std::fmt::Debug for Body {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.debug_set().entries(self.iter()).finish()
    }
}

impl PartialEq for Body {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl PartialEq<std::collections::BTreeSet<i64>> for Body {
    fn eq(
        &self,
        other: &std::collections::BTreeSet<i64>,
    ) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl Eq for Body {}

impl std::hash::Hash for Body {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.len().hash(state);
        self.iter().for_each(|one| one.hash(state));
    }
}

impl<'a> IntoIterator for &'a Body {
    type Item = &'a i64;
    type IntoIter = std::slice::Iter<'a, i64>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A CFG given as addresses and successors, for [`LoopInfo`]: block `n` is the
/// `n`th of `nodes`, edges to an address no node has are dropped.
struct AddressGraph {
    ats: Vec<i64>,
    succs: Vec<Vec<u32>>,
    preds: Vec<Vec<u32>>,
    entry: Option<u32>,
}

/// A block of an [`AddressGraph`]: its place among the nodes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Slot(u32);

impl BlockNumber for Slot {
    fn number(self) -> usize {
        self.0 as usize
    }
}

impl AddressGraph {
    fn new(
        nodes: &[(i64, &[i64])],
        entry: Option<i64>,
    ) -> Self {
        let ats: Vec<i64> = nodes.iter().map(|(at, _)| *at).collect();
        let mut sorted: Vec<(i64, u32)> = ats.iter().enumerate().map(|(slot, &at)| (at, slot as u32)).collect();
        sorted.sort_unstable();
        let slot = |at: i64| sorted.binary_search_by_key(&at, |one| one.0).ok().map(|found| sorted[found].1);
        let succs: Vec<Vec<u32>> =
            nodes.iter().map(|(_, succ)| succ.iter().filter_map(|&to| slot(to)).collect()).collect();
        let mut preds: Vec<Vec<u32>> = vec![Vec::new(); ats.len()];
        for (from, to) in succs.iter().enumerate() {
            for &to in to {
                preds[to as usize].push(from as u32);
            }
        }
        let entry = entry.and_then(slot).or_else(|| (!ats.is_empty()).then_some(0));
        Self { ats, succs, preds, entry }
    }
}

impl LoopGraph for AddressGraph {
    type Block = Slot;

    fn entry(&self) -> Option<Slot> {
        self.entry.map(Slot)
    }

    fn max_number(&self) -> usize {
        self.ats.len()
    }

    fn child(
        &self,
        block: Slot,
        cursor: &mut usize,
    ) -> Option<Slot> {
        let found = self.succs[block.0 as usize].get(*cursor).copied();
        *cursor += 1;
        found.map(Slot)
    }

    fn inverse_children(
        &self,
        block: Slot,
    ) -> impl Iterator<Item = Slot> {
        self.preds[block.0 as usize].iter().map(|&from| Slot(from))
    }
}

/// What the caller says of reachability and dominance, by address.
struct Asked<'a> {
    ats: &'a [i64],
    reachable: &'a dyn Fn(i64) -> bool,
    dominates: &'a dyn Fn(i64, i64) -> bool,
}

impl Dominance<Slot> for Asked<'_> {
    fn is_reachable(
        &self,
        block: Slot,
    ) -> bool {
        (self.reachable)(self.ats[block.0 as usize])
    }

    fn dominates(
        &self,
        dominator: Slot,
        block: Slot,
    ) -> bool {
        (self.dominates)(self.ats[dominator.0 as usize], self.ats[block.0 as usize])
    }
}

/// The [`Forest`] of the blocks `nodes` name, each an address and its
/// successors, from `entry` (the first block where none is given). `reachable`
/// says whether the entry reaches a block and `dominates` whether one block
/// dominates another; they are asked only for a cycle entered other than
/// through its header.
pub fn forest(
    nodes: &[(i64, &[i64])],
    entry: Option<i64>,
    reachable: &dyn Fn(i64) -> bool,
    dominates: &dyn Fn(i64, i64) -> bool,
) -> Forest {
    let graph = AddressGraph::new(nodes, entry);
    let info = LoopInfo::analyze(&graph, || Asked { ats: &graph.ats, reachable, dominates });
    Forest::of_graph(&graph, &info, (0..nodes.len() as u32).map(Slot), |block| graph.ats[block.0 as usize])
}

impl Forest {
    /// The forest of the loops `info` found in `graph`: the form the graph
    /// walks and `Body` read, until they read `LoopInfo`. DEVIATE (temporary):
    /// an adapter, not in LLVM; it goes with the flat `Loop` in #2197.
    /// `order` names every block once, `address` gives the number a block goes
    /// by. Loops are numbered in the order their first back edge comes in
    /// `order`, a loop's latches likewise, as the finder this replaces
    /// numbered them.
    pub fn of_graph<G: LoopGraph>(
        graph: &G,
        info: &LoopInfo<G::Block>,
        order: impl Iterator<Item = G::Block>,
        address: impl Fn(G::Block) -> i64,
    ) -> Self {
        let preorder = info.get_loops_in_preorder();
        // Where each block comes in `order`, by number.
        let mut position = vec![u32::MAX; graph.max_number()];
        for (at, block) in order.enumerate() {
            position[block.number()] = at as u32;
        }
        let at_block = |block: G::Block| position[block.number()];
        // A loop's first back edge in `order`: the earliest latch, and among
        // its edges the one to the header.
        let mut first: Vec<usize> = (0..preorder.len()).collect();
        let key = |one: &crate::loopinfo::Loop<'_, G::Block>| {
            one.get_loop_latches(graph)
                .into_iter()
                .map(|latch| {
                    let edge = graph.children(latch).position(|child| child == one.get_header()).unwrap_or(0);
                    (at_block(latch), edge)
                })
                .min()
        };
        let keys: Vec<_> = preorder.iter().map(key).collect();
        first.sort_by_key(|&one| keys[one]);
        debug_assert!(keys.iter().all(Option::is_some), "every loop has a latch among the blocks");
        let mut place: Vec<u32> = vec![NONE; preorder.len()];
        for (index, &one) in first.iter().enumerate() {
            place[preorder[one].id().0 as usize] = index as u32;
        }
        let mut loops: Vec<TreeLoop> = Vec::with_capacity(first.len());
        for (preorder_number, one) in preorder.iter().enumerate() {
            let mut latches = one.get_loop_latches(graph);
            latches.sort_by_key(|&latch| at_block(latch));
            latches.dedup();
            let index = place[one.id().0 as usize] as usize;
            if loops.len() <= index {
                loops.resize_with(first.len(), || TreeLoop {
                    header: 0,
                    latches: Vec::new(),
                    parent: None,
                    children: Vec::new(),
                    own: Vec::new(),
                    size: 0,
                    depth: 0,
                    enter: 0,
                    last: 0,
                });
            }
            loops[index] = TreeLoop {
                header: address(one.get_header()),
                latches: latches.into_iter().map(&address).collect(),
                parent: one.get_parent_loop().map(|parent| place[parent.id().0 as usize] as usize),
                children: one.get_sub_loops().map(|child| place[child.id().0 as usize] as usize).collect(),
                own: Vec::new(),
                size: one.get_num_blocks(),
                depth: one.get_loop_depth(),
                enter: preorder_number as u32,
                last: 0,
            };
        }
        let mut ats: Vec<(i64, usize)> = Vec::new();
        for top in info.top_level_loops() {
            for &block in top.get_blocks() {
                let inner = info.get_loop_for(block).expect("a block of a loop");
                ats.push((address(block), place[inner.id().0 as usize] as usize));
            }
        }
        ats.sort_unstable();
        ats.dedup_by_key(|one| one.0);
        let mut innermost = vec![NONE; ats.len()];
        for (slot, &(at, index)) in ats.iter().enumerate() {
            innermost[slot] = index as u32;
            loops[index].own.push(at);
        }
        // `enter` numbers loops in preorder, so a subtree's last number is the
        // largest of its children's, itself if none.
        for one in preorder.iter().rev() {
            let index = place[one.id().0 as usize] as usize;
            loops[index].last =
                loops[index].children.iter().map(|&child| loops[child].last).max().unwrap_or(loops[index].enter);
        }
        let ats: Vec<i64> = ats.into_iter().map(|one| one.0).collect();
        let table = lookup(&ats);
        Forest { loops, ats, innermost, table }
    }
}

/// `ats`' places by distance from the first, when they lie within four times
/// their number of it.
fn lookup(ats: &[i64]) -> Vec<u32> {
    let (Some(&first), Some(&last)) = (ats.first(), ats.last()) else { return Vec::new() };
    let span = (last - first) as usize + 1;
    if span > 4 * ats.len() + 64 {
        return Vec::new();
    }
    let mut table = vec![NONE; span];
    for (place, &at) in ats.iter().enumerate() {
        table[(at - first) as usize] = place as u32;
    }
    table
}
