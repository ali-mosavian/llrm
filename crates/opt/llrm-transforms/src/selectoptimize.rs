//! `SelectOptimize`: a group of selects (or select-like `or`/`add`/`sub` of an
//! extended condition) becomes a branch where the branch is the cheaper form
//! ([SelectOptimize.cpp#L336](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L336)).
//!
//! # How it works
//! The target gates the pass (`Target`): it must enable it and support scalar
//! selects, and the function must not be `optsize`/`minsize`. Consecutive
//! select-likes of one condition in a block form a group. Outside innermost
//! loops a group converts when its branch is highly predictable and the target
//! finds predictable selects expensive, when its cold operand's dependence
//! slice costs at least `TCC_Expensive`, or when it is three or more selects
//! in a loop's latch. In an innermost loop the loop's critical path over two
//! iterations is priced with the selects as selects and as branches (latency
//! plus the dependence on the operands, mispredictions at the default rate);
//! the loop converts when the branches cut it by 4 cycles and 12.5%, with a
//! gradient of 25% between the iterations, and each group whose branch cost is
//! below its select cost converts.
//!
//! A converted group splits its block: the instructions only its operands need
//! sink into `select.true.sink` / `select.false.sink` (interleaved across the
//! group's selects), the condition is frozen, and phis replace the selects.
//!
//! ```text
//!  start: %s = select i1 %c, %a, %b       start: %c.frozen = freeze i1 %c
//!         ...                                    br i1 %c.frozen, end, select.false
//!                                         select.false: br end
//!                                         end:   %s = phi [%a, start], [%b, select.false]
//! ```
//!
//! DEVIATE SO1 (exit 2026-12-04, pin permalink above): the MIR carries no
//! `ProfileSummaryInfo`, so no block is cold, `shouldOptimizeForSize` is the
//! `optsize`/`minsize` attribute alone, and `setExplicitlyUnknownBranchWeights`
//! is a no-op.
//!
//! DEVIATE SO2 (exit 2026-12-04, pin permalink above): no optimization
//! remarks, statistics or debug locations; `shouldTreatInstructionLikeSelect`
//! is `TargetTransformInfoImplBase`'s, as no target of ours overrides it.
//!
//! # Runs
//! Not wired: the pin's `llc` leaves it off (`-disable-select-optimize`
//! defaults to true, `TargetPassConfig.cpp` L278), so no pipeline slot holds
//! it. Where the pin runs it, it is after `ExpandReductions` in
//! `addIRPasses`.

use std::collections::VecDeque;

use llrm_analysis::blockfreq::BlockFrequencies;
use llrm_analysis::occurrence::from_arms;
use llrm_mir::blockfrequency::{BlockFrequency, BranchProbability, Scaled64};
use llrm_mir::context::{ConstantKind, Context, mask};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::loopinfo::{Loop, LoopInfo};
use llrm_mir::memory::Callees;
use llrm_mir::module::{
    BlockId, Function, GlobalValue, InstId, MetadataId, MetadataNode, MetadataOperand, Operand, ValueDef,
};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::tti::{ArithOp, Cost, CostKind, OperandInfo, Tti, Ty};
use llrm_mir::types::TypeId;

use crate::edges::split_block_at;

/// `cold-operand-threshold`.
const COLD_OPERAND_THRESHOLD: u64 = 20;
/// `cold-operand-max-cost-multiplier`.
const COLD_OPERAND_MAX_COST_MULTIPLIER: i64 = 1;
/// `select-opti-loop-gradient-gain-threshold`.
const GAIN_GRADIENT_THRESHOLD: u64 = 25;
/// `select-opti-loop-cycle-gain-threshold`.
const GAIN_CYCLE_THRESHOLD: u64 = 4;
/// `select-opti-loop-relative-gain-threshold`.
const GAIN_RELATIVE_THRESHOLD: u64 = 8;
/// `mispredict-default-rate`.
const MISPREDICT_DEFAULT_RATE: u64 = 25;

/// What the pass asks of the target: `TargetTransformInfo`,
/// `TargetLowering` and `TargetSubtargetInfo`.
pub struct Target<'a> {
    pub tti: &'a dyn Tti,
    /// `TTI::enableSelectOptimize` (true for x86 and the base class).
    pub enabled: bool,
    /// `TLI::isSelectSupported(ScalarValSelect)`.
    pub scalar_select: bool,
    /// `TLI::isSelectSupported(ScalarCondVectorVal)`.
    pub scalar_cond_vector_select: bool,
    /// `TLI::isSelectSupported(VectorMaskSelect)`.
    pub vector_mask_select: bool,
    /// `TLI::isPredictableSelectExpensive` (x86: out-of-order cores).
    pub predictable_select_expensive: bool,
    /// `TTI::getPredictableBranchThreshold`.
    pub predictable_branch_threshold: BranchProbability,
    /// `TSI::getMispredictionPenalty`, the scheduling model's.
    pub misprediction_penalty: u64,
    /// DEVIATE SELOPT-C: a select whose arms are both constants is select-like.
    /// The pin's `shouldTreatInstructionLikeSelect` leaves it a select.
    pub constant_arm_selects: bool,
}

impl<'a> Target<'a> {
    /// The base classes' answers: every select supported, the 99% threshold,
    /// `MCSchedModel::DefaultMispredictPenalty`.
    pub fn base(tti: &'a dyn Tti) -> Self {
        Target {
            tti,
            enabled: true,
            scalar_select: true,
            scalar_cond_vector_select: true,
            vector_mask_select: true,
            predictable_select_expensive: false,
            predictable_branch_threshold: BranchProbability::new(99, 100),
            misprediction_penalty: 10,
            constant_arm_selects: false,
        }
    }
}

