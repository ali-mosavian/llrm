//! Block placement: the order of a function's blocks that makes the likely
//! edges fall through, translated from `MachineBlockPlacement` in
//! [MachineBlockPlacement.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp).
//!
//! The chain builder ([`place`]) is generic over [`Layout`], the few facts it
//! asks of a function's blocks, so a second machine function can use it; the
//! spike's [`MFunction`] is one ([`Graph`]), and [`place_blocks`] reorders it
//! and fixes the terminators (`updateTerminator`, `optimizeBranches`).
//!
//! ```text
//!  before                          after
//!  b0: ..                          b0: ..
//!  b1: cmp; beq b3                 b2: body        (loop rotated: the exit
//!  b2: body; bra b1                b1: cmp; bne b2  test sits at the bottom)
//!  b3: rts                         b3: rts
//! ```
//!
//! DEVIATE: tail duplication (`TailDupPlacement`, `precomputeTriangleChains`),
//! trellis layouts (`isTrellis`), `rotateLoopWithProfile`, ExtTSP, tail merging
//! after placement, `alignBlocks`, EH pads and profile data are not
//! translated; a cold loop block is never outlined. Reason: no row needs them
//! yet; each is a self-contained addition to the functions below. Exit: the
//! neutral-LIR task, where `llrm-codegen`'s port takes over.
//! DEVIATE: [`Graph`] derives edge probabilities from the IR's by block number
//! (even odds for an edge a pass made), where the pin carries them through
//! every transformation. A loop's block order is its header, then reverse
//! post-order, as `LoopInfo` leaves it.

use llrm_mir::blockfrequency::{BlockFrequency, BlockFrequencyInfo, BranchProbability, EdgeProbabilities};
use llrm_mir::blocklist::BlockList;
use llrm_mir::cycleinfo::{CycleInfo, FlowGraph};
use llrm_mir::domtree::DomGraph;
use llrm_mir::module::BlockId;
use llrm_support::hash::{HashMap, HashSet};

use crate::cfg::Cfg;
use crate::fold::{Branches, analyze, insert_branch, remove_branch};
use crate::mfunc::MFunction;
use crate::sink::Jump;

/// `StaticLikelyProb`: a successor with this share of the odds is hot.
const STATIC_LIKELY_PROB: u32 = 80;
/// `PredecessorLimit`.
const PREDECESSOR_LIMIT: usize = 1000;
/// `ExitBlockBias`.
const EXIT_BLOCK_BIAS: u32 = 0;

/// A natural loop: its header, its blocks (header first) and the loops inside
/// it.
#[derive(Clone, Debug)]
pub struct Loop {
    pub header: usize,
    pub blocks: Vec<usize>,
    pub inner: Vec<Loop>,
}

/// What placement asks of a function. Blocks are numbered by their position in
/// the current layout.
pub trait Layout {
    fn len(&self) -> usize;
    /// `successors()`: each block once, in the order the terminators name them.
    fn successors(
        &self,
        block: usize,
    ) -> &[usize];
    fn predecessors(
        &self,
        block: usize,
    ) -> &[usize];
    /// `MBPI->getEdgeProbability`.
    fn probability(
        &self,
        from: usize,
        to: usize,
    ) -> BranchProbability;
    /// `MBFI->getBlockFreq`.
    fn frequency(
        &self,
        block: usize,
    ) -> BlockFrequency;
    /// `analyzeBranch` fails on the block and it can fall through: it stays
    /// with its layout successor.
    fn keeps_fall_through(
        &self,
        block: usize,
    ) -> bool;
    /// The top-level loops, in `MachineLoopInfo` order.
    fn loops(&self) -> Vec<Loop>;
    /// `shouldOptimizeForSize`.
    fn optimize_for_size(&self) -> bool;
    /// Whether a taken jump has a price the layout should count: a rotated
    /// loop then pays for the jump at its entry only when the loop runs
    /// less often than that jump saves (the pin's compare is entry against
    /// exit, which a loop with one entry and one exit always ties).
    fn counts_jumps(&self) -> bool {
        false
    }
}

/// A chain of blocks laid out in a row (`BlockChain`).
struct Chain {
    blocks: Vec<usize>,
    /// Predecessors of its blocks not yet placed.
    unscheduled: u32,
}

/// The blocks a loop's chain may take (`BlockFilterSet`).
struct Filter {
    order: Vec<usize>,
    member: Vec<bool>,
}

struct FlatLoop {
    header: usize,
    blocks: Vec<usize>,
    parent: Option<usize>,
    depth: usize,
}

struct Placer<'a, L: Layout> {
    layout: &'a L,
    chain_of: Vec<usize>,
    chains: Vec<Chain>,
    work: Vec<usize>,
    /// `ComputedEdges`: the block the layout successor of a block must be.
    computed: HashMap<usize, usize>,
    loops: Vec<FlatLoop>,
    innermost: Vec<Option<usize>>,
    preferred_exit: Option<usize>,
}

const NONE: usize = usize::MAX;

