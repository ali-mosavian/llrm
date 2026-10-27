//! `BranchProbabilityInfo` of a MIR function: LLVM's static branch
//! probabilities, translated from llvm/lib/Analysis/BranchProbabilityInfo.cpp
//! at d1106deb71cc.
//!
//! Each block of two or more successors takes the first of: its
//! `branch_weights` metadata, the estimated block weights (unreachable,
//! noreturn, unwind and cold blocks, loop exits scaled by a trip count), the
//! pointer, zero and floating-point heuristics. A block none of them speaks for
//! has 1/N on each edge.
//!
//! DEVIATE B1: `TargetLibraryInfo` names the routines whose result is a
//! three-way compare (`strcmp` and its kin); here the language states it of the
//! routine (the `threeway` fact). Exit: none, the fact is llrm's form of it.
//!
//! DEVIATE B2: `getTerminatingDeoptimizeCall`: MIR has no deoptimize
//! intrinsic, so a block is unreachable only by its `unreachable`.
//!
//! # Example
//! `pointer` in `bpi_golden.ll`, `if (p == NULL)`: the pointer heuristic takes
//! the equal edge 12 times in 32, as `opt -passes='print<branch-prob>'` does:
//!
//! ```text
//! edge %entry -> %yes probability is 0x30000000 / 0x80000000 = 37.50%
//! edge %entry -> %no probability is 0x50000000 / 0x80000000 = 62.50%
//! ```
//!
//! From unit test `test_probabilities_are_llvms_on_every_heuristic` in
//! `bpi_tests.rs`.
//!
//! # Used by
//! `blockfreq` (`BlockFrequencyInfo`'s edge probabilities, once its callers
//! move from `branchprob`).

use llrm_mir::blockfrequency::{BranchProbability, EdgeProbabilities};
use llrm_mir::constantfolding::{self, Env};
use llrm_mir::context::{ConstantId, ConstantKind, Context};
use llrm_mir::cycleinfo::{CycleInfo, CycleRef};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::domtree::{DomTree, PostDomTree};
use llrm_mir::facts::Fact;
use llrm_mir::loopinfo::{children, inverse_children};
use llrm_mir::module::{BlockId, Function, MetadataNode, MetadataOperand, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Analysis};
use llrm_mir::types::Type;
use llrm_support::hash::HashMap;

use crate::effects::{self, Declarations};

// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L94>
const LBH_TAKEN_WEIGHT: u32 = 124;
const LBH_NONTAKEN_WEIGHT: u32 = 4;

const UR_TAKEN_PROB: BranchProbability = BranchProbability::get_raw(1);

const PH_TAKEN_WEIGHT: u32 = 20;
const PH_NONTAKEN_WEIGHT: u32 = 12;
const ZH_TAKEN_WEIGHT: u32 = 20;
const ZH_NONTAKEN_WEIGHT: u32 = 12;
const FPH_TAKEN_WEIGHT: u32 = 20;
const FPH_NONTAKEN_WEIGHT: u32 = 12;
const FPH_ORD_WEIGHT: u32 = 1024 * 1024 - 1;
const FPH_UNO_WEIGHT: u32 = 1;

fn prob(
    n: u32,
    d: u32,
) -> BranchProbability {
    BranchProbability::new(n, d)
}

fn ptr_taken() -> BranchProbability {
    prob(PH_TAKEN_WEIGHT, PH_TAKEN_WEIGHT + PH_NONTAKEN_WEIGHT)
}
fn ptr_untaken() -> BranchProbability {
    prob(PH_NONTAKEN_WEIGHT, PH_TAKEN_WEIGHT + PH_NONTAKEN_WEIGHT)
}
fn zero_taken() -> BranchProbability {
    prob(ZH_TAKEN_WEIGHT, ZH_TAKEN_WEIGHT + ZH_NONTAKEN_WEIGHT)
}
fn zero_untaken() -> BranchProbability {
    prob(ZH_NONTAKEN_WEIGHT, ZH_TAKEN_WEIGHT + ZH_NONTAKEN_WEIGHT)
}
fn fp_ord_taken() -> BranchProbability {
    prob(FPH_ORD_WEIGHT, FPH_ORD_WEIGHT + FPH_UNO_WEIGHT)
}
fn fp_ord_untaken() -> BranchProbability {
    prob(FPH_UNO_WEIGHT, FPH_ORD_WEIGHT + FPH_UNO_WEIGHT)
}
fn fp_taken() -> BranchProbability {
    prob(FPH_TAKEN_WEIGHT, FPH_TAKEN_WEIGHT + FPH_NONTAKEN_WEIGHT)
}
fn fp_untaken() -> BranchProbability {
    prob(FPH_NONTAKEN_WEIGHT, FPH_TAKEN_WEIGHT + FPH_NONTAKEN_WEIGHT)
}

