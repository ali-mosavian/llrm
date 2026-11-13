//! `BranchFolder::TailMergeBlocks`: blocks that end in the same instructions
//! share one copy of them (BranchFolding.cpp, `ProfitableToMerge`,
//! `ComputeSameTails`, `CreateCommonTailOnlyBlock`, `TryTailMergeBlocks`,
//! `TailMergeBlocks`, pin d1106deb71cc).
//!
//! The algorithm is written once against [`TailMergeFunction`]; [`merge_tails`]
//! runs it over an [`MFunction`].
//!
//! DEVIATE: no EH scopes, debug locations, memory operands, block frequencies,
//! live-in lists or loop info (the spike has none), and only the pass before
//! block placement (`AfterBlockPlacement` false). A function with phis is left
//! alone: the pass runs after allocation, where there are none. Exit: the
//! neutral machine function carries them (neutral-lir-need.md).
//!
//! DEVIATE: blocks of different loops are never merged ([`TailMergeFunction::
//! loop_header`], checked first in `profitable` where the pin checks EH
//! scopes). The pin merges them and relies on `MachineBlockPlacement::
//! rotateLoop`
//! ([MachineBlockPlacement.cpp#L2423](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L2423))
//! to put the loop's test back at the bottom; for a loop with one entry and
//! one exit its `FallThrough2Top >= ExitFreq` ties and nothing rotates, so a
//! latch merged into the entry's compare stays a `jump` per trip (m68k gcd,
//! +20 clk at 68000). Reason: with the odds carried and the 6502's counted
//! rotation instead, gcd was 248 -> 256 clk and 75 seed cells were worse (the
//! entry jump costs more than the short loops save). Exit: 2026-11-14, when
//! odds and rotation price the entry jump against the trips.

use std::hash::Hash;

use llrm_support::hash::{HashMap, HashSet};

use crate::fold::{self, Branches};
use crate::mfunc::{Block, Inst, MFunction, Operand};
use crate::regalloc::Assignment;
use crate::sink::Jump;

/// `tail-merge-threshold`: the most blocks one round looks at.
const TAIL_MERGE_THRESHOLD: usize = 150;

/// `analyzeBranch`: where a block's terminators go.
pub struct Exits<B, C> {
    pub taken: Option<B>,
    pub other: Option<B>,
    pub cond: Option<C>,
}

/// What the algorithm asks of a function. A block is named by a stable value,
/// not a position: a split inserts a block.
pub trait TailMergeFunction {
    type Block: Copy + Eq + Ord + Hash;
    type Inst;
    /// `Cond`: a conditional branch, opaque.
    type Cond: Clone;

    /// Every block, in layout order.
    fn blocks(&self) -> Vec<Self::Block>;
    fn successors(
        &self,
        block: Self::Block,
    ) -> Vec<Self::Block>;
    fn predecessors(
        &self,
        block: Self::Block,
    ) -> Vec<Self::Block>;
    /// The whole block, terminators last.
    fn insts(
        &self,
        block: Self::Block,
    ) -> &[Self::Inst];
    /// `isIdenticalTo`.
    fn same_inst(
        &self,
        one: &Self::Inst,
        other: &Self::Inst,
    ) -> bool;
    /// `HashMachineInstr`: equal for instructions that are `same_inst`.
    fn hash_inst(
        &self,
        inst: &Self::Inst,
    ) -> u32;
    fn is_terminator(
        &self,
        inst: &Self::Inst,
    ) -> bool;
    fn is_barrier(
        &self,
        inst: &Self::Inst,
    ) -> bool;
    fn is_call(
        &self,
        inst: &Self::Inst,
    ) -> bool;
    fn may_load_or_store(
        &self,
        inst: &Self::Inst,
    ) -> bool;
    /// `isReturn() || isIndirectBranch()`.
    fn is_return_or_indirect(
        &self,
        inst: &Self::Inst,
    ) -> bool;
    /// `canFallThrough`.
    fn can_fall_through(
        &self,
        block: Self::Block,
    ) -> bool;
    /// `shouldOptimizeForSize`.
    fn optimize_for_size(
        &self,
        block: Self::Block,
    ) -> bool;
    /// `analyzeBranch`; `None` for a terminator it does not know.
    fn analyze_branch(
        &self,
        block: Self::Block,
    ) -> Option<Exits<Self::Block, Self::Cond>>;
    /// `reverseBranchCondition`.
    fn reverse_branch_condition(
        &self,
        cond: &Self::Cond,
    ) -> Option<Self::Cond>;
    /// `removeBranch`.
    fn remove_branch(
        &mut self,
        block: Self::Block,
    );
    /// `insertBranch`: `cond` to `taken` and a jump to `other`, or a jump to
    /// `taken`.
    fn insert_branch(
        &mut self,
        block: Self::Block,
        taken: Self::Block,
        other: Option<Self::Block>,
        cond: Option<&Self::Cond>,
    );
    /// `SplitMBBAt`: the instructions from `at` on move to a new block laid out
    /// next, which takes over the successors; the old block falls into it.
    /// `None` when the target cannot split there.
    fn split_block_at(
        &mut self,
        block: Self::Block,
        at: usize,
    ) -> Option<Self::Block>;
    /// `ReplaceTailWithBranchTo`: drops `block`'s instructions from `at` on and
    /// leaves it going to `dest`, by fall-through or a jump.
    fn replace_tail_with_branch_to(
        &mut self,
        block: Self::Block,
        at: usize,
        dest: Self::Block,
    );
    /// The header of the innermost loop `block` is in, `None` outside every
    /// loop.
    fn loop_header(
        &self,
        _block: Self::Block,
    ) -> Option<Self::Block> {
        None
    }
    /// Whether blocks `one` and `other` may merge as far as their loops go:
    /// those of one loop, or of loops the caller lets merge (`Crossing`).
    fn may_cross(
        &self,
        one: Self::Block,
        other: Self::Block,
    ) -> bool {
        self.loop_header(one) == self.loop_header(other)
    }
    /// `MBBFreqInfo.setBlockFreq` of the common tail: `from`'s tail now runs
    /// through `common`, which takes its frequency.
    fn add_tail_frequency(
        &mut self,
        _common: Self::Block,
        _from: Self::Block,
    ) {
    }
    /// `getTailMergeSize`.
    fn tail_merge_size(&self) -> usize {
        3
    }
}

