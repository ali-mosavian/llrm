//! Runtime memory checks: code that proves, when a loop is entered, that the
//! groups of pointers it accesses do not overlap.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopUtils.cpp` (`expandBounds`,
//! `addRuntimeChecks`, `addDiffRuntimeChecks`). Each check names two groups
//! of a [`RuntimePointerChecking`]; their bounds are expanded before `loc`
//! with the SCEV expander, and the groups conflict when
//! `start(A) <u end(B) && start(B) <u end(A)`. The conflicts are or-ed into
//! one `i1`. The diff form compares `(sink - src - 1) <u (VF * IC * size - 1)`
//! instead.
//!
//! # Example
//! `a[i] = b[i]` over `n` trips: one check, `a < b + 4n && b < a + 4n`, true
//! when the two ranges meet.
//!
//! # DEVIATE 28 (fixed vectors)
//! `addDiffRuntimeChecks` takes `VF` as a count, not an `ElementCount`: MIR
//! has no scalable vectors (DEVIATE 2). Exit: scalable types in MIR.
//!
//! # Runs
//! Called by [`crate::loopversioning`]; the vectorizer will call it too.

use llrm_analysis::runtimecheck::{
    PointerDiffInfo, RuntimeCheckingPtrGroup, RuntimePointerCheck, RuntimePointerChecking,
};
use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expander::{Env, Expander};
use llrm_analysis::scev::expr::{Kind, Scev, Ty, Use};
use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::simplify::{simplify_binop_operands, simplify_icmp_operands};
use llrm_support::hash::HashMap;

/// What expanding and building check code needs.
pub struct Rewriter<'a> {
    pub context: &'a mut Context,
    pub declared: &'a mut Declared,
    pub function: &'a mut Function,
    pub se: &'a mut ScalarEvolution,
    pub env: Env<'a>,
    pub callees: &'a Callees,
}

/// `PointerBounds`: the lower and upper bound of a pointer group in IR, and
/// the stride that must be non-negative for them to hold.
#[derive(Clone, Copy, Debug)]
struct PointerBounds {
    start: Operand,
    end: Operand,
    stride_to_check: Option<Operand>,
}

