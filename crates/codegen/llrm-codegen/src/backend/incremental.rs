//! A dataflow answer over a body's blocks, held between asks and brought up to
//! date from the blocks that changed.
//!
//! # How it works
//! The sub-passes of a clean-up ask for the same fact about the body in turn
//! (the lanes live out of each block, the flags live out of each block, the
//! roots zero above their word) and each leaves most blocks as they were. A
//! [`Problem`] says what the fact is: what one block does to it (a summary,
//! worked out from the block's instructions), what enters a block from its
//! neighbours, and what the block makes of that. [`Kept`] holds the last answer
//! and the summaries.
//!
//! ```text
//!  ask(body)
//!    every block the same instructions (element by element) and successors  -> the held answer
//!    else summaries for the changed blocks only, then the strongly connected
//!    components in the order the facts flow:
//!      a component with a changed block, or fed by a component whose value
//!      changed, is solved again from the problem's start; the rest keep theirs
//! ```
//!
//! A component is solved from the problem's start (nothing live; every root
//! zero), never from the held answer: what is live round a loop may be live
//! only because of a read that is gone, and a root zero round a loop only
//! because of a write that is gone, and a solve from the old answer keeps both.
//!
//! A block is the same while its instructions are the same allocations
//! (`Insns::same_insns`), so a pass that rebuilds a block it left as it was
//! costs nothing. The answer says which blocks to work out again for what is
//! derived from it per instruction (`Solution::refreshed`).
//!
//! # Example
//! No example: the answers are printed by no dump. See
//! [`crate::backend::liveness`] for lanes, [`crate::backend::upperzero`] for
//! roots and [`crate::backend::peephole`]'s `_flags_live_out` for flags.
//!
//! # Used by
//! `liveness`, `upperzero` and `peephole` (flag liveness).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::model::lir::{Block, Insns, LirBody};
use crate::support::hash::{HashSet, IndexMap};

/// A fact about the blocks of a body, solved block by block.
pub trait Problem {
    /// What one block does to the fact, worked out from its instructions.
    type Summary;
    type Value: Copy + Eq;
    /// The fact flows from a block to its successors, else to its predecessors.
    const FORWARD: bool;

    fn summarize(
        &self,
        body: &LirBody,
        block: &Block,
    ) -> Self::Summary;

    /// What a component is solved from.
    fn start(&self) -> Self::Value;

    /// The value entering block `at` (the entry block of the body is `entry`):
    /// from the values leaving the neighbours it is fed by that are blocks of
    /// the body (`present`), with `missing` of the neighbours it names that
    /// are not.
    fn entering(
        &self,
        at: i64,
        entry: i64,
        present: &[Self::Value],
        missing: usize,
    ) -> Self::Value;

    /// The value leaving a block that `entering` enters.
    fn leaving(
        &self,
        summary: &Self::Summary,
        entering: Self::Value,
    ) -> Self::Value;
}

/// The answer for a body.
pub struct Solution<P: Problem> {
    /// Per block, the value entering it (forward: before it; backward: after
    /// it).
    pub entering: IndexMap<i64, P::Value>,
    /// Per block, the value leaving it (forward: after it; backward: before
    /// it).
    pub leaving: IndexMap<i64, P::Value>,
    pub summaries: IndexMap<i64, Rc<P::Summary>>,
    /// The blocks that are not as the last answer had them: new, changed, or
    /// entered with another value.
    pub refreshed: HashSet<i64>,
}

struct Seen<P: Problem> {
    insns: Insns,
    /// Where the block says it goes.
    raw: Rc<Vec<i64>>,
    summary: Rc<P::Summary>,
}

/// The edges of a body and the components in the order the facts flow, kept
/// while no block's successors change.
struct Graph {
    ats: Vec<i64>,
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
    components: Vec<Vec<usize>>,
}

struct Held<P: Problem> {
    key: (u32, i64),
    blocks: IndexMap<i64, Seen<P>>,
    solution: Rc<Solution<P>>,
    graph: Rc<Graph>,
}

/// The last answer, per thread, and how it was made.
pub struct Kept<P: Problem> {
    held: RefCell<Option<Held<P>>>,
    asks: Cell<[usize; 3]>,
    visits: Cell<usize>,
    graphs: Cell<usize>,
}