struct Potential<B> {
    hash: u32,
    block: B,
}

/// A block of the set with the first instruction of its common tail.
#[derive(Clone, Copy)]
struct SameTail<B> {
    block: B,
    start: usize,
}

impl<B> SameTail<B> {
    fn whole_block(&self) -> bool {
        self.start == 0
    }
}

struct Merger<'a, F: TailMergeFunction> {
    function: &'a mut F,
    potentials: Vec<Potential<F::Block>>,
    same_tails: Vec<SameTail<F::Block>>,
    tried: HashSet<F::Block>,
}

/// `TailMergeBlocks`: `true` when anything merged.
pub fn tail_merge_blocks<F: TailMergeFunction>(function: &mut F) -> bool {
    let mut merger = Merger { function, potentials: Vec::new(), same_tails: Vec::new(), tried: HashSet::default() };
    merger.run()
}

impl<F: TailMergeFunction> Merger<'_, F> {
    fn layout_next(
        &self,
        block: F::Block,
    ) -> Option<F::Block> {
        let blocks = self.function.blocks();
        let at = blocks.iter().position(|one| *one == block)?;
        blocks.get(at + 1).copied()
    }

    fn layout_prev(
        &self,
        block: F::Block,
    ) -> Option<F::Block> {
        let blocks = self.function.blocks();
        let at = blocks.iter().position(|one| *one == block)?;
        at.checked_sub(1).map(|prev| blocks[prev])
    }

    /// `a->isLayoutSuccessor(b)`: `b` is laid out right after `a`.
    fn follows(
        &self,
        a: F::Block,
        b: F::Block,
    ) -> bool {
        self.layout_next(a) == Some(b)
    }

    /// `HashEndOfMBB`.
    fn hash_end(
        &self,
        block: F::Block,
    ) -> u32 {
        self.function.insts(block).last().map_or(0, |last| self.function.hash_inst(last))
    }

    fn count_terminators(
        &self,
        block: F::Block,
    ) -> usize {
        self.function.insts(block).iter().rev().take_while(|inst| self.function.is_terminator(inst)).count()
    }

    /// `blockEndsInUnreachable`.
    fn ends_in_unreachable(
        &self,
        block: F::Block,
    ) -> bool {
        if !self.function.successors(block).is_empty() {
            return false;
        }
        self.function.insts(block).last().is_none_or(|last| !self.function.is_return_or_indirect(last))
    }

    /// `EstimateRuntime`.
    fn estimate_runtime(
        &self,
        block: F::Block,
        end: usize,
    ) -> usize {
        self.function.insts(block)[..end]
            .iter()
            .map(|inst| {
                if self.function.is_call(inst) {
                    10
                } else if self.function.may_load_or_store(inst) {
                    2
                } else {
                    1
                }
            })
            .sum()
    }

    /// `ComputeCommonTailLength`: the length and where the tail starts in each.
    fn common_tail(
        &self,
        one: F::Block,
        other: F::Block,
    ) -> (usize, usize, usize) {
        let (a, b) = (self.function.insts(one), self.function.insts(other));
        let mut length = 0;
        while length < a.len() && length < b.len() {
            if !self.function.same_inst(&a[a.len() - 1 - length], &b[b.len() - 1 - length]) {
                break;
            }
            length += 1;
        }
        (length, a.len() - length, b.len() - length)
    }

    /// `ProfitableToMerge`: the common tail length and where it starts in each.
    fn profitable(
        &self,
        one: F::Block,
        other: F::Block,
        min_tail: usize,
        succ: Option<F::Block>,
        pred: Option<F::Block>,
    ) -> Option<(usize, usize, usize)> {
        if !self.function.may_cross(one, other) {
            return None;
        }
        let (length, start1, start2) = self.common_tail(one, other);
        if length == 0 {
            return None;
        }
        let (full1, full2) = (start1 == 0, start2 == 0);
        let found = Some((length, start1, start2));

        // the block that falls through into the common successor takes any
        // number of non-terminators
        if pred == Some(one) || pred == Some(other) {
            let other_of_pred = if pred == Some(one) { other } else { one };
            if length > self.count_terminators(other_of_pred) {
                return found;
            }
        }
        // identical cold blocks with no way out
        if full1 && full2 && self.ends_in_unreachable(one) && self.ends_in_unreachable(other) {
            return found;
        }
        // one is the whole tail and the other can fall into it
        if self.follows(one, other) && full2 || self.follows(other, one) && full1 {
            return found;
        }
        // an unconditional branch taken out counts as one more in common
        let mut effective = length;
        if succ.is_some()
            && pred != Some(one)
            && pred != Some(other)
            && !self.function.insts(one).last().is_some_and(|inst| self.function.is_barrier(inst))
            && !self.function.insts(other).last().is_some_and(|inst| self.function.is_barrier(inst))
        {
            effective += 1;
        }
        if effective >= min_tail {
            return found;
        }
        // for size, two in common is enough when no block is split
        let for_size = self.function.optimize_for_size(one) && self.function.optimize_for_size(other);
        (effective >= 2 && for_size && (full1 || full2)).then_some((length, start1, start2))
    }

    /// `ComputeSameTails`: the longest tail and every block that has it.
    fn compute_same_tails(
        &mut self,
        hash: u32,
        min_tail: usize,
        succ: Option<F::Block>,
        pred: Option<F::Block>,
    ) {
        let mut longest = 0;
        self.same_tails.clear();
        let mut highest = self.potentials.len() - 1;
        let mut cur = self.potentials.len() - 1;
        while cur > 0 && self.potentials[cur].hash == hash {
            let mut i = cur - 1;
            while self.potentials[i].hash == hash {
                let (block_cur, block_i) = (self.potentials[cur].block, self.potentials[i].block);
                if let Some((length, start_cur, start_i)) = self.profitable(block_cur, block_i, min_tail, succ, pred) {
                    if length > longest {
                        self.same_tails.clear();
                        longest = length;
                        highest = cur;
                        self.same_tails.push(SameTail { block: block_cur, start: start_cur });
                    }
                    if highest == cur && length == longest {
                        self.same_tails.push(SameTail { block: block_i, start: start_i });
                    }
                }
                if i == 0 {
                    break;
                }
                i -= 1;
            }
            cur -= 1;
        }
    }

    /// `FixTail`: puts the branch to `succ` back at the end of `block`.
    fn fix_tail(
        &mut self,
        block: F::Block,
        succ: F::Block,
    ) {
        if self.layout_next(block).is_some()
            && let Some(exits) = self.function.analyze_branch(block)
        {
            let next = self.layout_next(block);
            if exits.taken == next
                && exits.other.is_none()
                && let Some(cond) = exits.cond.as_ref().and_then(|cond| self.function.reverse_branch_condition(cond))
            {
                self.function.remove_branch(block);
                self.function.insert_branch(block, succ, None, Some(&cond));
                return;
            }
        }
        self.function.insert_branch(block, succ, None, None);
    }

    /// `RemoveBlocksWithHash`.
    fn remove_blocks_with_hash(
        &mut self,
        hash: u32,
        succ: Option<F::Block>,
        pred: Option<F::Block>,
    ) {
        let first = self.potentials.iter().position(|one| one.hash == hash).expect("the hash is in the set");
        let blocks: Vec<F::Block> = self.potentials[first..].iter().rev().map(|one| one.block).collect();
        for block in blocks {
            if let Some(succ) = succ
                && Some(block) != pred
            {
                self.fix_tail(block, succ);
            }
        }
        self.potentials.truncate(first);
    }

    /// `CreateCommonTailOnlyBlock`: the index of the block that now is the
    /// common tail, or `None` when the split failed.
    fn create_common_tail_only_block(
        &mut self,
        pred: &mut Option<F::Block>,
    ) -> Option<usize> {
        let mut index = 0;
        let mut time = usize::MAX;
        for (i, tail) in self.same_tails.iter().enumerate() {
            if Some(tail.block) == *pred {
                index = i;
                break;
            }
            let estimate = self.estimate_runtime(tail.block, tail.start);
            if estimate <= time {
                time = estimate;
                index = i;
            }
        }
        let SameTail { block, start } = self.same_tails[index];
        let new = self.function.split_block_at(block, start)?;
        self.same_tails[index] = SameTail { block: new, start: 0 };
        // `SameTailElt::setBlock` writes through to the worklist element
        if let Some(element) = self.potentials.iter_mut().find(|one| one.block == block) {
            element.block = new;
        }
        if *pred == Some(block) {
            *pred = Some(new);
        }
        Some(index)
    }

    /// `TryTailMergeBlocks`.
    fn try_tail_merge(
        &mut self,
        succ: Option<F::Block>,
        mut pred: Option<F::Block>,
        min_tail: usize,
    ) -> bool {
        let mut changed = false;
        self.potentials.sort_by_key(|one| (one.hash, one.block));
        while self.potentials.len() > 1 {
            let hash = self.potentials.last().expect("two or more").hash;
            self.compute_same_tails(hash, min_tail, succ, pred);
            if self.same_tails.is_empty() {
                self.remove_blocks_with_hash(hash, succ, pred);
                continue;
            }
            let entry = self.function.blocks()[0];
            let tails = self.same_tails.clone();
            let mut index = tails.len();
            if tails.len() == 2 && self.follows(tails[0].block, tails[1].block) && tails[1].whole_block() {
                index = 1;
            } else if tails.len() == 2 && self.follows(tails[1].block, tails[0].block) && tails[0].whole_block() {
                index = 0;
            } else {
                for (i, tail) in tails.iter().enumerate() {
                    if tail.block == entry && tail.whole_block() {
                        continue;
                    }
                    if Some(tail.block) == pred {
                        index = i;
                        break;
                    }
                    if tail.whole_block() {
                        index = i;
                    }
                }
            }
            if index == tails.len() || (Some(tails[index].block) == pred && !tails[index].whole_block()) {
                match self.create_common_tail_only_block(&mut pred) {
                    Some(made) => index = made,
                    None => {
                        self.remove_blocks_with_hash(hash, succ, pred);
                        continue;
                    }
                }
            }
            let tails = self.same_tails.clone();
            let common = tails[index].block;
            for (i, tail) in tails.iter().enumerate() {
                if i == index {
                    continue;
                }
                self.function.add_tail_frequency(common, tail.block);
                self.function.replace_tail_with_branch_to(tail.block, tail.start, common);
                self.potentials.retain(|one| one.block != tail.block);
            }
            changed = true;
        }
        changed
    }

    fn run(&mut self) -> bool {
        let mut changed = false;
        let min_tail = self.function.tail_merge_size();

        // blocks with no successors
        self.potentials.clear();
        for block in self.function.blocks() {
            if self.potentials.len() == TAIL_MERGE_THRESHOLD {
                break;
            }
            if !self.tried.contains(&block) && self.function.successors(block).is_empty() {
                let hash = self.hash_end(block);
                self.potentials.push(Potential { hash, block });
            }
        }
        if self.potentials.len() == TAIL_MERGE_THRESHOLD {
            self.tried.extend(self.potentials.iter().map(|one| one.block));
        }
        if self.potentials.len() >= 2 {
            changed |= self.try_tail_merge(None, None, min_tail);
        }

        // blocks with several predecessors, each predecessor in canonical form
        let mut cursor = self.function.blocks().first().and_then(|entry| self.layout_next(*entry));
        while let Some(ibb) = cursor {
            cursor = self.layout_next(ibb);
            if self.function.predecessors(ibb).len() < 2 {
                continue;
            }
            let pred_bb = self.layout_prev(ibb);
            self.potentials.clear();
            let mut unique = HashSet::default();
            for pbb in self.function.predecessors(ibb) {
                if self.potentials.len() == TAIL_MERGE_THRESHOLD {
                    break;
                }
                if self.tried.contains(&pbb) || pbb == ibb || !unique.insert(pbb) {
                    continue;
                }
                let Some(exits) = self.function.analyze_branch(pbb) else { continue };
                let mut new_cond = exits.cond.clone();
                let mut fbb = exits.other;
                if let Some(cond) = &exits.cond
                    && exits.taken == Some(ibb)
                {
                    let Some(reversed) = self.function.reverse_branch_condition(cond) else { continue };
                    new_cond = Some(reversed);
                    if fbb.is_none() {
                        fbb = self.layout_next(pbb);
                    }
                }
                // take the unconditional branch off; keep the conditional one
                if exits.taken.is_some() && (exits.cond.is_none() || fbb.is_some()) {
                    self.function.remove_branch(pbb);
                    if let Some(cond) = &new_cond {
                        let to = if exits.taken == Some(ibb) { fbb } else { exits.taken };
                        if let Some(to) = to {
                            self.function.insert_branch(pbb, to, None, Some(cond));
                        }
                    }
                }
                let hash = self.hash_end(pbb);
                self.potentials.push(Potential { hash, block: pbb });
            }
            if self.potentials.len() == TAIL_MERGE_THRESHOLD {
                self.tried.extend(self.potentials.iter().map(|one| one.block));
            }
            if self.potentials.len() >= 2 {
                changed |= self.try_tail_merge(Some(ibb), pred_bb, min_tail);
            }
            // the unconditional branch comes back when a lone block is left
            let pred_bb = self.layout_prev(ibb);
            if self.potentials.len() == 1 && Some(self.potentials[0].block) != pred_bb {
                let lone = self.potentials[0].block;
                self.fix_tail(lone, ibb);
            }
        }
        changed
    }
}

