//! `createAddRecFromPHIWithCasts`, `SCEVPredicateRewriter` and
//! `convertSCEVToAddRecWithPredicates` (P2 of
//! docs/working/scev-predicates-plan.md).
//!
//! # How it works
//! A header phi whose backedge value adds `sext(trunc(phi))` (or the `zext`)
//! to a loop-invariant step is not an add recurrence outright. Under three
//! run-time checks it is: the truncated recurrence does not wrap
//! ([`WrapPredicate`]), and the start and the step each equal their own
//! trunc-then-extend (two equal predicates).
//! [`create_add_rec_from_phi_with_casts`] finds the pattern and returns the
//! recurrence with those predicates; the rewriter walks an expression, replaces
//! such phis by their recurrences and widens `zext`/`sext` of a recurrence
//! under a `nusw`/`nssw` assumption, either recording the predicates it needed
//! or checking them against the ones given.
//!
//! [`create_add_rec_from_phi_with_casts`]:
//! ScalarEvolution::create_add_rec_from_phi_with_casts
//!
//! # Example
//! `%x = phi i64 [0, entry], [%x.next, loop]` with `%x.next = %step +
//! sext(trunc(%x to i32) to i64)` is `{0,+,%step}` under `{trunc(0),+,trunc
//! (%step)}<nssw>` and `%step == sext(trunc(%step))`.

use llrm_mir::module::{BlockId, Operand};
use llrm_mir::opcode::{IntPredicate, Opcode};

use super::expr::{Kind, Scev, Ty};
use super::wrap_predicate::{IncrementWrapFlags, Predicate, PredicateUnion};
use super::{Cx, Flags, ScalarEvolution};

/// What the `SCEVPredicateRewriter` carries: the loop, the predicates it may
/// add (`NewPreds`), and the ones it may assume (`Pred`).
struct Rewriter<'a> {
    header: BlockId,
    new_preds: Option<&'a mut Vec<Predicate>>,
    pred: Option<&'a PredicateUnion>,
}

impl ScalarEvolution {
    /// `isSimpleCastedPHI`: the truncated type and whether it is extended
    /// signed, if `op` is `ext(trunc(symbolic_phi))` of the phi's own width.
    fn is_simple_casted_phi(
        &self,
        cx: &Cx,
        op: Scev,
        symbolic_phi: Scev,
    ) -> Option<(Ty, bool)> {
        if op == symbolic_phi {
            return None;
        }
        if self.get_type_size_in_bits(cx, self.type_of(symbolic_phi))
            != self.get_type_size_in_bits(cx, self.type_of(op))
        {
            return None;
        }
        let (inner, signed) = match self.node(op).kind {
            Kind::SignExtend(inner) => (inner.scev, true),
            Kind::ZeroExtend(inner) => (inner.scev, false),
            _ => return None,
        };
        match self.node(inner).kind {
            Kind::Truncate(base) if base.scev == symbolic_phi => Some((self.type_of(inner), signed)),
            _ => None,
        }
    }

    /// `createAddRecFromPHIWithCasts`: the recurrence `symbolic_phi` is under
    /// predicates, if it is one; remembered per phi and loop.
    pub fn create_add_rec_from_phi_with_casts(
        &mut self,
        cx: &Cx,
        symbolic_phi: Scev,
    ) -> Option<(Scev, Vec<Predicate>)> {
        let Kind::Unknown(value) = self.node(symbolic_phi).kind else { return None };
        let pn = self.defining_inst(cx, value)?;
        if !matches!(cx.function.instruction(pn).opcode, Opcode::Phi) || self.type_of(symbolic_phi).is_pointer() {
            return None;
        }
        let header = cx.function.parent(pn)?;
        let loop_ = cx.loops.get_loop_for(header)?;
        if loop_.get_header() != header {
            return None;
        }
        // Check to see if we already analyzed this PHI.
        if let Some((rewrite, preds)) = self.predicated_rewrites.get(&(symbolic_phi, header)) {
            // Analysis was done before and failed to create an AddRec.
            return if *rewrite == symbolic_phi { None } else { Some((*rewrite, preds.clone())) };
        }
        let found = self.create_add_rec_from_phi_with_casts_impl(cx, symbolic_phi, pn, header);
        // Record in the cache that the analysis failed.
        if found.is_none() {
            self.predicated_rewrites.insert((symbolic_phi, header), (symbolic_phi, Vec::new()));
        }
        found
    }