impl<P: Problem> Default for Kept<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: Problem> Kept<P> {
    pub const fn new() -> Self {
        Self { held: RefCell::new(None), asks: Cell::new([0; 3]), visits: Cell::new(0), graphs: Cell::new(0) }
    }

    /// Answers made from scratch (no block left as the last answer had it),
    /// brought up to date, and asks of a body no pass changed.
    pub fn asks(&self) -> [usize; 3] {
        self.asks.get()
    }

    /// Blocks worked out so far, whole or in part.
    pub fn visits(&self) -> usize {
        self.visits.get()
    }

    /// Graphs made so far: one for each answer whose blocks or successors
    /// were not the last answer's.
    pub fn graphs(&self) -> usize {
        self.graphs.get()
    }

    fn counted(
        &self,
        which: usize,
    ) {
        let mut now = self.asks.get();
        now[which] += 1;
        self.asks.set(now);
    }

    /// The answer for `body`.
    pub fn ask(
        &self,
        problem: &P,
        body: &LirBody,
    ) -> Rc<Solution<P>> {
        let key = (body.bits, body.entry);
        let held = self.held.borrow_mut().take().filter(|held| held.key == key);
        if let Some(same) = held.as_ref().filter(|held| {
            held.blocks.len() == body.blocks.len()
                && body.blocks.iter().all(|block| {
                    held.blocks
                        .get(&block.at)
                        .is_some_and(|old| old.insns.same_insns(&block.insns) && *old.raw == *block.succ)
                })
        }) {
            self.counted(2);
            let solution = Rc::clone(&same.solution);
            *self.held.borrow_mut() = held;
            return solution;
        }
        let mut held = held;
        // The same blocks in the same order: the last answer's table is
        // updated in place where a block changed, not written again.
        let mut now: IndexMap<i64, Seen<P>> = match held.as_mut() {
            Some(held)
                if held.blocks.len() == body.blocks.len()
                    && body.blocks.iter().zip(held.blocks.keys()).all(|(block, at)| block.at == *at) =>
            {
                std::mem::take(&mut held.blocks)
            }
            _ => IndexMap::default(),
        };
        let reused = !now.is_empty();
        let mut dirty: HashSet<i64> = HashSet::default();
        // Whether any block goes somewhere else than the last answer had it.
        let mut rerouted = false;
        let seen = |block: &Block| Seen {
            insns: block.insns.clone(),
            raw: Rc::new(block.succ.to_vec()),
            summary: Rc::new(problem.summarize(body, block)),
        };
        for (slot, block) in body.blocks.iter().enumerate() {
            if reused {
                let old = &mut now[slot];
                if old.insns.same_insns(&block.insns) && *old.raw == *block.succ {
                    continue;
                }
                rerouted |= *old.raw != *block.succ;
                dirty.insert(block.at);
                *old = seen(block);
                continue;
            }
            match held.as_ref().and_then(|held| held.blocks.get(&block.at)) {
                Some(old) if old.insns.same_insns(&block.insns) && *old.raw == *block.succ => {
                    now.insert(
                        block.at,
                        Seen { insns: block.insns.clone(), raw: Rc::clone(&old.raw), summary: Rc::clone(&old.summary) },
                    );
                }
                other => {
                    rerouted |= other.is_none_or(|old| *old.raw != *block.succ);
                    dirty.insert(block.at);
                    now.insert(block.at, seen(block));
                }
            }
        }
        self.counted(usize::from(dirty.len() < now.len()));
        let start = problem.start();
        // The solve works by position in `now`: a vector slot for each block
        // where a map keyed by block address was written on every visit.
        let ats: Vec<i64> = now.keys().copied().collect();
        let was = |at: &i64, of: fn(&Solution<P>) -> &IndexMap<i64, P::Value>| {
            held.as_ref().and_then(|held| of(&held.solution).get(at)).copied().unwrap_or(start)
        };
        let mut leaving: Vec<P::Value> = ats.iter().map(|at| was(at, |s| &s.leaving)).collect();
        let mut entering: Vec<P::Value> = ats.iter().map(|at| was(at, |s| &s.entering)).collect();
        let dirty: Vec<bool> = ats.iter().map(|at| dirty.contains(at)).collect();
        // Who feeds each block, and whom it feeds: as the last answer had it
        // while no block was rerouted and the blocks are the same in the same
        // order.
        let graph = match held.as_ref().map(|held| &held.graph) {
            Some(graph) if !rerouted && graph.ats == ats => Rc::clone(graph),
            _ => {
                self.graphs.set(self.graphs.get() + 1);
                let mut successors: Vec<Vec<usize>> = Vec::with_capacity(ats.len());
                let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); ats.len()];
                for (from, seen) in now.values().enumerate() {
                    successors.push(seen.raw.iter().filter_map(|next| now.get_index_of(next)).collect());
                    for next in &successors[from] {
                        predecessors[*next].push(from);
                    }
                }
                let mut components = llrm_support::graph::components(&body.blocks);
                if P::FORWARD {
                    components.reverse();
                }
                let components = components
                    .into_iter()
                    .map(|component| component.iter().filter_map(|at| now.get_index_of(at)).collect())
                    .collect();
                Rc::new(Graph { ats: ats.clone(), successors, predecessors, components })
            }
        };
        let (fed_by, feeds) = if P::FORWARD {
            (&graph.predecessors, &graph.successors)
        } else {
            (&graph.successors, &graph.predecessors)
        };
        let mut gathered: Vec<P::Value> = Vec::new();
        let mut moved = vec![false; ats.len()];
        let mut inside = vec![false; ats.len()];
        let mut queued = vec![false; ats.len()];
        for component in &graph.components {
            for at in component {
                inside[*at] = true;
            }
            let reached = |at: &usize| fed_by[*at].iter().any(|from| moved[*from] && !inside[*from]);
            if component.iter().any(|at| dirty[*at] || reached(at)) {
                let before: Vec<P::Value> = component.iter().map(|at| leaving[*at]).collect();
                for at in component {
                    leaving[*at] = start;
                }
                // The deepest block first for a fact that flows back, the first
                // reached for one that flows on.
                let mut pending: Vec<usize> =
                    if P::FORWARD { component.to_vec() } else { component.iter().rev().copied().collect() };
                for at in component {
                    queued[*at] = true;
                }
                while let Some(at) = pending.pop() {
                    self.visits.set(self.visits.get() + 1);
                    queued[at] = false;
                    gathered.clear();
                    gathered.extend(fed_by[at].iter().map(|from| leaving[*from]));
                    let missing = if P::FORWARD { 0 } else { now[at].raw.len() - fed_by[at].len() };
                    let value = problem.entering(ats[at], body.entry, &gathered, missing);
                    entering[at] = value;
                    let after = problem.leaving(&now[at].summary, value);
                    if after != leaving[at] {
                        leaving[at] = after;
                        for next in feeds[at].iter().filter(|next| inside[**next]) {
                            if !queued[*next] {
                                queued[*next] = true;
                                pending.push(*next);
                            }
                        }
                    }
                }
                for (at, before) in component.iter().zip(before) {
                    if leaving[*at] != before {
                        moved[*at] = true;
                    }
                }
            }
            for at in component {
                inside[*at] = false;
            }
        }
        let refreshed: HashSet<i64> = ats
            .iter()
            .enumerate()
            .filter(|(at, address)| {
                dirty[*at]
                    || held.as_ref().and_then(|held| held.solution.entering.get(*address)) != Some(&entering[*at])
            })
            .map(|(_, address)| *address)
            .collect();
        // Over the last answer's maps when the blocks are the same, which
        // hashes none of them again.
        let patched = |was: Option<&IndexMap<i64, P::Value>>, values: Vec<P::Value>| match was {
            Some(was) if reused => {
                let mut made = was.clone();
                for (slot, value) in values.into_iter().enumerate() {
                    *made.get_index_mut(slot).expect("a block of the last answer").1 = value;
                }
                made
            }
            _ => ats.iter().copied().zip(values).collect(),
        };
        let entering = patched(held.as_ref().map(|held| &held.solution.entering), entering);
        let leaving = patched(held.as_ref().map(|held| &held.solution.leaving), leaving);
        let summaries = now.iter().map(|(at, seen)| (*at, Rc::clone(&seen.summary))).collect();
        let solution = Rc::new(Solution { entering, leaving, summaries, refreshed });
        *self.held.borrow_mut() = Some(Held { key, blocks: now, solution: Rc::clone(&solution), graph });
        solution
    }
}