/// [`TailMergeFunction`] over an [`MFunction`] whose terminators are `J`'s.
struct Mf<'a, C, J> {
    function: &'a mut MFunction<C>,
    /// Where the registers are, when the instructions still name virtual ones.
    assignment: Option<&'a Assignment>,
    /// The unconditional jump `remove_branch` took off a block, to put back as
    /// it was: the target's own opcode for it (the 65C02's `bra`), not `J`'s.
    removed: HashMap<u32, Inst>,
    /// Block to the header of its innermost loop, from the blocks at the start;
    /// a split block takes its origin's.
    loops: HashMap<u32, u32>,
    /// The loops (by header) whose blocks may merge with those of other loops:
    /// [`Crossing::Loops`].
    crossing: Option<HashSet<u32>>,
    /// Block frequencies by id, kept as `BranchFolder` keeps `MBBFreqInfo`: a
    /// split block's tail takes its frequency and a common tail adds those
    /// of the tails merged into it.
    frequencies: Option<&'a mut HashMap<u32, f64>>,
    /// A loop's header to the header of the loop around it, for the loops that
    /// have one.
    enclosing: HashMap<u32, u32>,
    /// Positions and edges of the blocks as they are, built on the first ask
    /// after a change (LLVM keeps these on the block).
    graph: std::cell::RefCell<Option<Graph>>,
    jump: std::marker::PhantomData<J>,
}

