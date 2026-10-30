//! `computeExitLimit` and what it reads of an exit's condition.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ScalarEvolution.cpp`: the exit
//! limit of one exiting block, found from the branch's condition (an `and` or
//! `or` of conditions, an `icmp`, a constant, an overflow flag), a switch with
//! one exit, or a shift compare. `how_many.rs` answers the comparisons.
//!
//! # Deviations
//! - Predicates (DEVIATE 1 of `docs/architecture/scev-port.md`): no
//!   `AllowPredicates`; the second try with predicates is absent.
//! - `getPtrToAddrExpr` of a pointer compare operand (DEVIATE 3): a pointer
//!   `!=` or `==` is taken through `get_ptr_to_addr_expr` as the port has it.
//!
//! # Example
//! `br i1 (icmp slt i32 %i, %n), loop, exit` with `%i = {0,+,1}` exits after
//! `(-1 + %n)` backedges.

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, Operand};
use llrm_mir::opcode::{IntPredicate, Opcode};
use llrm_mir::valuetracking::pattern;

use super::expr::{Kind, Scev, Use, flags};
use super::trip::ExitLimit;
use super::{Cx, ScalarEvolution};

/// `ExitLimitCache`: the exit limits found for the conditions of one exit, by
/// condition and whether it controls the only exit.
#[derive(Default)]
pub struct ExitLimitCache {
    limits: Vec<((Operand, bool, bool), ExitLimit)>,
}