fn adjusted_probability(
    original: BranchProbability,
    sum: BranchProbability,
) -> BranchProbability {
    let (n, d) = (original.get_numerator(), sum.get_numerator());
    if n >= d { BranchProbability::get_one() } else { BranchProbability::new(n, d) }
}

impl<'a, L: Layout> Placer<'a, L> {
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

    fn probability(
        &self,
        from: usize,
        to: usize,
    ) -> BranchProbability {
        self.layout.probability(from, to)
    }

    fn edge_frequency(
        &self,
        from: usize,
        to: usize,
    ) -> BlockFrequency {
        self.layout.frequency(from) * self.probability(from, to)
    }

    /// `Chain.merge(BB, Chain)`: the chain `other` (headed by `block`) joins
    /// `into`.
    fn merge(
        &mut self,
        into: usize,
        block: usize,
        other: Option<usize>,
    ) {
        match other {
            None => {
                self.chains[into].blocks.push(block);
                self.chain_of[block] = into;
            }
            Some(other) => {
                debug_assert_eq!(self.head(other), block);
                for moved in std::mem::take(&mut self.chains[other].blocks) {
                    self.chains[into].blocks.push(moved);
                    self.chain_of[moved] = into;
                }
            }
        }
    }

    /// `markChainSuccessors`.
    fn mark_chain_successors(
        &mut self,
        chain: usize,
        loop_header: usize,
        filter: Option<&Filter>,
    ) {
        for block in self.chains[chain].blocks.clone() {
            for &succ in self.layout.successors(block) {
                if filter.is_some_and(|filter| !filter.member[succ]) {
                    continue;
                }
                let succ_chain = self.chain_of[succ];
                if succ_chain == chain || succ == loop_header {
                    continue;
                }
                if self.chains[succ_chain].unscheduled == 0 {
                    continue;
                }
                self.chains[succ_chain].unscheduled -= 1;
                if self.chains[succ_chain].unscheduled > 0 {
                    continue;
                }
                self.work.push(self.head(succ_chain));
            }
        }
    }

    /// `collectViableSuccessors`: the successors that may follow `block`, and
    /// the odds that remain for them.
    fn collect_viable_successors(
        &self,
        block: usize,
        chain: usize,
        filter: Option<&Filter>,
    ) -> (Vec<usize>, BranchProbability) {
        let mut adjusted = BranchProbability::get_one();
        let mut out = Vec::new();
        for &succ in self.layout.successors(block) {
            let skip = if filter.is_some_and(|filter| !filter.member[succ]) {
                true
            } else {
                let succ_chain = self.chain_of[succ];
                if succ_chain == chain {
                    true
                } else if succ != self.head(succ_chain) {
                    // mid chain
                    continue;
                } else {
                    false
                }
            };
            if skip {
                adjusted = adjusted - self.probability(block, succ);
            } else {
                out.push(succ);
            }
        }
        (out, adjusted)
    }

    /// `hasBetterLayoutPredecessor`: another predecessor of `succ` is the
    /// better one to fall into it.
    fn has_better_layout_predecessor(
        &self,
        block: usize,
        succ: usize,
        succ_chain: usize,
        real_prob: BranchProbability,
        chain: usize,
        filter: Option<&Filter>,
    ) -> bool {
        if self.chains[succ_chain].unscheduled == 0 {
            return false;
        }
        if self.layout.predecessors(succ).len() > PREDECESSOR_LIMIT {
            return false;
        }
        // no profile: the static threshold
        let hot = BranchProbability::new(STATIC_LIKELY_PROB, 100);
        let candidate = self.layout.frequency(block) * real_prob;
        for &pred in self.layout.predecessors(succ) {
            let pred_chain = self.chain_of[pred];
            if pred == succ
                || pred_chain == succ_chain
                || filter.is_some_and(|filter| !filter.member[pred])
                || pred_chain == chain
                || pred != self.tail(pred_chain)
                || pred == block
            {
                continue;
            }
            let pred_edge = self.edge_frequency(pred, succ);
            if pred_edge * hot >= candidate * hot.get_compl() {
                return true;
            }
        }
        false
    }

    /// `selectBestSuccessor`.
    fn select_best_successor(
        &mut self,
        block: usize,
        chain: usize,
        filter: Option<&Filter>,
    ) -> Option<usize> {
        let (successors, adjusted) = self.collect_viable_successors(block, chain, filter);
        if let Some(result) = self.computed.remove(&block) {
            let result_chain = self.chain_of[result];
            if self.layout.successors(block).contains(&result)
                && filter.is_none_or(|filter| filter.member[result])
                && result_chain != chain
                && result == self.head(result_chain)
            {
                return Some(result);
            }
        }
        let mut best = None;
        let mut best_prob = BranchProbability::get_zero();
        for succ in successors {
            let real = self.probability(block, succ);
            let prob = adjusted_probability(real, adjusted);
            let succ_chain = self.chain_of[succ];
            if self.has_better_layout_predecessor(block, succ, succ_chain, real, chain, filter) {
                continue;
            }
            if best.is_some() && best_prob >= prob {
                continue;
            }
            best = Some(succ);
            best_prob = prob;
        }
        best
    }