/// What the module gives a function pass.
pub struct Env<'a> {
    pub context: &'a Context,
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
    pub layout: &'a DataLayout,
    pub callees: &'a Callees,
}

#[derive(Clone, Copy)]
struct CostInfo {
    pred: Scaled64,
    non_pred: Scaled64,
}

fn zero_cost() -> CostInfo {
    CostInfo { pred: Scaled64::get_zero(), non_pred: Scaled64::get_zero() }
}

/// `std::max` on `ScaledNumber`s.
fn larger(
    a: Scaled64,
    b: Scaled64,
) -> Scaled64 {
    if b > a { b } else { a }
}

/// A select or an operation that acts as one (`Or(Zext(icmp), X)` is
/// `select(icmp, X|1, X)`).
#[derive(Clone, Copy)]
struct SelectLike {
    inst: InstId,
    /// `not(cond), FalseVal, TrueVal` against the group's condition.
    inverted: bool,
    /// The operand that depends on the condition, for a binary operation.
    cond_idx: usize,
}

impl SelectLike {
    /// `getTrueValue`: none for a binary operation, whose true value does not
    /// exist yet.
    fn true_value(
        &self,
        function: &Function,
        honor_inverts: bool,
    ) -> Option<Operand> {
        if self.inverted && honor_inverts {
            return self.false_value(function, false);
        }
        let instruction = function.instruction(self.inst);
        match instruction.opcode {
            Opcode::Select => Some(instruction.operands[1]),
            _ => None,
        }
    }

    /// `getFalseValue`: the operand not related to the condition.
    fn false_value(
        &self,
        function: &Function,
        honor_inverts: bool,
    ) -> Option<Operand> {
        if self.inverted && honor_inverts {
            return self.true_value(function, false);
        }
        let instruction = function.instruction(self.inst);
        match instruction.opcode {
            Opcode::Select => Some(instruction.operands[2]),
            _ => Some(instruction.operands[1 - self.cond_idx]),
        }
    }

    fn is_select(
        &self,
        function: &Function,
    ) -> bool {
        function.instruction(self.inst).opcode == Opcode::Select
    }
}

/// Consecutive select-likes with one condition.
#[derive(Clone)]
struct SelectGroup {
    condition: Operand,
    selects: Vec<SelectLike>,
}

/// `BlockFrequencyInfo`, with the frequencies the pass sets on the blocks it
/// makes (`setBlockFreq`).
struct Frequencies {
    base: BlockFrequencies,
    set: IdMap<BlockId, BlockFrequency>,
}

impl Frequencies {
    fn get(
        &self,
        block: BlockId,
    ) -> BlockFrequency {
        self.set.get(&block).copied().unwrap_or_else(|| self.base.get_block_freq(block))
    }
}

fn definition(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        _ => None,
    }
}

fn operand_type(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> TypeId {
    match operand {
        Operand::Value(value) => function.value(value).ty,
        Operand::Constant(id) => context.get(id).ty,
        Operand::Block(_) => unreachable!("a block is not a value"),
    }
}

fn int_bits(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<u32> {
    context.types.int_bits(operand_type(context, function, operand))
}

fn constant_int(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        ConstantKind::Zero if context.types.int_bits(context.get(id).ty).is_some() => Some(0),
        _ => None,
    }
}

fn is_all_ones(
    context: &Context,
    operand: Operand,
) -> bool {
    let Operand::Constant(id) = operand else { return false };
    context.types.int_bits(context.get(id).ty).is_some_and(|bits| constant_int(context, operand) == Some(mask(bits)))
}

fn has_one_use(
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).result.is_some_and(|value| function.users(value).has_one_use())
}

/// `m_Not(m_Value(X))`.
fn matched_not(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<Operand> {
    let inst = definition(function, operand)?;
    let instruction = function.instruction(inst);
    let Opcode::Binary(BinaryOp::Xor) = instruction.opcode else { return None };
    let [a, b] = instruction.operands[..] else { return None };
    if is_all_ones(context, b) {
        Some(a)
    } else if is_all_ones(context, a) {
        Some(b)
    } else {
        None
    }
}

/// `m_Select(m_Value(), m_Constant(), m_Constant())`, or a logical and/or:
/// the select `shouldTreatInstructionLikeSelect` refuses.
fn treated_like_select(
    context: &Context,
    function: &Function,
    inst: InstId,
    constant_arms: bool,
) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Select {
        return false;
    }
    let [_, on_true, on_false] = instruction.operands[..] else { return false };
    if !constant_arms && matches!(on_true, Operand::Constant(_)) && matches!(on_false, Operand::Constant(_)) {
        return false;
    }
    let boolean = context.types.int_bits(instruction.ty) == Some(1);
    let logical_and = boolean && constant_int(context, on_false) == Some(0);
    let logical_or = boolean && is_all_ones(context, on_true);
    !(logical_and || logical_or)
}

