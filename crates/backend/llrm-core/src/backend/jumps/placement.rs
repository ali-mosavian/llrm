//! `MachineBlockPlacement`: chains, loops and tail duplication.
//!
//! Every block starts as a chain of its own; chains are joined along the best
//! edge until one is left, innermost loops first. A short block is copied into
//! the blocks that jump to it where that saves a taken branch (`taildup`).
//! Translated from `MachineBlockPlacement.cpp` at llvm-project
//! d1106deb71cc81016406a1917d0508d2df296266; the function-by-function check
//! is `docs/architecture/jumps-placement-check.md`.

mod postdom;
mod taildup;

use crate::analysis::frequency::Frequency;
use crate::analysis::loops::Loop;
use crate::model::lir::{Block, BlockOdds};
use crate::support::hash::{HashSet, IndexMap};

/// `block-placement-predecessor-limit`.
const PREDECESSOR_LIMIT: usize = 1000;

/// `TailDupPlacementThreshold`: the instructions a block may hold and be
/// copied (1 for size).
const TAIL_DUPLICATION: usize = 2;

/// `TailDupPlacementAggressiveThreshold`, at -O3.
const TAIL_DUPLICATION_AGGRESSIVE: usize = 4;

/// `TailDupPlacementPenalty` (2 per cent).
const TAIL_DUPLICATION_PENALTY: f64 = 0.02;

/// `TriangleChainCount`.
const TRIANGLE_CHAIN_COUNT: usize = 2;

/// Frequencies are compared as `BlockFrequency` does, in fixed point: two that
/// are equal in the odds (a loop of 31 in 32 leaves as often as it is entered)
/// are equal here, not a rounding of `f64` apart.
const FREQUENCY_TOLERANCE: f64 = 1e-9;

fn at_least(
    a: f64,
    b: f64,
) -> bool {
    a >= b * (1.0 - FREQUENCY_TOLERANCE)
}

fn above(
    a: f64,
    b: f64,
) -> bool {
    a > b * (1.0 + FREQUENCY_TOLERANCE)
}

/// `StaticLikelyProb` (80): without a profile, the probability an edge needs
/// to be placed over a block that other blocks also reach.
const HOT: f64 = llrm_analysis::branchprob::LIKELY;

struct Chain {
    blocks: Vec<usize>,
    /// Predecessors outside the chain not yet placed
    /// (`UnscheduledPredecessors`).
    unscheduled: usize,
}

struct Placer {
    succ: Vec<Vec<usize>>,
    pred: Vec<Vec<usize>>,
    /// The probability of each edge, in the order of `succ`.
    chance: Vec<Vec<f64>>,
    edge: IndexMap<(usize, usize), f64>,
    freq: Vec<f64>,
    chain_of: Vec<usize>,
    chains: Vec<Chain>,
    /// `ComputedEdges`: the successor chosen for a block ahead of time.
    computed: IndexMap<usize, (usize, bool)>,
    /// `BlockWorkList`.
    work: Vec<usize>,
    /// The loop being placed is the blocks stamped with `active`
    /// (`BlockFilter`); 0: the whole function.
    stamp: Vec<u32>,
    active: u32,
    serial: u32,
    optimize_size: bool,
    entry: usize,
    /// The loop's blocks in layout order (`BlockFilterSet`), and where
    /// `getFirstUnplacedBlock` stands in them and in the function.
    members: Vec<usize>,
    next_member: usize,
    next_block: usize,
    /// `PreferredLoopExit`.
    exit: Option<usize>,
    /// The blocks as copies have made them, and those with nothing left
    /// reaching them.
    blocks: Vec<Block>,
    dead: Vec<bool>,
    /// The tail-duplication threshold in instructions (`TailDupSize`).
    dup_size: usize,
    postdom: postdom::PostDom,
    /// Edges rerouted by copies, for the odds: (from, its successors before,
    /// the tail, the tail's successors with their chances).
    rerouted: Vec<(i64, Vec<i64>, i64, Vec<(i64, f64)>)>,
}

fn succ_seen(
    seen: &[usize],
    index: &IndexMap<i64, usize>,
    to: i64,
) -> bool {
    index.get(&to).is_some_and(|to| seen.contains(to))
}