    /// `selectBestCandidateBlock`: the most frequent block of the work list.
    fn select_best_candidate_block(
        &mut self,
        chain: usize,
    ) -> Option<usize> {
        let chain_of = &self.chain_of;
        self.work.retain(|&block| chain_of[block] != chain);
        let mut best: Option<(usize, BlockFrequency)> = None;
        for &block in &self.work {
            let frequency = self.layout.frequency(block);
            if best.is_some_and(|(_, freq)| freq >= frequency) {
                continue;
            }
            best = Some((block, frequency));
        }
        best.map(|(block, _)| block)
    }

    /// `getFirstUnplacedBlock`, with or without the loop's filter.
    fn first_unplaced(
        &self,
        placed: usize,
        filter: Option<&Filter>,
        from: &mut usize,
    ) -> Option<usize> {
        let blocks: &[usize] = filter.map_or(&[], |filter| &filter.order);
        let count = if filter.is_some() { blocks.len() } else { self.layout.len() };
        while *from < count {
            let block = if filter.is_some() { blocks[*from] } else { *from };
            let chain = self.chain_of[block];
            if chain != placed {
                return Some(self.head(chain));
            }
            *from += 1;
        }
        None
    }

    /// `fillWorkLists`.
    fn fill_work_lists(
        &mut self,
        block: usize,
        updated: &mut HashSet<usize>,
        filter: Option<&Filter>,
    ) {
        let chain = self.chain_of[block];
        if !updated.insert(chain) {
            return;
        }
        for member in self.chains[chain].blocks.clone() {
            for &pred in self.layout.predecessors(member) {
                if filter.is_some_and(|filter| !filter.member[pred]) || self.chain_of[pred] == chain {
                    continue;
                }
                self.chains[chain].unscheduled += 1;
            }
        }
        if self.chains[chain].unscheduled == 0 {
            self.work.push(self.head(chain));
        }
    }

    /// `buildChain`: grows the chain of `head` by the best successor, else
    /// the best ready block, else the first block not placed.
    fn build_chain(
        &mut self,
        head: usize,
        chain: usize,
        filter: Option<&Filter>,
    ) {
        let mut from = 0;
        self.mark_chain_successors(chain, head, filter);
        let mut block = self.tail(chain);
        loop {
            let mut best = self.select_best_successor(block, chain, filter);
            if best.is_none() {
                best = self.select_best_candidate_block(chain);
            }
            if best.is_none() {
                best = self.first_unplaced(chain, filter, &mut from);
            }
            let Some(best) = best else { break };
            let succ_chain = self.chain_of[best];
            // a successor that did not fit the CFG
            self.chains[succ_chain].unscheduled = 0;
            self.mark_chain_successors(succ_chain, head, filter);
            self.merge(chain, best, Some(succ_chain));
            block = self.tail(chain);
        }
    }

    /// `canMoveBottomBlockToTop`.
    fn can_move_bottom_block_to_top(
        &self,
        bottom: usize,
        old_top: usize,
    ) -> bool {
        let preds = self.layout.predecessors(bottom);
        if preds.len() != 1 {
            return true;
        }
        let succs = self.layout.successors(preds[0]);
        if succs.len() != 2 {
            return true;
        }
        let other = if succs[0] == bottom { succs[1] } else { succs[0] };
        other != old_top
    }

    /// `TopFallThroughFreq`.
    fn top_fall_through_freq(
        &self,
        top: usize,
        loop_set: &Filter,
    ) -> BlockFrequency {
        let mut max = BlockFrequency(0);
        for &pred in self.layout.predecessors(top) {
            let pred_chain = self.chain_of[pred];
            if loop_set.member[pred] || pred != self.tail(pred_chain) {
                continue;
            }
            let top_prob = self.probability(pred, top);
            let mut top_ok = true;
            for &succ in self.layout.successors(pred) {
                let succ_prob = self.probability(pred, succ);
                let succ_chain = self.chain_of[succ];
                if !loop_set.member[succ] && succ_prob > top_prob && succ == self.head(succ_chain) {
                    top_ok = false;
                    break;
                }
            }
            if top_ok {
                max = max.max(self.edge_frequency(pred, top));
            }
        }
        max
    }