    fn create_add_rec_from_phi_with_casts_impl(
        &mut self,
        cx: &Cx,
        symbolic_phi: Scev,
        pn: llrm_mir::module::InstId,
        header: BlockId,
    ) -> Option<(Scev, Vec<Predicate>)> {
        let loop_ = cx.loops.get_loop_for(header)?;
        // The loop may have multiple entrances or multiple exits; we can
        // analyze this phi as an addrec if it has a unique entry value and a
        // unique backedge value.
        let mut be_value_v: Option<Operand> = None;
        let mut start_value_v: Option<Operand> = None;
        for pair in cx.function.instruction(pn).operands.chunks(2) {
            let v = pair[0];
            let Operand::Block(block) = pair[1] else { unreachable!("a phi's block") };
            if loop_.contains(block) {
                match be_value_v {
                    None => be_value_v = Some(v),
                    Some(be) if be != v => {
                        be_value_v = None;
                        break;
                    }
                    Some(_) => {}
                }
            } else {
                match start_value_v {
                    None => start_value_v = Some(v),
                    Some(start) if start != v => {
                        start_value_v = None;
                        break;
                    }
                    Some(_) => {}
                }
            }
        }
        let (be_value_v, start_value_v) = (be_value_v?, start_value_v?);
        let be_value = self.get_scev(cx, be_value_v);

        // If the value coming around the backedge is an add with the symbolic
        // value we just inserted, possibly with casts that we can ignore under
        // an appropriate runtime guard, then we found a simple induction
        // variable!
        let Kind::Add(add_ops) = self.node(be_value).kind.clone() else { return None };

        // If there is a single occurrence of the symbolic value, possibly
        // casted, replace it with a recurrence.
        let (found_index, trunc_ty, signed) = add_ops
            .iter()
            .enumerate()
            .find_map(
                |(i, op)| self.is_simple_casted_phi(cx, op.scev, symbolic_phi).map(|(ty, signed)| (i, ty, signed)),
            )?;

        // Create an add with everything but the specified operand.
        let ops = add_ops.iter().enumerate().filter(|(i, _)| *i != found_index).map(|(_, op)| *op).collect();
        let accum = self.get_add_expr(cx, ops, Flags::default(), 0).scev;

        // The runtime checks will not be valid if the step amount is varying
        // inside the loop.
        if !self.is_loop_invariant(cx, accum, Some(header)) {
            return None;
        }

        // Create a truncated addrec for which we will add a no overflow check
        // (P1).
        let start_val = self.get_scev(cx, start_value_v);
        let truncated_start = self.get_truncate_expr(cx, super::expr::Use::of(start_val), trunc_ty, 0);
        let truncated_accum = self.get_truncate_expr(cx, super::expr::Use::of(accum), trunc_ty, 0);
        let phi_scev = self
            .get_add_rec_expr_two(
                cx,
                super::expr::Use::of(truncated_start),
                super::expr::Use::of(truncated_accum),
                header,
                Flags::default(),
            )
            .scev;

        let mut predicates: Vec<Predicate> = Vec::new();
        // PHISCEV can be either a SCEVConstant or a SCEVAddRecExpr: if it is a
        // constant, P1 degenerates into P2 or P3, so it is not added.
        if matches!(self.node(phi_scev).kind, Kind::AddRec(..)) {
            let added = if signed { IncrementWrapFlags::NSSW } else { IncrementWrapFlags::NUSW };
            predicates.push(self.get_wrap_predicate(phi_scev, added));
        }

        // Construct the extended SCEV: (Ext ix (Trunc iy (Expr) to ix) to iy)
        // for each of StartVal and Accum.
        let extended = |se: &mut Self, expr: Scev, sign: bool| -> Scev {
            debug_assert!(se.is_loop_invariant(cx, expr, Some(header)), "Expr is expected to be invariant");
            let truncated = se.get_truncate_expr(cx, super::expr::Use::of(expr), trunc_ty, 0);
            let ty = se.type_of(expr);
            if sign {
                se.get_sign_extend_expr(cx, super::expr::Use::of(truncated), ty, 0)
            } else {
                se.get_zero_extend_expr(cx, super::expr::Use::of(truncated), ty, 0)
            }
        };

        // Determine whether the predicate P: Expr == ExtendedExpr is known to
        // be false at compile time.
        let start_extended = extended(self, start_val, signed);
        if start_val != start_extended && self.is_known_predicate_scev(cx, IntPredicate::Ne, start_val, start_extended)
        {
            return None;
        }
        // The Step is always Signed (because the overflow checks are either
        // NSSW or NUSW).
        let accum_extended = extended(self, accum, true);
        if accum != accum_extended && self.is_known_predicate_scev(cx, IntPredicate::Ne, accum, accum_extended) {
            return None;
        }
        for (expr, extended) in [(start_val, start_extended), (accum, accum_extended)] {
            if expr != extended && !self.is_known_predicate_scev(cx, IntPredicate::Eq, expr, extended) {
                predicates.push(self.get_equal_predicate(expr, extended));
            }
        }

        // Predicates are ready. Now go ahead and create the new addrec in which
        // the casts had been folded away.
        let new_ar = self
            .get_add_rec_expr_two(
                cx,
                super::expr::Use::of(start_val),
                super::expr::Use::of(accum),
                header,
                Flags::default(),
            )
            .scev;
        self.predicated_rewrites.insert((symbolic_phi, header), (new_ar, predicates.clone()));
        Some((new_ar, predicates))
    }