/// The blocks of `blocks` in the order `MachineBlockPlacement` lays them out,
/// as they are after the copies it made, with the odds those leave. The first
/// is the entry; blocks nothing reaches any more come last.
pub(super) fn order(
    blocks: &[Block],
    entry: i64,
    odds: &BlockOdds,
    busy: &Frequency,
    natural: &[Loop],
    size: bool,
    aggressive: bool,
) -> (Vec<Block>, BlockOdds) {
    let index: IndexMap<i64, usize> = blocks.iter().enumerate().map(|(place, block)| (block.at, place)).collect();
    let entry = index.get(&entry).copied().unwrap_or(0);
    let mut placer = Placer::new(blocks, odds, busy, &index, size, aggressive, entry);
    let order = placer.build(natural, &index);
    let mut out = odds.clone();
    for (from, succ, tail, into) in &placer.rerouted {
        out.rerouted(*from, succ, *tail, into);
    }
    let placed: HashSet<usize> = order.iter().copied().collect();
    let rest = (0..blocks.len()).filter(|one| !placed.contains(one));
    (order.iter().copied().chain(rest).map(|one| placer.blocks[one].clone()).collect(), out)
}

impl Placer {
    fn new(
        blocks: &[Block],
        odds: &BlockOdds,
        busy: &Frequency,
        index: &IndexMap<i64, usize>,
        size: bool,
        aggressive: bool,
        entry: usize,
    ) -> Self {
        let n = blocks.len();
        let (mut succ, mut chance) = (vec![Vec::new(); n], vec![Vec::new(); n]);
        let mut pred = vec![Vec::new(); n];
        let mut edge = IndexMap::default();
        for (from, block) in blocks.iter().enumerate() {
            // In the block's own order, which breaks ties (`chances` sorts).
            let chances: IndexMap<i64, f64> = odds.chances(block.at, &block.succ).into_iter().collect();
            for to in block.succ.iter() {
                if succ_seen(&succ[from], index, *to) {
                    continue;
                }
                let probability = chances.get(to).copied().unwrap_or(0.0);
                let Some(&to) = index.get(to) else { continue };
                succ[from].push(to);
                chance[from].push(probability);
                edge.insert((from, to), probability);
                pred[to].push(from);
            }
        }
        let postdom = postdom::PostDom::of(&succ, &pred);
        Self {
            postdom,
            succ,
            pred,
            chance,
            edge,
            freq: blocks.iter().map(|block| busy.block(block.at)).collect(),
            chain_of: (0..n).collect(),
            chains: (0..n).map(|one| Chain { blocks: vec![one], unscheduled: 0 }).collect(),
            computed: IndexMap::default(),
            work: Vec::new(),
            stamp: vec![0; n],
            active: 0,
            serial: 0,
            optimize_size: size,
            entry,
            members: Vec::new(),
            next_member: 0,
            next_block: 0,
            exit: None,
            blocks: blocks.to_vec(),
            dead: vec![false; n],
            dup_size: if size {
                1
            } else if aggressive {
                TAIL_DUPLICATION_AGGRESSIVE
            } else {
                TAIL_DUPLICATION
            },
            rerouted: Vec::new(),
        }
    }

    fn prob(
        &self,
        from: usize,
        to: usize,
    ) -> f64 {
        self.edge.get(&(from, to)).copied().unwrap_or(0.0)
    }

    fn head(
        &self,
        chain: usize,
    ) -> usize {
        self.chains[chain].blocks[0]
    }

    fn tail(
        &self,
        chain: usize,
    ) -> usize {
        *self.chains[chain].blocks.last().expect("a chain has a block")
    }

    fn ends_chain(
        &self,
        block: usize,
    ) -> bool {
        self.tail(self.chain_of[block]) == block
    }

    fn starts_chain(
        &self,
        block: usize,
    ) -> bool {
        self.head(self.chain_of[block]) == block
    }

    /// `BlockFilter`: in the loop being placed, or anywhere with none.
    fn allowed(
        &self,
        block: usize,
    ) -> bool {
        self.active == 0 || self.stamp[block] == self.active
    }

    /// `LoopBlockSet.count`, in a loop.
    fn inside(
        &self,
        block: usize,
    ) -> bool {
        self.stamp[block] == self.active
    }

    fn merge(
        &mut self,
        into: usize,
        from: usize,
    ) {
        let moved = std::mem::take(&mut self.chains[from].blocks);
        for &block in &moved {
            self.chain_of[block] = into;
        }
        self.chains[into].blocks.extend(moved);
    }