    /// `FallThroughGains`: what moving `new_top` before `old_top` gains in
    /// fall-through frequency, less what it loses.
    fn fall_through_gains(
        &self,
        new_top: usize,
        old_top: usize,
        exit: Option<usize>,
        loop_set: &Filter,
    ) -> BlockFrequency {
        let to_top = self.top_fall_through_freq(old_top, loop_set);
        let to_exit = exit.map_or(BlockFrequency(0), |exit| self.edge_frequency(new_top, exit));
        let back_edge = self.edge_frequency(new_top, old_top);
        let mut best_pred = None;
        let mut from_pred = BlockFrequency(0);
        for &pred in self.layout.predecessors(new_top) {
            if !loop_set.member[pred] {
                continue;
            }
            if pred == self.tail(self.chain_of[pred]) {
                let edge = self.edge_frequency(pred, new_top);
                if edge > from_pred {
                    from_pred = edge;
                    best_pred = Some(pred);
                }
            }
        }
        let mut new_freq = BlockFrequency(0);
        if let Some(best_pred) = best_pred {
            for &succ in self.layout.successors(best_pred) {
                if succ == new_top || succ == best_pred || !loop_set.member[succ] || self.computed.contains_key(&succ) {
                    continue;
                }
                let succ_chain = self.chain_of[succ];
                if succ != self.head(succ_chain) || succ_chain == self.chain_of[best_pred] {
                    continue;
                }
                new_freq = new_freq.max(self.edge_frequency(best_pred, succ));
            }
            if new_freq > self.edge_frequency(best_pred, new_top) {
                new_freq = BlockFrequency(0);
                from_pred = BlockFrequency(0);
            }
        }
        let gains = back_edge + new_freq;
        let lost = to_top + to_exit + from_pred;
        if gains > lost { gains - lost } else { BlockFrequency(0) }
    }

    /// `findBestLoopTopHelper`: a block that is strictly better laid out
    /// before the loop's top, if one is.
    fn find_best_loop_top_helper(
        &self,
        old_top: usize,
        header: usize,
        loop_set: &Filter,
    ) -> usize {
        let header_chain = self.chain_of[old_top];
        if !loop_set.member[self.head(header_chain)] || old_top != self.head(header_chain) {
            return old_top;
        }
        let mut best_gains = BlockFrequency(0);
        let mut best_pred = None;
        for &pred in self.layout.predecessors(old_top) {
            if !loop_set.member[pred] || pred == header {
                continue;
            }
            let succs = self.layout.successors(pred);
            if succs.len() > 2 {
                continue;
            }
            let other = (succs.len() == 2).then(|| if succs[0] == old_top { succs[1] } else { succs[0] });
            if !self.can_move_bottom_block_to_top(pred, old_top) {
                continue;
            }
            let gains = self.fall_through_gains(pred, old_top, other, loop_set);
            if gains > BlockFrequency(0) && (gains > best_gains || (gains == best_gains && pred + 1 == old_top)) {
                best_pred = Some(pred);
                best_gains = gains;
            }
        }
        let Some(mut best_pred) = best_pred else { return old_top };
        // walk back through a straight line of predecessors
        loop {
            let preds = self.layout.predecessors(best_pred);
            if preds.len() == 1 && self.layout.successors(preds[0]).len() == 1 && preds[0] != header {
                best_pred = preds[0];
            } else {
                break;
            }
        }
        best_pred
    }

    /// `findBestLoopTop`.
    fn find_best_loop_top(
        &mut self,
        header: usize,
        loop_set: &Filter,
    ) -> usize {
        if self.layout.optimize_for_size() {
            return header;
        }
        let mut old_top = NONE;
        let mut new_top = header;
        while new_top != old_top {
            old_top = new_top;
            new_top = self.find_best_loop_top_helper(old_top, header, loop_set);
            if new_top != old_top {
                self.computed.insert(new_top, old_top);
            }
        }
        new_top
    }

    /// `findBestLoopExit`: the block whose exit edge is best laid out at the
    /// loop's bottom, and that edge's frequency.
    fn find_best_loop_exit(
        &self,
        this: usize,
        loop_set: &Filter,
    ) -> Option<(usize, BlockFrequency)> {
        let (header, blocks) = (self.loops[this].header, self.loops[this].blocks.clone());
        let header_chain = self.chain_of[header];
        if !loop_set.member[self.head(header_chain)] {
            return None;
        }
        let mut best_freq = BlockFrequency(0);
        // the pin never raises `BestExitLoopDepth` from 0
        let best_depth = 0;
        let mut exiting: Option<usize> = None;
        let mut to_outer: HashSet<usize> = HashSet::default();
        for &block in &blocks {
            let chain = self.chain_of[block];
            if block != self.tail(chain) {
                continue;
            }
            let (old_exiting, old_freq) = (exiting, best_freq);
            let mut has_looping = false;
            for &succ in self.layout.successors(block) {
                if succ == block {
                    continue;
                }
                if self.chain_of[succ] == chain {
                    continue;
                }
                let prob = self.probability(block, succ);
                if loop_set.member[succ] {
                    has_looping = true;
                    continue;
                }
                let mut depth = 0;
                if let Some(exit_loop) = self.innermost[succ] {
                    depth = self.loops[exit_loop].depth;
                    if self.contains(exit_loop, this) {
                        to_outer.insert(block);
                    }
                }
                let exit_freq = self.layout.frequency(block) * prob;
                let bias = BranchProbability::new(100 - EXIT_BLOCK_BIAS, 100);
                if exiting.is_none()
                    || depth > best_depth
                    || exit_freq > best_freq
                    || (block + 1 == succ && exit_freq >= best_freq * bias)
                {
                    best_freq = exit_freq;
                    exiting = Some(block);
                }
            }
            if !has_looping {
                exiting = old_exiting;
                best_freq = old_freq;
            }
        }
        let exiting = exiting?;
        if blocks.len() == 1 {
            return None;
        }
        if !to_outer.is_empty() && !to_outer.contains(&exiting) {
            return None;
        }
        Some((exiting, best_freq))
    }

