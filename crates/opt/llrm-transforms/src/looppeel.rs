//! How many iterations of a loop to peel off its front, from `LoopPeel.cpp`'s
//! `computePeelCount`: the iterations after which its header phis, or the
//! compares in its body, are decided.
//!
//! # How it works
//! `compute_peel_count` is asked by the loop unroller after the unroll count
//! it would choose has failed. A loop must be in simplified form, an
//! innermost loop (unless nests are allowed), and at most
//! `Threshold / LoopSize - 1` iterations (and 7, `unroll-peel-max-count`) are
//! peeled, counting those already peeled. The count is the largest of
//! - the iterations after which every header phi is the same on every trip
//!   (`PhiAnalyzer`: a phi takes its back-edge value's count plus one, an
//!   operation the largest of its operands', a loop-invariant value none), and
//! - the iterations after which a compare in the body is decided
//!   (`countToEliminateCompares`): an `icmp` of an affine recurrence of the
//!   loop against an invariant, under a branch, a `select`, or the `and`/`or`
//!   of such compares, is stepped through the iterations while it holds, and
//!   the count is the iterations until it flips; `min`/`max` on a recurrence
//!   likewise.
//!
//! ```text
//!   for (i = 1; i <= n; i++) { ...; if (i > 1) f(i); }     peel 1:  body without f; for (i = 2; ...) { ...; f(i) }
//! ```
//!
//! The peeling is [`crate::loopclone::peeled_any_latch`]: the iterations are
//! copied ahead of the loop, each keeping its exits.
//!
//! # Deviations
//! - PL1 `llvm.loop.peeled.count` is not written (a pass cannot add a metadata
//!   node): the caller keeps the count for the header in the run, so a loop
//!   revisited in the same run is bounded as the pin's is. Exit: metadata nodes
//!   made by function passes.
//! - PL2 `isDereferenceablePointer` of
//!   `peelToTurnInvariantLoadsDereferenceable` is
//!   `valuetracking::dereferenceable`: an argument attribute, an alloca or a
//!   global of known size; not the pin's walk with the dominator tree and the
//!   assumptions, so a load the pin proves dereferenceable by those is peeled
//!   for here. Exit: that walk.
//! - PL3 Peeling the last iteration (`PeelLast`, `canPeelLastIteration`,
//!   `shouldPeelLastIteration`) is not translated: no loop is peeled from the
//!   end, and a count that would have been is dropped.
//! - PL4 Profile-based peeling is not translated (no profile data).
//!
//! # Example
//! The loop of `bintree` (`for (i = 1; i <= 300; ++i) { t[3*i] = ...; if (i >
//! 1) insert(t, 1, i); }`) tests `i > 1` on each trip; `i` is 1 on the first
//! and more after, so one iteration is peeled and the test is gone from both.
//!
//! # Runs
//! Inside [`crate::loopunrollpass`], in LLVM's order after full and bounded
//! unrolling and before partial unrolling.
//!
//! # See also
//! [`computePeelCount`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopPeel.cpp#L753),
//! [`countToEliminateCompares`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopPeel.cpp#L549),
//! [`PhiAnalyzer`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopPeel.cpp#L195).

use llrm_analysis::scev::expr::{Kind, Scev, Use};
use llrm_analysis::scev::{Cx, Flags, ScalarEvolution};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::loopinfo::Loop;
use llrm_mir::memory::Callees;
use llrm_mir::module::{InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, IntPredicate, Opcode};

/// `-unroll-peel-max-count`.
const UNROLL_PEEL_MAX_COUNT: u32 = 7;
/// `-enable-peeling-for-iv`.
const ENABLE_PEELING_FOR_IV: bool = false;
/// The depth `countToEliminateCompares` follows `and` and `or` to.
const MAX_DEPTH: u32 = 4;

/// `TargetTransformInfo::PeelingPreferences`, less what the translation does
/// not read (PL3, PL4).
#[derive(Clone, Copy, Debug)]
pub struct PeelingPreferences {
    pub peel_count: u32,
    pub allow_peeling: bool,
    pub allow_loop_nests_peeling: bool,
}