/// `BlockExecWeight`.
mod weight {
    pub const ZERO: u32 = 0x0;
    pub const LOWEST_NON_ZERO: u32 = 0x1;
    pub const UNREACHABLE: u32 = ZERO;
    pub const NORETURN: u32 = LOWEST_NON_ZERO;
    pub const UNWIND: u32 = LOWEST_NON_ZERO;
    pub const COLD: u32 = u16::MAX as u32;
    pub const DEFAULT: u32 = 0xfffff;
}

/// What a function's probabilities are worked out from: its module's
/// constants, declarations, metadata and layout.
#[derive(Clone, Copy)]
pub struct Module<'a> {
    pub context: &'a Context,
    pub declarations: &'a Declarations,
    pub metadata: &'a [MetadataNode],
    pub layout: &'a DataLayout,
}

/// The probabilities of each block's edges, by successor index; none where no
/// heuristic set them (each edge is then 1/N).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BranchProbabilityInfo {
    probs: Vec<Option<Vec<BranchProbability>>>,
}

impl BranchProbabilityInfo {
    /// `calculate`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L981>
    pub fn of(
        module: Module,
        function: &Function,
    ) -> Self {
        let mut bpi = Self { probs: vec![None; function.block_capacity()] };
        let Some(entry) = function.entry() else { return bpi };
        let cycles = CycleInfo::compute(function);
        let dominators = DomTree::of(function);
        let mut post_dominators = PostDomTree::new();
        post_dominators.recalculate(function);
        let mut construction = Construction {
            module,
            function,
            cycles: &cycles,
            block_weight: IdMap::new(),
            loop_weight: HashMap::default(),
        };
        let (post, rpo) = orders(function, entry);
        construction.estimate_block_weights(&rpo, &dominators, &post_dominators);
        for &block in &post {
            let successors: Vec<BlockId> = children(function, block).collect();
            if successors.len() < 2 {
                continue;
            }
            let found = construction
                .calc_metadata_weights(block, &successors)
                .or_else(|| construction.calc_estimated_heuristics(block, &successors))
                .or_else(|| construction.calc_pointer_heuristics(block))
                .or_else(|| construction.calc_zero_heuristics(block))
                .or_else(|| construction.calc_floating_point_heuristics(block));
            if let Some(probs) = found {
                bpi.probs[block.0 as usize] = Some(probs);
            }
        }
        bpi
    }

    /// `getEdgeProbability(Src, IndexInSuccessors)`.
    pub fn get_edge_probability(
        &self,
        function: &Function,
        src: BlockId,
        index: usize,
    ) -> BranchProbability {
        match self.probs.get(src.0 as usize).and_then(Option::as_ref) {
            Some(probs) => probs[index],
            None => BranchProbability::new(1, children(function, src).count() as u32),
        }
    }