/// Where each block is and which blocks reach it, as flat tables indexed by
/// block id: a rebuild costs a few allocations, not one per block.
struct Graph {
    position: Vec<usize>,
    /// The predecessors of block `b` are `from[start[b]..start[b + 1]]`, in
    /// layout order.
    start: Vec<u32>,
    from: Vec<u32>,
}

const ABSENT: usize = usize::MAX;

impl<C, J> Mf<'_, C, J> {
    fn with_graph<R>(
        &self,
        read: impl FnOnce(&Graph) -> R,
    ) -> R {
        let mut cached = self.graph.borrow_mut();
        let graph = cached.get_or_insert_with(|| self.build_graph());
        read(graph)
    }

    /// The edges out of the block at `at`, sorted, without repeats: its jumps
    /// and, when it can, the block after it.
    fn edges(
        &self,
        at: usize,
        out: &mut Vec<u32>,
    ) {
        let blocks = &self.function.blocks;
        out.clear();
        out.extend(blocks[at].insts.iter().filter(|inst| inst.terminator).filter_map(targets));
        if at + 1 < blocks.len() && !blocks[at].insts.last().is_some_and(|last| last.barrier) {
            out.push(blocks[at + 1].id);
        }
        out.sort_unstable();
        out.dedup();
    }

    fn build_graph(&self) -> Graph {
        let blocks = &self.function.blocks;
        let size = blocks.iter().map(|block| block.id as usize + 1).max().unwrap_or(0);
        let mut position = vec![ABSENT; size];
        for (at, block) in blocks.iter().enumerate() {
            position[block.id as usize] = at;
        }
        let mut edges: Vec<(u32, u32)> = Vec::new();
        let mut out = Vec::new();
        for (at, block) in blocks.iter().enumerate() {
            self.edges(at, &mut out);
            // a jump to a block that is not there reaches no one
            edges.extend(out.iter().filter(|&&to| (to as usize) < size).map(|&to| (to, block.id)));
        }
        let mut start = vec![0u32; size + 1];
        for &(to, _) in &edges {
            start[to as usize + 1] += 1;
        }
        for b in 0..size {
            start[b + 1] += start[b];
        }
        // filled in edge order, which is layout order of the sources
        let mut next = start.clone();
        let mut from = vec![0u32; edges.len()];
        for &(to, source) in &edges {
            from[next[to as usize] as usize] = source;
            next[to as usize] += 1;
        }
        Graph { position, start, from }
    }

    /// The function changed: the next question rebuilds the graph.
    fn changed(&mut self) {
        *self.graph.get_mut() = None;
    }

    fn at(
        &self,
        id: u32,
    ) -> usize {
        self.with_graph(|graph| graph.position.get(id as usize).copied().filter(|&at| at != ABSENT))
            .expect("a block that exists")
    }
}