/// The select's `branch_weights` (`extractBranchWeights`).
fn branch_weights(
    context: &Context,
    metadata: &[MetadataNode],
    declared: &[MetadataNode],
    function: &Function,
    inst: InstId,
) -> Option<(MetadataId, [u64; 2])> {
    let (_, id) = function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "prof")?;
    let node = match (id.0 as usize).checked_sub(metadata.len()) {
        None => metadata.get(id.0 as usize)?,
        Some(made) => declared.get(made)?,
    };
    let mut operands = node.operands.iter();
    let Some(MetadataOperand::String(name)) = operands.next() else { return None };
    if name != "branch_weights" {
        return None;
    }
    let weights = operands
        .map(|one| match one {
            MetadataOperand::Constant(at) => match context.get(*at).kind {
                ConstantKind::Int(bits) => Some(bits as u32 as u64),
                ConstantKind::Zero => Some(0),
                _ => None,
            },
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((*id, weights.try_into().ok()?))
}

struct Info {
    condition: Operand,
    auxiliary: bool,
    inverted: bool,
    condition_index: usize,
}

/// `collectSelectGroups`' `SelectInfo` and `SeenCmp`.
#[derive(Default)]
struct Collector {
    info: IdMap<InstId, (Operand, bool, bool, usize)>,
    seen_cmp: Vec<InstId>,
}

impl Collector {
    /// `ProcessSelectInfo`.
    fn process(
        &mut self,
        context: &Context,
        function: &Function,
        inst: InstId,
    ) -> Option<Info> {
        let instruction = function.instruction(inst);
        let one_use_extension = |operand: Operand| -> Option<Operand> {
            let inner = definition(function, operand)?;
            let extension = function.instruction(inner);
            (matches!(extension.opcode, Opcode::Cast(CastOp::ZExt | CastOp::SExt)) && has_one_use(function, inner))
                .then(|| extension.operands[0])
        };
        let one_use_shift = |operand: Operand| -> Option<(Operand, u128)> {
            let inner = definition(function, operand)?;
            let shift = function.instruction(inner);
            if !matches!(shift.opcode, Opcode::Binary(BinaryOp::LShr | BinaryOp::AShr)) || !has_one_use(function, inner)
            {
                return None;
            }
            Some((shift.operands[0], constant_int(context, shift.operands[1])?))
        };
        let record = |this: &mut Self, condition, auxiliary, inverted, condition_index| {
            this.info.insert(inst, (condition, auxiliary, inverted, condition_index));
            Some(Info { condition, auxiliary, inverted, condition_index })
        };
        match instruction.opcode {
            Opcode::ICmp(_) | Opcode::FCmp(_) => {
                if !self.seen_cmp.contains(&inst) {
                    self.seen_cmp.push(inst);
                }
                return None;
            }
            _ => {}
        }
        if matches!(instruction.opcode, Opcode::Cast(CastOp::ZExt | CastOp::SExt))
            && has_one_use(function, inst)
            && int_bits(context, function, instruction.operands[0]) == Some(1)
        {
            let condition = instruction.operands[0];
            return match matched_not(context, function, condition) {
                Some(inner) => record(self, inner, true, true, 0),
                None => record(self, condition, true, false, 0),
            };
        }
        if let Some(condition) = matched_not(context, function, Operand::Value(instruction.result?)) {
            return record(self, condition, true, true, 0);
        }
        if instruction.opcode == Opcode::Select {
            let condition = instruction.operands[0];
            return match matched_not(context, function, condition) {
                Some(inner) => record(self, inner, false, true, 0),
                None => record(self, condition, false, false, 0),
            };
        }
        let Opcode::Binary(op) = instruction.opcode else { return None };
        let width = context.types.int_bits(instruction.ty);
        if matches!(op, BinaryOp::LShr | BinaryOp::AShr)
            && let Some(shift) = constant_int(context, instruction.operands[1])
            && width.is_some_and(|width| u128::from(width) == shift + 1)
        {
            let value = instruction.operands[0];
            for &compare in &self.seen_cmp.clone() {
                let cmp = function.instruction(compare);
                let Opcode::ICmp(predicate) = cmp.opcode else { continue };
                if value != cmp.operands[0] {
                    continue;
                }
                let right = cmp.operands[1];
                let minus_one = is_all_ones(context, right);
                let zero = constant_int(context, right) == Some(0);
                let sign_test = (predicate == IntPredicate::Sgt && minus_one)
                    || (predicate == IntPredicate::Sge && zero)
                    || (predicate == IntPredicate::Slt && zero)
                    || (predicate == IntPredicate::Sle && minus_one);
                if sign_test {
                    let inverted = matches!(predicate, IntPredicate::Sgt | IntPredicate::Sge);
                    let condition = Operand::Value(cmp.result.expect("a compare's value"));
                    return record(self, condition, true, inverted, 0);
                }
            }
            return None;
        }
        // `m_c_BinOp(m_Value(), P)`: P on the second operand, else the first.
        let commuted = |matcher: &dyn Fn(Operand) -> Option<Operand>| {
            matcher(instruction.operands[1]).or_else(|| matcher(instruction.operands[0]))
        };
        let extended = commuted(&one_use_extension).is_some_and(|x| int_bits(context, function, x) == Some(1));
        let shifted = commuted(&|operand| {
            let (x, shift) = one_use_shift(operand)?;
            int_bits(context, function, x).is_some_and(|bits| u128::from(bits) == shift + 1).then_some(x)
        })
        .is_some();
        if !(extended || shifted) || !matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Or) {
            return None;
        }
        if op == BinaryOp::Or && width == Some(1) {
            return None;
        }
        // For a subtraction only the second operand can be the select-like.
        let first = usize::from(op == BinaryOp::Sub);
        for index in first..2 {
            let Some(found) = definition(function, instruction.operands[index]) else { continue };
            if let Some(&(condition, true, inverted, _)) = self.info.get(&found) {
                return record(self, condition, false, inverted, index);
            }
        }
        None
    }
}

struct Analysis<'a> {
    env: &'a Env<'a>,
    target: &'a Target<'a>,
    function: &'a Function,
    declared: &'a [MetadataNode],
    loops: &'a LoopInfo,
    frequencies: &'a Frequencies,
}