    /// `getEdgeProbability(Src, Dst)`: the sum over the edges from `src` to
    /// `dst`.
    pub fn get_edge_probability_to(
        &self,
        function: &Function,
        src: BlockId,
        dst: BlockId,
    ) -> BranchProbability {
        let successors: Vec<BlockId> = children(function, src).collect();
        match self.probs.get(src.0 as usize).and_then(Option::as_ref) {
            None => BranchProbability::new(
                successors.iter().filter(|&&one| one == dst).count() as u32,
                successors.len() as u32,
            ),
            Some(probs) => successors
                .iter()
                .zip(probs)
                .filter(|(one, _)| **one == dst)
                .fold(BranchProbability::get_zero(), |sum, (_, &p)| sum + p),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L1025
    /// `allocEdges`: room for `src`'s edges, none if it has no successors.
    fn alloc_edges(
        &mut self,
        function: &Function,
        src: BlockId,
    ) -> Option<&mut Vec<BranchProbability>> {
        let successors = children(function, src).count();
        if successors == 0 {
            self.erase_block(src);
            return None;
        }
        if self.probs.len() <= src.0 as usize {
            self.probs.resize(function.block_capacity().max(src.0 as usize + 1), None);
        }
        let slot = &mut self.probs[src.0 as usize];
        *slot = Some(vec![BranchProbability::get_zero(); successors]);
        slot.as_mut()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L1115
    /// `setEdgeProbability`: all of `src`'s edges at once, one per successor.
    pub fn set_edge_probability(
        &mut self,
        function: &Function,
        src: BlockId,
        probs: &[BranchProbability],
    ) {
        assert_eq!(children(function, src).count(), probs.len(), "one probability per successor");
        let Some(edges) = self.alloc_edges(function, src) else { return };
        edges.copy_from_slice(probs);
        // Rounding keeps the total within one part in 2^31 per edge of 1.
        let total: u64 = probs.iter().map(|one| u64::from(one.get_numerator())).sum();
        let one = u64::from(BranchProbability::get_denominator());
        debug_assert!(total <= one + probs.len() as u64 && total + probs.len() as u64 >= one);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L1140
    /// `copyEdgeProbabilities`: `dst` takes `src`'s edges (the same number of
    /// successors); with none stored for `src`, none for `dst`.
    pub fn copy_edge_probabilities(
        &mut self,
        function: &Function,
        src: BlockId,
        dst: BlockId,
    ) {
        assert_eq!(children(function, src).count(), children(function, dst).count());
        let from = self.probs.get(src.0 as usize).cloned().flatten();
        let Some(from) = from else {
            self.erase_block(dst);
            return;
        };
        if let Some(edges) = self.alloc_edges(function, dst) {
            edges.copy_from_slice(&from);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L1158
    /// `swapSuccEdgesProbabilities`: a two-way block's edges exchanged.
    pub fn swap_succ_edges_probabilities(
        &mut self,
        function: &Function,
        src: BlockId,
    ) {
        assert_eq!(children(function, src).count(), 2);
        if let Some(Some(edges)) = self.probs.get_mut(src.0 as usize) {
            edges.swap(0, 1);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L1183
    /// `eraseBlock`: nothing stored for `block`; it reads 1/N again.
    pub fn erase_block(
        &mut self,
        block: BlockId,
    ) {
        if let Some(slot) = self.probs.get_mut(block.0 as usize) {
            *slot = None;
        }
    }

    /// `isEdgeHot`: more than 4/5.
    pub fn is_edge_hot(
        &self,
        function: &Function,
        src: BlockId,
        dst: BlockId,
    ) -> bool {
        self.get_edge_probability_to(function, src, dst) > BranchProbability::new(4, 5)
    }

    /// `print`, block names as `labels` gives them: `opt
    /// -passes='print<branch-prob>'`'s lines.
    pub fn print(
        &self,
        function: &Function,
        labels: &dyn Fn(BlockId) -> String,
    ) -> String {
        let mut out = String::from("---- Branch Probabilities ----\n");
        for &block in function.layout() {
            for successor in children(function, block) {
                let p = self.get_edge_probability_to(function, block, successor);
                let hot = if self.is_edge_hot(function, block, successor) { " [HOT edge]" } else { "" };
                out += &format!("  edge %{} -> %{} probability is {p}{hot}\n", labels(block), labels(successor));
            }
        }
        out
    }

    /// The probabilities as `BlockFrequencyInfo` reads them.
    pub fn edges<'a>(
        &'a self,
        function: &'a Function,
    ) -> impl EdgeProbabilities + 'a {
        Edges { bpi: self, function }
    }
}

struct Edges<'a> {
    bpi: &'a BranchProbabilityInfo,
    function: &'a Function,
}

impl EdgeProbabilities for Edges<'_> {
    fn get_edge_probability(
        &self,
        src: BlockId,
        index: usize,
    ) -> BranchProbability {
        self.bpi.get_edge_probability(self.function, src, index)
    }
}

/// `post_order(&F.getEntryBlock())` and `ReversePostOrderTraversal`, successors
/// in their order.
fn orders(
    function: &Function,
    entry: BlockId,
) -> (Vec<BlockId>, Vec<BlockId>) {
    let mut seen = IdSet::new();
    let mut post = Vec::new();
    seen.insert(entry);
    let mut stack = vec![(entry, children(function, entry).collect::<Vec<_>>(), 0usize)];
    while let Some((block, successors, next)) = stack.last_mut() {
        if let Some(&successor) = successors.get(*next) {
            *next += 1;
            if seen.insert(successor) {
                stack.push((successor, children(function, successor).collect(), 0));
            }
        } else {
            post.push(*block);
            stack.pop();
        }
    }
    let rpo = post.iter().rev().copied().collect();
    (post, rpo)
}

/// `BPIConstruction`.
struct Construction<'a> {
    module: Module<'a>,
    function: &'a Function,
    cycles: &'a CycleInfo,
    block_weight: IdMap<BlockId, u32>,
    loop_weight: HashMap<CycleRef, u32>,
}

impl Construction<'_> {
    /// `isLoopEnteringEdge`.
    fn is_loop_entering_edge(
        &self,
        (src, dst): (BlockId, BlockId),
    ) -> bool {
        let (src_cycle, dst_cycle) = (self.cycles.get_cycle(src), self.cycles.get_cycle(dst));
        if !dst_cycle.is_valid() {
            return false;
        }
        if !src_cycle.is_valid() {
            return true;
        }
        !self.cycles.contains_cycle(dst_cycle, src_cycle)
    }

    fn is_loop_exiting_edge(
        &self,
        (src, dst): (BlockId, BlockId),
    ) -> bool {
        self.is_loop_entering_edge((dst, src))
    }

    fn is_loop_entering_exiting_edge(
        &self,
        edge: (BlockId, BlockId),
    ) -> bool {
        self.is_loop_entering_edge(edge) || self.is_loop_exiting_edge(edge)
    }

    /// `getLoopEnterBlocks`.
    fn loop_enter_blocks(
        &self,
        block: BlockId,
        enters: &mut Vec<BlockId>,
    ) {
        let cycle = self.cycles.get_cycle(block);
        for &entry in self.cycles.get_entries(cycle) {
            for pred in inverse_children(self.function, entry) {
                if !self.cycles.contains(cycle, pred) {
                    enters.push(pred);
                }
            }
        }
    }

    /// `GenericCycle::getExitBlocks`.
    fn exit_blocks(
        &self,
        cycle: CycleRef,
    ) -> Vec<BlockId> {
        let mut exits = Vec::new();
        let mut seen = IdSet::new();
        for &block in self.cycles.get_blocks(cycle) {
            for successor in children(self.function, block) {
                if !self.cycles.contains(cycle, successor) && seen.insert(successor) {
                    exits.push(successor);
                }
            }
        }
        exits
    }

    /// `calcMetadataWeights`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L283>
    fn calc_metadata_weights(
        &self,
        block: BlockId,
        successors: &[BlockId],
    ) -> Option<Vec<BranchProbability>> {
        let terminator = self.function.terminator(block)?;
        let instruction = self.function.instruction(terminator);
        if !matches!(
            instruction.opcode,
            Opcode::Br | Opcode::Switch | Opcode::Invoke(_)
        ) {
            return None;
        }
        let mut weights = branch_weights(self.module, instruction, successors.len())?;
        let mut weight_sum: u64 = 0;
        let (mut unreachable, mut reachable) = (Vec::new(), Vec::new());
        for (i, &successor) in successors.iter().enumerate() {
            weight_sum += u64::from(weights[i]);
            match self.estimated_edge_weight((block, successor)) {
                Some(estimated) if estimated <= weight::UNREACHABLE => unreachable.push(i),
                _ => reachable.push(i),
            }
        }
        let scaling = if weight_sum > u64::from(u32::MAX) { weight_sum / u64::from(u32::MAX) + 1 } else { 1 };
        if scaling > 1 {
            weight_sum = 0;
            for one in &mut weights {
                *one = (u64::from(*one) / scaling) as u32;
                weight_sum += u64::from(*one);
            }
        }
        if weight_sum == 0 || reachable.is_empty() {
            weights.iter_mut().for_each(|one| *one = 1);
            weight_sum = successors.len() as u64;
        }
        let mut probs: Vec<BranchProbability> =
            weights.iter().map(|&one| BranchProbability::new(one, weight_sum as u32)).collect();
        if unreachable.is_empty() || reachable.is_empty() {
            return Some(probs);
        }
        for &i in &unreachable {
            if UR_TAKEN_PROB < probs[i] {
                probs[i] = UR_TAKEN_PROB;
            }
        }
        let new_unreachable_sum = unreachable.iter().fold(BranchProbability::get_zero(), |sum, &i| sum + probs[i]);
        let new_reachable_sum = BranchProbability::get_one() - new_unreachable_sum;
        let old_reachable_sum = reachable.iter().fold(BranchProbability::get_zero(), |sum, &i| sum + probs[i]);
        if old_reachable_sum != new_reachable_sum {
            if old_reachable_sum.is_zero() {
                let per_edge = new_reachable_sum.div_u32(reachable.len() as u32);
                for &i in &reachable {
                    probs[i] = per_edge;
                }
            } else {
                for &i in &reachable {
                    let mul = u64::from(new_reachable_sum.get_numerator()) * u64::from(probs[i].get_numerator());
                    let div = divide_nearest(mul, u64::from(old_reachable_sum.get_numerator()));
                    probs[i] = BranchProbability::get_raw(div as u32);
                }
            }
        }
        Some(probs)
    }

    /// `calcPointerHeuristics`.
    fn calc_pointer_heuristics(
        &self,
        block: BlockId,
    ) -> Option<Vec<BranchProbability>> {
        let (predicate, lhs, _) = self.branch_icmp(block)?;
        if !matches!(
            self.module.context.types.get(self.function.operand_type(self.module.context, lhs)?),
            Type::Pointer(_)
        ) {
            return None;
        }
        match predicate {
            IntPredicate::Ne => Some(vec![ptr_taken(), ptr_untaken()]),
            IntPredicate::Eq => Some(vec![ptr_untaken(), ptr_taken()]),
            _ => None,
        }
    }

    /// `calcZeroHeuristics`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L859>
    fn calc_zero_heuristics(
        &self,
        block: BlockId,
    ) -> Option<Vec<BranchProbability>> {
        let (predicate, lhs, rhs) = self.branch_icmp(block)?;
        let (bits, width) = self.constant_int(rhs)?;
        if let Some(and) = self.defining(lhs)
            && self.function.instruction(and).opcode == Opcode::Binary(BinaryOp::And)
            && let Some((mask, _)) = self.constant_int(self.function.instruction(and).operands[1])
            && mask.is_power_of_two()
        {
            return None;
        }
        // DEVIATE B1: the `threeway` fact stands for LibFunc's strcmp family.
        let three_way = self
            .defining(lhs)
            .filter(|&call| matches!(self.function.instruction(call).opcode, Opcode::Call(_)))
            .is_some_and(|call| {
                effects::states(
                    self.module.context,
                    self.module.declarations,
                    self.function,
                    call,
                    Fact::ThreeWayCompare,
                )
            });
        let all_ones = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
        let likely = if three_way {
            match predicate {
                IntPredicate::Eq => false,
                IntPredicate::Ne => true,
                _ => return None,
            }
        } else if bits == 0 {
            match predicate {
                IntPredicate::Eq | IntPredicate::Slt => false,
                IntPredicate::Ne | IntPredicate::Sgt => true,
                _ => return None,
            }
        } else if bits == 1 {
            match predicate {
                IntPredicate::Slt => false,
                _ => return None,
            }
        } else if bits == all_ones {
            match predicate {
                IntPredicate::Eq => false,
                IntPredicate::Ne | IntPredicate::Sgt => true,
                _ => return None,
            }
        } else {
            return None;
        };
        Some(if likely { vec![zero_taken(), zero_untaken()] } else { vec![zero_untaken(), zero_taken()] })
    }

    /// `calcFloatingPointHeuristics`.
    fn calc_floating_point_heuristics(
        &self,
        block: BlockId,
    ) -> Option<Vec<BranchProbability>> {
        let condition = self.branch_condition(block)?;
        let compare = self.defining(condition)?;
        let Opcode::FCmp(predicate) = self.function.instruction(compare).opcode else { return None };
        // `isEquality` and `isTrueWhenEqual`: of the four equality predicates
        // only `ueq` holds of equal operands whatever they are.
        Some(match predicate {
            FloatPredicate::Oeq | FloatPredicate::One | FloatPredicate::Une => vec![fp_taken(), fp_untaken()],
            FloatPredicate::Ueq => vec![fp_untaken(), fp_taken()],
            FloatPredicate::Ord => vec![fp_ord_taken(), fp_ord_untaken()],
            FloatPredicate::Uno => vec![fp_ord_untaken(), fp_ord_taken()],
            _ => return None,
        })
    }

    /// `computeUnlikelySuccessors`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L450>
    fn unlikely_successors(
        &self,
        block: BlockId,
        cycle: CycleRef,
    ) -> IdSet<BlockId> {
        let mut unlikely = IdSet::new();
        let Some(terminator) = self.function.terminator(block) else { return unlikely };
        let branch = self.function.instruction(terminator);
        if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
            return unlikely;
        }
        let (Operand::Block(taken), Operand::Block(untaken)) = (branch.operands[1], branch.operands[2]) else {
            return unlikely;
        };
        let Some(compare) = self.defining(branch.operands[0]) else { return unlikely };
        let compare_inst = self.function.instruction(compare);
        let Opcode::ICmp(predicate) = compare_inst.opcode else { return unlikely };
        let (Some(mut lhs), Operand::Constant(bound)) =
            (self.defining(compare_inst.operands[0]), compare_inst.operands[1])
        else {
            return unlikely;
        };
        let mut chain = Vec::new();
        while self.function.instruction(lhs).opcode != Opcode::Phi {
            let one = self.function.instruction(lhs);
            let (Opcode::Binary(op), Some(&Operand::Constant(rhs))) = (&one.opcode, one.operands.get(1)) else {
                return unlikely;
            };
            if !self.function.parent(lhs).is_some_and(|at| self.cycles.contains(cycle, at)) {
                return unlikely;
            }
            chain.push((*op, rhs));
            let Some(next) = self.defining(one.operands[0]) else { return unlikely };
            lhs = next;
        }
        if !self.function.parent(lhs).is_some_and(|at| self.cycles.contains(cycle, at)) {
            return unlikely;
        }
        let env = Env { globals: self.module.declarations, layout: self.module.layout };
        let successors: Vec<BlockId> = children(self.function, block).collect();
        let mut visited = IdSet::new();
        let mut work = vec![lhs];
        visited.insert(lhs);
        while let Some(phi) = work.pop() {
            let operands = &self.function.instruction(phi).operands;
            for pair in operands.chunks(2) {
                let (value, Operand::Block(from)) = (pair[0], pair[1]) else { continue };
                if !self.cycles.contains(cycle, from) {
                    continue;
                }
                if let Some(def) = self.defining(value)
                    && self.function.instruction(def).opcode == Opcode::Phi
                {
                    if visited.insert(def) {
                        work.push(def);
                    }
                    continue;
                }
                let Operand::Constant(mut constant) = value else { continue };
                if !successors.contains(&from) {
                    continue;
                }
                let mut folded = true;
                for &(op, rhs) in chain.iter().rev() {
                    match constantfolding::constant_fold_binary_op_operands(self.module.context, env, op, constant, rhs)
                    {
                        Some(one) => constant = one,
                        None => {
                            folded = false;
                            break;
                        }
                    }
                }
                if !folded {
                    continue;
                }
                let result = constantfolding::constant_fold_compare_inst_operands(
                    self.module.context,
                    self.module.declarations,
                    self.module.layout,
                    predicate,
                    constant,
                    bound,
                );
                match result.map(|one| &self.module.context.get(one).kind) {
                    Some(ConstantKind::Int(0)) if from == taken => {
                        unlikely.insert(from);
                    }
                    Some(ConstantKind::Int(1)) if from == untaken => {
                        unlikely.insert(from);
                    }
                    _ => {}
                }
            }
        }
        unlikely
    }

    fn estimated_block_weight(
        &self,
        block: BlockId,
    ) -> Option<u32> {
        self.block_weight.get(&block).copied()
    }

    fn estimated_loop_weight(
        &self,
        cycle: CycleRef,
    ) -> Option<u32> {
        self.loop_weight.get(&cycle).copied()
    }

    /// `getEstimatedEdgeWeight`.
    fn estimated_edge_weight(
        &self,
        edge: (BlockId, BlockId),
    ) -> Option<u32> {
        if self.is_loop_entering_edge(edge) {
            self.estimated_loop_weight(self.cycles.get_cycle(edge.1))
        } else {
            self.estimated_block_weight(edge.1)
        }
    }

    /// `getMaxEstimatedEdgeWeight`.
    fn max_estimated_edge_weight(
        &self,
        src: BlockId,
        successors: impl IntoIterator<Item = BlockId>,
    ) -> Option<u32> {
        let mut max: Option<u32> = None;
        for dst in successors {
            let weight = self.estimated_edge_weight((src, dst))?;
            if max.is_none_or(|one| one < weight) {
                max = Some(weight);
            }
        }
        max
    }

    /// `updateEstimatedBlockWeight`.
    fn update_estimated_block_weight(
        &mut self,
        block: BlockId,
        weight: u32,
        block_work: &mut Vec<BlockId>,
        loop_work: &mut Vec<BlockId>,
    ) -> bool {
        if self.block_weight.contains_key(&block) {
            return false;
        }
        self.block_weight.insert(block, weight);
        for pred in inverse_children(self.function, block) {
            if self.is_loop_exiting_edge((pred, block)) {
                if !self.loop_weight.contains_key(&self.cycles.get_cycle(pred)) {
                    loop_work.push(pred);
                }
            } else if !self.block_weight.contains_key(&pred) {
                block_work.push(pred);
            }
        }
        true
    }

    /// `propagateEstimatedBlockWeight`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L632>
    fn propagate_estimated_block_weight(
        &mut self,
        block: BlockId,
        dominators: &DomTree,
        post_dominators: &PostDomTree,
        weight: u32,
        block_work: &mut Vec<BlockId>,
        loop_work: &mut Vec<BlockId>,
    ) {
        if dominators.level(block).is_none() {
            return;
        }
        let mut at = Some(block);
        while let Some(dom) = at {
            if !post_dominators.dominates(block, dom) {
                break;
            }
            let edge = (dom, block);
            if !self.is_loop_entering_exiting_edge(edge) {
                if !self.update_estimated_block_weight(dom, weight, block_work, loop_work) {
                    break;
                }
            } else if self.is_loop_exiting_edge(edge) {
                loop_work.push(dom);
            }
            at = dominators.immediate_dominator(dom);
        }
    }

    /// `getInitialEstimatedBlockWeight`.
    fn initial_estimated_block_weight(
        &self,
        block: BlockId,
    ) -> Option<u32> {
        let (context, declarations, function) = (self.module.context, self.module.declarations, self.function);
        let calls_with = |fact: Fact| {
            function
                .iter_block(block)
                .any(
                    |inst| matches!(function.instruction(inst).opcode, Opcode::Call(_))
                        && effects::states(context, declarations, function, inst, fact),
                )
        };
        // DEVIATE B2: no deoptimize call ends a MIR block.
        let terminator = function.terminator(block)?;
        if function.instruction(terminator).opcode == Opcode::Unreachable {
            return Some(if calls_with(Fact::NoReturn) { weight::NORETURN } else { weight::UNREACHABLE });
        }
        if function
            .iter_block(block)
            .next()
            .is_some_and(|first| matches!(function.instruction(first).opcode, Opcode::LandingPad { .. }))
        {
            return Some(weight::UNWIND);
        }
        if calls_with(Fact::Cold) {
            return Some(weight::COLD);
        }
        None
    }

    /// `estimateBlockWeights`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L704>
    fn estimate_block_weights(
        &mut self,
        rpo: &[BlockId],
        dominators: &DomTree,
        post_dominators: &PostDomTree,
    ) {
        let (mut block_work, mut loop_work) = (Vec::new(), Vec::new());
        let mut loop_exits: HashMap<CycleRef, Vec<BlockId>> = HashMap::default();
        for &block in rpo {
            if let Some(weight) = self.initial_estimated_block_weight(block) {
                self.propagate_estimated_block_weight(
                    block,
                    dominators,
                    post_dominators,
                    weight,
                    &mut block_work,
                    &mut loop_work,
                );
            }
        }
        loop {
            while let Some(loop_block) = loop_work.pop() {
                let cycle = self.cycles.get_cycle(loop_block);
                if self.loop_weight.contains_key(&cycle) {
                    continue;
                }
                let exits = loop_exits.entry(cycle).or_insert_with(|| self.exit_blocks(cycle)).clone();
                if let Some(mut weight) = self.max_estimated_edge_weight(loop_block, exits) {
                    if weight <= weight::UNREACHABLE {
                        weight = weight::LOWEST_NON_ZERO;
                    }
                    self.loop_weight.insert(cycle, weight);
                    self.loop_enter_blocks(loop_block, &mut block_work);
                }
            }
            while let Some(block) = block_work.pop() {
                if self.block_weight.contains_key(&block) {
                    continue;
                }
                let successors: Vec<BlockId> = children(self.function, block).collect();
                if let Some(weight) = self.max_estimated_edge_weight(block, successors) {
                    self.propagate_estimated_block_weight(
                        block,
                        dominators,
                        post_dominators,
                        weight,
                        &mut block_work,
                        &mut loop_work,
                    );
                }
            }
            if block_work.is_empty() && loop_work.is_empty() {
                break;
            }
        }
    }

    /// `calcEstimatedHeuristics`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BranchProbabilityInfo.cpp#L773>
    fn calc_estimated_heuristics(
        &self,
        block: BlockId,
        successors: &[BlockId],
    ) -> Option<Vec<BranchProbability>> {
        let cycle = self.cycles.get_cycle(block);
        let trip_count = LBH_TAKEN_WEIGHT / LBH_NONTAKEN_WEIGHT;
        let unlikely = if cycle.is_valid() { self.unlikely_successors(block, cycle) } else { IdSet::new() };
        let mut found = false;
        let mut weights = Vec::with_capacity(successors.len());
        let mut total: u64 = 0;
        for &successor in successors {
            let edge = (block, successor);
            let mut weight = self.estimated_edge_weight(edge);
            if self.is_loop_exiting_edge(edge) && weight != Some(weight::ZERO) {
                weight = Some(weight::LOWEST_NON_ZERO.max(weight.unwrap_or(weight::DEFAULT) / trip_count));
            }
            if cycle.is_valid() && unlikely.contains(&successor) && weight != Some(weight::ZERO) {
                weight = Some(weight::LOWEST_NON_ZERO.max(weight.unwrap_or(weight::DEFAULT) / 2));
            }
            found |= weight.is_some();
            let value = weight.unwrap_or(weight::DEFAULT);
            total += u64::from(value);
            weights.push(value);
        }
        if !found || total == 0 {
            return None;
        }
        if total > u64::from(u32::MAX) {
            let scaling = total / u64::from(u32::MAX) + 1;
            total = 0;
            for one in &mut weights {
                *one = (u64::from(*one) / scaling) as u32;
                if *one == weight::ZERO {
                    *one = weight::LOWEST_NON_ZERO;
                }
                total += u64::from(*one);
            }
        }
        Some(weights.iter().map(|&one| BranchProbability::new(one, total as u32)).collect())
    }

    /// The condition of `block`'s conditional branch.
    fn branch_condition(
        &self,
        block: BlockId,
    ) -> Option<Operand> {
        let branch = self.function.instruction(self.function.terminator(block)?);
        (branch.opcode == Opcode::Br && branch.operands.len() == 3).then(|| branch.operands[0])
    }

    /// The predicate and operands of the `icmp` `block` branches on.
    fn branch_icmp(
        &self,
        block: BlockId,
    ) -> Option<(IntPredicate, Operand, Operand)> {
        let compare = self.defining(self.branch_condition(block)?)?;
        let instruction = self.function.instruction(compare);
        let Opcode::ICmp(predicate) = instruction.opcode else { return None };
        Some((predicate, instruction.operands[0], instruction.operands[1]))
    }

    fn defining(
        &self,
        operand: Operand,
    ) -> Option<llrm_mir::module::InstId> {
        let Operand::Value(value) = operand else { return None };
        match self.function.value(value).def {
            ValueDef::Instruction(def) => Some(def),
            _ => None,
        }
    }

    /// `GetConstantInt`: an integer constant, or a bitcast of one; its bits and
    /// width.
    fn constant_int(
        &self,
        operand: Operand,
    ) -> Option<(u128, u32)> {
        let constant = match operand {
            Operand::Constant(one) => one,
            _ => {
                let cast = self.function.instruction(self.defining(operand)?);
                match (&cast.opcode, cast.operands.first()) {
                    (Opcode::Cast(CastOp::BitCast), Some(&Operand::Constant(one))) => one,
                    _ => return None,
                }
            }
        };
        int_bits(self.module.context, constant)
    }
}

fn int_bits(
    context: &Context,
    constant: ConstantId,
) -> Option<(u128, u32)> {
    let one = context.get(constant);
    let width = context.types.int_bits(one.ty)?;
    match one.kind {
        ConstantKind::Int(bits) => Some((bits, width)),
        ConstantKind::Zero => Some((0, width)),
        _ => None,
    }
}

/// `divideNearest`.
fn divide_nearest(
    numerator: u64,
    denominator: u64,
) -> u64 {
    (numerator + denominator / 2) / denominator
}

/// `getValidBranchWeightMDNode` and `extractBranchWeights`: the terminator's
/// `branch_weights`, one per successor.
fn branch_weights(
    module: Module,
    terminator: &llrm_mir::module::Instruction,
    successors: usize,
) -> Option<Vec<u32>> {
    let (_, node) = terminator.metadata.iter().find(|(kind, _)| kind == "prof")?;
    let mut operands = module.metadata.get(node.0 as usize)?.operands.iter().peekable();
    let Some(MetadataOperand::String(name)) = operands.next() else { return None };
    if name != "branch_weights" {
        return None;
    }
    if matches!(operands.peek(), Some(MetadataOperand::String(_))) {
        operands.next();
    }
    let weights = operands
        .map(|one| match one {
            MetadataOperand::Constant(at) => int_bits(module.context, *at).map(|(bits, _)| bits as u32),
            _ => None,
        })
        .collect::<Option<Vec<u32>>>()?;
    (weights.len() == successors).then_some(weights)
}

impl Analysis for BranchProbabilityInfo {
    type Result = BranchProbabilityInfo;
    const NAME: &'static str = "branch-probabilities";

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let outer = analyses.outer();
        Self::of(Module { context, declarations: &outer.globals, metadata: &outer.metadata, layout }, function)
    }

    const READS_OUTER: bool = true;
}

/// `opt -passes='print<branch-prob>'` of every defined function of `module`
/// (`llrm-mir --branch-prob`, for `tools/bpi/oracle.py`).
pub fn print_module_branch_prob(module: &llrm_mir::module::Module) -> String {
    let layout = DataLayout::default();
    let mut out = String::new();
    for (_, global, function) in module.functions().filter(|(_, _, one)| !one.is_declaration()) {
        let bpi = BranchProbabilityInfo::of(
            Module {
                context: &module.context,
                declarations: &module.globals,
                metadata: &module.metadata,
                layout: &layout,
            },
            function,
        );
        let name = global.name.as_deref().unwrap_or_default();
        out += &format!("Printing analysis 'Branch Probability Analysis' for function '{name}':\n");
        let labels = llrm_mir::print::block_labels(function);
        out += &bpi.print(function, &|block| labels.get(&block).cloned().unwrap_or_default());
    }
    out
}

#[cfg(test)]
#[path = "bpi_tests.rs"]
mod tests;
