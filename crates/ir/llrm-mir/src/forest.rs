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

/// A block's successors or predecessors by slot: three held in the vector
/// (24 bytes, a `Vec`'s own size), a `Vec` past them.
type Slots = crate::inline::InlineVec<u32, 3>;

/// The [`Forest`] of the blocks `nodes` name, each an address and its
/// successors, from `entry` (the first block where none is given). `reachable`
/// says whether the entry reaches a block and `dominates` whether one block
/// dominates another; a back edge is an edge from a reachable block to one
/// that dominates it.
pub fn forest(
    nodes: &[(i64, &[i64])],
    entry: Option<i64>,
    reachable: &dyn Fn(i64) -> bool,
    dominates: &dyn Fn(i64, i64) -> bool,
) -> Forest {
    crate::counts::bump(crate::counts::Event::Forests);
    let mut ats: Vec<i64> = nodes.iter().map(|(at, _)| *at).collect();
    ats.sort_unstable();
    ats.dedup();
    let count = ats.len();
    let slot = |at: i64| ats.binary_search(&at).ok();
    let mut succs: Vec<Slots> = vec![Slots::new(); count];
    for (at, succ) in nodes {
        let from = slot(*at).expect("a block address");
        succs[from] = succ.iter().filter_map(|to| slot(*to)).map(|to| to as u32).collect();
    }
    let up: Vec<bool> = ats.iter().map(|&at| reachable(at)).collect();
    let mut preds: Vec<Slots> = vec![Slots::new(); count];
    for from in 0..count {
        if up[from] {
            for &to in &succs[from] {
                preds[to as usize].push(from as u32);
            }
        }
    }
    // The back edges, in the order the blocks and their edges come.
    let mut headers: Vec<(u32, Vec<u32>)> = Vec::new();
    let mut header_of: Vec<u32> = vec![NONE; count];
    for (at, _) in nodes {
        let from = slot(*at).expect("a block address");
        if !up[from] {
            continue;
        }
        for &to in &succs[from] {
            if dominates(ats[to as usize], ats[from]) {
                let index = if header_of[to as usize] == NONE {
                    header_of[to as usize] = headers.len() as u32;
                    headers.push((to, Vec::new()));
                    headers.len() - 1
                } else {
                    header_of[to as usize] as usize
                };
                if !headers[index].1.contains(&(from as u32)) {
                    headers[index].1.push(from as u32);
                }
            }
        }
    }
    let mut loops: Vec<TreeLoop> = headers
        .iter()
        .map(|(header, latches)| TreeLoop {
            header: ats[*header as usize],
            latches: latches.iter().map(|&one| ats[one as usize]).collect(),
            parent: None,
            children: Vec::new(),
            own: Vec::new(),
            size: 0,
            depth: 0,
            enter: 0,
            last: 0,
        })
        .collect();
    // The outer loop's header dominates the inner's, so it comes first in a
    // reverse postorder: the later header is walked first.
    let place = {
        let mut place = vec![NONE; count];
        let mut post = Vec::new();
        let start = entry.or_else(|| nodes.first().map(|(at, _)| *at)).and_then(slot);
        if let Some(start) = start {
            let mut seen = vec![false; count];
            seen[start] = true;
            let mut stack = vec![(start, 0_usize)];
            while let Some(&(at, next)) = stack.last() {
                if let Some(&to) = succs[at].get(next) {
                    stack.last_mut().expect("nonempty").1 += 1;
                    if !seen[to as usize] {
                        seen[to as usize] = true;
                        stack.push((to as usize, 0));
                    }
                } else {
                    post.push(at);
                    stack.pop();
                }
            }
        }
        for (index, &at) in post.iter().enumerate() {
            place[at] = (post.len() - index) as u32;
        }
        place
    };
    let mut order: Vec<usize> = (0..loops.len()).collect();
    order.sort_by_key(|&index| std::cmp::Reverse(place[headers[index].0 as usize]));
    let mut innermost: Vec<u32> = vec![NONE; count];
    for &index in &order {
        let header = headers[index].0;
        innermost[header as usize] = index as u32;
        loops[index].own.push(ats[header as usize]);
        let mut pending: Vec<u32> = headers[index].1.clone();
        while let Some(at) = pending.pop() {
            FOREST_VISITS.with(|visits| visits.set(visits.get() + 1));
            match innermost[at as usize] {
                NONE => {
                    innermost[at as usize] = index as u32;
                    loops[index].own.push(ats[at as usize]);
                    pending.extend(preds[at as usize].iter().copied());
                }
                inside => {
                    let mut top = inside as usize;
                    while let Some(parent) = loops[top].parent {
                        top = parent;
                    }
                    if top == index {
                        continue;
                    }
                    loops[top].parent = Some(index);
                    let entry = headers[top].0 as usize;
                    pending.extend(preds[entry].iter().copied().filter(|&from| innermost[from as usize] != top as u32));
                }
            }
        }
    }
    // Each loop's size, from the inside out; then the numbers a subtree holds,
    // and the depths, from the outside in.
    for &index in &order {
        loops[index].size += loops[index].own.len();
        if let Some(parent) = loops[index].parent {
            loops[parent].size += loops[index].size;
            loops[parent].children.push(index);
        }
    }
    let mut number = 0;
    let mut pending: Vec<(usize, bool)> =
        (0..loops.len()).rev().filter(|&index| loops[index].parent.is_none()).map(|index| (index, false)).collect();
    while let Some((index, done)) = pending.pop() {
        if done {
            loops[index].last = number - 1;
            continue;
        }
        loops[index].enter = number;
        number += 1;
        loops[index].depth = loops[index].parent.map_or(1, |parent| loops[parent].depth + 1);
        pending.push((index, true));
        let children = loops[index].children.clone();
        pending.extend(children.into_iter().rev().map(|child| (child, false)));
    }
    let table = lookup(&ats);
    Forest { loops, ats, innermost, table }
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

thread_local! {
    static FOREST_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Blocks `forest`'s walks have taken up on this thread, for a test of how
/// they grow with the nesting.
pub fn forest_visits() -> usize {
    FOREST_VISITS.with(std::cell::Cell::get)
}