impl Analysis<'_> {
    fn weights(
        &self,
        select: &SelectLike,
    ) -> Option<[u64; 2]> {
        if !select.is_select(self.function) {
            return None;
        }
        branch_weights(self.env.context, self.env.metadata, self.declared, self.function, select.inst)
            .map(|(_, weights)| weights)
    }

    /// `optimizeSelectsBase` then `optimizeSelectsInnerLoops`.
    fn profitable_groups(&self) -> Vec<SelectGroup> {
        let mut profitable = Vec::new();
        let mut groups = Vec::new();
        for &block in self.function.layout() {
            if self.loops.get_loop_for(block).is_some_and(|one| one.is_innermost()) {
                continue;
            }
            self.collect_select_groups(block, &mut groups);
        }
        profitable.extend(groups.into_iter().filter(|group| self.is_convert_to_branch_profitable_base(group)));

        let mut queue: VecDeque<Loop<'_>> = self.loops.top_level_loops().collect();
        while let Some(one) = queue.pop_front() {
            queue.extend(one.get_sub_loops());
            if !one.is_innermost() {
                continue;
            }
            let mut groups = Vec::new();
            for &block in one.get_blocks() {
                self.collect_select_groups(block, &mut groups);
            }
            self.find_profitable_groups_inner_loops(&one, groups, &mut profitable);
        }
        profitable
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L783>
    fn collect_select_groups(
        &self,
        block: BlockId,
        groups: &mut Vec<SelectGroup>,
    ) {
        let context = self.env.context;
        let function = self.function;
        let instructions: Vec<InstId> = function.iter_block(block).collect();
        let mut collector = Collector::default();
        let mut already_processed = false;
        let mut current: Option<Info> = None;
        let mut at = 0;
        while at < instructions.len() {
            let inst = instructions[at];
            at += 1;
            let info = if already_processed {
                already_processed = false;
                current.take()
            } else {
                collector.process(context, function, inst)
            };
            let Some(info) = info else { continue };
            if info.auxiliary || !treated_like_select(context, function, inst, self.target.constant_arm_selects) {
                continue;
            }
            let condition = info.condition;
            if int_bits(context, function, condition) != Some(1) {
                continue;
            }
            let first = SelectLike { inst, inverted: info.inverted, cond_idx: info.condition_index };
            if !self.is_select_kind_supported(&first) {
                continue;
            }
            let mut group = SelectGroup { condition, selects: vec![first] };
            while at < instructions.len() {
                let next = instructions[at];
                let Some(info) = collector.process(context, function, next) else {
                    already_processed = true;
                    current = None;
                    break;
                };
                if info.condition != condition {
                    already_processed = true;
                    current = Some(info);
                    break;
                }
                if !info.auxiliary {
                    group.selects.push(SelectLike {
                        inst: next,
                        inverted: info.inverted,
                        cond_idx: info.condition_index,
                    });
                }
                at += 1;
            }
            groups.push(group);
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1462>
    fn is_select_kind_supported(
        &self,
        select: &SelectLike,
    ) -> bool {
        let ty = self.function.instruction(select.inst).ty;
        if matches!(self.env.context.types.get(ty), llrm_mir::types::Type::Vector { .. }) {
            self.target.scalar_cond_vector_select
        } else {
            self.target.scalar_select
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1030>
    fn is_convert_to_branch_profitable_base(
        &self,
        group: &SelectGroup,
    ) -> bool {
        let first = &group.selects[0];
        let function = self.function;
        if function.instruction(first.inst).metadata.iter().any(|(kind, _)| kind == "unpredictable") {
            return false;
        }
        if self.is_select_highly_predictable(first) && self.target.predictable_select_expensive {
            return true;
        }
        if self.has_expensive_cold_operand(group) {
            return true;
        }
        // A latch with several selects usually converts; innermost loops are
        // decided by their loop-level costs.
        let block = function.parent(first.inst).expect("a placed select");
        self.loops.get_loop_for(block).is_some_and(|one| {
            !one.is_innermost() && one.get_loop_latch(self.function) == Some(block) && group.selects.len() >= 3
        })
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1222>
    fn is_select_highly_predictable(
        &self,
        select: &SelectLike,
    ) -> bool {
        let Some([on_true, on_false]) = self.weights(select) else { return false };
        let (max, sum) = (on_true.max(on_false), on_true.saturating_add(on_false));
        sum != 0
            && BranchProbability::get_branch_probability(max, sum).get_numerator()
                > self.target.predictable_branch_threshold.get_numerator()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1101>
    fn has_expensive_cold_operand(
        &self,
        group: &SelectGroup,
    ) -> bool {
        let Some([true_weight, false_weight]) = self.weights(&group.selects[0]) else { return false };
        let total_weight = true_weight + false_weight;
        let min_weight = true_weight.min(false_weight);
        // Is there a path with frequency < ColdOperandThreshold%?
        if total_weight * COLD_OPERAND_THRESHOLD <= 100 * min_weight {
            return false;
        }
        for select in &group.selects {
            let (cold, hot_weight) = if true_weight < false_weight {
                (select.true_value(self.function, true), false_weight)
            } else {
                (select.false_value(self.function, true), true_weight)
            };
            let Some(cold) = cold.and_then(|operand| definition(self.function, operand)) else { continue };
            let mut slice_cost = Cost::FREE;
            for inst in self.exclusive_backwards_slice(cold, select.inst, false) {
                slice_cost = slice_cost + self.latency(inst);
            }
            // The colder the operand, the more its cost counts.
            let adjusted = (i128::from(slice_cost.value) * i128::from(hot_weight) + i128::from(total_weight / 2))
                / i128::from(total_weight);
            if adjusted >= i128::from(COLD_OPERAND_MAX_COST_MULTIPLIER * Cost::EXPENSIVE.value) {
                return true;
            }
        }
        false
    }

    fn latency(
        &self,
        inst: InstId,
    ) -> Cost {
        let env = self.env;
        instruction_cost(self.target.tti, env.context, env.layout, env.globals, self.function, inst, CostKind::Latency)
    }

    fn exclusive_backwards_slice(
        &self,
        source: InstId,
        select: InstId,
        for_sinking: bool,
    ) -> Vec<InstId> {
        exclusive_backwards_slice(self.env, self.function, self.frequencies, source, select, for_sinking)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L980>
    fn find_profitable_groups_inner_loops(
        &self,
        one: &Loop<'_>,
        groups: Vec<SelectGroup>,
        profitable: &mut Vec<SelectGroup>,
    ) {
        let mut costs: IdMap<InstId, CostInfo> = IdMap::new();
        let mut loop_cost = [zero_cost(), zero_cost()];
        if !self.compute_loop_costs(one, &groups, &mut costs, &mut loop_cost) || !check_loop_heuristics(&loop_cost) {
            return;
        }
        for group in groups {
            // Infinite resources: a group costs its most expensive select.
            let (mut select_cost, mut branch_cost) = (Scaled64::get_zero(), Scaled64::get_zero());
            for select in &group.selects {
                let cost = costs.get(&select.inst).copied().unwrap_or(zero_cost());
                select_cost = larger(select_cost, cost.pred);
                branch_cost = larger(branch_cost, cost.non_pred);
            }
            if branch_cost < select_cost {
                profitable.push(group);
            }
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1305>
    fn compute_loop_costs(
        &self,
        one: &Loop<'_>,
        groups: &[SelectGroup],
        costs: &mut IdMap<InstId, CostInfo>,
        loop_cost: &mut [CostInfo; 2],
    ) -> bool {
        let function = self.function;
        let mut selects: IdMap<InstId, (SelectLike, &SelectGroup)> = IdMap::new();
        for group in groups {
            for select in &group.selects {
                if !selects.contains_key(&select.inst) {
                    selects.insert(select.inst, (*select, group));
                }
            }
        }
        for max_cost in loop_cost.iter_mut() {
            for &block in one.get_blocks() {
                for inst in function.iter_block(block) {
                    let (mut pred, mut non_pred) = (Scaled64::get_zero(), Scaled64::get_zero());
                    // InstCost = InstLatency + max(Op1Cost, ..., OpNCost)
                    for &operand in &function.instruction(inst).operands {
                        let Some(operand) = definition(function, operand) else { continue };
                        if let Some(cost) = costs.get(&operand) {
                            pred = larger(pred, cost.pred);
                            non_pred = larger(non_pred, cost.non_pred);
                        }
                    }
                    let latency = self.latency(inst);
                    if !latency.valid {
                        return false;
                    }
                    let latency = Scaled64::get(latency.value as u64);
                    pred += latency;
                    non_pred += latency;
                    // BranchCost = PredictedPathCost + MispredictCost
                    if let Some(&(select, group)) = selects.get(&inst) {
                        let on_true = self.op_cost_on_branch(&select, true, costs);
                        let on_false = self.op_cost_on_branch(&select, false, costs);
                        let predicted_path = self.predicted_path_cost(on_true, on_false, &select);
                        let condition_cost = definition(function, group.condition)
                            .and_then(|one| costs.get(&one))
                            .map_or(Scaled64::get_zero(), |cost| cost.non_pred);
                        non_pred = predicted_path + self.misprediction_cost(&select, condition_cost);
                    }
                    costs.insert(inst, CostInfo { pred, non_pred });
                    max_cost.pred = larger(max_cost.pred, pred);
                    max_cost.non_pred = larger(max_cost.non_pred, non_pred);
                }
            }
        }
        true
    }

    /// `SelectLike::getOpCostOnBranch`.
    fn op_cost_on_branch(
        &self,
        select: &SelectLike,
        is_true: bool,
        costs: &IdMap<InstId, CostInfo>,
    ) -> Scaled64 {
        let function = self.function;
        let value = if is_true { select.true_value(function, true) } else { select.false_value(function, true) };
        let cost_of = |operand: Operand| {
            definition(function, operand)
                .and_then(|one| costs.get(&one))
                .map_or(Scaled64::get_zero(), |cost| cost.non_pred)
        };
        if let Some(value) = value {
            return cost_of(value);
        }
        // The branch where the computation happens: the operation's own
        // latency plus the operand that does not depend on the condition.
        let instruction = function.instruction(select.inst);
        let Opcode::Binary(op) = instruction.opcode else { return Scaled64::get_zero() };
        let arith = match op {
            BinaryOp::Add => ArithOp::Add,
            BinaryOp::Sub => ArithOp::Sub,
            _ => ArithOp::Or,
        };
        let cost = Ty::of(&self.env.context.types, instruction.ty).map_or(Cost::INVALID, |ty| {
            self.target.tti.arithmetic_cost(
                arith,
                ty,
                CostKind::Latency,
                OperandInfo::ANY,
                OperandInfo { constant: true, power_of_two: true, negated_power_of_two: false, value: None },
            )
        });
        Scaled64::get(cost.value as u64) + cost_of(instruction.operands[1 - select.cond_idx])
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1414>
    fn misprediction_cost(
        &self,
        select: &SelectLike,
        condition_cost: Scaled64,
    ) -> Scaled64 {
        // An obviously predictable condition never mispredicts.
        let rate = if self.is_select_highly_predictable(select) { 0 } else { MISPREDICT_DEFAULT_RATE };
        // The condition's own chain delays detecting a misprediction.
        larger(Scaled64::get(self.target.misprediction_penalty), condition_cost) * Scaled64::get(rate)
            / Scaled64::get(100)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1440>
    fn predicted_path_cost(
        &self,
        true_cost: Scaled64,
        false_cost: Scaled64,
        select: &SelectLike,
    ) -> Scaled64 {
        if let Some([true_weight, false_weight]) = self.weights(select) {
            let sum = true_weight + false_weight;
            if sum != 0 {
                return (true_cost * Scaled64::get(true_weight) + false_cost * Scaled64::get(false_weight))
                    / Scaled64::get(sum);
            }
        }
        // Without weights: 75% one path, 25% the other, the dearer taken.
        let three = Scaled64::get(3);
        larger(true_cost * three + false_cost, false_cost * three + true_cost) / Scaled64::get(4)
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1236>
fn check_loop_heuristics(loop_cost: &[CostInfo; 2]) -> bool {
    if loop_cost[0].non_pred > loop_cost[0].pred || loop_cost[1].non_pred >= loop_cost[1].pred {
        return false;
    }
    let gain = [loop_cost[0].pred - loop_cost[0].non_pred, loop_cost[1].pred - loop_cost[1].non_pred];
    // At least GainCycleThreshold cycles and 1/GainRelativeThreshold of the
    // loop's critical path.
    if gain[1] < Scaled64::get(GAIN_CYCLE_THRESHOLD)
        || gain[1] * Scaled64::get(GAIN_RELATIVE_THRESHOLD) < loop_cost[1].pred
    {
        return false;
    }
    // The gain must keep growing beyond the two analysed iterations.
    if gain[1] > gain[0] {
        let gradient = Scaled64::get(100) * (gain[1] - gain[0]) / (loop_cost[1].pred - loop_cost[0].pred);
        if gradient < Scaled64::get(GAIN_GRADIENT_THRESHOLD) {
            return false;
        }
    } else if gain[1] < gain[0] {
        return false;
    }
    true
}

/// Safe to move `load` next to `select`: nothing between them writes memory.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1156>
fn is_safe_to_sink_load(
    env: &Env,
    function: &Function,
    load: InstId,
    select: InstId,
) -> bool {
    if function.parent(load) != function.parent(select) {
        return false;
    }
    for inst in function.iter_from(load) {
        if inst == select {
            return true;
        }
        if llrm_mir::memory::of(env.context, env.callees, function, inst).writes {
            return false;
        }
    }
    false
}

/// The instructions exclusively computed to produce `source`: the one-use
/// chain of its operands, nearest first.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L1175>
fn exclusive_backwards_slice(
    env: &Env,
    function: &Function,
    frequencies: &Frequencies,
    source: InstId,
    select: InstId,
    for_sinking: bool,
) -> Vec<InstId> {
    let mut slice = Vec::new();
    let mut visited: IdSet<InstId> = IdSet::new();
    let mut worklist = VecDeque::from([source]);
    let source_frequency = frequencies.get(function.parent(source).expect("a placed instruction"));
    while let Some(inst) = worklist.pop_front() {
        if !visited.insert(inst) || !has_one_use(function, inst) {
            continue;
        }
        let instruction = function.instruction(inst);
        // No side effects, terminators, phis or other selects in a sunk slice.
        if for_sinking
            && (instruction.opcode.is_terminator()
                || crate::local::may_have_side_effects(env.context, env.callees, env.globals, function, inst)
                || matches!(instruction.opcode, Opcode::Select | Opcode::Phi))
        {
            continue;
        }
        // A load sinks only past nothing that may alias it.
        if for_sinking
            && llrm_mir::memory::of(env.context, env.callees, function, inst).reads
            && !is_safe_to_sink_load(env, function, inst, select)
        {
            continue;
        }
        if frequencies.get(function.parent(inst).expect("a placed instruction")) < source_frequency {
            continue;
        }
        slice.push(inst);
        worklist.extend(instruction.operands.iter().filter_map(|&operand| definition(function, operand)));
    }
    slice
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L486>
fn true_or_false_value(
    context: &Context,
    function: &mut Function,
    select: &SelectLike,
    is_true: bool,
    optimized: &IdMap<InstId, (Operand, Operand)>,
    block: BlockId,
) -> Operand {
    let mapped = |function: &Function, value: Operand| -> Operand {
        match definition(function, value).and_then(|inst| optimized.get(&inst)) {
            Some(&(on_true, on_false)) => {
                if is_true {
                    on_true
                } else {
                    on_false
                }
            }
            None => value,
        }
    };
    let value = if is_true { select.true_value(function, true) } else { select.false_value(function, true) };
    if let Some(value) = value {
        return mapped(function, value);
    }
    let clone = function.clone_instruction(select.inst);
    let ty = function.instruction(clone).ty;
    let auxiliary =
        definition(function, function.instruction(clone).operands[select.cond_idx]).expect("an extension or shift");
    let constant = match function.instruction(auxiliary).opcode {
        Opcode::Cast(CastOp::ZExt) | Opcode::Binary(BinaryOp::LShr) => 1,
        _ => -1,
    };
    function.set_operand(clone, select.cond_idx, Operand::Constant(context.int(ty, constant)));
    let other = 1 - select.cond_idx;
    let operand = function.instruction(clone).operands[other];
    if definition(function, operand).is_some_and(|inst| optimized.contains_key(&inst)) {
        function.set_operand(clone, other, mapped(function, operand));
    }
    let terminator = function.terminator(block).expect("a terminated block");
    function.insert(clone, Position::Before(terminator)).expect("an unplaced instruction");
    Operand::Value(function.instruction(clone).result.expect("a binary operation's value"))
}

/// Takes the `n`th instruction of every slice in turn.
fn interleaved(mut slices: Vec<Vec<InstId>>) -> Vec<InstId> {
    let longest = slices.iter().map(Vec::len).max().unwrap_or(0);
    let mut order = Vec::new();
    for _ in 0..longest {
        order.extend(slices.iter_mut().filter_map(Vec::pop));
    }
    order
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L524>
fn convert_profitable_groups(
    env: &Env,
    function: &mut Function,
    declared: &mut Declared,
    frequencies: &mut Frequencies,
    groups: &mut [SelectGroup],
) {
    let context = env.context;
    for at in 0..groups.len() {
        let group = groups[at].clone();
        let mut true_slices = Vec::new();
        let mut false_slices = Vec::new();
        let mut profiled: Option<(InstId, bool)> = None;
        for select in &group.selects {
            if !select.is_select(function) {
                continue;
            }
            let slice = |function: &Function, value: Option<Operand>, frequencies: &Frequencies| {
                let inst = value.and_then(|operand| definition(function, operand))?;
                Some(exclusive_backwards_slice(env, function, frequencies, inst, select.inst, true))
            };
            true_slices.extend(slice(function, select.true_value(function, true), frequencies));
            false_slices.extend(slice(function, select.false_value(function, true), frequencies));
            let condition = function.instruction(select.inst).operands[0];
            if function.instruction(select.inst).metadata.iter().any(|(kind, _)| kind == "prof") {
                if group.condition == condition {
                    profiled = Some((select.inst, false));
                } else if matched_not(context, function, condition).is_some() {
                    profiled = Some((select.inst, true));
                }
            }
        }
        // Interleaving the slices lets the backend find more ILP than one
        // whole chain after another.
        let true_order = interleaved(true_slices);
        let false_order = interleaved(false_slices);

        let first = group.selects[0];
        let last = *group.selects.last().expect("a group has a select");
        let start = function.parent(first.inst).expect("a placed select");
        let split_point = function.next_in_block(last.inst).expect("a terminated block");
        let void = function.instruction(function.terminator(start).expect("a terminated block")).ty;
        let end = split_block_at(function, start, split_point);
        frequencies.set.insert(end, frequencies.get(start));
        let jump = function.terminator(start).expect("the split's jump");
        function.erase(jump).expect("nothing reads a branch");

        // What stood between the selects moves to the end block.
        let members: IdSet<InstId> = group.selects.iter().map(|one| one.inst).collect();
        let between: Vec<InstId> =
            function.iter_range(first.inst, last.inst).filter(|inst| !members.contains(inst)).collect();
        let anchor = function.iter_block(end).next().expect("a non-empty end block");
        for inst in between {
            function.move_to(inst, Position::Before(anchor)).expect("a placed instruction");
        }

        let has_select_like = |function: &Function, is_true: bool| {
            group.selects.iter().any(|one| {
                (if is_true { one.true_value(function, true) } else { one.false_value(function, true) }).is_none()
            })
        };
        let side_block = |function: &mut Function, name: &str, order: &[InstId]| {
            let block = function.create_block(Some(name));
            function.insert_block_before(block, end).expect("a new block");
            let branch =
                function.create_instruction(Opcode::Br, void, vec![Operand::Block(end)], Flags::default(), None);
            function.insert(branch, Position::End(block)).expect("a new block");
            for &inst in order {
                function.move_to(inst, Position::Before(branch)).expect("a placed instruction");
            }
            block
        };
        let mut true_block = (!true_order.is_empty() || has_select_like(function, true))
            .then(|| side_block(function, "select.true.sink", &true_order));
        let mut false_block = (!false_order.is_empty() || has_select_like(function, false))
            .then(|| side_block(function, "select.false.sink", &false_order));
        // With nothing to sink the `false` side is the arbitrary new input.
        if true_block.is_none() && false_block.is_none() {
            false_block = Some(side_block(function, "select.false", &[]));
        }
        // A side without a block of its own goes straight to the end block
        // and its phi input comes from the start block.
        let (on_true, on_false) = match (true_block, false_block) {
            (None, Some(one)) => {
                true_block = Some(start);
                (end, one)
            }
            (Some(one), None) => {
                false_block = Some(start);
                (one, end)
            }
            (Some(a), Some(b)) => (a, b),
            (None, None) => unreachable!("one side has a block"),
        };
        let (true_block, false_block) = (true_block.expect("set above"), false_block.expect("set above"));

        let condition_type = operand_type(context, function, group.condition);
        let frozen_name = match group.condition {
            Operand::Value(value) => function.value(value).name.as_ref().map(|name| format!("{name}.frozen")),
            _ => None,
        };
        let freeze = function.create_instruction(
            Opcode::Freeze,
            condition_type,
            vec![group.condition],
            Flags::default(),
            frozen_name.as_deref(),
        );
        function.insert(freeze, Position::Before(first.inst)).expect("an unplaced instruction");
        let frozen = Operand::Value(function.instruction(freeze).result.expect("a freeze's value"));

        // A later select may read an earlier one: the values pass through the
        // phi made for it.
        let anchor = function.iter_block(end).next().expect("a non-empty end block");
        let mut optimized: IdMap<InstId, (Operand, Operand)> = IdMap::new();
        for select in &group.selects {
            let old = function.instruction(select.inst).result.expect("a select's value");
            let ty = function.instruction(select.inst).ty;
            let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), None);
            function.insert(phi, Position::Before(anchor)).expect("an unplaced instruction");
            let new = function.instruction(phi).result.expect("a phi's value");
            function.take_name(old, new);
            // The select may be another group's condition.
            if context.types.int_bits(ty) == Some(1) {
                for other in groups.iter_mut() {
                    if other.condition == Operand::Value(old) {
                        other.condition = Operand::Value(new);
                    }
                }
            }
            function.replace_all_uses_with(old, Operand::Value(new));
            let on_true_value = true_or_false_value(context, function, select, true, &optimized, true_block);
            let on_false_value = true_or_false_value(context, function, select, false, &optimized, false_block);
            optimized.insert(phi, (on_true_value, on_false_value));
            function.set_operands(phi, from_arms(&[(on_true_value, true_block), (on_false_value, false_block)]));
        }
        let branch = function.create_instruction(
            Opcode::Br,
            void,
            vec![frozen, Operand::Block(on_true), Operand::Block(on_false)],
            Flags::default(),
            None,
        );
        function.insert(branch, Position::End(start)).expect("an unplaced instruction");
        if let Some((select, inverted)) = profiled {
            let id = function.instruction(select).metadata.iter().find(|(kind, _)| kind == "prof").map(|(_, id)| *id);
            let weights = branch_weights(context, env.metadata, declared.nodes(), function, select);
            match (id, weights) {
                (Some(id), _) if !inverted => function.set_metadata(branch, "prof", Some(id)),
                (_, Some((_, [on_true_weight, on_false_weight]))) => {
                    let word = context.types.int(32);
                    let mut operands = vec![MetadataOperand::String("branch_weights".to_owned())];
                    for weight in [on_false_weight, on_true_weight] {
                        operands.push(MetadataOperand::Constant(context.int(word, i128::from(weight))));
                    }
                    let node = declared.node(MetadataNode { distinct: false, operands });
                    function.set_metadata(branch, "prof", Some(node));
                }
                _ => {}
            }
        }
        for select in &group.selects {
            function.erase(select.inst).expect("the select's uses were replaced");
        }
    }
}

/// `SelectOptimizeImpl::run`: whether any select group became a branch.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L357>
pub fn select_optimize(
    env: &Env,
    target: &Target,
    declared: &mut Declared,
    function: &mut Function,
) -> bool {
    // Nothing to do where no kind of select is supported or the target does
    // not enable the pass.
    if !(target.scalar_select || target.scalar_cond_vector_select || target.vector_mask_select) || !target.enabled {
        return false;
    }
    // Branches are bigger than selects (DEVIATE SO1).
    if function
        .attrs
        .iter()
        .any(|attr| matches!(attr, llrm_mir::opcode::Attribute::Flag(name) if name == "optsize" || name == "minsize"))
    {
        return false;
    }
    // A group needs a `select` (`treated_like_select`): without one the
    // dominators, loops and block frequencies below would be built for nothing.
    let has_select = function
        .layout()
        .iter()
        .any(|&block| function.iter_block(block).any(|inst| function.instruction(inst).opcode == Opcode::Select));
    if !has_select {
        return false;
    }
    let (mut groups, mut frequencies) = {
        let function: &Function = function;
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let bpi = llrm_analysis::bpi::BranchProbabilityInfo::of(
            llrm_analysis::bpi::Module {
                context: env.context,
                declarations: env.globals,
                metadata: env.metadata,
                layout: env.layout,
            },
            function,
        );
        let frequencies = Frequencies { base: BlockFrequencies::of_bpi(function, &bpi), set: IdMap::new() };
        let analysis =
            Analysis { env, target, function, declared: declared.nodes(), loops: &loops, frequencies: &frequencies };
        (analysis.profitable_groups(), frequencies)
    };
    convert_profitable_groups(env, function, declared, &mut frequencies, &mut groups);
    !groups.is_empty()
}

/// `SelectOptimize` as a pipeline pass, where the target asks for it
/// (`Tti::branches_constant_arm_selects`).
///
/// DEVIATE SELOPT-C (owner m64-b, exit 2026-11-24): the pin runs the pass in no
/// `llc` (`-disable-select-optimize` defaults to true) and its
/// `shouldTreatInstructionLikeSelect` leaves a select of two constants alone.
/// bintree -O3: `insert`'s `t[3*at + (key < k ? 1 : 2)]` walk is a loop-carried
/// load-address chain; the select becomes `setg; sub` on it (19.5k clk, as
/// clang) where gcc keeps the branch (10.8k). Here the pass's own loop cost
/// model decides, constant arms included: 9.95k.
pub struct SelectOptimizePass;

impl llrm_mir::passes::FunctionPass for SelectOptimizePass {
    fn name(&self) -> &'static str {
        "selectoptimize"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        use llrm_mir::passes::PreservedAnalyses;
        let Some(tti) = analyses.outer().target().tti().filter(|tti| tti.branches_constant_arm_selects()) else {
            return PreservedAnalyses::all();
        };
        let mut target = Target::base(&*tti);
        // x86: out-of-order cores; the Sandy Bridge model's penalty.
        target.predictable_select_expensive = true;
        target.misprediction_penalty = 16;
        target.constant_arm_selects = true;
        let outer = analyses.outer().clone();
        let env = Env {
            context: &*unit.context,
            globals: &outer.globals,
            metadata: unit.metadata,
            layout: unit.layout,
            callees: outer.callees(),
        };
        if select_optimize(&env, &target, unit.declared, unit.function) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "selectoptimize_tests.rs"]
mod tests;