impl Rewriter<'_> {
    /// `IRBuilder<InstSimplifyFolder>::CreateICmp` before `loc`.
    fn icmp(
        &mut self,
        loc: InstId,
        predicate: IntPredicate,
        a: Operand,
        b: Operand,
        name: &str,
    ) -> Operand {
        if let Some(found) = simplify_icmp_operands(self.context, self.env.layout, self.function, predicate, a, b) {
            return found;
        }
        let ty = self.context.types.int(1);
        self.build(loc, Opcode::ICmp(predicate), ty, vec![a, b], name)
    }

    /// `IRBuilder<InstSimplifyFolder>::CreateBinOp` before `loc`.
    fn binop(
        &mut self,
        loc: InstId,
        op: BinaryOp,
        a: Operand,
        b: Operand,
        name: &str,
    ) -> Operand {
        if let Some(found) =
            simplify_binop_operands(self.context, self.env.layout, self.function, op, Flags::default(), a, b)
        {
            return found;
        }
        let ty = self.function.operand_type(self.context, a).expect("a typed operand");
        self.build(loc, Opcode::Binary(op), ty, vec![a, b], name)
    }

    /// `IRBuilder::CreateFreeze(v, v's name + ".fr")` before `loc`.
    fn freeze(
        &mut self,
        loc: InstId,
        v: Operand,
    ) -> Operand {
        let ty = self.function.operand_type(self.context, v).expect("a typed operand");
        let name = match v {
            Operand::Value(value) => self.function.value(value).name.clone().unwrap_or_default(),
            _ => String::new(),
        };
        self.build(loc, Opcode::Freeze, ty, vec![v], &format!("{name}.fr"))
    }

    fn build(
        &mut self,
        loc: InstId,
        opcode: Opcode,
        ty: llrm_mir::types::TypeId,
        operands: Vec<Operand>,
        name: &str,
    ) -> Operand {
        let name = (!name.is_empty()).then_some(name);
        let inst = self.function.create_instruction(opcode, ty, operands, Flags::default(), name);
        self.function.insert(inst, Position::Before(loc)).expect("a placed insert point");
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    fn expand(
        &mut self,
        exp: &mut Expander,
        s: Scev,
        ty: Option<Ty>,
    ) -> Operand {
        exp.expand_code_for(&self.env, self.context, self.declared, self.function, self.se, s, ty)
    }

    /// `eraseDeadInstructions(root)` over the expander's code.
    fn erase_dead(
        &mut self,
        exp: &mut Expander,
        root: Option<Operand>,
    ) {
        let (context, callees, globals) = (&*self.context, self.callees, self.env.globals);
        exp.erase_dead_instructions(self.function, root, &|function, inst| {
            crate::local::is_instruction_trivially_dead(context, callees, globals, function, inst)
        });
    }

    /// The range of the outer loop's iterations that covers `low..high`, when
    /// both are recurrences of the loop around `the_loop` with one step: the
    /// bounds and the step that must then be checked non-negative.
    fn hoisted_range(
        &mut self,
        the_loop: BlockId,
        low: Scev,
        high: Scev,
    ) -> Option<(Scev, Scev, Option<Scev>)> {
        let cx = self.env.cx(self.context, self.function);
        let outer = cx.loops.get_loop_for(the_loop)?.get_parent_loop()?;
        let (Kind::AddRec(low_operands, low_loop), Kind::AddRec(_, high_loop)) =
            (self.se.node(low).kind.clone(), self.se.node(high).kind.clone())
        else {
            return None;
        };
        let recur = self.se.get_step_recurrence(&cx, low);
        if recur != self.se.get_step_recurrence(&cx, high)
            || high_loop != outer.get_header()
            || low_loop != outer.get_header()
        {
            return None;
        }
        let latch = outer.get_loop_latch(cx.function)?;
        let outer_exit_count = self.se.get_exit_count(&cx, outer.get_header(), latch, ExitCountKind::Exact);
        if self.se.is_could_not_compute(outer_exit_count) || !matches!(self.se.type_of(outer_exit_count), Ty::Int(_)) {
            return None;
        }
        let new_high = self.se.evaluate_at_iteration(&cx, high, outer_exit_count);
        if self.se.is_could_not_compute(new_high) {
            return None;
        }
        // A stride that may be negative needs a check of its own.
        let guarded = self.se.apply_loop_guards(&cx, recur, high_loop);
        let stride = (!self.se.is_known_non_negative(&cx, guarded)).then_some(recur);
        Some((low_operands[0].scev, new_high, stride))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L2186
    /// `expandBounds(CG, TheLoop, Loc, Exp, HoistRuntimeChecks)`: code for the
    /// lower and upper bound of the group `cg` before `loc`.
    fn expand_group_bounds(
        &mut self,
        exp: &mut Expander,
        cg: &RuntimeCheckingPtrGroup,
        the_loop: BlockId,
        loc: InstId,
        hoist_runtime_checks: bool,
    ) -> PointerBounds {
        let ptr_arith_ty = Ty::Ptr(cg.address_space);
        let (mut low, mut high, mut stride) = (cg.low, cg.high, None);

        // If the Low and High values are themselves loop-variant, expand the
        // range to include those covered by the outer loop as well, so that
        // the checks can be hoisted out of it. The trade-off is the chance
        // that the checked loop is never entered: hence the parameter.
        if hoist_runtime_checks && let Some((new_low, new_high, new_stride)) = self.hoisted_range(the_loop, low, high) {
            (low, high, stride) = (new_low, new_high, new_stride);
        }

        exp.set_insert_point(loc);
        let mut start = self.expand(exp, low, Some(ptr_arith_ty));
        let mut end = self.expand(exp, high, Some(ptr_arith_ty));
        if cg.needs_freeze {
            start = self.freeze(loc, start);
            end = self.freeze(loc, end);
        }
        let stride_to_check = stride.map(|stride| {
            let ty = self.se.type_of(stride);
            self.expand(exp, stride, Some(ty))
        });
        PointerBounds { start, end, stride_to_check }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L2252
    /// `expandBounds(PointerChecks, L, Loc, Exp, HoistRuntimeChecks)`: the
    /// bounds of both groups of every check. The expander's cache emits the
    /// code of one bound once.
    fn expand_bounds(
        &mut self,
        exp: &mut Expander,
        rtc: &RuntimePointerChecking,
        checks: &[RuntimePointerCheck],
        the_loop: BlockId,
        loc: InstId,
        hoist_runtime_checks: bool,
    ) -> Vec<(PointerBounds, PointerBounds)> {
        checks
            .iter()
            .map(|&(first, second)| {
                let first =
                    self.expand_group_bounds(exp, &rtc.checking_groups[first], the_loop, loc, hoist_runtime_checks);
                let second =
                    self.expand_group_bounds(exp, &rtc.checking_groups[second], the_loop, loc, hoist_runtime_checks);
                (first, second)
            })
            .collect()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L2273
    /// `addRuntimeChecks(Loc, TheLoop, PointerChecks, Exp,
    /// HoistRuntimeChecks)`: the memory checks of `checks` before `loc`,
    /// or-ed into one `i1` that is true when some pair of groups may
    /// overlap; none where there is no check or all fold away.
    pub fn add_runtime_checks(
        &mut self,
        exp: &mut Expander,
        loc: InstId,
        the_loop: BlockId,
        rtc: &RuntimePointerChecking,
        checks: &[RuntimePointerCheck],
        hoist_runtime_checks: bool,
    ) -> Option<Operand> {
        let expanded = self.expand_bounds(exp, rtc, checks, the_loop, loc, hoist_runtime_checks);
        // Our instructions might fold to a constant.
        let mut memory_runtime_check: Option<Operand> = None;

        for (a, b) in expanded {
            // [A|B].Start points to the first accessed byte under base
            // [A|B], [A|B].End to the last accessed byte plus one. The
            // intervals are disjoint when
            // NoConflict = (B.Start >= A.End) || (A.Start >= B.End), so
            // IsConflict = (B.Start < A.End) & (A.Start < B.End).
            debug_assert!(
                self.space_of(a.start) == self.space_of(b.end) && self.space_of(b.start) == self.space_of(a.end),
                "Trying to bounds check pointers with different address spaces"
            );
            let cmp0 = self.icmp(loc, IntPredicate::Ult, a.start, b.end, "bound0");
            let cmp1 = self.icmp(loc, IntPredicate::Ult, b.start, a.end, "bound1");
            let mut is_conflict = self.binop(loc, BinaryOp::And, cmp0, cmp1, "found.conflict");
            for stride in [a.stride_to_check, b.stride_to_check].into_iter().flatten() {
                let ty = self.function.operand_type(self.context, stride).expect("a typed stride");
                let zero = Operand::Constant(self.context.int(ty, 0));
                let is_negative_stride = self.icmp(loc, IntPredicate::Slt, stride, zero, "stride.check");
                is_conflict = self.binop(loc, BinaryOp::Or, is_conflict, is_negative_stride, "");
            }
            if let Some(previous) = memory_runtime_check {
                is_conflict = self.binop(loc, BinaryOp::Or, previous, is_conflict, "conflict.rdx");
            }
            memory_runtime_check = Some(is_conflict);
        }

        self.erase_dead(exp, memory_runtime_check);
        memory_runtime_check
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L2316
    /// `addDiffRuntimeChecks(Loc, Checks, Expander, VF, IC)`: for each pair
    /// the check `(sink - src - 1) <u (VF * IC * size - 1)`, which is
    /// `0 < sink - src <u VF * IC * size`: equal pointers are safe. A compare
    /// of the same two operands is made once.
    pub fn add_diff_runtime_checks(
        &mut self,
        exp: &mut Expander,
        loc: InstId,
        checks: &[PointerDiffInfo],
        vf: u32,
        ic: u32,
    ) -> Option<Operand> {
        // Our instructions might fold to a constant.
        let mut memory_runtime_check: Option<Operand> = None;
        let mut seen_compares: HashMap<(Operand, Operand), Operand> = HashMap::default();
        exp.set_insert_point(loc);
        for check in checks {
            assert!(ic * check.access_size > 0, "Threshold must be non-zero to use diff-check");
            let ty = self.se.type_of(check.sink_start);
            let (threshold_scev, diff_scev) = {
                let cx = self.env.cx(self.context, self.function);
                let total = self.se.get_constant_of_type(&cx, ty, i128::from(vf * ic * check.access_size));
                let one = self.se.get_constant_of_type(&cx, ty, 1);
                (
                    self.se.get_minus_scev(&cx, Use::of(total), Use::of(one), 0, 0),
                    self.se.get_minus_scev(&cx, Use::of(check.sink_start), Use::of(check.src_start), 0, 0),
                )
            };
            let threshold_minus_one = self.expand(exp, threshold_scev, Some(ty));
            let diff = self.expand(exp, diff_scev, Some(ty));

            // The same compare made earlier need not be made again.
            if seen_compares.contains_key(&(diff, threshold_minus_one)) {
                continue;
            }
            // Use (Diff - 1) <u (Threshold - 1), equivalent to 0 < Diff <u
            // Threshold, to exclude Diff == 0.
            let int_ty = self.function.operand_type(self.context, diff).expect("a typed diff");
            let one = Operand::Constant(self.context.int(int_ty, 1));
            let less_one = self.binop(loc, BinaryOp::Sub, diff, one, "");
            let mut is_conflict = self.icmp(loc, IntPredicate::Ult, less_one, threshold_minus_one, "diff.check");
            seen_compares.insert((diff, threshold_minus_one), is_conflict);
            if check.needs_freeze {
                is_conflict = self.freeze(loc, is_conflict);
            }
            if let Some(previous) = memory_runtime_check {
                is_conflict = self.binop(loc, BinaryOp::Or, previous, is_conflict, "conflict.rdx");
            }
            memory_runtime_check = Some(is_conflict);
        }

        self.erase_dead(exp, memory_runtime_check);
        memory_runtime_check
    }

    /// `getPointerAddressSpace` of a pointer value.
    fn space_of(
        &self,
        v: Operand,
    ) -> Option<u32> {
        let ty = self.function.operand_type(self.context, v)?;
        match self.context.types.get(ty) {
            llrm_mir::types::Type::Pointer(space) => Some(*space),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "runtimechecks_tests.rs"]
pub(crate) mod tests;