impl ExitLimitCache {
    fn find(
        &self,
        exit_cond: Operand,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> Option<ExitLimit> {
        self.limits
            .iter()
            .find(|(key, _)| *key == (exit_cond, controls_only_exit, allow_predicates))
            .map(|(_, limit)| limit.clone())
    }

    fn insert(
        &mut self,
        exit_cond: Operand,
        controls_only_exit: bool,
        allow_predicates: bool,
        limit: ExitLimit,
    ) {
        debug_assert!(
            self.find(exit_cond, controls_only_exit, allow_predicates).is_none(),
            "Expected successful insertion!"
        );
        self.limits.push(((exit_cond, controls_only_exit, allow_predicates), limit));
    }
}

fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9130
    /// `computeExitLimit`.
    pub(super) fn compute_exit_limit(
        &mut self,
        cx: &Cx,
        header: BlockId,
        exiting_block: BlockId,
        is_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        let loop_ = cx.loops.get_loop_for(header).expect("the header of a loop");
        debug_assert!(loop_.contains(exiting_block), "Exit count for non-loop block?");
        // If our exiting block does not dominate the latch, then its
        // connection with loop's exit limit may be far from trivial.
        let latch = loop_.get_loop_latch(cx.function);
        if !latch.is_some_and(|latch| cx.tree.dominates(exiting_block, latch)) {
            return ExitLimit::could_not_compute(self);
        }

        let term = cx.function.terminator(exiting_block).expect("an exiting block ends");
        let instruction = cx.function.instruction(term);
        match (&instruction.opcode, instruction.operands.as_slice()) {
            (Opcode::Br, [condition, Operand::Block(first), Operand::Block(second)]) => {
                let exit_if_true = !loop_.contains(*first);
                debug_assert!(
                    exit_if_true == loop_.contains(*second),
                    "It should have one successor in loop and one exit block!"
                );
                // Proceed to the next level to examine the exit condition
                // expression.
                self.compute_exit_limit_from_cond(cx, header, *condition, exit_if_true, is_only_exit, allow_predicates)
            }
            (Opcode::Switch, _) => {
                // For switch, make sure that there is a single exit from the
                // loop.
                let mut exit = None;
                for successor in cx.function.successors(exiting_block) {
                    if !loop_.contains(successor) {
                        if exit.is_some() {
                            // Multiple exit successors.
                            return ExitLimit::could_not_compute(self);
                        }
                        exit = Some(successor);
                    }
                }
                let exit = exit.expect("Exiting block must have at least one exit");
                self.compute_exit_limit_from_single_exit_switch(cx, header, term, exit, is_only_exit, allow_predicates)
            }
            _ => ExitLimit::could_not_compute(self),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9167
    /// `computeExitLimitFromCond`.
    pub fn compute_exit_limit_from_cond(
        &mut self,
        cx: &Cx,
        header: BlockId,
        exit_cond: Operand,
        exit_if_true: bool,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        let mut cache = ExitLimitCache::default();
        self.compute_exit_limit_from_cond_cached(
            cx,
            &mut cache,
            header,
            exit_cond,
            exit_if_true,
            controls_only_exit,
            allow_predicates,
        )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9207
    /// `computeExitLimitFromCondCached`.
    fn compute_exit_limit_from_cond_cached(
        &mut self,
        cx: &Cx,
        cache: &mut ExitLimitCache,
        header: BlockId,
        exit_cond: Operand,
        exit_if_true: bool,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        if let Some(found) = cache.find(exit_cond, controls_only_exit, allow_predicates) {
            return found;
        }

        let limit = self.compute_exit_limit_from_cond_impl(
            cx,
            cache,
            header,
            exit_cond,
            exit_if_true,
            controls_only_exit,
            allow_predicates,
        );
        cache.insert(exit_cond, controls_only_exit, allow_predicates, limit.clone());
        limit
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9221
    /// `computeExitLimitFromCondImpl`.
    fn compute_exit_limit_from_cond_impl(
        &mut self,
        cx: &Cx,
        cache: &mut ExitLimitCache,
        header: BlockId,
        exit_cond: Operand,
        exit_if_true: bool,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        // Handle BinOp conditions (And, Or).
        if let Some(limit) =
            self.compute_exit_limit_from_cond_from_bin_op(cx, cache, header, exit_cond, exit_if_true, allow_predicates)
        {
            return limit;
        }

        let query = cx.query(None);

        // With an icmp, it may be feasible to compute an exact backedge-taken
        // count. Proceed to the next level to examine the icmp.
        if let Some((_, instruction, _, _, _)) = pattern::icmp(&query, exit_cond) {
            let limit = self.compute_exit_limit_from_icmp_inst(
                cx,
                header,
                exit_cond,
                instruction,
                exit_if_true,
                controls_only_exit,
                false,
            );
            if limit.has_full_info(self) || !allow_predicates {
                return limit;
            }
            // Try again, but use SCEV predicates this time.
            return self.compute_exit_limit_from_icmp_inst(
                cx,
                header,
                exit_cond,
                instruction,
                exit_if_true,
                controls_only_exit,
                true,
            );
        }

        // Check for a constant condition. These are normally stripped out by
        // SimplifyCFG, but ScalarEvolution may be used by a pass which wishes
        // to preserve the CFG and is temporarily leaving constant conditions
        // in place.
        if let Operand::Constant(id) = exit_cond
            && let Some(bits) = match cx.context.get(id).kind {
                ConstantKind::Int(bits) => Some(bits),
                ConstantKind::Zero => Some(0),
                _ => None,
            }
        {
            if exit_if_true == (bits & 1 == 0) {
                // The backedge is always taken.
                return ExitLimit::could_not_compute(self);
            }
            // The backedge is never taken.
            let ty = self.ty_of(cx, cx.context.get(id).ty).expect("a SCEVable condition");
            let zero = self.get_zero(cx, ty);
            return ExitLimit::exact(self, zero);
        }

        // If we're exiting based on the overflow flag of an x.with.overflow
        // intrinsic with a constant step, we can form an equivalent icmp
        // predicate and figure out how many iterations will be taken before
        // we exit.
        if let Some(limit) = self.exit_limit_from_overflow_flag(
            cx,
            header,
            exit_cond,
            exit_if_true,
            controls_only_exit,
            allow_predicates,
        ) {
            return limit;
        }

        // If it's not an integer or pointer comparison then compute it the
        // hard way.
        let exhaustive = self.compute_exit_count_exhaustively(cx, header, exit_cond, exit_if_true);
        ExitLimit::exact(self, exhaustive)
    }

    /// The `m_ExtractValue<1>(m_WithOverflowInst(WO))` case of
    /// `computeExitLimitFromCondImpl`: the limit if it has any information.
    fn exit_limit_from_overflow_flag(
        &mut self,
        cx: &Cx,
        header: BlockId,
        exit_cond: Operand,
        exit_if_true: bool,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> Option<ExitLimit> {
        use llrm_mir::valuetracking::pattern::{IntrinsicId, intrinsic_id};

        let query = cx.query(None);
        let (_, extract, wo) = pattern::extract_value(&query, exit_cond)?;
        if !matches!(
            &extract.opcode,
            Opcode::ExtractValue(indices) if indices.as_slice() == [1]
        ) {
            return None;
        }
        let (wo_inst, wo_instruction) = pattern::with_overflow_inst(&query, wo)?;
        let [lhs_value, rhs_value] = pattern::call_arguments(wo_instruction)[..] else { return None };
        let (c_bits, c_width) = pattern::apint(&query, rhs_value)?;
        let (op, no_wrap_kind) = match intrinsic_id(&query, wo_inst)? {
            IntrinsicId::SaddWithOverflow => (llrm_mir::opcode::BinaryOp::Add, 2),
            IntrinsicId::UaddWithOverflow => (llrm_mir::opcode::BinaryOp::Add, 1),
            IntrinsicId::SsubWithOverflow => (llrm_mir::opcode::BinaryOp::Sub, 2),
            IntrinsicId::UsubWithOverflow => (llrm_mir::opcode::BinaryOp::Sub, 1),
            IntrinsicId::SmulWithOverflow => (llrm_mir::opcode::BinaryOp::Mul, 2),
            IntrinsicId::UmulWithOverflow => (llrm_mir::opcode::BinaryOp::Mul, 1),
            _ => return None,
        };
        let nwr = ConstantRange::make_exact_no_wrap_region(op, c_width, c_bits, no_wrap_kind);
        let (mut pred, new_rhs_c, offset) = nwr.get_equivalent_icmp_offset();
        if !exit_if_true {
            pred = pred.inverse();
        }
        let mut lhs = self.get_scev(cx, lhs_value);
        if offset != 0 {
            let offset = self.get_constant(super::ap::Ap::new(c_width, offset));
            lhs = self.get_add_expr(cx, vec![Use::of(lhs), Use::of(offset)], Default::default(), 0).scev;
        }
        let rhs = self.get_constant(super::ap::Ap::new(c_width, new_rhs_c));
        let limit =
            self.compute_exit_limit_from_icmp_pred(cx, header, pred, lhs, rhs, controls_only_exit, allow_predicates);
        limit.has_any_info(self).then_some(limit)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9284
    /// `computeExitLimitFromCondFromBinOp`: none where the condition is not a
    /// logical and or or.
    fn compute_exit_limit_from_cond_from_bin_op(
        &mut self,
        cx: &Cx,
        cache: &mut ExitLimitCache,
        header: BlockId,
        exit_cond: Operand,
        exit_if_true: bool,
        allow_predicates: bool,
    ) -> Option<ExitLimit> {
        // Check if the controlling expression for this loop is an And or Or.
        let query = cx.query(None);
        let (op0, op1, is_and) = if let Some((a, b)) = pattern::logical_and(&query, exit_cond) {
            (a, b, true)
        } else if let Some((a, b)) = pattern::logical_or(&query, exit_cond) {
            (a, b, false)
        } else {
            return None;
        };

        // A sub-condition of a non-trivial binop never solely controls the
        // exit, whether we exit always depends on both conditions.
        let el0 =
            self.compute_exit_limit_from_cond_cached(cx, cache, header, op0, exit_if_true, false, allow_predicates);
        let el1 =
            self.compute_exit_limit_from_cond_cached(cx, cache, header, op1, exit_if_true, false, allow_predicates);

        // EitherMayExit is true in these two cases:
        //   br (and Op0 Op1), loop, exit
        //   br (or  Op0 Op1), exit, loop
        let either_may_exit = is_and ^ exit_if_true;

        let unknown = self.get_could_not_compute();
        let mut be_count = unknown;
        let mut constant_max_be_count = unknown;
        let mut symbolic_max_be_count = unknown;
        if either_may_exit {
            let use_sequential_umin = !matches!(
                query.instruction_of(exit_cond),
                Some((_, one)) if matches!(one.opcode, Opcode::Binary(_))
            );
            // Both conditions must be same for the loop to continue
            // executing. Choose the less conservative count.
            if el0.exact_not_taken != unknown && el1.exact_not_taken != unknown {
                be_count = self.get_umin_from_mismatched_types(
                    cx,
                    vec![Use::of(el0.exact_not_taken), Use::of(el1.exact_not_taken)],
                    use_sequential_umin,
                );
            }
            if el0.constant_max_not_taken == unknown {
                constant_max_be_count = el1.constant_max_not_taken;
            } else if el1.constant_max_not_taken == unknown {
                constant_max_be_count = el0.constant_max_not_taken;
            } else {
                constant_max_be_count = self.get_umin_from_mismatched_types_pair(
                    cx,
                    el0.constant_max_not_taken,
                    el1.constant_max_not_taken,
                );
            }
            if el0.symbolic_max_not_taken == unknown {
                symbolic_max_be_count = el1.symbolic_max_not_taken;
            } else if el1.symbolic_max_not_taken == unknown {
                symbolic_max_be_count = el0.symbolic_max_not_taken;
            } else {
                symbolic_max_be_count = self.get_umin_from_mismatched_types(
                    cx,
                    vec![Use::of(el0.symbolic_max_not_taken), Use::of(el1.symbolic_max_not_taken)],
                    use_sequential_umin,
                );
            }
        } else {
            // Both conditions must be same at the same time for the loop to
            // exit. For now, be conservative.
            if el0.exact_not_taken == el1.exact_not_taken {
                be_count = el0.exact_not_taken;
            }
        }

        // There are cases (e.g. PR26207) where computeExitLimitFromCond is
        // able to be more aggressive when computing BECount than when
        // computing ConstantMaxBECount. In these cases it is possible for
        // EL0.ExactNotTaken and EL1.ExactNotTaken to match, but for
        // EL0.ConstantMaxNotTaken and EL1.ConstantMaxNotTaken to not.
        if constant_max_be_count == unknown && be_count != unknown {
            let max = self.get_unsigned_range_max(cx, be_count);
            constant_max_be_count = self.get_constant(max);
        }
        if symbolic_max_be_count == unknown {
            symbolic_max_be_count = if be_count == unknown { constant_max_be_count } else { be_count };
        }
        Some(ExitLimit::new(self, be_count, constant_max_be_count, symbolic_max_be_count, false))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9360
    /// `computeExitLimitFromICmp(L, ICmpInst *, ExitIfTrue, ControlsOnlyExit)`.
    fn compute_exit_limit_from_icmp_inst(
        &mut self,
        cx: &Cx,
        header: BlockId,
        exit_cond_value: Operand,
        exit_cond: &llrm_mir::module::Instruction,
        exit_if_true: bool,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        let Opcode::ICmp(cmp) = exit_cond.opcode else { unreachable!("an icmp") };
        // If the condition was exit on true, convert the condition to exit on
        // false.
        let pred = if !exit_if_true { cmp } else { cmp.inverse() };
        let original_pred = pred;

        let lhs = self.get_scev(cx, exit_cond.operands[0]);
        let rhs = self.get_scev(cx, exit_cond.operands[1]);

        let limit =
            self.compute_exit_limit_from_icmp_pred(cx, header, pred, lhs, rhs, controls_only_exit, allow_predicates);
        if limit.has_any_info(self) {
            return limit;
        }

        let exhaustive_count = self.compute_exit_count_exhaustively(cx, header, exit_cond_value, exit_if_true);

        if exhaustive_count != self.get_could_not_compute() {
            return ExitLimit::exact(self, exhaustive_count);
        }

        self.compute_shift_compare_exit_limit(cx, header, exit_cond.operands[0], exit_cond.operands[1], original_pred)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9388
    /// `computeExitLimitFromICmp(L, Pred, LHS, RHS, ControlsOnlyExit)`.
    pub(super) fn compute_exit_limit_from_icmp_pred(
        &mut self,
        cx: &Cx,
        header: BlockId,
        mut pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        // Try to evaluate any dependencies out of the loop.
        lhs = self.get_scev_at_scope(cx, lhs, Some(header));
        rhs = self.get_scev_at_scope(cx, rhs, Some(header));

        // At this point, we would like to compute how many iterations of the
        // loop the predicate will return true for these inputs.
        if self.is_loop_invariant(cx, lhs, Some(header)) && !self.is_loop_invariant(cx, rhs, Some(header)) {
            // If there is a loop-invariant, force it into the RHS.
            std::mem::swap(&mut lhs, &mut rhs);
            pred = pred.swapped();
        }

        let controlling_finite_loop = controls_only_exit
            && self.loop_has_no_abnormal_exits(cx, header)
            && self.loop_is_finite_by_assumption(cx, header);
        // Simplify the operands before analyzing them.
        self.simplify_icmp_operands(cx, &mut pred, &mut lhs, &mut rhs, 0);

        // If we have a comparison of a chrec against a constant, try to use
        // value ranges to answer this query.
        if let Some(rhs_c) = self.match_apint(rhs)
            && self.is_add_rec_of(lhs, header)
        {
            // Form the constant range.
            let comp_range = ConstantRange::make_exact_icmp_region(pred, rhs_c.width, rhs_c.bits);
            let ret = self.get_num_iterations_in_range(cx, lhs, &comp_range);
            if ret != self.get_could_not_compute() {
                return ExitLimit::exact(self, ret);
            }
        }

        // If this loop must exit based on this condition (or execute
        // undefined behaviour), see if we can improve wrap flags. This is
        // essentially a must execute style proof.
        if controlling_finite_loop && self.is_loop_invariant(cx, rhs, Some(header)) {
            self.strengthen_flags_of_finite_loop(cx, header, pred, lhs);
        }
        match pred {
            IntPredicate::Ne => {
                // while (X != Y)  -->  while (X-Y != 0)
                let (Some(lhs), Some(rhs)) = (self.pointer_to_address(cx, lhs), self.pointer_to_address(cx, rhs))
                else {
                    return ExitLimit::could_not_compute(self);
                };
                let diff = self.get_minus_scev(cx, Use::of(lhs), Use::of(rhs), 0, 0);
                let limit = self.how_far_to_zero(cx, diff, header, controls_only_exit, allow_predicates);
                if limit.has_any_info(self) {
                    return limit;
                }
            }
            IntPredicate::Eq => {
                // while (X == Y)  -->  while (X-Y == 0)
                let (Some(lhs), Some(rhs)) = (self.pointer_to_address(cx, lhs), self.pointer_to_address(cx, rhs))
                else {
                    return ExitLimit::could_not_compute(self);
                };
                let diff = self.get_minus_scev(cx, Use::of(lhs), Use::of(rhs), 0, 0);
                let limit = self.how_far_to_non_zero(cx, diff, header);
                if limit.has_any_info(self) {
                    return limit;
                }
            }
            IntPredicate::Sle | IntPredicate::Ule | IntPredicate::Slt | IntPredicate::Ult => {
                if matches!(pred, IntPredicate::Sle | IntPredicate::Ule) {
                    // Since the loop is finite, an invariant RHS cannot include
                    // the boundary value, otherwise it would loop forever.
                    if !ENABLE_FINITE_LOOP_CONTROL
                        || !controlling_finite_loop
                        || !self.is_loop_invariant(cx, rhs, Some(header))
                    {
                        // Otherwise, perform the addition in a wider type, to
                        // avoid overflow. If the LHS is an addrec with the
                        // appropriate nowrap flag, the extension will be sunk
                        // into it and the exit count can be analyzed.
                        let super::expr::Ty::Int(old_bits) = self.type_of(lhs) else {
                            return ExitLimit::could_not_compute(self);
                        };
                        // Prefer doubling the bitwidth over adding a single
                        // bit to make it more likely that we use a legal type.
                        let new_type = super::expr::Ty::Int(old_bits * 2);
                        if is_signed(pred) {
                            lhs = self.get_sign_extend_expr(cx, Use::of(lhs), new_type, 0);
                            rhs = self.get_sign_extend_expr(cx, Use::of(rhs), new_type, 0);
                        } else {
                            lhs = self.get_zero_extend_expr(cx, Use::of(lhs), new_type, 0);
                            rhs = self.get_zero_extend_expr(cx, Use::of(rhs), new_type, 0);
                        }
                    }
                    let one = self.get_one(cx, self.type_of(rhs));
                    rhs = self.get_add_expr(cx, vec![Use::of(one), Use::of(rhs)], Default::default(), 0).scev;
                }
                // while (X < Y)
                let limit = self.how_many_less_thans(
                    cx,
                    lhs,
                    rhs,
                    header,
                    is_signed(pred),
                    controls_only_exit,
                    allow_predicates,
                );
                if limit.has_any_info(self) {
                    return limit;
                }
            }
            IntPredicate::Sge | IntPredicate::Uge | IntPredicate::Sgt | IntPredicate::Ugt => {
                let mut stop = false;
                if matches!(pred, IntPredicate::Sge | IntPredicate::Uge) {
                    // Since the loop is finite, an invariant RHS cannot include
                    // the boundary value, otherwise it would loop forever.
                    if !ENABLE_FINITE_LOOP_CONTROL
                        || !controlling_finite_loop
                        || !self.is_loop_invariant(cx, rhs, Some(header))
                    {
                        stop = true;
                    } else {
                        let minus_one = self.get_minus_one(cx, self.type_of(rhs));
                        rhs = self.get_add_expr(cx, vec![Use::of(minus_one), Use::of(rhs)], Default::default(), 0).scev;
                    }
                }
                if !stop {
                    // while (X > Y)
                    let limit = self.how_many_greater_thans(
                        cx,
                        lhs,
                        rhs,
                        header,
                        is_signed(pred),
                        controls_only_exit,
                        allow_predicates,
                    );
                    if limit.has_any_info(self) {
                        return limit;
                    }
                }
            }
        }

        ExitLimit::could_not_compute(self)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L9561
    /// `computeExitLimitFromSingleExitSwitch`.
    fn compute_exit_limit_from_single_exit_switch(
        &mut self,
        cx: &Cx,
        header: BlockId,
        switch: llrm_mir::module::InstId,
        exiting_block: BlockId,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        let instruction = cx.function.instruction(switch);
        // A switch's operands: the condition, the default, then (value, block)
        // pairs.
        let (condition, default) = (instruction.operands[0], instruction.operands[1]);
        debug_assert!(!cx.loops.get_loop_for(header).expect("a loop").contains(exiting_block), "Not an exiting block!");

        // Give up if the exit is the default dest of a switch.
        if default == Operand::Block(exiting_block) {
            return ExitLimit::could_not_compute(self);
        }

        let lhs = self.get_scev(cx, condition);
        let lhs = self.get_scev_at_scope(cx, lhs, Some(header));
        let case_value = instruction.operands[2..]
            .chunks(2)
            .find(|pair| pair[1] == Operand::Block(exiting_block))
            .and_then(|pair| match pair[0] {
                Operand::Constant(id) => match cx.context.get(id).kind {
                    ConstantKind::Int(bits) => Some(bits),
                    ConstantKind::Zero => Some(0),
                    _ => None,
                },
                _ => None,
            })
            .expect("a case that leaves");
        let width = match self.type_of(lhs) {
            super::expr::Ty::Int(bits) => bits,
            super::expr::Ty::Ptr(_) => return ExitLimit::could_not_compute(self),
        };
        let rhs = self.get_constant(super::ap::Ap::new(width, case_value));

        // while (X != Y) --> while (X-Y != 0)
        let diff = self.get_minus_scev(cx, Use::of(lhs), Use::of(rhs), 0, 0);
        let limit = self.how_far_to_zero(cx, diff, header, controls_only_exit, allow_predicates);
        if limit.has_any_info(self) {
            return limit;
        }

        ExitLimit::could_not_compute(self)
    }
}

/// `EnableFiniteLoopControl`.
const ENABLE_FINITE_LOOP_CONTROL: bool = true;

impl ScalarEvolution {
    /// `getPtrToAddrExpr` of a pointer operand, the operand itself otherwise;
    /// none where the cast is CouldNotCompute.
    fn pointer_to_address(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> Option<Scev> {
        if !self.type_of(s).is_pointer() {
            return Some(s);
        }
        let address = self.get_ptr_to_addr_expr(cx, s);
        (address != self.get_could_not_compute()).then_some(address)
    }

    /// The "if this loop must exit based on this condition (or execute
    /// undefined behaviour), see if we can improve wrap flags" block of
    /// `computeExitLimitFromICmp` (ScalarEvolution.cpp L9423-9460).
    fn strengthen_flags_of_finite_loop(
        &mut self,
        cx: &Cx,
        header: BlockId,
        pred: IntPredicate,
        lhs: Scev,
    ) {
        use super::expr::Type;
        // If we can prove the test sequence produced must repeat the same
        // values on self-wrap of the IV, then we can infer that IV doesn't
        // self wrap because if it did, we'd have an infinite (undefined) loop.
        // TODO (LLVM): We can peel off any functions which are invertible *in
        // L*. Loop invariant terms are effectively constants for our purposes
        // here.
        let inner_lhs = match self.node(lhs).kind {
            Kind::ZeroExtend(operand) => operand.scev,
            _ => lhs,
        };
        if self.is_add_rec_of(inner_lhs, header)
            && self.get_no_wrap_flags(inner_lhs, flags::NW) == 0
            && self.is_affine(inner_lhs)
        {
            let step = self.get_step_recurrence(cx, inner_lhs);
            if self.is_known_to_be_a_power_of_two(cx, step, true, true) {
                let mut f = self.get_no_wrap_flags(inner_lhs, flags::MASK);
                f |= flags::NW;
                let operands = self.operands(inner_lhs).to_vec();
                f = self.strengthen_no_wrap_flags(cx, Type::AddRec, &operands, f);
                self.set_no_wrap_flags(inner_lhs, f);
            }
        }

        // For a slt/ult condition with a positive step, can we prove nsw/nuw?
        // From no-self-wrap, this follows trivially from the fact that every
        // (un)signed-wrapped, but not self-wrapped value must be LT than the
        // last value before (un)signed wrap. Since we know that last value
        // didn't exit, nor will any smaller one.
        if matches!(pred, IntPredicate::Slt | IntPredicate::Ult) {
            let wrap_type = if pred == IntPredicate::Slt { flags::NSW } else { flags::NUW };
            if self.is_add_rec_of(lhs, header)
                && self.is_affine(lhs)
                && self.get_no_wrap_flags(lhs, wrap_type) == 0
                && self.get_no_wrap_flags(lhs, flags::NW) != 0
            {
                let step = self.get_step_recurrence(cx, lhs);
                if self.is_known_positive(cx, step) {
                    let mut f = self.get_no_wrap_flags(lhs, flags::MASK);
                    f |= wrap_type;
                    let operands = self.operands(lhs).to_vec();
                    f = self.strengthen_no_wrap_flags(cx, Type::AddRec, &operands, f);
                    self.set_no_wrap_flags(lhs, f);
                }
            }
        }
    }
}