    /// `buildCFGChains`.
    fn build(
        &mut self,
        natural: &[Loop],
        index: &IndexMap<i64, usize>,
    ) -> Vec<usize> {
        let (n, entry) = (self.succ.len(), self.entry);
        let (depth, innermost) = self.nesting(natural, index);
        // Innermost first, as `loops` gives them: an inner loop's chain is
        // whole before the loop round it is placed.
        for found in natural {
            self.loop_chain(found, natural, index, &depth, &innermost);
        }
        self.precompute_triangle_chains();
        let mut updated: HashSet<usize> = HashSet::default();
        for block in 0..n {
            if !self.dead[block] {
                self.fill_work_lists(block, &mut updated);
            }
        }
        let function = self.chain_of[entry];
        self.build_chain(entry, function, false);
        let mut order = std::mem::take(&mut self.chains[self.chain_of[entry]].blocks);
        order.retain(|&block| !self.dead[block]);
        debug_assert_eq!(order.len(), self.dead.iter().filter(|dead| !**dead).count(), "a block left out of the chain");
        order
    }

    /// How deep each block is in loops, and the loop nearest round it.
    fn nesting(
        &self,
        natural: &[Loop],
        index: &IndexMap<i64, usize>,
    ) -> (Vec<u32>, Vec<Option<usize>>) {
        let n = self.succ.len();
        let (mut depth, mut innermost) = (vec![0_u32; n], vec![None; n]);
        // Largest first, so the nearest loop is the last to write.
        for (which, found) in natural.iter().enumerate().rev() {
            for at in found.body.to_set() {
                if let Some(&block) = index.get(&at) {
                    depth[block] += 1;
                    innermost[block] = Some(which);
                }
            }
        }
        (depth, innermost)
    }

    /// `markChainSuccessors`.
    fn mark_chain_successors(
        &mut self,
        chain: usize,
        header: usize,
    ) {
        for place in 0..self.chains[chain].blocks.len() {
            let block = self.chains[chain].blocks[place];
            self.mark_block_successors(chain, block, header);
        }
    }

    /// `markBlockSuccessors`.
    fn mark_block_successors(
        &mut self,
        chain: usize,
        block: usize,
        header: usize,
    ) {
        for to in 0..self.succ[block].len() {
            let to = self.succ[block][to];
            if !self.allowed(to) {
                continue;
            }
            let onward = self.chain_of[to];
            if onward == chain || to == header || self.chains[onward].unscheduled == 0 {
                continue;
            }
            self.chains[onward].unscheduled -= 1;
            if self.chains[onward].unscheduled > 0 {
                continue;
            }
            let first = self.head(onward);
            self.work.push(first);
        }
    }

    /// `fillWorkLists`.
    fn fill_work_lists(
        &mut self,
        block: usize,
        updated: &mut HashSet<usize>,
    ) {
        let chain = self.chain_of[block];
        if !updated.insert(chain) {
            return;
        }
        let mut waiting = 0;
        for &inside in &self.chains[chain].blocks {
            for &from in &self.pred[inside] {
                if self.allowed(from) && self.chain_of[from] != chain {
                    waiting += 1;
                }
            }
        }
        self.chains[chain].unscheduled += waiting;
        if self.chains[chain].unscheduled == 0 {
            let first = self.head(chain);
            self.work.push(first);
        }
    }

    /// `collectViableSuccessors`: the successors that may follow `block`, and
    /// the probability left once the others are taken out.
    fn viable(
        &self,
        block: usize,
        chain: usize,
    ) -> (Vec<usize>, f64) {
        let mut left = 1.0;
        let mut out = Vec::new();
        for (place, &to) in self.succ[block].iter().enumerate() {
            let mut skip = false;
            if !self.allowed(to) {
                skip = true;
            } else {
                let onward = self.chain_of[to];
                if onward == chain {
                    skip = true;
                } else if to != self.head(onward) {
                    continue;
                }
            }
            if skip {
                left -= self.chance[block][place];
            } else {
                out.push(to);
            }
        }
        (out, f64::max(left, 0.0))
    }