/// The block a jump names; none for a return or a multiway indirect branch
/// (a table jump names every block of its table).
fn targets(inst: &Inst) -> Option<u32> {
    if fold::names_many(inst) {
        return None;
    }
    inst.operands
        .iter()
        .find_map(
            |operand| match operand {
                Operand::Block(id) => Some(*id),
                _ => None,
            },
        )
}

impl<C, J: Jump> TailMergeFunction for Mf<'_, C, J> {
    type Block = u32;
    type Inst = Inst;
    type Cond = Inst;

    fn blocks(&self) -> Vec<u32> {
        self.function.blocks.iter().map(|block| block.id).collect()
    }

    fn successors(
        &self,
        block: u32,
    ) -> Vec<u32> {
        let mut out = Vec::new();
        self.edges(self.at(block), &mut out);
        out
    }

    fn predecessors(
        &self,
        block: u32,
    ) -> Vec<u32> {
        self.with_graph(|graph| match (graph.start.get(block as usize), graph.start.get(block as usize + 1)) {
            (Some(&from), Some(&to)) => graph.from[from as usize..to as usize].to_vec(),
            _ => Vec::new(),
        })
    }

    fn insts(
        &self,
        block: u32,
    ) -> &[Inst] {
        &self.function.blocks[self.at(block)].insts
    }

    fn same_inst(
        &self,
        one: &Inst,
        other: &Inst,
    ) -> bool {
        match self.assignment {
            Some(assignment) => fold::identical(one, other, assignment),
            None => {
                one.kind == other.kind
                    && one.sets_flags() == other.sets_flags()
                    && one.reads_flags() == other.reads_flags()
                    && one.operands == other.operands
            }
        }
    }

    fn hash_inst(
        &self,
        inst: &Inst,
    ) -> u32 {
        let operands: Vec<String> = inst
            .operands
            .iter()
            .map(|operand| match (operand, self.assignment) {
                (Operand::Def(reg), Some(assignment)) => format!("d{:?}", fold::resolve(*reg, assignment)),
                (Operand::Use(reg), Some(assignment)) => format!("u{:?}", fold::resolve(*reg, assignment)),
                (operand, _) => format!("{operand:?}"),
            })
            .collect();
        format!("{:?}{operands:?}", inst.kind)
            .bytes()
            .fold(2166136261u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(16777619))
    }

    fn is_terminator(
        &self,
        inst: &Inst,
    ) -> bool {
        inst.terminator
    }

    fn is_barrier(
        &self,
        inst: &Inst,
    ) -> bool {
        inst.barrier
    }

    fn is_call(
        &self,
        inst: &Inst,
    ) -> bool {
        inst.call
    }

    fn may_load_or_store(
        &self,
        inst: &Inst,
    ) -> bool {
        inst.may_load || inst.may_store
    }

    fn is_return_or_indirect(
        &self,
        inst: &Inst,
    ) -> bool {
        inst.terminator && inst.barrier && targets(inst).is_none()
    }

    fn can_fall_through(
        &self,
        block: u32,
    ) -> bool {
        let at = self.at(block);
        at + 1 < self.function.blocks.len() && !self.function.blocks[at].insts.last().is_some_and(|last| last.barrier)
    }

    fn optimize_for_size(
        &self,
        _block: u32,
    ) -> bool {
        self.function.optimize_for_size
    }

    fn analyze_branch(
        &self,
        block: u32,
    ) -> Option<Exits<u32, Inst>> {
        let Branches { taken, other, cond } = fold::analyze(self.function, self.at(block))?;
        Some(Exits { taken, other, cond })
    }

    fn reverse_branch_condition(
        &self,
        cond: &Inst,
    ) -> Option<Inst> {
        J::reverse(cond)
    }

    fn remove_branch(
        &mut self,
        block: u32,
    ) {
        let at = self.at(block);
        if let Some(jump) = self.function.blocks[at]
            .insts
            .last()
            .filter(|inst| inst.terminator && inst.barrier && targets(inst).is_some())
        {
            self.removed.insert(block, jump.clone());
        }
        fold::remove_branch(self.function, at);
        self.changed();
    }

    fn insert_branch(
        &mut self,
        block: u32,
        taken: u32,
        other: Option<u32>,
        cond: Option<&Inst>,
    ) {
        let at = self.at(block);
        if cond.is_none()
            && let Some(mut jump) = self.removed.remove(&block)
        {
            for operand in &mut jump.operands {
                if matches!(operand, Operand::Block(_)) {
                    *operand = Operand::Block(taken);
                }
            }
            self.function.blocks[at].insts.push(jump);
            self.changed();
            return;
        }
        fold::insert_branch::<C, J>(self.function, at, taken, other, cond);
        self.changed();
    }

    fn loop_header(
        &self,
        block: u32,
    ) -> Option<u32> {
        self.loops.get(&block).copied()
    }

    fn may_cross(
        &self,
        one: u32,
        other: u32,
    ) -> bool {
        let (x, y) = (self.loop_header(one), self.loop_header(other));
        // A hot loop's own blocks join those of the loop around it (its guard
        // and latch), and no others.
        let guards = |inner: Option<u32>, outer: Option<u32>| {
            inner.is_some_and(|header| {
                self.crossing.as_ref().is_some_and(|hot| hot.contains(&header))
                    && self.enclosing.get(&header).copied() == outer
            })
        };
        x == y || guards(x, y) || guards(y, x)
    }

    fn split_block_at(
        &mut self,
        block: u32,
        at: usize,
    ) -> Option<u32> {
        let position = self.at(block);
        let id = self.function.fresh_block_id();
        if let Some(header) = self.loops.get(&block).copied() {
            self.loops.insert(id, header);
        }
        if let Some(frequency) = self.frequencies.as_deref().and_then(|all| all.get(&block).copied()) {
            self.frequencies.as_deref_mut().expect("read above").insert(id, frequency);
        }
        let tail = self.function.blocks[position].insts.split_off(at);
        self.function.insert_block(position + 1, Block { id, insts: tail, phis: Vec::new() });
        self.changed();
        Some(id)
    }

    fn add_tail_frequency(
        &mut self,
        common: u32,
        from: u32,
    ) {
        if let Some(all) = self.frequencies.as_deref_mut() {
            if let Some(added) = all.get(&from).copied() {
                *all.entry(common).or_insert(0.0) += added;
            }
        }
    }

    fn replace_tail_with_branch_to(
        &mut self,
        block: u32,
        at: usize,
        dest: u32,
    ) {
        let position = self.at(block);
        self.function.blocks[position].insts.truncate(at);
        if self.function.blocks.get(position + 1).map(|next| next.id) != Some(dest) {
            fold::insert_branch::<C, J>(self.function, position, dest, None, None);
        }
        self.changed();
    }
}