impl Default for PeelingPreferences {
    /// `gatherPeelingPreferences`' defaults: no target states any.
    fn default() -> Self {
        Self { peel_count: 0, allow_peeling: true, allow_loop_nests_peeling: false }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CounterType {
    Invariant,
    Induction,
}

/// `PeelCounter`: the iterations after which a value is invariant or an
/// induction, or none that is within the limit.
type Counter = Option<(u32, CounterType)>;

/// `PhiAnalyzer`.
struct PhiAnalyzer<'a, 'b> {
    cx: &'a Cx<'a>,
    lp: Loop<'b>,
    max_iterations: u32,
    peel_for_iv: bool,
    known: IdMap<ValueId, Counter>,
}

impl<'a, 'b> PhiAnalyzer<'a, 'b> {
    fn new(
        cx: &'a Cx<'a>,
        lp: Loop<'b>,
        max_iterations: u32,
        peel_for_iv: bool,
    ) -> Self {
        debug_assert!(max_iterations > 0, "no peeling is allowed?");
        Self { cx, lp, max_iterations, peel_for_iv, known: IdMap::new() }
    }

    fn add_one(
        &self,
        counter: Counter,
    ) -> Counter {
        let (value, ty) = counter?;
        (value + 1 <= self.max_iterations).then_some((value + 1, ty))
    }

    fn def_of(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = operand else { return None };
        match self.cx.function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    /// `isInductionPHI`: the back-edge value leads back to the phi through
    /// casts and additions or subtractions of constants, at least one of them.
    fn is_induction_phi(
        &self,
        phi: InstId,
    ) -> bool {
        let function = self.cx.function;
        let Some(latch) = self.lp.get_loop_latch(function) else { return false };
        let Some(mut current) =
            llrm_analysis::occurrence::arms(function, phi).into_iter().find(|&(_, from)| from == latch).map(|(v, _)| v)
        else {
            return false;
        };
        let mut visited = Vec::new();
        let mut visited_binary = false;
        loop {
            if self.def_of(current) == Some(phi) {
                return visited_binary;
            }
            if visited.contains(&current) {
                return false;
            }
            visited.push(current);
            let Some(inst) = self.def_of(current).filter(|&inst| self.lp.contains_instruction(function, inst)) else {
                return false;
            };
            let end = function.instruction(inst);
            match &end.opcode {
                Opcode::Cast(_) => current = end.operands[0],
                Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) if matches!(end.operands[1], Operand::Constant(_)) => {
                    visited_binary = true;
                    current = end.operands[0];
                }
                _ => return false,
            }
        }
    }

    /// `mergeTwoCounters`.
    fn merge(
        &self,
        inst: InstId,
        lhs: (u32, CounterType),
        rhs: (u32, CounterType),
    ) -> Counter {
        let value = lhs.0.max(rhs.0);
        if lhs.1 == CounterType::Induction || rhs.1 == CounterType::Induction {
            return matches!(
                self.cx.function.instruction(inst).opcode,
                Opcode::Binary(BinaryOp::Add | BinaryOp::Sub)
            )
                .then_some((value, CounterType::Induction));
        }
        Some((value, CounterType::Invariant))
    }

    /// `calculate`: the iterations after which `operand` is invariant or an
    /// induction.
    fn calculate(
        &mut self,
        operand: Operand,
    ) -> Counter {
        let function = self.cx.function;
        if self.lp.is_loop_invariant(function, operand) {
            return Some((0, CounterType::Invariant));
        }
        let Operand::Value(value) = operand else { return None };
        // A cycle never ends on an invariant.
        if let Some(&known) = self.known.get(&value) {
            return known;
        }
        self.known.insert(value, None);
        let inst = self.def_of(operand)?;
        let end = function.instruction(inst);
        let counter = match &end.opcode {
            Opcode::Phi => {
                // A phi outside the header is unknown.
                if function.parent(inst) != Some(self.lp.get_header()) {
                    return None;
                }
                if self.peel_for_iv && self.is_induction_phi(inst) {
                    Some((0, CounterType::Induction))
                } else {
                    let latch = self.lp.get_loop_latch(function)?;
                    let input = llrm_analysis::occurrence::arms(function, inst)
                        .into_iter()
                        .find(|&(_, from)| from == latch)
                        .map(|(v, _)| v)?;
                    let iterations = self.calculate(input);
                    self.add_one(iterations)
                }
            }
            Opcode::Binary(_) | Opcode::ICmp(_) | Opcode::FCmp(_) => {
                let (a, b) = (end.operands[0], end.operands[1]);
                let lhs = self.calculate(a)?;
                let rhs = self.calculate(b)?;
                self.merge(inst, lhs, rhs)
            }
            Opcode::Cast(_) => self.calculate(end.operands[0]),
            _ => None,
        };
        self.known.insert(value, counter);
        counter
    }

    /// `calculateIterationsToPeel`.
    fn iterations_to_peel(&mut self) -> Option<u32> {
        let function = self.cx.function;
        let mut iterations = 0;
        for phi in crate::edges::phis(function, self.lp.get_header()) {
            let result = function.instruction(phi).result.expect("a phi's value");
            if let Some((value, _)) = self.calculate(Operand::Value(result)) {
                debug_assert!(value <= self.max_iterations, "bad result in phi analysis");
                iterations = iterations.max(value);
                if iterations == self.max_iterations {
                    break;
                }
            }
        }
        (iterations != 0).then_some(iterations)
    }
}

/// `canPeel`: the loop is in simplified form.
pub fn can_peel(
    cx: &Cx,
    lp: Loop,
) -> bool {
    let function = cx.function;
    lp.get_loop_preheader(function).is_some()
        && lp.has_dedicated_exits(function)
        && lp.get_loop_latch(function).is_some()
}

/// `countToEliminateCompares`: the iterations to peel from the front so that
/// a compare in the body is decided. (PL3: not those to peel from the end.)
fn count_to_eliminate_compares(
    se: &mut ScalarEvolution,
    cx: &Cx,
    lp: Loop,
    mut max_peel_count: u32,
) -> u32 {
    let function = cx.function;
    let header = lp.get_header();
    let mut desired = 0u32;

    // Do not peel the entire loop.
    let backedges =
        se.get_backedge_taken_count(cx, header, llrm_analysis::scev::backedge::ExitCountKind::ConstantMaximum);
    if let Some(count) = se.as_constant(backedges) {
        let limit = count.zext_value().min(u128::from(u32::MAX)) as u32;
        max_peel_count = max_peel_count.min(limit.wrapping_sub(1));
    }

    // Increase the count while `iter pred bound` holds; whether the inverse
    // condition becomes known before the limit.
    let peel_while_predicate_is_known = |se: &mut ScalarEvolution,
                                         count: &mut u32,
                                         iter: &mut Scev,
                                         bound: Scev,
                                         step: Scev,
                                         pred: IntPredicate|
     -> bool {
        while *count < max_peel_count && se.is_known_predicate_scev(cx, pred, *iter, bound) {
            *iter = se.get_add_expr(cx, vec![Use::of(*iter), Use::of(step)], Flags::default(), 0).scev;
            *count += 1;
        }
        se.is_known_predicate_scev(cx, pred.inverse(), *iter, bound)
    };

    // The compares to look at: those under a condition, as `ComputePeelCount`.
    let mut conditions: Vec<Operand> = Vec::new();
    let mut minmax: Vec<InstId> = Vec::new();
    for &block in lp.get_blocks() {
        for inst in function.iter_block(block) {
            let end = function.instruction(inst);
            if end.opcode == Opcode::Select {
                conditions.push(end.operands[0]);
            }
            if is_min_max(cx, inst) {
                minmax.push(inst);
            }
        }
        // Ignore the loop exit condition.
        if lp.get_loop_latch(function) == Some(block) {
            continue;
        }
        if let Some(term) = function.terminator(block) {
            let end = function.instruction(term);
            if end.opcode == Opcode::Br && end.operands.len() == 3 {
                conditions.push(end.operands[0]);
            }
        }
    }
    // LLVM visits the instructions of a block, then its branch, in order: the
    // maximum is the same whatever the order.
    let mut work: Vec<(Operand, u32)> = conditions.into_iter().map(|one| (one, 0)).collect();
    while let Some((condition, depth)) = work.pop() {
        if depth >= MAX_DEPTH {
            continue;
        }
        let Some(inst) = def_inst(cx, condition) else { continue };
        let end = function.instruction(inst);
        if cx.context.types.int_bits(end.ty).is_none() {
            continue;
        }
        if let Opcode::Binary(BinaryOp::And | BinaryOp::Or) = end.opcode {
            work.push((end.operands[0], depth + 1));
            work.push((end.operands[1], depth + 1));
            continue;
        }
        let Opcode::ICmp(mut pred) = end.opcode else { continue };
        let (left, right) = (end.operands[0], end.operands[1]);
        if !se.is_value_scevable(cx, left) || !se.is_value_scevable(cx, right) {
            continue;
        }
        let mut left_scev = se.get_scev(cx, left);
        let mut right_scev = se.get_scev(cx, right);

        // Do not consider predicates that are known to be true or false
        // independently of the loop iteration.
        if se.evaluate_predicate(cx, pred, left_scev, right_scev).is_some() {
            continue;
        }
        // One add recurrence and one that is not; the recurrence on the left.
        if !matches!(se.node(left_scev).kind, Kind::AddRec(..)) {
            if matches!(se.node(right_scev).kind, Kind::AddRec(..)) {
                std::mem::swap(&mut left_scev, &mut right_scev);
                pred = pred.swapped();
            } else {
                continue;
            }
        }
        // Only affine recurrences of this loop.
        if !se.is_affine(left_scev) || se.addrec_header(left_scev) != Some(header) {
            continue;
        }
        let is_equality = matches!(pred, IntPredicate::Eq | IntPredicate::Ne);
        if !(is_equality && se.has_no_self_wrap(left_scev))
            && se.get_monotonic_predicate_type(cx, left_scev, pred).is_none()
        {
            continue;
        }

        // Does extending the count let us evaluate `pred` or `!pred` in the
        // body statically?
        let mut new_count = desired;
        let at = se.get_constant_of_type(cx, se.type_of(left_scev), i128::from(new_count));
        let mut iter = se.evaluate_at_iteration(cx, left_scev, at);

        // If the original condition is not known, `!pred` holds on the else
        // branch: peel the iterations that make it false.
        if !se.is_known_predicate_scev(cx, pred, iter, right_scev) {
            pred = pred.inverse();
        }
        let step = se.get_step_recurrence(cx, left_scev);
        if !peel_while_predicate_is_known(se, &mut new_count, &mut iter, right_scev, step, pred) {
            continue;
        }

        // An equality is not always decided by that: one more iteration may be
        // needed. See if it makes `!pred` unknown again.
        let next = se.get_add_expr(cx, vec![Use::of(iter), Use::of(step)], Flags::default(), 0).scev;
        if matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
            && !se.is_known_predicate_scev(cx, pred.inverse(), next, right_scev)
            && !se.is_known_predicate_scev(cx, pred, iter, right_scev)
            && se.is_known_predicate_scev(cx, pred, next, right_scev)
        {
            if new_count >= max_peel_count {
                // Need to peel one more iteration, but can't. Give up.
                continue;
            }
            new_count += 1;
        }
        desired = desired.max(new_count);
    }

    for inst in minmax {
        let Some((lhs, rhs, signed)) = min_max_operands(cx, inst) else { continue };
        let (bound, iterated) = if lp.is_loop_invariant(function, lhs) {
            (lhs, rhs)
        } else if lp.is_loop_invariant(function, rhs) {
            (rhs, lhs)
        } else {
            continue;
        };
        if !se.is_value_scevable(cx, bound) || !se.is_value_scevable(cx, iterated) {
            continue;
        }
        let bound_scev = se.get_scev(cx, bound);
        let iter_scev = se.get_scev(cx, iterated);
        // For simplicity, only affine recurrences.
        if !matches!(se.node(iter_scev).kind, Kind::AddRec(..))
            || !se.is_affine(iter_scev)
            || se.addrec_header(iter_scev) != Some(header)
        {
            continue;
        }
        let step = se.get_step_recurrence(cx, iter_scev);
        // To minimize the peeled iterations, strict predicates.
        let pred = if se.is_known_positive(cx, step) {
            if signed { IntPredicate::Slt } else { IntPredicate::Ult }
        } else if se.is_known_negative(cx, step) {
            if signed { IntPredicate::Sgt } else { IntPredicate::Ugt }
        } else {
            continue;
        };
        // The recurrence must not wrap.
        let wrap_flag =
            if signed { llrm_analysis::scev::expr::flags::NSW } else { llrm_analysis::scev::expr::flags::NUW };
        let wraps = se.get_no_wrap_flags(iter_scev, wrap_flag) == 0;
        if wraps {
            continue;
        }
        let mut new_count = desired;
        let at = se.get_constant_of_type(cx, se.type_of(iter_scev), i128::from(new_count));
        let mut iter = se.evaluate_at_iteration(cx, iter_scev, at);
        if !peel_while_predicate_is_known(se, &mut new_count, &mut iter, bound_scev, step, pred) {
            continue;
        }
        desired = new_count;
    }
    desired
}

fn def_inst(
    cx: &Cx,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match cx.function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// `smax`, `smin`, `umax` or `umin` of two integers: its operands and whether
/// it is signed.
fn min_max_operands(
    cx: &Cx,
    inst: InstId,
) -> Option<(Operand, Operand, bool)> {
    use llrm_mir::valuetracking::pattern::{IntrinsicId, intrinsic_id_of_name};
    let end = cx.function.instruction(inst);
    let Opcode::Call(_) = end.opcode else { return None };
    cx.context.types.int_bits(end.ty)?;
    let callee = llrm_mir::memory::callee(cx.context, cx.function, inst)?;
    let name = cx.globals.get(callee.0 as usize)?.name.as_deref()?;
    let signed = match intrinsic_id_of_name(name)? {
        IntrinsicId::Smax | IntrinsicId::Smin => true,
        IntrinsicId::Umax | IntrinsicId::Umin => false,
        _ => return None,
    };
    Some((end.operands[0], end.operands[1], signed))
}

fn is_min_max(
    cx: &Cx,
    inst: InstId,
) -> bool {
    min_max_operands(cx, inst).is_some()
}

/// `peelToTurnInvariantLoadsDereferenceable`: 1 if an invariant load that is
/// not known dereferenceable, and dominates the latch, feeds (transitively) a
/// condition that leaves the loop, in a loop whose other exits are
/// `unreachable`: once an iteration is peeled the load is known to be
/// dereferenceable in the rest, and can be hoisted.
fn peel_to_turn_invariant_loads_dereferenceable(
    cx: &Cx,
    callees: &Callees,
    lp: Loop,
) -> u32 {
    let function = cx.function;
    // Loops with a single exiting block have nothing to gain.
    if lp.get_exiting_block(function).is_some() {
        return 0;
    }
    // All non-latch exit blocks must end in `unreachable`; otherwise the
    // heuristic may not be profitable.
    if lp.get_unique_non_latch_exit_blocks(function).iter().any(|&exit| {
        function.terminator(exit).is_none_or(|term| function.instruction(term).opcode != Opcode::Unreachable)
    }) {
        return 0;
    }
    let (Some(header), Some(latch)) = (Some(lp.get_header()), lp.get_loop_latch(function)) else { return 0 };
    let mut load_users: IdSet<InstId> = IdSet::new();
    let mut sizes = None;
    for &block in lp.get_blocks() {
        for inst in function.iter_block(block) {
            // A write may alias the loads; calls that reach only inaccessible
            // memory never do.
            if llrm_mir::memory::of(cx.context, callees, function, inst).writes {
                let inaccessible_only =
                    matches!(
                        function.instruction(inst).opcode,
                        Opcode::Call(_) | Opcode::Invoke(_)
                    )
                        && llrm_mir::memory::callee(cx.context, function, inst)
                            .and_then(|callee| callees.get(&callee))
                            .is_some_and(|summary| summary.accessible == llrm_mir::memory::Effects::NONE);
                if !inaccessible_only {
                    return 0;
                }
            }
            if load_users.contains(&inst) {
                add_users(function, inst, &mut load_users);
            }
            // Reads in the header can be hoisted without peeling.
            if block == header {
                continue;
            }
            let end = function.instruction(inst);
            let Opcode::Load { .. } = end.opcode else { continue };
            let pointer = end.operands[0];
            if cx.tree.dominates(block, latch) && lp.is_loop_invariant(function, pointer) {
                let sizes =
                    sizes.get_or_insert_with(|| llrm_mir::valuetracking::sizes_of(cx.context, cx.globals, cx.layout));
                let bytes = cx.layout.alloc_size(&cx.context.types, end.ty);
                if !llrm_mir::valuetracking::dereferenceable(cx.context, cx.layout, sizes, function, pointer, bytes) {
                    add_users(function, inst, &mut load_users);
                }
            }
        }
    }
    let feeds_an_exit = lp
        .get_exiting_blocks(function)
        .into_iter()
        .any(|exiting| function.terminator(exiting).is_some_and(|term| load_users.contains(&term)));
    u32::from(feeds_an_exit)
}

/// `LoadUsers.insert_range(I.users())`.
fn add_users(
    function: &llrm_mir::module::Function,
    inst: InstId,
    users: &mut IdSet<InstId>,
) {
    if let Some(result) = function.instruction(inst).result {
        users.extend(function.users(result).iter().map(|one| one.user));
    }
}

/// `computePeelCount`: how many iterations of `lp` to peel (`pp.peel_count`,
/// zero for none) given its size, the exact trip count (zero if unknown), the
/// unroll threshold and the iterations already peeled.
#[allow(clippy::too_many_arguments)]
pub fn compute_peel_count(
    se: &mut ScalarEvolution,
    cx: &Cx,
    callees: &Callees,
    lp: Loop,
    loop_size: u32,
    pp: &mut PeelingPreferences,
    threshold: u32,
    already_peeled: u32,
) {
    debug_assert!(loop_size > 0, "Zero loop size is not allowed!");
    // The count a target states.
    let target_peel_count = pp.peel_count;
    pp.peel_count = 0;
    if !can_peel(cx, lp) {
        return;
    }
    // Only innermost loops by default.
    if !pp.allow_loop_nests_peeling && !lp.is_innermost() {
        return;
    }
    // Skip peeling if it is disabled.
    if !pp.allow_peeling {
        return;
    }
    // Check that we can peel at least one iteration.
    if loop_size.saturating_mul(2) > threshold {
        return;
    }
    // Stop if we already peeled off the maximum number of iterations.
    if already_peeled >= UNROLL_PEEL_MAX_COUNT {
        return;
    }
    // Pay respect to the limits of the loop size and the max peel count.
    let max_peel_count = UNROLL_PEEL_MAX_COUNT.min((threshold / loop_size).saturating_sub(1));

    let mut desired = target_peel_count;

    // Get rid of the phis that become invariants (or inductions) after 1, 2,
    // ..., N iterations: peel the largest of those counts.
    if max_peel_count > desired
        && let Some(count) = PhiAnalyzer::new(cx, lp, max_peel_count, ENABLE_PEELING_FOR_IV).iterations_to_peel()
    {
        desired = desired.max(count);
    }

    desired = desired.max(count_to_eliminate_compares(se, cx, lp, max_peel_count));

    if desired == 0 {
        desired = peel_to_turn_invariant_loads_dereferenceable(cx, callees, lp);
    }

    if desired > 0 {
        desired = desired.min(max_peel_count);
        // Consider the max peel count limitation.
        if desired + already_peeled <= UNROLL_PEEL_MAX_COUNT {
            llrm_support::debug!("unroll", "peel {desired} iteration(s) to turn some phis into invariants");
            pp.peel_count = desired;
        }
    }
}

#[cfg(test)]
#[path = "looppeel_tests.rs"]
mod tests;