    /// `hasBetterLayoutPredecessor`.
    fn has_better_layout_predecessor(
        &self,
        block: usize,
        to: usize,
        real: f64,
        chain: usize,
    ) -> bool {
        let onward = self.chain_of[to];
        if self.chains[onward].unscheduled == 0 || self.pred[to].len() > PREDECESSOR_LIMIT {
            return false;
        }
        let candidate = self.freq[block] * real;
        for &from in &self.pred[to] {
            let from_chain = self.chain_of[from];
            if from == to
                || from_chain == onward
                || !self.allowed(from)
                || from_chain == chain
                || from != self.tail(from_chain)
                || from == block
            {
                continue;
            }
            let theirs = self.freq[from] * self.prob(from, to);
            if theirs * HOT >= candidate * (1.0 - HOT) {
                return true;
            }
        }
        false
    }

    /// `selectBestSuccessor`: the block to lay out after `block`, and whether
    /// it is to be copied into its predecessors.
    fn best_successor(
        &mut self,
        block: usize,
        chain: usize,
    ) -> (Option<usize>, bool) {
        let (successors, left) = self.viable(block, chain);
        if let Some((ahead, dup)) = self.computed.swap_remove(&block) {
            let onward = self.chain_of[ahead];
            if self.succ[block].contains(&ahead) && self.allowed(ahead) && onward != chain && ahead == self.head(onward)
            {
                return (Some(ahead), dup);
            }
        }
        if self.is_trellis(block, &successors, chain) {
            return self.best_trellis_successor(block, &successors, left, chain);
        }
        let (mut best, mut best_prob) = (None, 0.0);
        let mut candidates: Vec<(f64, usize)> = Vec::new();
        for to in successors {
            let real = self.prob(block, to);
            let adjusted = if real >= left { 1.0 } else { real / left };
            if self.has_better_layout_predecessor(block, to, real, chain) {
                if self.should_tail_duplicate(to) {
                    candidates.push((adjusted, to));
                }
                continue;
            }
            if best.is_some() && best_prob >= adjusted {
                continue;
            }
            best = Some(to);
            best_prob = adjusted;
        }
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut dup = false;
        for (probability, to) in candidates {
            if probability < best_prob {
                break;
            }
            if self.can_tail_duplicate_unplaced_preds(block, to, chain)
                && self.profitable_to_tail_duplicate(block, to, best_prob, chain)
            {
                best = Some(to);
                dup = true;
                break;
            }
        }
        (best, dup)
    }

    /// `hasSameSuccessors`.
    fn has_same_successors(
        &self,
        block: usize,
        successors: &[usize],
    ) -> bool {
        self.succ[block].len() == successors.len()
            && !successors.contains(&block)
            && self.succ[block].iter().all(|to| successors.contains(to))
    }

    /// `isTrellis`: `block` and the other blocks that go to its two viable
    /// successors have those two as their only successors.
    fn is_trellis(
        &self,
        block: usize,
        viable: &[usize],
        chain: usize,
    ) -> bool {
        if self.succ[block].len() != 2 || viable.len() != 2 {
            return false;
        }
        let successors = &self.succ[block];
        let mut seen: HashSet<usize> = HashSet::default();
        for &to in viable {
            if self.pred[to].len() > PREDECESSOR_LIMIT {
                return false;
            }
            let mut count = 0;
            for &from in &self.pred[to] {
                if successors.contains(&from) {
                    if self.succ[from].iter().any(|one| !successors.contains(one)) {
                        return false;
                    }
                    continue;
                }
                let from_chain = self.chain_of[from];
                if from == block || !self.allowed(from) || from_chain == chain || from_chain == self.chain_of[to] {
                    continue;
                }
                count += 1;
                if !seen.insert(from) {
                    continue;
                }
                if !self.has_same_successors(from, successors) {
                    return false;
                }
            }
            if count < 1 {
                return false;
            }
        }
        true
    }