    /// `loop(outer).contains(loop(inner))`.
    fn contains(
        &self,
        outer: usize,
        inner: usize,
    ) -> bool {
        let mut at = Some(inner);
        while let Some(one) = at {
            if one == outer {
                return true;
            }
            at = self.loops[one].parent;
        }
        false
    }

    /// `hasViableTopFallthrough`.
    fn has_viable_top_fallthrough(
        &self,
        top: usize,
        loop_set: &Filter,
    ) -> bool {
        for &pred in self.layout.predecessors(top) {
            let pred_chain = self.chain_of[pred];
            if loop_set.member[pred] || pred != self.tail(pred_chain) {
                continue;
            }
            let top_prob = self.probability(pred, top);
            let mut top_ok = true;
            for &succ in self.layout.successors(pred) {
                let succ_prob = self.probability(pred, succ);
                if succ == self.head(self.chain_of[succ]) && succ_prob > top_prob {
                    top_ok = false;
                    break;
                }
            }
            if top_ok {
                return true;
            }
        }
        false
    }

    /// `rotateLoop`: the chain's exiting block goes to the bottom, when that
    /// adds no branch.
    fn rotate_loop(
        &mut self,
        chain: usize,
        exiting: Option<(usize, BlockFrequency)>,
        loop_set: &Filter,
    ) {
        let Some((exiting, exit_freq)) = exiting else { return };
        let top = self.head(chain);
        let bottom = self.tail(chain);
        if bottom == exiting || top == 0 {
            return;
        }
        let viable_top = self.has_viable_top_fallthrough(top, loop_set);
        if viable_top {
            for &succ in self.layout.successors(bottom) {
                let succ_chain = self.chain_of[succ];
                if !loop_set.member[succ] && succ == self.head(succ_chain) {
                    return;
                }
            }
            // DEVIATE (6502-4): against the back edge's frequency when the
            // layout counts jumps, where the pin's `ExitFreq` ties
            // with the entry's for a loop with one entry and one
            // exit. Reason: a 6502 `jmp` is 3 clocks and 3 bytes;
            // the rotation drops one per trip and adds one per
            // entry. Exit: none while the pin's rotation ignores the
            // cost of the jump it removes; drop it if the pin counts that cost.
            let saved = if self.layout.counts_jumps() { self.edge_frequency(bottom, top) } else { exit_freq };
            if self.top_fall_through_freq(top, loop_set) >= saved {
                return;
            }
        }
        let Some(at) = self.chains[chain].blocks.iter().position(|&one| one == exiting) else { return };
        if viable_top {
            let next = self.chains[chain].blocks[at + 1];
            if self.layout.successors(exiting).contains(&next) && !self.layout.successors(bottom).contains(&top) {
                return;
            }
        }
        self.chains[chain].blocks.rotate_left(at + 1);
    }

    /// `buildLoopChains`: innermost loops first.
    fn build_loop_chains(
        &mut self,
        this: usize,
        inner: &[Vec<usize>],
    ) {
        for &child in &inner[this] {
            self.build_loop_chains(child, inner);
        }
        debug_assert!(self.work.is_empty());
        let mut order = self.loops[this].blocks.clone();
        order.sort_unstable();
        let mut member = vec![false; self.layout.len()];
        for &block in &order {
            member[block] = true;
        }
        let filter = Filter { order, member };
        let header = self.loops[this].header;
        let top = self.find_best_loop_top(header, &filter);
        self.preferred_exit = None;
        let mut exiting = None;
        if top == header {
            exiting = self.find_best_loop_exit(this, &filter);
            self.preferred_exit = exiting.map(|(block, _)| block);
        }
        let chain = self.chain_of[top];
        let mut updated = HashSet::default();
        updated.insert(chain);
        for &block in &filter.order {
            self.fill_work_lists(block, &mut updated, Some(&filter));
        }
        self.build_chain(top, chain, Some(&filter));
        self.rotate_loop(chain, exiting, &filter);
        self.work.clear();
    }

    /// `buildCFGChains`: the final order of the blocks.
    fn build_cfg_chains(&mut self) -> Vec<usize> {
        let count = self.layout.len();
        let mut at = 0;
        while at < count {
            let chain = self.chains.len();
            self.chains.push(Chain { blocks: vec![at], unscheduled: 0 });
            self.chain_of[at] = chain;
            // blocks we cannot reason about keep their fall-through
            while self.layout.keeps_fall_through(at) && at + 1 < count {
                self.merge(chain, at + 1, None);
                at += 1;
            }
            at += 1;
        }
        // loops: nesting from containment
        let tops = self.layout.loops();
        let mut inner: Vec<Vec<usize>> = Vec::new();
        let mut roots = Vec::new();
        for one in &tops {
            let at = self.flatten(one, None, 1, &mut inner);
            roots.push(at);
        }
        for &root in &roots {
            self.build_loop_chains(root, &inner);
        }
        debug_assert!(self.work.is_empty());
        let mut updated = HashSet::default();
        for block in 0..count {
            self.fill_work_lists(block, &mut updated, None);
        }
        let entry = self.chain_of[0];
        self.build_chain(0, entry, None);
        self.work.clear();
        self.chains[self.chain_of[0]].blocks.clone()
    }