/// `TailMergeBlocks` over a machine function after allocation: `true` when a
/// tail was shared. Does nothing when a block has phis.
pub fn merge_tails<J: Jump, C>(
    function: &mut MFunction<C>,
    assignment: Option<&Assignment>,
) -> bool {
    merge_tails_across_loops::<J, C>(function, assignment, false)
}

/// Which loops' blocks tail merging may join with others'.
#[derive(Clone, Copy)]
pub enum Crossing<'a> {
    /// Those of one loop only.
    Never,
    /// Any, as the pin's `BranchFolder` does before block placement.
    Always,
    /// Those of the loops with these headers, with blocks of other loops or of
    /// none.
    Loops(&'a HashSet<u32>),
}

/// [`merge_tails`], and with `across_loops` blocks of different loops merge as
/// the pin's `BranchFolder` does before block placement. The merged guard and
/// latch of a loop leave its test at the bottom behind one entry jump: bytes
/// for clocks, so the caller says where bytes outrank clocks.
pub fn merge_tails_across_loops<J: Jump, C>(
    function: &mut MFunction<C>,
    assignment: Option<&Assignment>,
    across_loops: bool,
) -> bool {
    merge_tails_crossing::<J, C>(
        function,
        assignment,
        if across_loops { Crossing::Always } else { Crossing::Never },
        None,
    )
}

