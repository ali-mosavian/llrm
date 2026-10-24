//! How often each block of a LIR body runs per call, and each edge is taken:
//! the one estimate every consumer asks, LLVM's `MachineBlockFrequencyInfo`
//! over `MachineBranchProbabilityInfo`.
//!
//! The edge probabilities are `LirBody::odds`, which isel made from
//! `BranchProbabilityInfo` over the MIR (an edge made by splitting one
//! inherits its odds, `BlockOdds::redirected`); and `BlockFrequencyInfoImpl`
//! turns them into frequencies (`propagated`).
//!
//! DEVIATE 1: `counted`, the executed-work estimate `executed` makes and the
//! route choice reads, lets a loop's proven trips
//! (`loop_trip_counts`) replace the heuristic's 31 in 32; LLVM has no such
//! estimate. `of`, which every translated pass reads, has no trips (a bench
//! pair measured nothing for them, #2731). Exit: remove when the route choice's
//! use is replaced by a concept of the pin.

use std::sync::Arc;

use llrm_mir::blockfrequency::{BlockFrequencyInfo, BranchProbability, EdgeProbabilities, FlowGraph};
use llrm_mir::blocklist::BlockList;
use llrm_mir::cycleinfo::CycleInfo;
use llrm_mir::domtree::DomGraph;
use llrm_mir::module::BlockId;

use crate::analysis::facts::Fact;
use crate::analysis::loops;
use crate::model::lir::{Block, Insns, LirBlock, LirBody};
use crate::model::lir::{BlockFrequencies, BlockOdds};
use crate::support::hash::IndexMap;

/// The body's blocks as a frequency reads them: the flow between them, the odds
/// on its edges and the proven trips. No instruction is read, so a spill or a
/// reload leaves a frequency as it was.
pub struct Cfg {
    entry: i64,
    shape: Vec<(i64, Vec<i64>)>,
    odds: BlockOdds,
    trips: Vec<(i64, i64)>,
}

impl Cfg {
    fn of(body: &LirBody) -> Self {
        Self {
            entry: body.entry,
            shape: body.blocks.iter().map(|block| (block.at, block.succ.to_vec())).collect(),
            odds: body.odds.clone(),
            trips: body.loop_trip_counts.clone(),
        }
    }

    fn is_of(
        &self,
        body: &LirBody,
    ) -> bool {
        self.entry == body.entry
            && self.shape.len() == body.blocks.len()
            && self.shape.iter().zip(&body.blocks).all(|((at, succ), block)| *at == block.at && *succ == block.succ)
            && self.odds == body.odds
            && self.trips == body.loop_trip_counts
    }
}

/// Whether a block does nothing but jump on: no instruction that prints, but
/// jumps.
fn idle(block: &LirBlock) -> bool {
    block
        .insns
        .iter()
        .all(
            |one| !crate::backend::masm::prints(one)
                || one.what.as_ref().is_some_and(|what| what.op == crate::model::ir::Operation::Jump),
        )
}

/// What `Frequency::over` reads of the instructions: whether each block that
/// can be a bridge (one successor, not the entry, not its own) only jumps on.
/// A frequency worked out from the blocks alone (`Worked`) is good while these
/// are as they were; one carried from a table (`Carried`) reads the blocks
/// made since as bridges whatever is in them.
pub struct Bridges(Vec<Option<std::sync::Mutex<(Insns, bool)>>>);

impl Bridges {
    fn of(body: &LirBody) -> Self {
        Self(
            body.blocks
                .iter()
                .map(|block| {
                    (block.at != body.entry && block.succ.len() == 1 && block.succ[0] != block.at)
                        .then(|| std::sync::Mutex::new((block.insns.clone(), idle(block))))
                })
                .collect(),
        )
    }

    fn is_of(
        &self,
        body: &LirBody,
    ) -> bool {
        self.0
            .iter()
            .zip(&body.blocks)
            .all(
                |(kept, block)| {
                    // Changed instructions that are idle as they were are
                    // noted, so that the next ask looks at what changed since.
                    kept.as_ref().is_none_or(|held| {
                        let mut held = held.lock().expect("idle");
                        held.0.same_as(&block.insns)
                            || (held.1 == idle(block) && {
                                held.0 = block.insns.clone();
                                true
                            })
                    })
                },
            )
    }
}

/// Block frequencies worked out from the blocks alone.
pub struct Worked;

impl Fact for Worked {
    type Result = Frequency;
    type Inputs = (Cfg, Bridges);
    const NAME: &'static str = "frequency";
    fn run(body: &LirBody) -> Frequency {
        Frequency::over(body, &body.blocks)
    }
    fn inputs(body: &LirBody) -> (Cfg, Bridges) {
        (Cfg::of(body), Bridges::of(body))
    }
    fn held_by(
        (cfg, bridges): &(Cfg, Bridges),
        body: &LirBody,
    ) -> bool {
        cfg.is_of(body) && bridges.is_of(body)
    }
}

/// The table kept on the body, filled in for the blocks made since; none where
/// it cannot be.
pub struct Carried;