    fn flatten(
        &mut self,
        one: &Loop,
        parent: Option<usize>,
        depth: usize,
        inner: &mut Vec<Vec<usize>>,
    ) -> usize {
        let at = self.loops.len();
        self.loops.push(FlatLoop { header: one.header, blocks: one.blocks.clone(), parent, depth });
        inner.push(Vec::new());
        for &block in &one.blocks {
            self.innermost[block] = Some(at);
        }
        for child in &one.inner {
            let made = self.flatten(child, Some(at), depth + 1, inner);
            inner[at].push(made);
        }
        at
    }
}

/// The order `blocks` should be laid out in, as indices of the current layout.
pub fn place<L: Layout>(layout: &L) -> Vec<usize> {
    let count = layout.len();
    if count < 2 {
        return (0..count).collect();
    }
    let mut placer = Placer {
        layout,
        chain_of: vec![NONE; count],
        chains: Vec::new(),
        work: Vec::new(),
        computed: HashMap::default(),
        loops: Vec::new(),
        innermost: vec![None; count],
        preferred_exit: None,
    };
    let order = placer.build_cfg_chains();
    // every block exactly once, the entry first
    let mut seen = vec![false; count];
    let whole =
        order.len() == count && order[0] == 0 && order.iter().all(|&block| !std::mem::replace(&mut seen[block], true));
    if whole { order } else { (0..count).collect() }
}

/// The IR's branch probabilities by `(from, to)` block number, with the
/// position of `to` among `from`'s successors.
pub type Odds = HashMap<(u32, u32), (BranchProbability, usize)>;

/// A function's blocks as [`Layout`] asks for them.
pub struct Graph {
    counts_jumps: bool,
    succs: Vec<Vec<usize>>,
    preds: Vec<Vec<usize>>,
    probs: Vec<Vec<BranchProbability>>,
    freqs: Vec<BlockFrequency>,
    pinned: Vec<bool>,
    loops: Vec<Loop>,
    size: bool,
}

/// The machine function as `BlockFrequencyInfo` walks it.
struct Machine<'a> {
    nodes: Vec<BlockId>,
    succs: &'a [Vec<usize>],
    preds: &'a [Vec<usize>],
}

fn list(blocks: &[usize]) -> BlockList {
    blocks.iter().map(|&one| BlockId(one as u32)).collect()
}

impl DomGraph for Machine<'_> {
    fn entry(&self) -> Option<BlockId> {
        (!self.nodes.is_empty()).then_some(BlockId(0))
    }

    fn max_number(&self) -> usize {
        self.nodes.len()
    }

    fn nodes(&self) -> &[BlockId] {
        &self.nodes
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> BlockList {
        list(&self.succs[block.0 as usize])
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        list(&self.preds[block.0 as usize])
    }
}

impl FlowGraph for Machine<'_> {}

struct Probabilities<'a>(&'a [Vec<BranchProbability>]);

impl EdgeProbabilities for Probabilities<'_> {
    fn get_edge_probability(
        &self,
        src: BlockId,
        index: usize,
    ) -> BranchProbability {
        self.0[src.0 as usize][index]
    }
}

impl Graph {
    /// The graph of `function`. `odds` are the IR's edge probabilities and
    /// positions among the branch's successors by `(from, to)` block number;
    /// an edge it lacks shares what the others leave. Successors are in the
    /// IR's order, then the order a pass that made blocks adds them in: the
    /// one that falls through first (`EmitLoweredSelect`). A tie goes to the
    /// first.
    pub fn of<C, J: Jump>(
        function: &MFunction<C>,
        odds: &Odds,
        size: bool,
    ) -> Graph {
        let cfg = Cfg::new(function);
        let count = function.blocks.len();
        let id = |at: usize| function.blocks[at].id;
        let succs: Vec<Vec<usize>> = (0..count)
            .map(|from| {
                let mut list = cfg.succs[from].clone();
                // the fall-through successor is listed last; made blocks name
                // it first
                list.sort_by_key(|&to| match odds.get(&(id(from), id(to))) {
                    Some(&(_, rank)) => (0, rank),
                    None => (1, usize::from(to != from + 1)),
                });
                list
            })
            .collect();
        let preds = cfg.preds.clone();
        let odds_of = |from: usize, to: usize| odds.get(&(id(from), id(to))).map(|one| one.0);
        let probs: Vec<Vec<BranchProbability>> = (0..count)
            .map(|from| {
                let known: Vec<Option<BranchProbability>> = succs[from].iter().map(|&to| odds_of(from, to)).collect();
                let given = known.iter().flatten().fold(BranchProbability::get_zero(), |sum, &one| sum + one);
                let missing = known.iter().filter(|one| one.is_none()).count() as u32;
                let share = if missing == 0 {
                    BranchProbability::get_zero()
                } else if given.is_zero() {
                    BranchProbability::new(1, missing)
                } else {
                    (BranchProbability::get_one() - given).div_u32(missing)
                };
                known.into_iter().map(|one| one.unwrap_or(share)).collect()
            })
            .collect();
        let machine = Machine { nodes: (0..count as u32).map(BlockId).collect(), succs: &succs, preds: &preds };
        let freqs = if count == 0 {
            Vec::new()
        } else {
            let ci = CycleInfo::compute(&machine);
            let info = BlockFrequencyInfo::calculate(&machine, &Probabilities(&probs), &ci);
            (0..count).map(|block| info.get_block_freq(BlockId(block as u32))).collect()
        };
        let pinned = (0..count)
            .map(|at| {
                analyze(function, at).is_none()
                    && at + 1 < count
                    && !function.blocks[at].insts.last().is_some_and(|last| last.barrier)
            })
            .collect();
        let loops = loop_forest(&cfg);
        Graph { counts_jumps: false, succs, preds, probs, freqs, pinned, loops, size }
    }
}