    /// `getBestTrellisSuccessor`: of the edges into the two successors, the
    /// heaviest pair that share no source, and `block`'s if it is in it.
    fn best_trellis_successor(
        &mut self,
        block: usize,
        viable: &[usize],
        left: f64,
        chain: usize,
    ) -> (Option<usize>, bool) {
        let _ = left;
        if self.succ[block].len() != 2 || viable.len() != 2 {
            return (None, false);
        }
        let mut edges: [Vec<(f64, usize, usize)>; 2] = [Vec::new(), Vec::new()];
        for (which, &to) in viable.iter().enumerate() {
            for &from in &self.pred[to] {
                if from != block {
                    if !self.allowed(from) {
                        continue;
                    }
                    let from_chain = self.chain_of[from];
                    if from_chain == chain || from_chain == self.chain_of[to] {
                        continue;
                    }
                }
                edges[which].push((self.freq[from] * self.prob(from, to), from, to));
            }
        }
        for one in &mut edges {
            one.sort_by(|a, b| b.0.total_cmp(&a.0));
        }
        let (mut a, mut b) = (edges[0][0], edges[1][0]);
        if a.1 == b.1 {
            let (second_a, second_b) = (edges[0][1], edges[1][1]);
            if a.0 + second_b.0 < b.0 + second_a.0 {
                a = second_a;
            } else {
                b = second_b;
            }
        }
        if b.1 == block {
            std::mem::swap(&mut a, &mut b);
        }
        if a.1 != block {
            return (None, false);
        }
        if a.2 == b.1 {
            let (first, second) = (a.2, b.2);
            if self.should_tail_duplicate(second)
                && self.can_tail_duplicate_unplaced_preds(block, second, chain)
                && self.profitable_to_tail_duplicate(block, second, self.prob(block, first), chain)
            {
                return (Some(second), true);
            }
        }
        self.computed.insert(b.1, (b.2, false));
        (Some(a.2), false)
    }

    /// `selectBestCandidateBlock`: the most frequent block waiting.
    fn best_candidate(
        &mut self,
        chain: usize,
    ) -> Option<usize> {
        let chain_of = &self.chain_of;
        self.work.retain(|&block| chain_of[block] != chain);
        let mut best: Option<usize> = None;
        for &block in &self.work {
            if best.is_some_and(|best| self.freq[best] >= self.freq[block]) {
                continue;
            }
            best = Some(block);
        }
        best
    }

    /// `buildChain`: `chain` grown from its end until nothing is left to put
    /// after it. `in_loop`: the blocks are the loop's `members`.
    fn build_chain(
        &mut self,
        top: usize,
        chain: usize,
        in_loop: bool,
    ) {
        (self.next_block, self.next_member) = (0, 0);
        self.mark_chain_successors(chain, top);
        let mut block = self.tail(chain);
        loop {
            let (mut best, mut dup) = self.best_successor(block, chain);
            if let Some(next) = best {
                dup |= self.can_tail_duplicate_unplaced_preds(block, next, chain);
            }
            if best.is_none() {
                best = self.best_candidate(chain);
            }
            if best.is_none() {
                best = self.first_unplaced(chain, in_loop);
            }
            let Some(best) = best else { break };
            if dup {
                block = self.repeatedly_tail_duplicate(best, block, top, chain);
                if !self.succ[block].contains(&best) {
                    continue;
                }
            }
            let onward = self.chain_of[best];
            self.chains[onward].unscheduled = 0;
            self.mark_chain_successors(onward, top);
            self.merge(chain, onward);
            block = self.tail(chain);
        }
    }

    /// `getFirstUnplacedBlock`: where the CFG ends short of a block, as in a
    /// loop that is no natural one.
    fn first_unplaced(
        &mut self,
        chain: usize,
        in_loop: bool,
    ) -> Option<usize> {
        if in_loop {
            while self.next_member < self.members.len() && self.chain_of[self.members[self.next_member]] == chain {
                self.next_member += 1;
            }
            return self.members.get(self.next_member).map(|&one| self.head(self.chain_of[one]));
        }
        while self.next_block < self.succ.len()
            && (self.dead[self.next_block] || self.chain_of[self.next_block] == chain)
        {
            self.next_block += 1;
        }
        (self.next_block < self.succ.len()).then(|| self.head(self.chain_of[self.next_block]))
    }

