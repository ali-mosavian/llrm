//! A loop's exits are folded by what ScalarEvolution knows of their counts.
//! LLVM's `optimizeLoopExits` (`IndVarSimplify.cpp`) with
//! `optimizeLoopExitWithUnknownExitCount`.
//!
//! # How it works
//! For each loop in LoopSimplify form, innermost first, the exits that are in
//! the loop itself, end in a conditional branch on a non-constant, and
//! dominate the latch are taken in dominance order. A constant exit that is
//! always taken has the header phis replaced by their preheader values.
//!
//! - Exact count unknown: each `icmp` leaf of the exit's `and`/`or` tree is
//!   replaced by the compare of the counter's start when ScalarEvolution proves
//!   the compare is invariant over the loop's first iterations, or by the
//!   constant it decides (`getLoopInvariantExitCondDuringFirstIterations`).
//! - Count zero: the exit is taken on the first trip.
//! - Count above the loop's maximum, or equal to a dominating exit's: the exit
//!   is never taken.
//!
//! A decided exit gets the constant condition that decides it; `dead` and
//! `simplifycfg` remove the rest.
//!
//! # Example
//! The second exit `i < len` with `i < n` guarded by `n <= len`:
//! `br i1 %lt, label %loop, label %exit` becomes
//! `br i1 true, label %loop, label %exit`.
//!
//! # Used by
//! Not wired in: [`crate::indvars`] will run [`optimized`] before
//! [`crate::lftr`], as `IndVarSimplify::run` does.
//!
//! # See also
//! `docs/architecture/scev-port.md`.

use llrm_analysis::occurrence::arms;
use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expander::{Env, Expander};
use llrm_analysis::scev::expr::{Kind, Scev, Use};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Declared, Outer};
use llrm_mir::valuetracking::pattern;

use crate::exitvalues::replacement_preserves_lcssa_form;
use crate::lftr::{branch_of, def_of};

/// `LLRM_NO_SCEV_EXITOPT`, read once; set, the step is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_SCEV_EXITOPT")
}

/// What one run reads and edits.
struct ExitOpt<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    declared: &'a mut Declared,
    se: &'a mut ScalarEvolution,
    env: Env<'a>,
    rewriter: &'a mut Expander,
}

/// `IndVarSimplify::run`'s `optimizeLoopExits` over every loop of `function`,
/// innermost first; whether anything changed. Instructions left dead are
/// `dead`'s.
pub fn optimized(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    optimized_in(&mut session, context, layout, function, declared, outer)
}

/// [`optimized`] over `session`, whose expressions are valid for `function`.
pub fn optimized_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
) -> bool {
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let headers: Vec<BlockId> = {
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        crate::loopfilter::retain(&mut headers);
        headers
    };
    // `Rewriter.disableCanonicalMode()`.
    let mut rewriter = Expander::new("indvars");
    rewriter.set_canonical_mode(false);
    let mut run = ExitOpt { context, layout, function, declared, se, env, rewriter: &mut rewriter };
    let mut changed = false;
    for header in headers {
        // `IndVarSimplify::run`: stay out of trouble without LoopSimplify
        // form.
        let simple = {
            let cx = run.env.cx(&*run.context, &*run.function);
            cx.loops.get_loop_for(header).is_some_and(|found| found.is_loop_simplify_form(cx.function))
        };
        if simple {
            changed |= run.optimize_loop_exits(header);
        }
    }
    changed
}