/// [`merge_tails_across_loops`] with the loops that may merge named: the entry
/// jump costs a loop's entry once and saves its test a trip each, so the caller
/// names the loops that run more than once (the pin leaves it to `rotateLoop`).
pub fn merge_tails_crossing<J: Jump, C>(
    function: &mut MFunction<C>,
    assignment: Option<&Assignment>,
    crossing: Crossing<'_>,
    frequencies: Option<&mut HashMap<u32, f64>>,
) -> bool {
    if function.blocks.iter().any(|block| !block.phis.is_empty()) {
        return false;
    }
    let mut mf = Mf::<C, J> {
        function,
        assignment,
        removed: HashMap::default(),
        loops: HashMap::default(),
        enclosing: HashMap::default(),
        frequencies,
        crossing: match crossing {
            Crossing::Loops(hot) => Some(hot.clone()),
            _ => None,
        },
        graph: std::cell::RefCell::new(None),
        jump: std::marker::PhantomData,
    };
    if !matches!(crossing, Crossing::Always) {
        (mf.loops, mf.enclosing) = innermost_loops(&mf.blocks(), |block| mf.successors(block));
    }
    tail_merge_blocks(&mut mf)
}

/// The header of the innermost natural loop each block is in: a retreating edge
/// of a depth-first walk from the first block closes a loop, whose body is
/// what reaches the edge's source without passing the header.
fn innermost_loops(
    blocks: &[u32],
    successors: impl Fn(u32) -> Vec<u32>,
) -> (HashMap<u32, u32>, HashMap<u32, u32>) {
    let Some(&entry) = blocks.first() else { return Default::default() };
    let succ: HashMap<u32, Vec<u32>> = blocks.iter().map(|&b| (b, successors(b))).collect();
    let mut preds: HashMap<u32, Vec<u32>> = HashMap::default();
    for (&b, out) in &succ {
        for &s in out {
            preds.entry(s).or_default().push(b);
        }
    }
    let mut back_edges: Vec<(u32, u32)> = Vec::new();
    // true while on the stack
    let mut state: HashMap<u32, bool> = HashMap::default();
    let mut stack: Vec<(u32, usize)> = vec![(entry, 0)];
    state.insert(entry, true);
    while let Some(&mut (b, ref mut next)) = stack.last_mut() {
        if let Some(&s) = succ[&b].get(*next) {
            *next += 1;
            match state.get(&s) {
                None => {
                    state.insert(s, true);
                    stack.push((s, 0));
                }
                Some(true) => back_edges.push((b, s)),
                Some(false) => {}
            }
        } else {
            state.insert(b, false);
            stack.pop();
        }
    }
    let mut bodies: HashMap<u32, HashSet<u32>> = HashMap::default();
    for (latch, header) in back_edges {
        let body = bodies.entry(header).or_default();
        body.insert(header);
        let mut work = vec![latch];
        while let Some(b) = work.pop() {
            if body.insert(b) {
                work.extend(preds.get(&b).into_iter().flatten().copied());
            }
        }
    }
    let mut innermost: HashMap<u32, u32> = HashMap::default();
    for (&header, body) in &bodies {
        for &b in body {
            let smaller = innermost.get(&b).is_none_or(|current| body.len() < bodies[current].len());
            if smaller {
                innermost.insert(b, header);
            }
        }
    }
    // the loop around a loop: the smallest other body holding its header
    let mut enclosing: HashMap<u32, u32> = HashMap::default();
    for &header in bodies.keys() {
        let around = bodies
            .iter()
            .filter(|&(&other, body)| other != header && body.contains(&header))
            .min_by_key(|&(&other, body)| (body.len(), other))
            .map(|(&other, _)| other);
        if let Some(around) = around {
            enclosing.insert(header, around);
        }
    }
    (innermost, enclosing)
}