    /// `buildLoopChains` for one loop.
    fn loop_chain(
        &mut self,
        found: &Loop,
        natural: &[Loop],
        index: &IndexMap<i64, usize>,
        depth: &[u32],
        innermost: &[Option<usize>],
    ) {
        let Some(&header) = index.get(&found.header).filter(|&&one| !self.dead[one]) else { return };
        let mut members: Vec<usize> =
            found.body.to_set().iter().filter_map(|at| index.get(at).copied()).filter(|&one| !self.dead[one]).collect();
        members.sort_unstable();
        self.serial += 1;
        self.active = self.serial;
        for &member in &members {
            self.stamp[member] = self.active;
        }
        let top = self.best_loop_top(header);
        let mut exit_freq = 0.0;
        self.exit = None;
        if top == header {
            (self.exit, exit_freq) = self.best_loop_exit(found, header, &members, natural, depth, innermost);
        }
        let chain = self.chain_of[top];
        let mut updated: HashSet<usize> = HashSet::default();
        updated.insert(chain);
        for &member in &members {
            self.fill_work_lists(member, &mut updated);
        }
        self.members = members;
        self.build_chain(top, chain, true);
        let exit = self.exit.take();
        self.rotate(chain, exit, exit_freq);
        self.work.clear();
        self.members.clear();
        self.active = 0;
    }

    /// `canMoveBottomBlockToTop`.
    fn can_move_bottom_to_top(
        &self,
        bottom: usize,
        old: usize,
    ) -> bool {
        if self.pred[bottom].len() != 1 {
            return true;
        }
        let above = self.pred[bottom][0];
        if self.succ[above].len() != 2 {
            return true;
        }
        let other = if self.succ[above][0] == bottom { self.succ[above][1] } else { self.succ[above][0] };
        other != old
    }

    /// `TopFallThroughFreq`: how often the top of a loop is fallen into from
    /// outside, at best.
    fn top_fall_through(
        &self,
        top: usize,
    ) -> f64 {
        let mut most = 0.0_f64;
        for &from in &self.pred[top] {
            if self.inside(from) || !self.ends_chain(from) {
                continue;
            }
            let top_prob = self.prob(from, top);
            let better = self.succ[from]
                .iter()
                .enumerate()
                .any(|(place, &to)| !self.inside(to) && self.chance[from][place] > top_prob && self.starts_chain(to));
            if !better {
                most = most.max(self.freq[from] * top_prob);
            }
        }
        most
    }

    /// `FallThroughGains`: what moving `new` before `old` gains in fall
    /// throughs, less what it loses.
    fn fall_through_gains(
        &self,
        new: usize,
        old: usize,
        exit: Option<usize>,
    ) -> f64 {
        let to_top = self.top_fall_through(old);
        let to_exit = exit.map_or(0.0, |exit| self.freq[new] * self.prob(new, exit));
        let back_edge = self.freq[new] * self.prob(new, old);
        let (mut best, mut from_pred) = (None, 0.0);
        for &from in &self.pred[new] {
            if !self.inside(from) || !self.ends_chain(from) {
                continue;
            }
            let freq = self.freq[from] * self.prob(from, new);
            if freq > from_pred {
                from_pred = freq;
                best = Some(from);
            }
        }
        let mut other = 0.0_f64;
        if let Some(best) = best {
            for (place, &to) in self.succ[best].iter().enumerate() {
                if to == new || to == best || !self.inside(to) || self.computed.contains_key(&to) {
                    continue;
                }
                if !self.starts_chain(to) || self.chain_of[to] == self.chain_of[best] {
                    continue;
                }
                other = other.max(self.freq[best] * self.chance[best][place]);
            }
            if other > self.freq[best] * self.prob(best, new) {
                other = 0.0;
                from_pred = 0.0;
            }
        }
        let (gains, lost) = (back_edge + other, to_top + to_exit + from_pred);
        if gains > lost { gains - lost } else { 0.0 }
    }

    /// `findBestLoopTopHelper`.
    fn best_loop_top_helper(
        &self,
        old: usize,
        header: usize,
    ) -> usize {
        let chain = self.chain_of[old];
        if !self.inside(self.head(chain)) || old != self.head(chain) {
            return old;
        }
        let (mut best_gains, mut best) = (0.0, None);
        for &from in &self.pred[old] {
            if !self.inside(from) || from == header || self.succ[from].len() > 2 {
                continue;
            }
            let other = (self.succ[from].len() == 2)
                .then(|| if self.succ[from][0] == old { self.succ[from][1] } else { self.succ[from][0] });
            if !self.can_move_bottom_to_top(from, old) {
                continue;
            }
            let gains = self.fall_through_gains(from, old, other);
            if gains > 0.0 && (gains > best_gains || (gains == best_gains && from + 1 == old)) {
                best = Some(from);
                best_gains = gains;
            }
        }
        let Some(mut best) = best else { return old };
        for _ in 0..self.succ.len() {
            let [above] = self.pred[best][..] else { break };
            if self.succ[above].len() != 1 || above == header {
                break;
            }
            best = above;
        }
        best
    }