    /// `rewriteUsingPredicate`: `s` with the equalities `preds` states
    /// applied and the extensions it assumes widened.
    pub fn rewrite_using_predicate(
        &mut self,
        cx: &Cx,
        s: Scev,
        header: BlockId,
        preds: &PredicateUnion,
    ) -> Scev {
        let mut rewriter = Rewriter { header, new_preds: None, pred: Some(preds) };
        self.predicate_rewrite(cx, s, &mut rewriter)
    }

    /// `convertSCEVToAddRecWithPredicates`: `s` as an add recurrence of the
    /// loop under the predicates appended to `preds`, if it can be one.
    pub fn convert_scev_to_add_rec_with_predicates(
        &mut self,
        cx: &Cx,
        s: Scev,
        header: BlockId,
        preds: &mut Vec<Predicate>,
    ) -> Option<Scev> {
        let mut transform: Vec<Predicate> = Vec::new();
        let rewritten = {
            let mut rewriter = Rewriter { header, new_preds: Some(&mut transform), pred: None };
            self.predicate_rewrite(cx, s, &mut rewriter)
        };
        if !matches!(self.node(rewritten).kind, Kind::AddRec(..)) {
            return None;
        }
        // Check if any of the transformed predicates is known to be false. In
        // that case, it doesn't make sense to convert to a predicated AddRec,
        // as the versioned loop will never execute.
        for pred in &transform {
            let Predicate::Wrap(wrap) = pred else { continue };
            if wrap.flags != IncrementWrapFlags::NSSW {
                continue;
            }
            let Kind::AddRec(_, to_check) = self.node(wrap.ar).kind else { continue };
            let exit_count = self.get_backedge_taken_count(cx, to_check, super::backedge::ExitCountKind::Exact);
            if self.is_could_not_compute(exit_count) {
                continue;
            }
            let step = self.get_step_recurrence(cx, wrap.ar);
            if !self.is_one_scev(step) {
                continue;
            }
            let step_ty = self.type_of(step);
            let exit_count = self.get_truncate_or_sign_extend(cx, exit_count, step_ty, 0);
            let start = self.operands_of(wrap.ar)[0];
            let add = self.get_add_expr(cx, vec![start, super::expr::Use::of(exit_count)], Flags::default(), 0).scev;
            if self.is_known_predicate_scev(cx, IntPredicate::Slt, add, start.scev) {
                return None;
            }
        }
        // Since the transformation was successful, we can now transfer the SCEV
        // predicates.
        preds.extend(transform);
        Some(rewritten)
    }

    fn predicate_rewrite(
        &mut self,
        cx: &Cx,
        s: Scev,
        rewriter: &mut Rewriter<'_>,
    ) -> Scev {
        match self.node(s).kind.clone() {
            Kind::Unknown(_) => {
                // The equalities first: an expression the predicate says equals
                // another is the other.
                if let Some(union) = rewriter.pred {
                    for one in &union.preds {
                        if let Predicate::Compare(compare) = one
                            && compare.lhs == s
                            && compare.pred == IntPredicate::Eq
                        {
                            return compare.rhs;
                        }
                    }
                }
                self.convert_to_add_rec_with_preds(cx, s, rewriter)
            }
            Kind::ZeroExtend(op) => self.rewrite_extend(cx, s, op.scev, false, rewriter),
            Kind::SignExtend(op) => self.rewrite_extend(cx, s, op.scev, true, rewriter),
            _ => self.rewrite_operands(cx, s, &mut |se, op| se.predicate_rewrite(cx, op, rewriter)),
        }
    }