/// `MachineLoopInfo`'s order: top-level loops by the post-order of their
/// headers, inner loops by reverse post-order; a loop's blocks are its header
/// and then reverse post-order.
fn loop_forest(cfg: &Cfg) -> Vec<Loop> {
    let count = cfg.succs.len();
    let mut post = vec![usize::MAX; count];
    let mut order = Vec::new();
    let mut seen = vec![false; count];
    let mut stack = vec![(0usize, 0usize)];
    if count > 0 {
        seen[0] = true;
    }
    while let Some((node, next)) = stack.pop() {
        if let Some(&child) = cfg.succs[node].get(next) {
            stack.push((node, next + 1));
            if !seen[child] {
                seen[child] = true;
                stack.push((child, 0));
            }
        } else {
            post[node] = order.len();
            order.push(node);
        }
    }
    let loops = cfg.loops();
    let parent = |one: usize| {
        (0..loops.len())
            .filter(|&other| {
                other != one && loops[other].1[loops[one].0] && count_of(&loops[other].1) > count_of(&loops[one].1)
            })
            .min_by_key(|&other| count_of(&loops[other].1))
    };
    fn count_of(body: &[bool]) -> usize {
        body.iter().filter(|one| **one).count()
    }
    let build = |at: usize| -> Vec<usize> {
        let (header, body) = (&loops[at].0, &loops[at].1);
        let mut blocks = vec![*header];
        blocks.extend(order.iter().rev().copied().filter(|&block| body[block] && block != *header));
        blocks
    };
    fn grow(
        at: usize,
        loops: &[(usize, Vec<bool>)],
        parents: &[Option<usize>],
        post: &[usize],
        build: &dyn Fn(usize) -> Vec<usize>,
    ) -> Loop {
        let mut children: Vec<usize> = (0..loops.len()).filter(|&other| parents[other] == Some(at)).collect();
        children.sort_by_key(|&child| std::cmp::Reverse(post[loops[child].0]));
        Loop {
            header: loops[at].0,
            blocks: build(at),
            inner: children.into_iter().map(|child| grow(child, loops, parents, post, build)).collect(),
        }
    }
    let parents: Vec<Option<usize>> = (0..loops.len()).map(parent).collect();
    let mut roots: Vec<usize> = (0..loops.len()).filter(|&one| parents[one].is_none()).collect();
    roots.sort_by_key(|&one| post[loops[one].0]);
    roots.into_iter().map(|root| grow(root, loops, &parents, &post, &build)).collect()
}

impl Layout for Graph {
    fn len(&self) -> usize {
        self.succs.len()
    }

    fn counts_jumps(&self) -> bool {
        self.counts_jumps
    }

    fn successors(
        &self,
        block: usize,
    ) -> &[usize] {
        &self.succs[block]
    }

    fn predecessors(
        &self,
        block: usize,
    ) -> &[usize] {
        &self.preds[block]
    }

    fn probability(
        &self,
        from: usize,
        to: usize,
    ) -> BranchProbability {
        self.succs[from]
            .iter()
            .position(|&one| one == to)
            .map_or(BranchProbability::get_zero(), |at| self.probs[from][at])
    }

    fn frequency(
        &self,
        block: usize,
    ) -> BlockFrequency {
        self.freqs[block]
    }

    fn keeps_fall_through(
        &self,
        block: usize,
    ) -> bool {
        self.pinned[block]
    }

    fn loops(&self) -> Vec<Loop> {
        self.loops.clone()
    }

    fn optimize_for_size(&self) -> bool {
        self.size
    }
}