    /// `findBestLoopTop`.
    fn best_loop_top(
        &mut self,
        header: usize,
    ) -> usize {
        // The entry stays first: a loop round it is entered at its header.
        if self.optimize_size || header == self.entry {
            return header;
        }
        let (mut old, mut new) = (None, header);
        while Some(new) != old {
            old = Some(new);
            new = self.best_loop_top_helper(new, header);
            if Some(new) != old {
                self.computed.insert(new, (old.expect("set above"), false));
            }
        }
        new
    }

    /// `findBestLoopExit`: the block to put last, and how often it leaves.
    fn best_loop_exit(
        &self,
        found: &Loop,
        header: usize,
        members: &[usize],
        natural: &[Loop],
        depth: &[u32],
        innermost: &[Option<usize>],
    ) -> (Option<usize>, f64) {
        if !self.inside(self.head(self.chain_of[header])) {
            return (None, 0.0);
        }
        let mut best_freq = 0.0_f64;
        let mut exiting: Option<usize> = None;
        let mut to_outer: HashSet<usize> = HashSet::default();
        let ordered = std::iter::once(header).chain(members.iter().copied().filter(|&one| one != header));
        for block in ordered {
            if !self.ends_chain(block) {
                continue;
            }
            let (old_exiting, old_freq) = (exiting, best_freq);
            let mut looping = false;
            for (place, &to) in self.succ[block].iter().enumerate() {
                if to == block || self.chain_of[to] == self.chain_of[block] {
                    continue;
                }
                if self.inside(to) {
                    looping = true;
                    continue;
                }
                if innermost[to].is_some_and(|outer| natural[outer].body.contains(&found.header)) {
                    to_outer.insert(block);
                }
                let freq = self.freq[block] * self.chance[block][place];
                // LLVM never raises `BestExitLoopDepth` above 0 (D8).
                if exiting.is_none()
                    || depth[to] > 0
                    || above(freq, best_freq)
                    || (block + 1 == to && at_least(freq, best_freq))
                {
                    best_freq = freq;
                    exiting = Some(block);
                }
            }
            if !looping {
                exiting = old_exiting;
                best_freq = old_freq;
            }
        }
        let Some(exiting) = exiting else { return (None, 0.0) };
        if members.len() == 1 || (!to_outer.is_empty() && !to_outer.contains(&exiting)) {
            return (None, 0.0);
        }
        (Some(exiting), best_freq)
    }

    /// `hasViableTopFallthrough`.
    fn viable_top_fall_through(
        &self,
        top: usize,
    ) -> bool {
        for &from in &self.pred[top] {
            if self.inside(from) || !self.ends_chain(from) {
                continue;
            }
            let top_prob = self.prob(from, top);
            let better = self.succ[from]
                .iter()
                .enumerate()
                .any(|(place, &to)| self.starts_chain(to) && self.chance[from][place] > top_prob);
            if !better {
                return true;
            }
        }
        false
    }

    /// `rotateLoop`: the exiting block last, so that a pass round the loop
    /// falls through the test.
    fn rotate(
        &mut self,
        chain: usize,
        exiting: Option<usize>,
        exit_freq: f64,
    ) {
        let Some(exiting) = exiting else { return };
        let (top, bottom) = (self.head(chain), self.tail(chain));
        if bottom == exiting || top == self.entry {
            return;
        }
        let viable = self.viable_top_fall_through(top);
        if viable {
            for &to in &self.succ[bottom] {
                if !self.inside(to) && self.starts_chain(to) {
                    return;
                }
            }
            if at_least(self.top_fall_through(top), exit_freq) {
                return;
            }
        }
        let Some(at) = self.chains[chain].blocks.iter().position(|&one| one == exiting) else { return };
        if viable {
            let next = self.chains[chain].blocks[at + 1];
            if self.succ[exiting].contains(&next) && !self.succ[bottom].contains(&top) {
                return;
            }
        }
        self.chains[chain].blocks.rotate_left(at + 1);
    }
}