    /// `visitZeroExtendExpr` and `visitSignExtendExpr`.
    fn rewrite_extend(
        &mut self,
        cx: &Cx,
        s: Scev,
        op: Scev,
        signed: bool,
        rewriter: &mut Rewriter<'_>,
    ) -> Scev {
        let operand = self.predicate_rewrite(cx, op, rewriter);
        let ty = self.type_of(s);
        if let Kind::AddRec(ops, header) = self.node(operand).kind.clone()
            && header == rewriter.header
            && ops.len() == 2
        {
            // This couldn't be folded because the operand didn't have the
            // nuw/nsw flag. Add the nusw/nssw flag as an assumption that we
            // could make.
            let step = self.get_step_recurrence(cx, operand);
            let added = if signed { IncrementWrapFlags::NSSW } else { IncrementWrapFlags::NUSW };
            let assumption = self.get_wrap_predicate(operand, added);
            if self.add_overflow_assumption(cx, assumption, rewriter) {
                let start = ops[0];
                let start = if signed {
                    self.get_sign_extend_expr(cx, start, ty, 0)
                } else {
                    self.get_zero_extend_expr(cx, start, ty, 0)
                };
                let step = self.get_sign_extend_expr(cx, super::expr::Use::of(step), ty, 0);
                let flags = self.node(operand).flags;
                return self
                    .get_add_rec_expr_two(
                        cx,
                        super::expr::Use::of(start),
                        super::expr::Use::of(step),
                        header,
                        Flags::expr(flags),
                    )
                    .scev;
            }
        }
        if signed {
            self.get_sign_extend_expr(cx, super::expr::Use::of(operand), ty, 0)
        } else {
            self.get_zero_extend_expr(cx, super::expr::Use::of(operand), ty, 0)
        }
    }

    /// `addOverflowAssumption(P)`.
    fn add_overflow_assumption(
        &mut self,
        cx: &Cx,
        pred: Predicate,
        rewriter: &mut Rewriter<'_>,
    ) -> bool {
        match rewriter.new_preds.as_deref_mut() {
            None => match rewriter.pred {
                // Check if we've already made this assumption.
                Some(union) => self.union_implies(cx, union, &pred),
                None => false,
            },
            Some(new_preds) => {
                new_preds.push(pred);
                true
            }
        }
    }

    /// `convertToAddRecWithPreds`: the phi `expr` as an add recurrence, if it
    /// can be one with predicates that can be added; else `expr`.
    fn convert_to_add_rec_with_preds(
        &mut self,
        cx: &Cx,
        expr: Scev,
        rewriter: &mut Rewriter<'_>,
    ) -> Scev {
        let Some((rewrite, preds)) = self.create_add_rec_from_phi_with_casts(cx, expr) else { return expr };
        for pred in preds {
            // Wrap predicates from outer loops are not supported.
            if let Predicate::Wrap(wrap) = pred
                && !matches!(
                    self.node(wrap.ar).kind,
                    Kind::AddRec(_, header) if header == rewriter.header
                )
            {
                return expr;
            }
            if !self.add_overflow_assumption(cx, pred, rewriter) {
                return expr;
            }
        }
        rewrite
    }
}

#[cfg(test)]
mod tests {
    use llrm_mir::datalayout::DataLayout;
    use llrm_mir::domtree::DomTree;
    use llrm_mir::loopinfo::LoopInfo;
    use llrm_mir::valuetracking::AssumptionCache;

    use super::*;

    /// `check` on @f's phi `%x`, its loop header and the expression set.
    fn with_phi(
        text: &str,
        check: impl FnOnce(&mut ScalarEvolution, &Cx, Scev, BlockId),
    ) {
        let module = llrm_mir::parse::module(text).expect("a module");
        let (_, _, function) = module.functions().find(|(_, _, function)| function.entry().is_some()).expect("@f");
        let layout = DataLayout::default();
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let assumptions = AssumptionCache::new(&module, function);
        let cx = Cx {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout: &layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let phi = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| {
                function
                    .instruction(inst)
                    .result
                    .is_some_and(|value| function.value(value).name.as_deref() == Some("x"))
            })
            .expect("%x");
        let header = function.parent(phi).expect("a block");
        let mut se = ScalarEvolution::new();
        let value = Operand::Value(function.instruction(phi).result.expect("a result"));
        let x = se.get_scev(&cx, value);
        check(&mut se, &cx, x, header);
    }

    const SEXT: &str = "define void @f(i64 %step, i64 %n) {
entry:
  br label %loop
loop:
  %x = phi i64 [ 0, %entry ], [ %x.next, %loop ]
  %t = trunc i64 %x to i32
  %s = sext i32 %t to i64
  %x.next = add i64 %s, %step
  %c = icmp ne i64 %x.next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
";