/// `MachineBasicBlock::updateTerminator`: the branches of the block at `at`
/// after the layout changed. `old_next` is the block that followed it before.
fn update_terminator<C, J: Jump>(
    function: &mut MFunction<C>,
    at: usize,
    old_next: Option<u32>,
    successors: &[u32],
) {
    if successors.is_empty() {
        return;
    }
    let Some(Branches { taken, other, cond }) = analyze(function, at) else { return };
    let next = function.blocks.get(at + 1).map(|block| block.id);
    match (cond, taken, other) {
        (None, Some(taken), _) => {
            if next == Some(taken) {
                remove_branch(function, at);
            }
        }
        (None, None, _) => {
            let Some(previous) = old_next.filter(|previous| successors.contains(previous)) else { return };
            if next != Some(previous) {
                insert_branch::<C, J>(function, at, previous, None, None);
            }
        }
        (Some(cond), Some(taken), Some(other)) => {
            if next == Some(taken) {
                let Some(reversed) = J::reverse(&cond) else { return };
                remove_branch(function, at);
                insert_branch::<C, J>(function, at, other, None, Some(&reversed));
            } else if next == Some(other) {
                remove_branch(function, at);
                insert_branch::<C, J>(function, at, taken, None, Some(&cond));
            }
        }
        (Some(cond), Some(taken), None) => {
            let Some(previous) = old_next.filter(|previous| successors.contains(previous)) else { return };
            if previous == taken {
                remove_branch(function, at);
                if next != Some(taken) {
                    insert_branch::<C, J>(function, at, taken, None, None);
                }
                return;
            }
            if next == Some(taken) {
                let Some(reversed) = J::reverse(&cond) else {
                    remove_branch(function, at);
                    insert_branch::<C, J>(function, at, previous, None, None);
                    return;
                };
                remove_branch(function, at);
                insert_branch::<C, J>(function, at, previous, None, Some(&reversed));
            } else if next != Some(previous) {
                remove_branch(function, at);
                insert_branch::<C, J>(function, at, taken, Some(previous), Some(&cond));
            }
        }
        _ => {}
    }
}

/// `optimizeBranches`: a two-way branch goes first to the likelier successor.
fn optimize_branches<C, J: Jump>(
    function: &mut MFunction<C>,
    odds: &HashMap<(u32, u32), BranchProbability>,
) {
    for at in 0..function.blocks.len() {
        let Some(Branches { taken: Some(taken), other: Some(other), cond: Some(cond) }) = analyze(function, at) else {
            continue;
        };
        let id = function.blocks[at].id;
        let probability = |to: u32| odds.get(&(id, to)).copied().unwrap_or(BranchProbability::get_zero());
        if probability(taken) >= probability(other) {
            continue;
        }
        let Some(reversed) = J::reverse(&cond) else { continue };
        remove_branch(function, at);
        insert_branch::<C, J>(function, at, other, Some(taken), Some(&reversed));
    }
}

/// `MachineBlockPlacement::runOnMachineFunction`: lays the blocks out in
/// [`place`]'s order and fixes the branches. True when the order changed.
pub fn place_blocks<C, J: Jump>(
    function: &mut MFunction<C>,
    odds: &Odds,
    size: bool,
) -> bool {
    place_blocks_counting::<C, J>(function, odds, size, false)
}

/// [`place_blocks`] where `counts_jumps` says the target prices a taken jump
/// ([`Layout::counts_jumps`]).
pub fn place_blocks_counting<C, J: Jump>(
    function: &mut MFunction<C>,
    odds: &Odds,
    size: bool,
    counts_jumps: bool,
) -> bool {
    if function.blocks.len() < 2 || function.blocks.iter().any(|block| !block.phis.is_empty()) {
        return false;
    }
    let mut graph = Graph::of::<C, J>(function, odds, size);
    graph.counts_jumps = counts_jumps;
    let order = place(&graph);
    let identity = order.iter().enumerate().all(|(at, &block)| at == block);
    let ids: Vec<u32> = function.blocks.iter().map(|block| block.id).collect();
    let edge_odds: HashMap<(u32, u32), BranchProbability> = graph
        .succs
        .iter()
        .enumerate()
        .flat_map(|(from, list)| list.iter().enumerate().map(move |(at, &to)| (from, to, at)))
        .map(|(from, to, at)| ((ids[from], ids[to]), graph.probs[from][at]))
        .collect();
    if !identity {
        let successors: Vec<Vec<u32>> =
            graph.succs.iter().map(|list| list.iter().map(|&to| ids[to]).collect()).collect();
        let old_next: HashMap<u32, Option<u32>> =
            ids.iter().enumerate().map(|(at, &id)| (id, ids.get(at + 1).copied())).collect();
        let mut blocks: Vec<Option<crate::mfunc::Block>> =
            std::mem::take(&mut function.blocks).into_iter().map(Some).collect();
        function.blocks = order.iter().map(|&at| blocks[at].take().expect("each block once")).collect();
        // `updateTerminator` of every block: its layout successor changed
        for at in 0..function.blocks.len() {
            let id = function.blocks[at].id;
            let original = ids.iter().position(|&one| one == id).expect("a block that was there");
            update_terminator::<C, J>(function, at, old_next[&id], &successors[original]);
        }
    }
    if !size {
        optimize_branches::<C, J>(function, &edge_odds);
    }
    !identity
}