impl ExitOpt<'_> {
    fn exiting_blocks(
        &self,
        header: BlockId,
    ) -> Vec<BlockId> {
        let cx = self.env.cx(&*self.context, &*self.function);
        cx.loops.get_loop_for(header).expect("a loop").get_exiting_blocks(cx.function)
    }

    /// Whether `block` is in the loop headed by `header`.
    fn contains(
        &self,
        header: BlockId,
        block: BlockId,
    ) -> bool {
        let cx = self.env.cx(&*self.context, &*self.function);
        cx.loops.get_loop_for(header).expect("a loop").contains(block)
    }

    /// `succ_begin(ExitingBB)` and its sibling of the conditional branch.
    fn successors(
        &self,
        exiting: BlockId,
    ) -> (BlockId, BlockId) {
        let (term, _) = branch_of(self.function, exiting).expect("a conditional branch");
        let operands = &self.function.instruction(term).operands;
        match (operands[1], operands[2]) {
            (Operand::Block(first), Operand::Block(second)) => (first, second),
            _ => unreachable!("a conditional branch has block targets"),
        }
    }

    /// An `i1` constant.
    fn bool_constant(
        &self,
        value: bool,
    ) -> Operand {
        let i1 = self.context.types.int(1);
        Operand::Constant(self.context.int(i1, i128::from(value)))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1292
    /// `createFoldedExitCond`.
    fn created_folded_exit_cond(
        &self,
        header: BlockId,
        exiting: BlockId,
        is_taken: bool,
    ) -> Operand {
        let exit_if_true = !self.contains(header, self.successors(exiting).0);
        self.bool_constant(if is_taken { exit_if_true } else { !exit_if_true })
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1301
    /// `foldExit`: the old condition is left to `dead` (`replaceExitCond`).
    fn fold_exit(
        &mut self,
        header: BlockId,
        exiting: BlockId,
        is_taken: bool,
    ) {
        let new_cond = self.created_folded_exit_cond(header, exiting, is_taken);
        let (term, _) = branch_of(self.function, exiting).expect("a conditional branch");
        self.function.set_operand(term, 0, new_cond);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1308
    /// `replaceLoopPHINodesWithPreheaderValues`: the old phis are left to
    /// `dead`.
    fn replace_loop_phi_nodes_with_preheader_values(
        &mut self,
        header: BlockId,
    ) {
        let preheader = {
            let cx = self.env.cx(&*self.context, &*self.function);
            cx.loops
                .get_loop_for(header)
                .and_then(|l| l.get_loop_preheader(cx.function))
                .expect("Should only do it in simplify form!")
        };
        let phis: Vec<InstId> = self
            .function
            .iter_block(header)
            .take_while(|&inst| self.function.instruction(inst).opcode == Opcode::Phi)
            .collect();
        let mut worklist: Vec<InstId> = Vec::new();
        for phi in phis {
            let incoming =
                arms(self.function, phi).into_iter().find(|(_, from)| *from == preheader).expect("a preheader edge").0;
            let result = self.function.instruction(phi).result.expect("a phi has a value");
            for user in self.function.users(result) {
                worklist.push(user.user);
            }
            {
                let cx = self.env.cx(&*self.context, &*self.function);
                self.se.forget_value(&cx, Operand::Value(result));
            }
            self.function.replace_all_uses_with(result, incoming);
        }

        // Replacing with the preheader value will often allow IV users to
        // simplify (especially if the preheader value is a constant).
        let mut visited: IdSet<InstId> = IdSet::new();
        while let Some(inst) = worklist.pop() {
            if !visited.insert(inst) {
                continue;
            }
            // Don't simplify instructions outside the loop.
            let inside = {
                let cx = self.env.cx(&*self.context, &*self.function);
                cx.loops.get_loop_for(header).expect("a loop").contains_instruction(cx.function, inst)
            };
            if !inside {
                continue;
            }
            let Some(res) = llrm_mir::simplify::simplify_instruction(self.context, self.layout, self.function, inst)
            else {
                continue;
            };
            let preserves = {
                let cx = self.env.cx(&*self.context, &*self.function);
                replacement_preserves_lcssa_form(&cx, inst, res)
            };
            if preserves {
                let result = self.function.instruction(inst).result.expect("a value");
                for user in self.function.users(result) {
                    worklist.push(user.user);
                }
                self.function.replace_all_uses_with(result, res);
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1346
    /// `createInvariantCond`.
    fn create_invariant_cond(
        &mut self,
        header: BlockId,
        exiting: BlockId,
        lip: &llrm_analysis::scev::invariant::LoopInvariantPredicate,
    ) -> Operand {
        let preheader = {
            let cx = self.env.cx(&*self.context, &*self.function);
            cx.loops
                .get_loop_for(header)
                .and_then(|l| l.get_loop_preheader(cx.function))
                .expect("Preheader doesn't exist")
        };
        let term = self.function.terminator(preheader).expect("a terminator");
        self.rewriter.set_insert_point(term);
        let lhs = self
            .rewriter
            .expand_code_for(&self.env, self.context, self.declared, self.function, self.se, lip.lhs, None);
        let rhs = self
            .rewriter
            .expand_code_for(&self.env, self.context, self.declared, self.function, self.se, lip.rhs, None);
        let exit_if_true = !self.contains(header, self.successors(exiting).0);
        let pred = if exit_if_true { lip.pred.inverse() } else { lip.pred };
        let i1 = self.context.types.int(1);
        let cond = self
            .function
            .create_instruction(Opcode::ICmp(pred), i1, vec![lhs, rhs], Flags::default(), Some("first_iter"));
        self.function.insert(cond, Position::Before(term)).expect("a placed branch");
        Operand::Value(self.function.instruction(cond).result.expect("a compare value"))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1365
    /// `createReplacement`: the value `icmp` can be replaced by, if any.
    fn create_replacement(
        &mut self,
        header: BlockId,
        exiting: BlockId,
        icmp: InstId,
        mut max_iter: Scev,
        inverted: bool,
        skip_last_iter: bool,
    ) -> Option<Operand> {
        let Opcode::ICmp(mut pred) = self.function.instruction(icmp).opcode else { unreachable!("an icmp") };
        let (lhs, rhs) = (self.function.instruction(icmp).operands[0], self.function.instruction(icmp).operands[1]);
        let (term, _) = branch_of(self.function, exiting).expect("a conditional branch");

        // 'LHS pred RHS' should now mean that we stay in loop.
        if inverted {
            pred = pred.inverse();
        }

        let cx = self.env.cx(&*self.context, &*self.function);
        let lhss = self.se.get_scev_at_scope_of_value(&cx, lhs, Some(header)).scev;
        let rhss = self.se.get_scev_at_scope_of_value(&cx, rhs, Some(header)).scev;
        // Can we prove it to be trivially true or false?
        if let Some(ev) = self.se.evaluate_predicate_at(&cx, pred, lhss, rhss, exiting) {
            return Some(self.created_folded_exit_cond(header, exiting, !ev));
        }

        let ar_ty = self.se.type_of(lhss);
        let max_iter_ty = self.se.type_of(max_iter);
        // If possible, adjust types.
        let (ar_bits, iter_bits) =
            (self.se.get_type_size_in_bits(&cx, ar_ty), self.se.get_type_size_in_bits(&cx, max_iter_ty));
        if ar_bits > iter_bits {
            max_iter = self.se.get_zero_extend_expr(&cx, Use::of(max_iter), ar_ty, 0);
        } else if ar_bits < iter_bits {
            let minus_one = self.se.get_minus_one(&cx, ar_ty);
            let max_allowed_iter = self.se.get_zero_extend_expr(&cx, Use::of(minus_one), max_iter_ty, 0);
            if self.se.is_known_predicate_at(&cx, IntPredicate::Ule, max_iter, max_allowed_iter, Some(term)) {
                max_iter = self.se.get_truncate_expr(&cx, Use::of(max_iter), ar_ty, 0);
            }
        }

        if skip_last_iter {
            // Semantically skip last iter is "subtract 1, do not bother about
            // unsigned wrap". getLoopInvariantExitCondDuringFirstIterations
            // knows how to deal with umin in a smart way, but umin(a, b) - 1
            // will likely not simplify. So we manually construct umin(a - 1,
            // b - 1).
            if let Kind::UMin(ops) = self.se.node(max_iter).kind.clone() {
                let mut elements = Vec::new();
                for op in ops {
                    let one = self.se.get_one(&cx, self.se.type_of(op.scev));
                    elements.push(Use::of(self.se.get_minus_scev(&cx, op, Use::of(one), 0, 0)));
                }
                max_iter = self.se.get_umin_from_mismatched_types(&cx, elements, false);
            } else {
                let one = self.se.get_one(&cx, self.se.type_of(max_iter));
                max_iter = self.se.get_minus_scev(&cx, Use::of(max_iter), Use::of(one), 0, 0);
            }
        }

        // Check if there is a loop-invariant predicate equivalent to our
        // check.
        let lip = self
            .se
            .get_loop_invariant_exit_cond_during_first_iterations(&cx, pred, lhss, rhss, header, Some(term), max_iter)?;

        // Can we prove it to be trivially true?
        if self.se.is_known_predicate_at(&cx, lip.pred, lip.lhs, lip.rhs, Some(term)) {
            Some(self.created_folded_exit_cond(header, exiting, false))
        } else {
            Some(self.create_invariant_cond(header, exiting, &lip))
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1423
    /// `optimizeLoopExitWithUnknownExitCount`.
    fn optimize_loop_exit_with_unknown_exit_count(
        &mut self,
        header: BlockId,
        exiting: BlockId,
        max_iter: Scev,
        skip_last_iter: bool,
    ) -> bool {
        // For branch that stays in loop by TRUE condition, go through AND. For
        // branch that stays in loop by FALSE condition, go through OR. Both
        // gives the similar logic: "stay in loop iff all conditions are
        // true(false)".
        let inverted = self.contains(header, self.successors(exiting).1);
        let (_, old_cond) = branch_of(self.function, exiting).expect("a conditional branch");
        let mut leaf_conditions: Vec<InstId> = Vec::new();
        let mut worklist = vec![old_cond];
        let mut visited = vec![old_cond];

        loop {
            let Some(curr) = worklist.pop() else { break };
            // Go through AND/OR conditions. Collect leaf ICMPs. We only care
            // about those with one use, to avoid instruction duplication.
            let Operand::Value(value) = curr else { continue };
            if self.function.users(value).into_iter().take(2).count() != 1 {
                continue;
            }
            let through = {
                let cx = self.env.cx(&*self.context, &*self.function);
                let query = cx.query(None);
                if inverted { pattern::logical_or(&query, curr) } else { pattern::logical_and(&query, curr) }
            };
            if let Some((lhs, rhs)) = through {
                for side in [lhs, rhs] {
                    if !visited.contains(&side) {
                        visited.push(side);
                        worklist.push(side);
                    }
                }
            } else if let Some(icmp) = def_of(self.function, curr)
                .filter(|&one| matches!(self.function.instruction(one).opcode, Opcode::ICmp(_)))
            {
                leaf_conditions.push(icmp);
            }
        }

        // If the current basic block has the same exit count as the whole
        // loop, and it consists of multiple icmp's, try to collect all icmp's
        // that give exact same exit count. For all other icmp's, we could use
        // one less iteration, because their value on the last iteration
        // doesn't really matter.
        let mut icmps_failing_on_last_iter: Vec<InstId> = Vec::new();
        let same_max = !skip_last_iter && leaf_conditions.len() > 1 && {
            let cx = self.env.cx(&*self.context, &*self.function);
            self.se.get_exit_count(&cx, header, exiting, ExitCountKind::SymbolicMaximum) == max_iter
        };
        if same_max {
            for &icmp in &leaf_conditions {
                let cx = self.env.cx(&*self.context, &*self.function);
                let result = Operand::Value(self.function.instruction(icmp).result.expect("a compare value"));
                let limit = self.se.compute_exit_limit_from_cond(&cx, header, result, inverted, false, false);
                let exit_max = limit.symbolic_max_not_taken;
                if exit_max == self.se.get_could_not_compute() {
                    continue;
                }
                // They could be of different types (specifically this happens
                // after IV widening).
                let wider = self.se.get_wider_type(&cx, self.se.type_of(exit_max), self.se.type_of(max_iter));
                let wide_exit_max = self.se.get_noop_or_zero_extend(&cx, exit_max, wider);
                let wide_max_iter = self.se.get_noop_or_zero_extend(&cx, max_iter, wider);
                if wide_exit_max == wide_max_iter {
                    icmps_failing_on_last_iter.push(icmp);
                }
            }
        }

        let mut changed = false;
        for old in leaf_conditions {
            // Skip last iteration for this icmp under one of two conditions:
            // - We do it for all conditions;
            // - There is another ICmp that would fail on last iter, so this one
            //   doesn't really matter.
            let mut optimistic_skip_last_iter = skip_last_iter;
            if !optimistic_skip_last_iter {
                if icmps_failing_on_last_iter.len() > 1 {
                    optimistic_skip_last_iter = true;
                } else if icmps_failing_on_last_iter.len() == 1 {
                    optimistic_skip_last_iter = !icmps_failing_on_last_iter.contains(&old);
                }
            }
            if let Some(new_cond) =
                self.create_replacement(header, exiting, old, max_iter, inverted, optimistic_skip_last_iter)
            {
                changed = true;
                let old_result = self.function.instruction(old).result.expect("a compare value");
                self.function.replace_all_uses_with(old_result, new_cond);
                // Make sure we no longer consider this condition as failing on
                // last iteration.
                icmps_failing_on_last_iter.retain(|&one| one != old);
            }
        }
        changed
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1661
    /// `optimizeLoopExits` for the loop headed by `header`.
    fn optimize_loop_exits(
        &mut self,
        header: BlockId,
    ) -> bool {
        let latch = {
            let cx = self.env.cx(&*self.context, &*self.function);
            cx.loops.get_loop_for(header).and_then(|l| l.get_loop_latch(cx.function)).expect("a latch")
        };
        let mut replace_phis = false;
        // Remove all exits which aren't both rewriteable and execute on every
        // iteration.
        let mut exiting_blocks: Vec<BlockId> = Vec::new();
        for exiting in self.exiting_blocks(header) {
            // If our exitting block exits multiple loops, we can only rewrite
            // the innermost one. Otherwise, we're changing how many times the
            // innermost loop runs before it exits.
            let innermost = {
                let cx = self.env.cx(&*self.context, &*self.function);
                cx.loops.get_loop_for(exiting) == cx.loops.get_loop_for(header)
            };
            if !innermost {
                continue;
            }
            // Can't rewrite non-branch yet.
            let Some((term, cond)) = branch_of(self.function, exiting) else { continue };
            // Likewise, the loop latch must be dominated by the exiting BB.
            if !self.env.cx(&*self.context, &*self.function).tree.dominates(exiting, latch) {
                continue;
            }
            if let Operand::Constant(constant) = cond {
                // If already constant, nothing to do. However, if this is an
                // unconditional exit, we can still replace header phis with
                // their preheader value.
                let null = matches!(
                    self.context.get(constant).kind,
                    ConstantKind::Int(0) | ConstantKind::Zero | ConstantKind::Null
                );
                let taken = self.function.instruction(term).operands[if null { 2 } else { 1 }];
                let Operand::Block(taken) = taken else { unreachable!("a target") };
                if !self.contains(header, taken) {
                    replace_phis = true;
                }
                continue;
            }
            exiting_blocks.push(exiting);
        }
        let mut changed = false;
        if replace_phis {
            self.replace_loop_phi_nodes_with_preheader_values(header);
            changed = true;
        }

        if exiting_blocks.is_empty() {
            return changed;
        }

        // Get a symbolic upper bound on the loop backedge taken count.
        let mut max_be_count = {
            let cx = self.env.cx(&*self.context, &*self.function);
            self.se.get_backedge_taken_count(&cx, header, ExitCountKind::SymbolicMaximum)
        };
        if max_be_count == self.se.get_could_not_compute() {
            return changed;
        }

        // Visit our exit blocks in order of dominance. We know from the fact
        // that all exits must dominate the latch, so there is a total
        // dominance order between them.
        {
            let cx = self.env.cx(&*self.context, &*self.function);
            exiting_blocks.sort_by(|&a, &b| {
                if a == b {
                    std::cmp::Ordering::Equal
                } else if cx.tree.properly_dominates(a, b) {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            });
        }

        let mut skip_last_iter = false;
        let mut curr_max_exit = self.se.get_could_not_compute();
        let mut dominating_exact_exit_counts: Vec<Scev> = Vec::new();
        for exiting in exiting_blocks {
            let (mut exact_exit_count, max_exit_count) = {
                let cx = self.env.cx(&*self.context, &*self.function);
                (
                    self.se.get_exit_count(&cx, header, exiting, ExitCountKind::Exact),
                    self.se.get_exit_count(&cx, header, exiting, ExitCountKind::SymbolicMaximum),
                )
            };
            let could_not_compute = self.se.get_could_not_compute();
            if exact_exit_count == could_not_compute {
                // Okay, we do not know the exit count here. Can we at least
                // prove that it will remain the same within iteration space?
                //
                // TODO (LLVM): We might have proved that we can skip the last
                // iteration for this check. In this case, we only want to
                // check the condition on the pre-last iteration (MaxBECount -
                // 1). However, there is a nasty corner case:
                //
                //   for (i = len; i != 0; i--) { ... check (i ult X) ... }
                //
                // If we could not prove that len != 0, then we also could not
                // prove that (len - 1) is not a UINT_MAX. If we simply query
                // (len - 1), then OptimizeCond will likely not prove anything
                // for it, even if it could prove the same fact for len.
                //
                // As a temporary solution, we query both last and pre-last
                // iterations in hope that we will be able to prove triviality
                // for at least one of them. We can stop querying MaxBECount
                // for this case once SCEV understands that (MaxBECount - 1)
                // will not overflow here.
                if self.optimize_loop_exit_with_unknown_exit_count(header, exiting, max_be_count, false) {
                    changed = true;
                } else if skip_last_iter
                    && self.optimize_loop_exit_with_unknown_exit_count(header, exiting, max_be_count, true)
                {
                    changed = true;
                }
                self.update_skip_last_iter(max_exit_count, &mut skip_last_iter, &mut curr_max_exit, max_be_count);
                continue;
            }

            self.update_skip_last_iter(exact_exit_count, &mut skip_last_iter, &mut curr_max_exit, max_be_count);

            // If we know we'd exit on the first iteration, rewrite the exit to
            // reflect this. This does not imply the loop must exit through
            // this exit; there may be an earlier one taken on the first
            // iteration. We know that the backedge can't be taken, so we
            // replace all the header PHIs with values coming from the
            // preheader.
            if self.se.as_constant(exact_exit_count).is_some_and(|c| c.is_zero()) {
                self.fold_exit(header, exiting, true);
                self.replace_loop_phi_nodes_with_preheader_values(header);
                changed = true;
                continue;
            }

            let wider = {
                let cx = self.env.cx(&*self.context, &*self.function);
                let wider =
                    self.se.get_wider_type(&cx, self.se.type_of(max_be_count), self.se.type_of(exact_exit_count));
                exact_exit_count = self.se.get_noop_or_zero_extend(&cx, exact_exit_count, wider);
                max_be_count = self.se.get_noop_or_zero_extend(&cx, max_be_count, wider);
                wider
            };
            debug_assert_eq!(self.se.type_of(max_be_count), wider);

            // Can we prove that some other exit must be taken strictly before
            // this one?
            let before = {
                let cx = self.env.cx(&*self.context, &*self.function);
                self.se.is_loop_entry_guarded_by_cond_pred(
                    &cx,
                    header,
                    IntPredicate::Ult,
                    max_be_count,
                    exact_exit_count,
                )
            };
            if before {
                self.fold_exit(header, exiting, false);
                changed = true;
                continue;
            }

            // As we run, keep track of which exit counts we've encountered. If
            // we find a duplicate, we've found an exit which would have exited
            // on the exiting iteration, but (from the visit order) strictly
            // follows another which does the same and is thus dead.
            if dominating_exact_exit_counts.contains(&exact_exit_count) {
                self.fold_exit(header, exiting, false);
                changed = true;
                continue;
            }
            dominating_exact_exit_counts.push(exact_exit_count);

            // TODO (LLVM): There might be another oppurtunity to leverage
            // SCEV's reasoning here. If we kept track of the min of dominanting
            // exits so far, we could discharge exits with EC >= MDEC. This is
            // less powerful than the existing transform (since later exits
            // aren't considered), but potentially more powerful for any case
            // where SCEV can prove a >=u b, but neither a == b or a >u b. Such
            // a case is not currently known.
        }
        changed
    }

    /// `UpdateSkipLastIter`.
    fn update_skip_last_iter(
        &mut self,
        max_exit_count: Scev,
        skip_last_iter: &mut bool,
        curr_max_exit: &mut Scev,
        max_be_count: Scev,
    ) {
        let could_not_compute = self.se.get_could_not_compute();
        if *skip_last_iter || max_exit_count == could_not_compute {
            return;
        }
        if *curr_max_exit == could_not_compute {
            *curr_max_exit = max_exit_count;
        } else {
            let cx = self.env.cx(&*self.context, &*self.function);
            *curr_max_exit = self.se.get_umin_from_mismatched_types_pair(&cx, *curr_max_exit, max_exit_count);
        }
        // If the loop has more than 1 iteration, all further checks will be
        // executed 1 iteration less.
        if *curr_max_exit == max_be_count {
            *skip_last_iter = true;
        }
    }
}

#[cfg(test)]
#[path = "exitopt_tests.rs"]
mod tests;