    /// `{0,+,%step}` under `{0,+,trunc %step}<nssw>` and `%step ==
    /// sext(trunc %step)`: the start 0 is its own extension, so it adds none.
    #[test]
    fn a_phi_that_adds_its_own_sext_trunc_is_an_addrec_under_predicates() {
        with_phi(SEXT, |se, cx, x, header| {
            assert!(matches!(se.node(x).kind, Kind::Unknown(_)), "not an addrec outright");
            let mut preds = Vec::new();
            let ar = se.convert_scev_to_add_rec_with_predicates(cx, x, header, &mut preds).expect("an addrec");
            let Kind::AddRec(ops, h) = se.node(ar).kind.clone() else { panic!("an addrec") };
            assert_eq!(h, header);
            assert_eq!(ops.len(), 2);
            assert!(se.as_constant(ops[0].scev).is_some_and(|c| c.zext_value() == 0), "start 0");
            let step = ops[1].scev;
            assert_eq!(preds.len(), 2, "{preds:?}");
            assert!(matches!(preds[0], Predicate::Wrap(wrap) if wrap.flags == IncrementWrapFlags::NSSW));
            assert!(
                matches!(preds[1], Predicate::Compare(compare) if compare.lhs == step && compare.pred == IntPredicate::Eq)
            );
        });
    }

    /// The same under `zext`: the wrap check is `nusw`.
    #[test]
    fn a_zext_trunc_asks_for_the_unsigned_wrap_check() {
        let text = SEXT.replace("sext i32", "zext i32");
        with_phi(&text, |se, cx, x, header| {
            let mut preds = Vec::new();
            se.convert_scev_to_add_rec_with_predicates(cx, x, header, &mut preds).expect("an addrec");
            assert!(matches!(preds[0], Predicate::Wrap(wrap) if wrap.flags == IncrementWrapFlags::NUSW), "{preds:?}");
        });
    }

    /// A step that varies inside the loop voids the run-time checks: no
    /// recurrence, and the failure is remembered.
    #[test]
    fn a_varying_step_is_no_recurrence() {
        let text = SEXT
            .replace("add i64 %s, %step", "add i64 %s, %c.prev")
            .replace("  %x = phi", "  %c.prev = phi i64 [ 1, %entry ], [ %x.next, %loop ]\n  %x = phi");
        with_phi(&text, |se, cx, x, header| {
            let mut preds = Vec::new();
            assert!(se.convert_scev_to_add_rec_with_predicates(cx, x, header, &mut preds).is_none());
            assert!(preds.is_empty());
        });
    }

    /// With the predicates given, `rewrite_using_predicate` widens the phi to
    /// the recurrence and a union that holds them implies each of them.
    #[test]
    fn the_predicates_it_made_imply_themselves() {
        with_phi(SEXT, |se, cx, x, header| {
            let mut preds = Vec::new();
            se.convert_scev_to_add_rec_with_predicates(cx, x, header, &mut preds).expect("an addrec");
            let mut union = PredicateUnion::default();
            for &one in &preds {
                se.union_add(cx, &mut union, one);
            }
            for one in &preds {
                assert!(se.union_implies(cx, &union, one), "{one:?}");
            }
            let widened = se.rewrite_using_predicate(cx, x, header, &union);
            assert!(matches!(se.node(widened).kind, Kind::AddRec(..)), "{:?}", se.node(widened).kind);
        });
    }
}

/// `print<scalar-evolution>` of a loop whose exit test is on the `sext` of a
/// 16-bit counter: the pin proves a count under a run-time no-signed-wrap
/// check, where this printed only "Unpredictable" (3 of the oracle's
/// corpus files differed). The text is the pinned opt's.
#[test]
fn test_a_count_under_a_wrap_check_is_printed_predicated() {
    let module = llrm_mir::parse::module(
        "define void @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i.next, %loop ]
  %i.next = add i16 %i, 1
  %ext = sext i16 %i.next to i32
  %c = icmp slt i32 %ext, %n
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
",
    )
    .expect("a module");
    let counts = super::values::print_module_counts(&module);
    let predicates = " Predicates:\n    {1,+,1}<%loop> Added Flags: <nssw>\n";
    let expected = format!(
        "Loop %loop: Unpredictable backedge-taken count.\n\
         Loop %loop: Unpredictable constant max backedge-taken count. \n\
         Loop %loop: Unpredictable symbolic max backedge-taken count. \n\
         Loop %loop: Predicated backedge-taken count is (-1 + (1 smax %n))<nsw>\n{predicates}\
         Loop %loop: Predicated constant max backedge-taken count is i32 2147483646\n{predicates}\
         Loop %loop: Predicated symbolic max backedge-taken count is (-1 + (1 smax %n))<nsw>\n{predicates}"
    );
    assert!(counts.ends_with(&expected), "{counts}");
}