impl Fact for Carried {
    type Result = Option<Arc<Frequency>>;
    type Inputs = (Arc<BlockFrequencies>, Cfg);
    const NAME: &'static str = "carried-frequency";
    fn run(body: &LirBody) -> Option<Arc<Frequency>> {
        body.frequencies.as_deref().and_then(|table| Frequency::carried(body, &table.0)).map(Arc::new)
    }
    fn inputs(body: &LirBody) -> Self::Inputs {
        (Arc::clone(body.frequencies.as_ref().expect("asked of a body with a table")), Cfg::of(body))
    }
    fn held_by(
        (table, cfg): &Self::Inputs,
        body: &LirBody,
    ) -> bool {
        body.frequencies.as_ref().is_some_and(|now| Arc::ptr_eq(now, table)) && cfg.is_of(body)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frequency {
    block: IndexMap<i64, f64>,
    taken: IndexMap<(i64, i64), f64>,
}

// Blocks looked at to find what enters a bridge, for a test that the search
// is not over every block for each.
#[cfg(test)]
thread_local! {
    pub(crate) static SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// `Frequency::of` asked, for a test that greedy does not ask it.
#[cfg(test)]
thread_local! {
    pub(crate) static OF_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// Frequencies worked out from the odds, not answered from a body's facts.
    pub(crate) static WORKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Blocks `carried` asks about, for a test that it fills them in linearly.
    pub(crate) static FILLED_ASKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn tick() {
    #[cfg(test)]
    SCANNED.with(|one| one.set(one.get() + 1));
}

impl Frequency {
    /// `body`'s blocks' frequencies. They depend on the blocks' shape, the odds
    /// and the trip counts and on no instruction, so the last answer stands
    /// for every body of that shape: a rewrite that only inserts
    /// instructions (a spill, a reload) asks the same question again and again.
    pub fn of(body: &LirBody) -> Arc<Self> {
        #[cfg(test)]
        OF_CALLS.with(|one| one.set(one.get() + 1));
        if body.frequencies.is_some() {
            if let Some(kept) = body.facts.0.get::<Carried>(body).as_ref() {
                return Arc::clone(kept);
            }
        }
        body.facts.0.get::<Worked>(body)
    }

    /// The table kept on the body, with the blocks made since filled in from
    /// their predecessors' (a block put on an edge runs as often as the
    /// edge is taken). `None` where one cannot be: a block made since that only
    /// a cycle reaches, or the entry.
    fn carried(
        body: &LirBody,
        table: &IndexMap<i64, f64>,
    ) -> Option<Self> {
        let mut taken = IndexMap::default();
        let mut predecessors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        let mut successors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for block in &body.blocks {
            for (to, chance) in body.odds.chances(block.at, &block.succ) {
                taken.insert((block.at, to), chance);
                predecessors.entry(to).or_default().push(block.at);
                successors.entry(block.at).or_default().push(to);
            }
        }
        let mut block: IndexMap<i64, f64> =
            body.blocks.iter().filter_map(|one| table.get(&one.at).map(|runs| (one.at, *runs))).collect();
        // A block made since is filled in once all its predecessors are, from
        // the edges into it (a worklist: passes over the rest were quadratic
        // in a chain laid out last first).
        let mut unknown: IndexMap<i64, usize> = IndexMap::default();
        let mut ready: Vec<i64> = Vec::new();
        for at in body.blocks.iter().map(|one| one.at).filter(|at| !block.contains_key(at)) {
            #[cfg(test)]
            FILLED_ASKED.with(|one| one.set(one.get() + 1));
            let Some(from) = predecessors.get(&at) else {
                unknown.insert(at, usize::MAX);
                continue;
            };
            let count = from.iter().filter(|one| !block.contains_key(*one)).count();
            unknown.insert(at, count);
            if count == 0 {
                ready.push(at);
            }
        }
        while let Some(at) = ready.pop() {
            let runs = predecessors[&at].iter().map(|one| block[one] * taken[&(*one, at)]).sum();
            block.insert(at, runs);
            unknown.swap_remove(&at);
            for to in successors.get(&at).into_iter().flatten() {
                #[cfg(test)]
                FILLED_ASKED.with(|one| one.set(one.get() + 1));
                if let Some(count) = unknown.get_mut(to) {
                    *count -= 1;
                    if *count == 0 {
                        ready.push(*to);
                    }
                }
            }
        }
        let missing: Vec<i64> = unknown.keys().copied().collect();
        if !missing.is_empty() {
            // A loop made since: its shape is worked out alone and scaled
            // to the flow the table sends into it.
            let whole = Self::over(body, &body.blocks);
            let region: std::collections::BTreeSet<i64> = missing.iter().copied().collect();
            let (mut sent, mut guessed) = (0.0, 0.0);
            for (from, to) in taken.keys().filter(|(from, to)| region.contains(to) && !region.contains(from)) {
                sent += block.get(from).copied().unwrap_or(0.0) * taken[&(*from, *to)];
                guessed += whole.block(*from) * taken[&(*from, *to)];
            }
            let scale = if guessed > 0.0 { sent / guessed } else { 1.0 };
            for at in region {
                block.insert(at, whole.block(at) * scale);
            }
        }
        Some(Self { block, taken })
    }

    /// What no honest table breaks, whatever shape the blocks have: an entry
    /// for every block and none for others, no block running more often
    /// than the flow into it (each predecessor sends at most its own runs), and
    /// a block behind a predecessor that goes nowhere else running as often
    /// as it. A table left stale by an edge that moved breaks one.
    pub fn violations(body: &LirBody) -> Vec<String> {
        let Some(kept) = body.frequencies.as_deref() else { return Vec::new() };
        let mut out = Vec::new();
        let present: std::collections::BTreeSet<i64> = body.blocks.iter().map(|one| one.at).collect();
        for at in kept.0.keys().filter(|at| !present.contains(at)) {
            out.push(format!("frequency kept for block {at:#x}, which is gone"));
        }
        let Some(now) = Self::carried(body, &kept.0) else { return out };
        let mut inflow: IndexMap<i64, (f64, usize, Option<i64>)> = IndexMap::default();
        for one in &body.blocks {
            let list: std::collections::BTreeSet<i64> = one.succ.iter().copied().collect();
            for to in &list {
                let entry = inflow.entry(*to).or_insert((0.0, 0, None));
                entry.0 += now.block(one.at);
                entry.1 += 1;
                entry.2 = Some(one.at).filter(|_| list.len() == 1);
            }
        }
        // BlockFrequencyInfoImpl has "limited support" for a cycle entered at
        // more than one block (BlockFrequencyInfoImpl.h:650): its members get
        // mass by power iteration, not by the flow into each. A block in such a
        // cycle is not held to its inflow.
        let loose = Self::irreducible_cycles(body);
        for one in body.blocks.iter().filter(|one| one.at != body.entry && !loose.contains(&one.at)) {
            let Some((sum, count, only)) = inflow.get(&one.at).copied() else { continue };
            let runs = now.block(one.at);
            // The estimate is not conservative where proven trips are nested:
            // they fix a header's runs and the odds fix the blocks
            // around it (isel's own table reads 1.5x at most in gcc's memcpy-2,
            // 1.05x in QCport). A stale entry is off by factors.
            let slack = sum + 1e-9;
            if runs > sum + slack {
                out.push(format!("block {:#x} runs {runs:.3} times, its predecessors {sum:.3}", one.at));
            } else if count == 1 && only.is_some() && (runs - sum).abs() > slack {
                out.push(format!(
                    "block {:#x} runs {runs:.3} times, its only predecessor, which goes nowhere else, {sum:.3}",
                    one.at
                ));
            }
        }
        out
    }

    /// The blocks in a strongly connected component that has a block of an
    /// irreducible cycle (`loops::irreducible`).
    fn irreducible_cycles(body: &LirBody) -> std::collections::BTreeSet<i64> {
        let start = loops::irreducible(&body.blocks, Some(body.entry));
        if start.is_empty() {
            return start;
        }
        let reach = |edges: &IndexMap<i64, Vec<i64>>| {
            let mut seen: std::collections::BTreeSet<i64> = start.clone();
            let mut work: Vec<i64> = start.iter().copied().collect();
            while let Some(at) = work.pop() {
                for next in edges.get(&at).into_iter().flatten() {
                    if seen.insert(*next) {
                        work.push(*next);
                    }
                }
            }
            seen
        };
        let (mut forward, mut backward): (IndexMap<i64, Vec<i64>>, IndexMap<i64, Vec<i64>>) = Default::default();
        for one in &body.blocks {
            for to in one.succ.iter() {
                forward.entry(one.at).or_default().push(*to);
                backward.entry(*to).or_default().push(one.at);
            }
        }
        reach(&forward).intersection(&reach(&backward)).copied().collect()
    }

    /// Every block's frequency, to keep on the body.
    pub fn table(&self) -> std::sync::Arc<crate::model::lir::BlockFrequencies> {
        std::sync::Arc::new(crate::model::lir::BlockFrequencies(self.block.clone()))
    }

    /// `blocks`, a later arrangement of `body`'s, on `body`'s odds: LLVM's
    /// `MachineBlockFrequencyInfo`, which knows no trip count.
    pub fn over(
        body: &LirBody,
        blocks: &[Block],
    ) -> Self {
        Self::over_counting(body, blocks, false)
    }

    /// `body`'s blocks with each proven loop leaving one time in `trips + 1 -
    /// tested` (`trip_adjusted`): what `executed` estimates a body's run by.
    /// The frequencies a translated pass reads are `of`'s, without trips: a
    /// bench pair measured nothing for them there (#2731).
    pub fn counted(body: &LirBody) -> Self {
        Self::over_counting(body, &body.blocks, true)
    }

    fn over_counting(
        body: &LirBody,
        blocks: &[Block],
        counting: bool,
    ) -> Self {
        #[cfg(test)]
        WORKED.with(|one| one.set(one.get() + 1));
        let mut taken = IndexMap::default();
        for block in blocks {
            for (to, chance) in body.odds.chances(block.at, &block.succ) {
                taken.insert((block.at, to), chance);
            }
        }
        // A block that only jumps on says nothing about the loops, and a loop
        // is not another loop for one put on an edge: the estimate is
        // made without it (`_part_frame` read 268 without its bridges and 2786
        // with them, and 5 trips read 6 with one on a back edge). A
        // block with work in it stays: a body is not a bridge.
        let entry = body.entry;
        let by_at: IndexMap<i64, &Block> = blocks.iter().map(|block| (block.at, block)).collect();
        let bridge = |at: i64| -> Option<i64> {
            let one = by_at.get(&at)?;
            (at != entry && idle(one) && one.succ.len() == 1 && one.succ[0] != at).then(|| one.succ[0])
        };
        // Past the bridges, and the share of the edge that reaches there.
        let beyond = |mut at: i64| -> i64 {
            let mut hops = 0;
            while let Some(next) = bridge(at) {
                at = next;
                hops += 1;
                if hops > blocks.len() {
                    break;
                }
            }
            at
        };
        let mut successors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        let mut shared: IndexMap<(i64, i64), f64> = IndexMap::default();
        for block in blocks.iter().filter(|block| bridge(block.at).is_none() || block.at == entry) {
            let list = successors.entry(block.at).or_default();
            for to in crate::model::lir::distinct(&block.succ) {
                let past = beyond(to);
                *shared.entry((block.at, past)).or_default() += taken[&(block.at, to)];
                if !list.contains(&past) {
                    list.push(past);
                }
            }
        }
        let mut predecessors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for (from, list) in &successors {
            for to in list {
                predecessors.entry(*to).or_default().push(*from);
            }
        }
        // The loops are found only for the trips that scale them.
        let edges = if counting {
            let kept: Vec<Block> = blocks
                .iter()
                .filter(|block| successors.contains_key(&block.at))
                .map(|block| LirBlock {
                    succ: successors[&block.at].clone().into(),
                    ..LirBlock::new(block.at, Vec::new())
                })
                .collect();
            let natural = loops::loops(&kept, Some(body.entry));
            let trips: IndexMap<i64, i64> = body.loop_trip_counts.iter().copied().collect();
            trip_adjusted(&natural, &trips, &successors, &shared)
        } else {
            shared.clone()
        };
        let mut block = propagated(body.entry, &successors, &predecessors, &edges);
        // A bridge runs as often as the edges into it are taken; a chain of
        // them is settled from its first.
        let mut pending: Vec<i64> = blocks.iter().map(|one| one.at).filter(|at| bridge(*at).is_some()).collect();
        // The blocks that enter each, in the order of `blocks`: a scan of all
        // of them for each bridge was quadratic in a function of N cases. A few
        // bridges are scanned for, as the map costs more than they do.
        let mut entering: IndexMap<i64, Vec<&crate::model::lir::Block>> = IndexMap::default();
        let mapped = pending.len() > 16;
        for from in blocks.iter().filter(|_| mapped) {
            for to in crate::model::lir::distinct(&from.succ) {
                entering.entry(to).or_default().push(from);
            }
        }
        while !pending.is_empty() {
            let before = pending.len();
            pending.retain(|at| {
                let mut runs = 0.0;
                let found: Vec<&crate::model::lir::Block> = if mapped {
                    entering.get(at).cloned().unwrap_or_default()
                } else {
                    blocks.iter().filter(|from| from.succ.contains(at)).collect()
                };
                for from in found.into_iter().inspect(|_| tick()) {
                    let Some(found) = block.get(&from.at).copied() else { return true };
                    runs += if bridge(from.at).is_some() {
                        found
                    } else {
                        let past = beyond(*at);
                        found * edges.get(&(from.at, past)).copied().unwrap_or(0.0) * taken[&(from.at, *at)]
                            / shared[&(from.at, past)].max(f64::MIN_POSITIVE)
                    };
                }
                block.insert(*at, runs);
                false
            });
            if pending.len() == before {
                break;
            }
        }
        Self { block: block.into_iter().collect(), taken }
    }

    /// Per call; zero for a block the entry does not reach.
    pub fn block(
        &self,
        at: i64,
    ) -> f64 {
        self.block.get(&at).copied().unwrap_or(0.0)
    }

    /// How often `from` goes to `to`, per call.
    pub fn edge(
        &self,
        from: i64,
        to: i64,
    ) -> f64 {
        self.block(from) * self.taken.get(&(from, to)).copied().unwrap_or(0.0)
    }
}

/// A loop whose trips induction proved leaves by its exit test one time in
/// `trips + 1 - tested` (`tested` is 1 for a test after the trip, at a latch,
/// 0 for one before it, at the header), where it has one exiting block.
fn trip_adjusted(
    natural: &[loops::Loop],
    trips: &IndexMap<i64, i64>,
    successors: &IndexMap<i64, Vec<i64>>,
    shared: &IndexMap<(i64, i64), f64>,
) -> IndexMap<(i64, i64), f64> {
    let mut edges = shared.clone();
    for one in natural {
        let Some(&count) = trips.get(&one.header) else { continue };
        let exiting: Vec<i64> = successors
            .iter()
            .filter(|(from, to)| one.body.contains(from) && to.iter().any(|at| !one.body.contains(at)))
            .map(|(from, _)| *from)
            .collect();
        let [from] = exiting[..] else { continue };
        let tested = if one.latches.contains(&from) {
            1
        } else if from == one.header {
            0
        } else {
            continue;
        };
        let out = 1.0 / (count + 1 - tested).max(1) as f64;
        let list = &successors[&from];
        let (leaving, staying): (Vec<i64>, Vec<i64>) = list.iter().partition(|at| !one.body.contains(at));
        let left: f64 = leaving.iter().map(|to| shared.get(&(from, *to)).copied().unwrap_or(0.0)).sum();
        let stayed: f64 = staying.iter().map(|to| shared.get(&(from, *to)).copied().unwrap_or(0.0)).sum();
        for to in &leaving {
            let p = shared.get(&(from, *to)).copied().unwrap_or(0.0);
            let share = if left > 0.0 { p / left } else { 1.0 / leaving.len() as f64 };
            edges.insert((from, *to), out * share);
        }
        for to in &staying {
            let p = shared.get(&(from, *to)).copied().unwrap_or(0.0);
            let share = if stayed > 0.0 { p / stayed } else { 1.0 / staying.len() as f64 };
            edges.insert((from, *to), (1.0 - out) * share);
        }
    }
    edges
}

/// The blocks between `entry` and the rest, as `BlockFrequencyInfoImpl` runs
/// over a `MachineFunction` (`MachineBlockFrequencyInfo`): each block's
/// frequency per call, `edges` its edges' probabilities.
struct Flow {
    blocks: Vec<BlockId>,
    succs: Vec<BlockList>,
    preds: Vec<BlockList>,
    /// Each block's successors' probabilities, normalized once, as a
    /// `MachineBasicBlock` keeps them.
    probabilities: Vec<Vec<BranchProbability>>,
}

impl DomGraph for Flow {
    fn entry(&self) -> Option<BlockId> {
        (!self.blocks.is_empty()).then_some(BlockId(0))
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
        self.succs[block.0 as usize].clone()
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.preds[block.0 as usize].clone()
    }
}

impl FlowGraph for Flow {}

impl EdgeProbabilities for Flow {
    fn get_edge_probability(
        &self,
        src: BlockId,
        index: usize,
    ) -> BranchProbability {
        self.probabilities[src.0 as usize][index]
    }
}

/// Each block's runs per call over `successors` and `edges`.
fn propagated(
    entry: i64,
    successors: &IndexMap<i64, Vec<i64>>,
    predecessors: &IndexMap<i64, Vec<i64>>,
    edges: &IndexMap<(i64, i64), f64>,
) -> IndexMap<i64, f64> {
    // A LIR entry may be a loop's header, as a function's entry block may
    // not: a block before it enters it once, as a call does.
    let looped = predecessors.get(&entry).is_some_and(|from| !from.is_empty());
    let mut at: Vec<i64> = if looped { vec![BEFORE_ENTRY, entry] } else { vec![entry] };
    at.extend(successors.keys().copied().filter(|one| *one != entry));
    let index: IndexMap<i64, u32> = at.iter().enumerate().map(|(i, one)| (*one, i as u32)).collect();
    let list = |of: &IndexMap<i64, Vec<i64>>, one: &i64| -> BlockList {
        of.get(one).into_iter().flatten().filter_map(|to| index.get(to)).map(|&i| BlockId(i)).collect()
    };
    let mut succs: Vec<BlockList> = at.iter().map(|one| list(successors, one)).collect();
    let mut preds: Vec<BlockList> = at.iter().map(|one| list(predecessors, one)).collect();
    if looped {
        succs[0] = [BlockId(1)].into_iter().collect();
        preds[1].push(BlockId(0));
    }
    let probabilities = at
        .iter()
        .zip(&succs)
        .map(|(from, list)| {
            let mut probs: Vec<BranchProbability> = list
                .iter()
                .map(|to| {
                    let p = edges.get(&(*from, at[to.0 as usize])).copied().unwrap_or(0.0);
                    BranchProbability::get_branch_probability_f64(p.clamp(0.0, 1.0))
                })
                .collect();
            BranchProbability::normalize_probabilities(&mut probs);
            probs
        })
        .collect();
    let flow = Flow { blocks: (0..at.len() as u32).map(BlockId).collect(), succs, preds, probabilities };
    let cycles = CycleInfo::compute(&flow);
    let bfi = BlockFrequencyInfo::calculate(&flow, &flow, &cycles);
    let entry_freq = bfi.get_entry_freq().get_frequency().max(1) as f64;
    at.iter()
        .enumerate()
        .filter(|(_, one)| **one != BEFORE_ENTRY)
        .map(|(i, one)| (*one, bfi.get_block_freq(BlockId(i as u32)).get_frequency() as f64 / entry_freq))
        .collect()
}

/// The block `propagated` puts before an entry that is a loop's header.
const BEFORE_ENTRY: i64 = i64::MIN;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Frequency;
    use crate::model::ir::{Operation, Semantics};
    use crate::model::lir::{BlockOdds, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, 1)), Some(what), Vec::new(), Vec::new()))
    }

    fn block(
        at: i64,
        name: &str,
        op: Operation,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ: succ.into(), ..LirBlock::new(at, vec![insn(at, op, name)]) }
    }

    /// entry 1 -> loop 2 (back to 2, out to 3) -> 3.
    /// A spill or a reload inserts instructions and leaves the blocks as they
    /// were: asked again, the frequencies are not worked out again (3
    /// builds per rebuild of the allocator's facts, 8% of
    /// compiling `d_faces`, #559). A block's successors or odds changing is a
    /// new question.
    #[test]
    fn test_the_same_blocks_are_not_worked_out_twice() {
        let body = counted(Some(5), 124.0 / 128.0);
        let runs = |body: &LirBody| body.facts.0.runs::<super::Worked>();
        let before = runs(&body);
        let first = Frequency::of(&body);
        let more = body.with_blocks(
            body.blocks
                .iter()
                .map(|one| one.with_insns(vec![insn(one.at, Operation::Move, "mov"), one.insns[0].clone()]))
                .collect(),
        );
        let again = Frequency::of(&more);
        assert_eq!(runs(&body) - before, 1, "a second question of the same blocks was worked out");
        assert_eq!(first.block(2), again.block(2));
        let mut moved = body.clone();
        moved.odds.taken.insert((2, 2), 1);
        Frequency::of(&moved);
        assert_eq!(runs(&moved) - before, 2, "other odds were answered from the old frequencies");
    }

    /// The natural loops are read only to scale proven trips, but were found
    /// for every frequency question without them: 7.1 M of 1953 M
    /// instructions on d_faces.
    #[test]
    fn test_frequencies_without_trips_find_no_loops() {
        use llrm_mir::counts::{Event, get};
        let body = counted(Some(5), 124.0 / 128.0);
        let before = get(Event::LirLoops);
        Frequency::of(&body);
        assert_eq!(get(Event::LirLoops) - before, 0, "Frequency::of found the loops");
        Frequency::counted(&body);
        assert!(get(Event::LirLoops) > before, "premise: the trips are scaled over the loops");
    }

    /// A body that carries its frequencies is asked for them by every fact of
    /// the allocator, with the same table and blocks after each spill: the
    /// carried answer is worked out once for them (1.5% of compiling d_faces,
    /// 1.2 G, was working it out again).
    #[test]
    fn test_a_carried_table_is_not_carried_again_for_the_same_blocks() {
        let plain = counted(Some(5), 124.0 / 128.0);
        let mut body = plain.clone();
        body.frequencies = Some(Frequency::of(&plain).table());
        let runs = |body: &LirBody| body.facts.0.runs::<super::Carried>();
        let before = runs(&body);
        let first = Frequency::of(&body);
        let more = body.with_blocks(
            body.blocks
                .iter()
                .map(|one| one.with_insns(vec![insn(one.at, Operation::Move, "mov"), one.insns[0].clone()]))
                .collect(),
        );
        let again = Frequency::of(&more);
        assert_eq!(runs(&body) - before, 1, "the same table over the same blocks was carried again");
        assert_eq!(first.block(2), again.block(2));
        let mut moved = body.clone();
        moved.odds.taken.insert((2, 2), 1);
        Frequency::of(&moved);
        assert_eq!(runs(&moved) - before, 2, "other odds were answered from the old table");
    }

    /// A block phi elimination puts on the exit edge of a loop with a proven
    /// count ran as often as the odds took the edge (one in 32 of the loop's
    /// 5 trips: 0.16 a call) where the loop leaves once a call: its join read
    /// 1 against predecessors that sent 0.3 (`ivarg`, behind the header copy).
    #[test]
    fn test_a_block_on_the_exit_of_a_counted_loop_runs_as_often_as_the_loop_leaves() {
        let plain = counted(Some(5), 124.0 / 128.0);
        let mut body = plain.clone();
        body.frequencies = Some(Frequency::of(&plain).table());
        let loop_ = body.blocks[1].clone();
        body.odds.rerouted(2, &loop_.succ, 3, &[(4, 1.0)]);
        let blocks = vec![
            body.blocks[0].clone(),
            LirBlock { succ: vec![2, 4].into(), ..body.blocks[1].clone().into_inner() }.into(),
            body.blocks[2].clone(),
            block(4, "jmp", Operation::Jump, vec![3]).into(),
        ];
        let body = body.with_blocks(blocks);
        let found = Frequency::of(&body);
        assert!((found.block(4) - found.block(3)).abs() < 1e-6, "{} against {}", found.block(4), found.block(3));
    }

    /// Whether a block is a bridge, and so left out of the loops' flow, is read
    /// from its instructions. A bridge on a loop's back edge that took a
    /// reload kept the frequencies worked out while it was one: the kept
    /// `flow` had its edge and working it out again does not
    /// (`LLRM_CHECK_FACTS` failed every regalloc fuzz test on main).
    #[test]
    fn test_a_bridge_that_takes_work_is_asked_again() {
        let mut body = counted(Some(5), 124.0 / 128.0);
        body.frequencies = None;
        let mut split = body.clone();
        split.odds.rerouted(2, &[2, 3], 2, &[(9, 1.0)]);
        split.blocks[1].succ = vec![9, 3].into();
        split.blocks.push(block(9, "jmp", Operation::Jump, vec![2]).into());
        let bridge = Frequency::of(&split);
        let worked = split.with_blocks(
            split
                .blocks
                .iter()
                .map(|one| {
                    if one.at == 9 {
                        one.with_insns(vec![insn(9, Operation::Move, "mov"), one.insns[0].clone()])
                    } else {
                        one.clone()
                    }
                })
                .collect(),
        );
        let kept = Frequency::of(&worked);
        let fresh = Frequency::over(&worked, &worked.blocks);
        assert_eq!(*kept, fresh, "a block with work in it was still a bridge (bridge flow {:?})", bridge.table());
    }

    /// With no proven trips, the frequencies are `MachineBlockFrequencyInfo`'s
    /// over the odds: a loop of two exits, a nest, and one that never leaves
    /// (BFI's 4096). They were `branchprob`'s GCC-style propagation.
    #[test]
    fn test_frequencies_are_machine_block_frequency_infos() {
        let shapes: [&[(i64, &[i64], &[f64])]; 3] = [
            &[(1, &[2], &[]), (2, &[3, 5], &[0.9, 0.1]), (3, &[2, 4], &[0.8, 0.2]), (4, &[5], &[]), (5, &[], &[])],
            &[(1, &[2], &[]), (2, &[3, 5], &[0.9, 0.1]), (3, &[3, 4], &[0.75, 0.25]), (4, &[2], &[]), (5, &[], &[])],
            &[(1, &[2], &[]), (2, &[2, 3], &[1.0, 0.0]), (3, &[], &[])],
        ];
        for shape in shapes {
            let blocks: Vec<LirBlock> = shape
                .iter()
                .map(|(at, succ, _)| {
                    let op = match succ.len() {
                        0 => Operation::Return,
                        1 => Operation::Move,
                        _ => Operation::Branch,
                    };
                    block(*at, "op", op, succ.to_vec())
                })
                .collect();
            let mut body =
                LirBody::new("f", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
            for (at, succ, odds) in shape {
                for (to, p) in succ.iter().zip(*odds) {
                    body.odds.taken.insert((*at, *to), (p * BlockOdds::CERTAIN).round() as u32);
                }
            }
            let ours = Frequency::of(&body);
            let mbfi = crate::liveness::blockfrequency::MachineBlockFrequencyInfo::of(&body);
            for (index, (at, _, _)) in shape.iter().enumerate() {
                let want = mbfi.get_block_freq_relative_to_entry_block(index);
                assert!(
                    (ours.block(*at) - want).abs() <= 1e-6 * want.max(1.0),
                    "{shape:?} block {at}: {} against {want}",
                    ours.block(*at)
                );
            }
        }
    }

    /// A table with blocks made since: a chain of `n` new blocks behind the
    /// entry, laid out last first.
    fn carried_chain(n: i64) -> LirBody {
        let mut blocks = vec![block(1, "jmp", Operation::Jump, vec![n + 1])];
        blocks.extend((2..=n + 1).map(|at| {
            if at == 2 {
                block(at, "ret", Operation::Return, vec![])
            } else {
                block(at, "jmp", Operation::Jump, vec![at - 1])
            }
        }));
        let mut body =
            LirBody::new("f", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
        let table = [(1, 1.0)].into_iter().collect();
        body.frequencies = Some(Arc::new(crate::model::lir::BlockFrequencies(table)));
        body
    }

    /// The blocks made since were filled in from a second working-out of the
    /// whole function, for edge flows that equal the odds once `over` stopped
    /// reading trips (#2816): 6.5 M of `d_faces`'s 23 M in frequencies.
    #[test]
    fn test_a_carried_table_is_not_worked_out_again() {
        let body = carried_chain(3);
        let before = super::WORKED.with(|one| one.get());
        let carried = Frequency::of(&body);
        assert_eq!(super::WORKED.with(|one| one.get()), before, "the function was worked out again");
        assert!((2..=4).all(|at| (carried.block(at) - 1.0).abs() < 1e-12), "{carried:?}");
    }

    /// Filling in a chain of new blocks laid out last first asked every block
    /// once per pass, one block settled per pass: quadratic. Each block is
    /// asked about a bounded number of times.
    #[test]
    fn test_a_carried_table_fills_a_chain_linearly() {
        let asked = |n| {
            let body = carried_chain(n);
            let before = super::FILLED_ASKED.with(|one| one.get());
            let carried = Frequency::of(&body);
            assert!((carried.block(2) - 1.0).abs() < 1e-12);
            super::FILLED_ASKED.with(|one| one.get()) - before
        };
        let (one, four) = (asked(64), asked(256));
        assert!(four <= 5 * one, "64 blocks asked {one} times, 256 asked {four}");
    }

    fn counted(
        trips: Option<i64>,
        stay: f64,
    ) -> LirBody {
        let blocks = vec![
            block(1, "jmp", Operation::Jump, vec![2]),
            block(2, "jne", Operation::Branch, vec![2, 3]),
            block(3, "ret", Operation::Return, vec![]),
        ];
        let mut body =
            LirBody::new("f", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
        for (to, probability) in [(2, stay), (3, 1.0 - stay)] {
            body.odds.taken.insert((2, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }
        body.loop_trip_counts = trips.map(|count| (2, count)).into_iter().collect();
        body
    }

    /// Spill weights and the instrument read 10 per loop level, whatever the
    /// branch said; the loop's heuristic odds make it 32, and in `counted`
    /// (the estimate) a proven count its own.
    #[test]
    fn test_a_loop_runs_as_its_odds_and_proven_trips_say_not_ten() {
        let guessed = Frequency::of(&counted(None, 124.0 / 128.0));
        assert!(
            (guessed.block(2) - 32.0).abs() < 1e-6 && (guessed.block(3) - 1.0).abs() < 1e-6,
            "{} {}",
            guessed.block(2),
            guessed.block(3)
        );
        let proven = Frequency::counted(&counted(Some(5), 124.0 / 128.0));
        assert!(
            (proven.block(2) - 5.0).abs() < 1e-6 && (proven.block(3) - 1.0).abs() < 1e-6,
            "{} {}",
            proven.block(2),
            proven.block(3)
        );
    }

    /// A block almost never reached weighs almost nothing: a branch to it at 1
    /// in a million is the cold path 10^depth gave the weight of the code
    /// before it.
    #[test]
    fn test_a_block_that_is_all_but_never_taken_weighs_nothing() {
        let blocks = vec![
            block(1, "jne", Operation::Branch, vec![2, 3]),
            block(2, "ret", Operation::Return, vec![]),
            block(3, "ret", Operation::Return, vec![]),
        ];
        let mut body =
            LirBody::new("f", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default());
        for (to, probability) in [(2, 1e-6), (3, 1.0 - 1e-6)] {
            body.odds.taken.insert((1, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }
        let busy = Frequency::of(&body);
        assert!(busy.block(2) < 1e-5 && busy.block(3) > 0.99, "{} {}", busy.block(2), busy.block(3));
    }

    /// A block put on an edge (phielim's and the spiller's edge code) must
    /// leave the frequencies as they were: `d_faces`-sized `_part_frame`
    /// was estimated 268 on the blocks isel made, 2786 with the bridges, and
    /// 268 again once `jumps` removed them, and the route choice compared
    /// two bodies at different points of that.
    #[test]
    fn test_a_block_on_an_edge_leaves_the_frequencies_as_they_were() {
        for trips in [None, Some(5)] {
            for (from, old) in [(2, 3), (2, 2)] {
                let body = counted(trips, 124.0 / 128.0);
                let before = Frequency::of(&body);
                let mut split = body.clone();
                let bridge = 9;
                let succ: Vec<i64> = split.blocks.iter().find(|one| one.at == from).expect("a block").succ.to_vec();
                split.odds.rerouted(from, &succ, old, &[(bridge, 1.0)]);
                for one in &mut split.blocks {
                    if one.at == from {
                        one.succ = one.succ.iter().map(|to| if *to == old { bridge } else { *to }).collect();
                    }
                }
                split.blocks.push(block(bridge, "jmp", Operation::Jump, vec![old]).into());
                let after = Frequency::of(&split);
                for at in [1, 2, 3] {
                    assert!(
                        (before.block(at) - after.block(at)).abs() < 1e-6 * before.block(at).max(1.0),
                        "block {at} trips {trips:?} split {from}->{old}: {} against {}",
                        before.block(at),
                        after.block(at)
                    );
                }
            }
        }
    }

    /// Copies put on a loop's back edge make a block with work in it, which the
    /// shape alone reads as the loop's body: a loop tested after its trip
    /// then reads as tested before it, and 5 trips as 6 (`_part_frame`, 8x).
    /// The frequencies worked out before stay, and the copy block runs as
    /// often as its edge.
    #[test]
    fn test_copies_on_the_back_edge_do_not_change_what_the_loop_runs() {
        let mut body = counted(Some(5), 124.0 / 128.0);
        body.frequencies = Some(Frequency::of(&body).table());
        let before = Frequency::of(&body);
        let mut split = body.clone();
        split.odds.rerouted(2, &[2, 3], 2, &[(9, 1.0)]);
        split.blocks[1].succ = vec![9, 3].into();
        split
            .blocks
            .push(
                LirBlock {
                    succ: vec![2].into(),
                    ..LirBlock::new(9, vec![insn(9, Operation::Move, "mov"), insn(9, Operation::Jump, "jmp")])
                }
                .into(),
            );
        let after = Frequency::of(&split);
        for at in [1, 2, 3] {
            assert_eq!(before.block(at), after.block(at), "block {at}");
        }
        // The copies run as often as the back edge was taken, to the
        // precision of `BlockFrequencyInfo`'s integer frequencies.
        let back = before.edge(2, 2);
        assert!((after.block(9) - back).abs() < 1e-6 * back, "the copies run {} against {back}", after.block(9));
    }

    /// Each bridge looked through every block for what enters it: N cases, N
    /// bridges, N^2 (`cases` 1024: `Frequency::over` 3 calls of ~60 Minstr).
    #[test]
    fn test_what_enters_a_bridge_is_found_without_a_scan_of_every_block() {
        let bridges = 40;
        let mut blocks = vec![block(1, "jne", Operation::Branch, vec![100, 200])];
        blocks
            .extend((0..bridges).map(|at| {
                block(200 + at, "jmp", Operation::Jump, vec![if at + 1 < bridges { 201 + at } else { 100 }])
            }));
        blocks.push(block(100, "ret", Operation::Return, vec![]));
        let body = LirBody::new("f", 1, blocks.into_iter().collect(), IndexMap::default(), IndexMap::default());
        super::SCANNED.with(|one| one.set(0));
        let _ = Frequency::of(&body);
        let scanned = super::SCANNED.with(|one| one.get());
        assert!(scanned <= 3 * bridges as usize, "{scanned} blocks looked at for {bridges} bridges");
    }

    /// Each edge of a dispatch read the probability of every other edge of it
    /// (`chance` asks all the successors): N cases, N^2 reads, 3 calls of
    /// `Frequency::of` in `lir jumps` (`cases` 1024).
    #[test]
    fn test_a_dispatch_of_n_cases_reads_n_probabilities_not_n_squared() {
        let cases = 60;
        let targets: Vec<i64> = (0..cases).map(|case| 10 + case).collect();
        let mut blocks = vec![block(1, "jmp", Operation::Jump, targets.clone())];
        blocks.extend(targets.iter().map(|&at| block(at, "ret", Operation::Return, vec![])));
        let body = LirBody::new("f", 1, blocks.into_iter().collect(), IndexMap::default(), IndexMap::default());
        crate::model::lir::ASKED.with(|one| one.set(0));
        let table = Frequency::of(&body);
        let asked = crate::model::lir::ASKED.with(|one| one.get());
        assert!((table.block(10) - 1.0 / cases as f64).abs() < 1e-9);
        assert!(asked <= 8 * cases as usize, "{asked} probabilities read for {cases} cases");
    }

    /// A table left as it was after an edge moved (block 3 reached only through
    /// a rare edge now) ran the block as often as before: nothing said so.
    /// The invariant every honest table keeps catches it under LLRM_VERIFY.
    /// A cycle entered at two blocks (QB's ON ERROR pad switching back into a
    /// loop, a state machine's dispatch) gets the frequencies
    /// BlockFrequencyInfoImpl gives an irreducible SCC
    /// (BlockFrequencyInfoImpl.h:650, "limited support", power iteration),
    /// which are not the flow in: `violations` said "block runs 1.551
    /// times, its predecessors 0.582" of a table nothing had
    /// broken (LLRM_VERIFY failed on a QB program of eight lines). The blocks
    /// are that program's.
    #[test]
    fn test_an_irreducible_cycle_is_not_held_to_the_flow_into_its_blocks() {
        type Shape<'a> = &'a [(i64, &'a str, Operation, &'a [(i64, f64)])];
        let made = |shape: Shape| {
            let mut body = LirBody::new(
                "f",
                0x31,
                shape
                    .iter()
                    .map(|(at, text, what, to)| -> crate::model::lir::Block {
                        // Every block but 0x2b does something, so none else is
                        // a bridge (0x2b being one is
                        // what made isel's table read as it did).
                        LirBlock {
                            succ: to.iter().map(|(to, _)| *to).collect::<Vec<i64>>().into(),
                            ..LirBlock::new(
                                *at,
                                if *at == 0x2b {
                                    vec![insn(*at, *what, text)]
                                } else {
                                    vec![insn(*at, Operation::Nothing, "mov"), insn(*at, *what, text)]
                                },
                            )
                        }
                        .into()
                    })
                    .collect::<Vec<crate::model::lir::Block>>(),
                IndexMap::default(),
                IndexMap::default(),
            );
            for (at, _, _, to) in shape {
                for (to, probability) in *to {
                    body.odds.taken.insert((*at, *to), (probability * BlockOdds::CERTAIN).round() as u32);
                }
            }
            body
        };
        let before: Shape = &[
            (0x31, "jne", Operation::Branch, &[(0x19, 0.5), (0x0, 0.5)]),
            (0x0, "jmp", Operation::Jump, &[(0x8, 1.0)]),
            (0x8, "jmp", Operation::Jump, &[(0x9, 1.0)]),
            (0x9, "jne", Operation::Branch, &[(0xc, 0.375), (0xe, 0.625)]),
            (0xc, "jmp", Operation::Jump, &[(0x29, 1.0)]),
            (0xe, "jne", Operation::Branch, &[(0x14, 0.96875), (0x16, 0.03125)]),
            (0x14, "jmp", Operation::Jump, &[(0x28, 1.0)]),
            (0x16, "jmp", Operation::Jump, &[(0x17, 1.0)]),
            (0x17, "ret", Operation::Return, &[]),
            (0x19, "jne", Operation::Branch, &[(0x23, 0.96875), (0x25, 0.03125)]),
            (0x23, "jne", Operation::Branch, &[(0xc, 0.96875), (0x14, 0.03125)]),
            (0x25, "jne", Operation::Branch, &[(0xe, 0.96875), (0x17, 0.03125)]),
            (0x28, "jmp", Operation::Jump, &[(0x17, 1.0)]),
            (0x29, "jmp", Operation::Jump, &[(0x2b, 1.0)]),
            (0x2b, "jmp", Operation::Jump, &[(0x9, 1.0)]),
        ];
        let table = Frequency::over(&made(before), &made(before).blocks).table();
        // The edges 19 -> 25, 23 -> 14 and 25 -> 17 each got a block since, a
        // jump on that also reaches a block that returns (odds 0).
        let after: Shape = &[
            (0x31, "jne", Operation::Branch, &[(0x19, 0.5), (0x0, 0.5)]),
            (0x0, "jmp", Operation::Jump, &[(0x8, 1.0)]),
            (0x8, "jmp", Operation::Jump, &[(0x9, 1.0)]),
            (0x9, "jne", Operation::Branch, &[(0xc, 0.375), (0xe, 0.625)]),
            (0xc, "jmp", Operation::Jump, &[(0x29, 1.0)]),
            (0xe, "jne", Operation::Branch, &[(0x14, 0.96875), (0x16, 0.03125)]),
            (0x14, "jmp", Operation::Jump, &[(0x28, 1.0)]),
            (0x16, "jmp", Operation::Jump, &[(0x17, 1.0)]),
            (0x17, "ret", Operation::Return, &[]),
            (0x19, "jne", Operation::Branch, &[(0x23, 0.96875), (0x2e, 0.03125)]),
            (0x2e, "jne", Operation::Branch, &[(0x25, 1.0), (0x27, 0.0)]),
            (0x23, "jne", Operation::Branch, &[(0xc, 0.96875), (0x2f, 0.03125)]),
            (0x2f, "jne", Operation::Branch, &[(0x14, 1.0), (0x2c, 0.0)]),
            (0x25, "jne", Operation::Branch, &[(0xe, 0.96875), (0x30, 0.03125)]),
            (0x30, "jne", Operation::Branch, &[(0x17, 1.0), (0x2d, 0.0)]),
            (0x27, "ret", Operation::Return, &[]),
            (0x28, "jmp", Operation::Jump, &[(0x17, 1.0)]),
            (0x29, "jmp", Operation::Jump, &[(0x2b, 1.0)]),
            (0x2b, "jmp", Operation::Jump, &[(0x9, 1.0)]),
            (0x2c, "ret", Operation::Return, &[]),
            (0x2d, "ret", Operation::Return, &[]),
        ];
        let mut body = made(after);
        body.frequencies = Some(table);
        assert!(Frequency::violations(&body).is_empty(), "{:?}", Frequency::violations(&body));
        // The exemption is the cycle's: a block outside it that runs ten times
        // as often as the only block that reaches it is still said.
        let mut stale = body.frequencies.as_ref().expect("a table").0.clone();
        *stale.get_mut(&0x28).expect("a block outside the cycle") *= 10.0;
        body.frequencies = Some(Arc::new(crate::model::lir::BlockFrequencies(stale)));
        let said = Frequency::violations(&body);
        assert!(said.iter().any(|one| one.contains("block 0x28 runs")), "{said:?}");
    }

    #[test]
    fn test_a_table_left_stale_by_a_moved_edge_is_caught() {
        let mut body = LirBody::new(
            "f",
            1,
            vec![
                block(1, "jne", Operation::Branch, vec![2, 3]).into(),
                block(2, "jmp", Operation::Jump, vec![3]).into(),
                block(3, "ret", Operation::Return, vec![]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        for (to, probability) in [(2, 31.0 / 32.0), (3, 1.0 / 32.0)] {
            body.odds.taken.insert((1, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }
        body.frequencies = Some(Frequency::of(&body).table());
        assert!(Frequency::violations(&body).is_empty(), "{:?}", Frequency::violations(&body));
        // 1 -> 3 is gone and 3 is reached by 2 -> 9 -> 3, 9 taking 1 in 32
        // from 2: block 3 now runs a thirty-second as often.
        let mut moved = body.clone();
        moved.blocks[0].succ = vec![2].into();
        moved.blocks[1] = block(2, "jne", Operation::Branch, vec![4, 9]).into();
        moved.blocks.push(block(4, "ret", Operation::Return, vec![]).into());
        moved
            .blocks
            .push(LirBlock { succ: vec![3].into(), ..LirBlock::new(9, vec![insn(9, Operation::Jump, "jmp")]) }.into());
        moved.odds.taken.insert((2, 9), (BlockOdds::CERTAIN / 32.0).round() as u32);
        moved.odds.taken.insert((2, 4), (BlockOdds::CERTAIN * 31.0 / 32.0).round() as u32);
        let said = Frequency::violations(&moved);
        assert!(said.iter().any(|one| one.contains("block 0x3 runs")), "{said:?}");
        let mut gone = body.clone();
        gone.blocks.retain(|one| one.at != 3);
        gone.blocks[0].succ = vec![2].into();
        gone.blocks[1].succ = vec![].into();
        gone.frequencies = body.frequencies.clone();
        assert!(Frequency::violations(&gone).iter().any(|one| one.contains("gone")));
        assert!(Frequency::violations(&gone.with_blocks(gone.blocks.clone())).is_empty());
    }

    /// A loop made after the table, with no entries and only a cycle reaching
    /// it, was worked out with the whole body's shape again. Its region
    /// alone is: scaled to what the table sends in.
    #[test]
    fn test_a_loop_made_since_is_worked_out_alone_and_scaled_to_its_entering_flow() {
        let mut body = counted(Some(5), 124.0 / 128.0);
        body.frequencies = Some(Frequency::of(&body).table());
        let mut more = body.clone();
        // 2 -> 3 now goes through a new self loop 9.
        more.blocks[1].succ = vec![2, 8].into();
        more.blocks
            .push(LirBlock { succ: vec![9].into(), ..LirBlock::new(8, vec![insn(8, Operation::Jump, "jmp")]) }.into());
        more.blocks.push(
            LirBlock { succ: vec![9, 3].into(), ..LirBlock::new(9, vec![insn(9, Operation::Branch, "jne")]) }.into(),
        );
        more.blocks[2].succ = vec![].into();
        for (from, to, probability) in
            [(2, 8, 1.0 / 32.0), (2, 2, 31.0 / 32.0), (9, 9, 31.0 / 32.0), (9, 3, 1.0 / 32.0)]
        {
            more.odds.taken.insert((from, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }
        let busy = Frequency::of(&more);
        // The loop leaves by its 1-in-32 edge.
        let entering = busy.block(2) / 32.0;
        assert!((busy.block(8) - entering).abs() < 1e-6, "{} {}", busy.block(8), entering);
        assert!(
            (busy.block(9) - entering * 32.0).abs() < 1e-3 * entering * 32.0,
            "the new loop runs {} for {entering} sent in",
            busy.block(9)
        );
    }
}
