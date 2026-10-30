//! Division, remainder and recurrences: `getUDivExpr`, `getURemExpr`,
//! `getAddRecExpr`, and the negation, complement and difference built on the
//! folds, with `getPointerBase`.

use std::borrow::Cow;

use llrm_mir::module::BlockId;

use super::ap::Ap;
use super::expr::{Kind, Scev, Ty, Type, Use, flags};
use super::{Cx, Flags, Key, KeyLike, ScalarEvolution};

impl ScalarEvolution {
    /// `SCEVAddRecExpr::getStart`.
    pub fn addrec_start(
        &self,
        ar: Scev,
    ) -> Scev {
        self.operands(ar)[0].scev
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolutionExpressions.h#L394
    /// `SCEVAddRecExpr::getStepRecurrence`: the step, or for a non-affine
    /// recurrence the recurrence of the rest.
    pub fn get_step_recurrence(
        &mut self,
        cx: &Cx,
        ar: Scev,
    ) -> Scev {
        let Kind::AddRec(operands, header) = self.node(ar).kind.clone() else { unreachable!("an add recurrence") };
        if operands.len() == 2 {
            return operands[1].scev;
        }
        self.get_add_rec_expr(cx, operands[1..].to_vec(), header, Flags::default()).scev
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3062
    /// `getOrCreateUDivExpr`.
    fn get_or_create_udiv_expr(
        &mut self,
        lhs: Use,
        rhs: Use,
    ) -> Scev {
        let pair = [lhs, rhs];
        let key =
            Key { tag: Type::UDiv, ops: Cow::Borrowed(&pair), ty: None, constant: None, value: None, header: None };
        let ty = self.type_of(lhs.scev);
        let size = self.compute_expression_size(&[lhs, rhs]);
        self.intern(key, || Kind::UDiv(lhs, rhs), Some(ty), size)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3016
    /// `getOrCreateAddRecExpr`.
    fn get_or_create_add_rec_expr(
        &mut self,
        cx: &Cx,
        ops: &[Use],
        l: BlockId,
        f: u8,
    ) -> Scev {
        let key =
            Key { tag: Type::AddRec, ops: Cow::Borrowed(ops), ty: None, constant: None, value: None, header: Some(l) };
        let ty = self.type_of(ops[0].scev);
        let size = self.compute_expression_size(ops);
        let s = self.intern(key, || Kind::AddRec(ops.to_vec(), l), Some(ty), size);
        self.set_no_wrap_flags(s, f);
        let _ = cx;
        s
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3449
    /// `getURemExpr`: an unsigned remainder expression based on unsigned
    /// division.
    pub fn get_urem_expr(
        &mut self,
        cx: &Cx,
        lhs: Use,
        rhs: Use,
    ) -> Scev {
        debug_assert!(
            self.get_effective_scev_type(cx, self.type_of(lhs.scev))
                == self.get_effective_scev_type(cx, self.type_of(rhs.scev)),
            "SCEVURemExpr operand types don't match!"
        );

        // Short-circuit easy cases
        if let Some(rhsc) = self.as_constant(rhs.scev) {
            // If constant is one, the result is trivial
            if rhsc.is_one() {
                // X urem 1 --> 0
                return self.get_zero(cx, self.type_of(lhs.scev));
            }

            // If constant is a power of two, fold into a zext(trunc(LHS)).
            if rhsc.is_power_of_2() {
                let full_ty = self.type_of(lhs.scev);
                let trunc_ty = Ty::Int(rhsc.countr_zero());
                let truncated = self.get_truncate_expr(cx, lhs, trunc_ty, 0);
                return self.get_zero_extend_expr(cx, Use::of(truncated), full_ty, 0);
            }
        }

        // Fallback to %a == %x urem %y == %x -<nuw> ((%x udiv %y) *<nuw> %y)
        let udiv = self.get_udiv_expr(cx, lhs, rhs);
        let mult = self.get_mul_expr(cx, vec![Use::of(udiv), rhs], Flags::expr(flags::NUW), 0).scev;
        self.get_minus_scev(cx, lhs, Use::of(mult), flags::NUW, 0)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3477
    /// `getUDivExpr`: a canonical unsigned division expression, or something
    /// simpler if possible.
    pub fn get_udiv_expr(
        &mut self,
        cx: &Cx,
        lhs: Use,
        rhs: Use,
    ) -> Scev {
        debug_assert!(!self.type_of(lhs.scev).is_pointer(), "SCEVUDivExpr operand can't be pointer!");
        debug_assert!(self.type_of(lhs.scev) == self.type_of(rhs.scev), "SCEVUDivExpr operand types don't match!");

        let pair = [lhs, rhs];
        let key =
            Key { tag: Type::UDiv, ops: Cow::Borrowed(&pair), ty: None, constant: None, value: None, header: None };
        if let Some(&found) = self.unique.get(&key as &dyn KeyLike) {
            return found;
        }

        // 0 udiv Y == 0
        if self.match_zero(lhs.scev) {
            return lhs.scev;
        }

        if let Some(rhsc) = self.as_constant(rhs.scev) {
            if rhsc.is_one() {
                return lhs.scev; // X udiv 1 --> x
            }
            // If the denominator is zero, the result of the udiv is undefined.
            // Don't try to analyze it, because the resolution chosen here may
            // differ from the resolution chosen in other parts of the compiler.
            if !rhsc.is_zero() {
                // Determine if the division can be folded into the operands of
                // its operands.
                // TODO: Generalize this to non-constants by using known-bits
                // information.
                let ty = self.type_of(lhs.scev);
                let lz = rhsc.countl_zero();
                let mut max_shift_amt = self.get_type_size_in_bits(cx, ty) - lz - 1;
                // For non-power-of-two values, effectively round the value up
                // to the nearest power of two.
                if !rhsc.is_power_of_2() {
                    max_shift_amt += 1;
                }
                let ext_ty = Ty::Int(self.get_type_size_in_bits(cx, ty) + max_shift_amt);
                if matches!(self.node(lhs.scev).kind, Kind::AddRec(..)) {
                    let ar = lhs.scev;
                    let Kind::AddRec(_, ar_loop) = self.node(ar).kind else { unreachable!() };
                    let ar_step = self.get_step_recurrence(cx, ar);
                    if let Some(step_int) = self.as_constant(ar_step) {
                        let step = ar_step;
                        // {X,+,N}/C --> {X/C,+,N/C} if safe and N/C can be
                        // folded.
                        let div_int = rhsc;
                        if step_int.urem(div_int).is_zero() {
                            let ar_start = self.addrec_start(ar);
                            let wide_ar = self.get_zero_extend_expr(cx, Use::of(ar), ext_ty, 0);
                            let wide_start = self.get_zero_extend_expr(cx, Use::of(ar_start), ext_ty, 0);
                            let wide_step = self.get_zero_extend_expr(cx, Use::of(step), ext_ty, 0);
                            let wide_rec = self
                                .get_add_rec_expr(
                                    cx,
                                    vec![Use::of(wide_start), Use::of(wide_step)],
                                    ar_loop,
                                    Flags::default(),
                                )
                                .scev;
                            if wide_ar == wide_rec {
                                let mut operands: Vec<Use> = Vec::new();
                                for op in self.operands(ar).to_vec() {
                                    operands.push(Use::of(self.get_udiv_expr(cx, Use::of(op.scev), rhs)));
                                }
                                return self.get_add_rec_expr(cx, operands, ar_loop, Flags::expr(flags::NW)).scev;
                            }
                        }
                        // Get a canonical UDivExpr for a recurrence.
                        // {X,+,N}/C => {Y,+,N}/C where Y=X-(X%N). Safe when
                        // C%N=0.
                        let ar_start = self.addrec_start(ar);
                        if div_int.urem(step_int).is_zero() {
                            let start_rem_expr = self.get_urem_expr(cx, Use::of(ar_start), Use::of(step));
                            if let Some(start_rem) = self.match_apint(start_rem_expr) {
                                let wide_ar = self.get_zero_extend_expr(cx, Use::of(ar), ext_ty, 0);
                                let wide_start = self.get_zero_extend_expr(cx, Use::of(ar_start), ext_ty, 0);
                                let wide_step = self.get_zero_extend_expr(cx, Use::of(step), ext_ty, 0);
                                let wide_rec = self
                                    .get_add_rec_expr(
                                        cx,
                                        vec![Use::of(wide_start), Use::of(wide_step)],
                                        ar_loop,
                                        Flags::default(),
                                    )
                                    .scev;
                                let no_wrap = wide_ar == wide_rec;

                                // With N <= C and both N, C as powers-of-2, the
                                // transformation {X,+,N}/C => {(X - X%N),+,N}/C
                                // preserves division results even if wrapping
                                // occurs, as the division results remain
                                // equivalent for all offsets in [[(X - X%N),
                                // X).
                                let can_fold_with_wrap =
                                    // N <= C
                                    step_int.ule(div_int) && step_int.is_power_of_2() && div_int.is_power_of_2();
                                // Only fold if the subtraction can be folded in
                                // the start expression.
                                let start_rem_const = self.get_constant(start_rem);
                                let new_start = self.get_minus_scev(
                                    cx,
                                    Use::of(ar_start),
                                    Use::of(start_rem_const),
                                    flags::NONE,
                                    0,
                                );
                                if !start_rem.is_zero()
                                    && (no_wrap || can_fold_with_wrap)
                                    && !matches!(self.node(new_start).kind, Kind::Add(_))
                                {
                                    let new_lhs = self
                                        .get_add_rec_expr(
                                            cx,
                                            vec![Use::of(new_start), Use::of(step)],
                                            ar_loop,
                                            Flags::expr(if no_wrap { flags::NW } else { flags::NONE }),
                                        )
                                        .scev;
                                    if lhs.scev != new_lhs {
                                        return self.get_udiv_expr(cx, Use::of(new_lhs), rhs);
                                    }
                                }
                            }
                        }
                    }
                }
                // (A*B)/C --> A*(B/C) if safe and B/C can be folded.
                if matches!(self.node(lhs.scev).kind, Kind::Mul(_)) {
                    let m = lhs.scev;
                    if self.get_no_wrap_flags(m, flags::NUW) != 0 {
                        // Find an operand that's safely divisible.
                        let operands = self.operands(m).to_vec();
                        for (i, op) in operands.iter().enumerate() {
                            let div = self.get_udiv_expr(cx, Use::of(op.scev), Use::of(rhs.scev));
                            let back =
                                self.get_mul_expr(cx, vec![Use::of(div), Use::of(rhs.scev)], Flags::default(), 0).scev;
                            if !matches!(self.node(div).kind, Kind::UDiv(..)) && back == op.scev {
                                let mut new_operands = operands.clone();
                                new_operands[i] = Use::of(div);
                                return self.get_mul_expr(cx, new_operands, Flags::default(), 0).scev;
                            }
                        }

                        // Even if it's not divisible, try to remove a common
                        // factor.
                        if let Some(lhsc) = self.as_constant(operands[0].scev) {
                            let factor = gcd_ap(lhsc, rhsc);
                            if factor.active_bits() > 1 {
                                let mut new_operands: Vec<Use> = Vec::new();
                                new_operands.push(Use::of(self.get_constant(lhsc.udiv(factor))));
                                new_operands.extend_from_slice(&operands[1..]);
                                let new_mul = self.get_mul_expr(cx, new_operands, Flags::default(), 0).scev;
                                let denominator = self.get_constant(rhsc.udiv(factor));
                                return self.get_udiv_expr(cx, Use::of(new_mul), Use::of(denominator));
                            }
                        }
                    }
                }

                // (A/B)/C --> A/(B*C) if safe and B*C can be folded.
                if let Kind::UDiv(other_lhs, other_rhs) = self.node(lhs.scev).kind
                    && let Some(divisor_constant) = self.as_constant(other_rhs.scev)
                {
                    let (new_rhs, overflow) = divisor_constant.umul_ov(rhsc);
                    if overflow {
                        return self.get_constant_of_type(cx, self.type_of(rhs.scev), 0);
                    }
                    let denominator = self.get_constant(new_rhs);
                    return self.get_udiv_expr(cx, other_lhs, Use::of(denominator));
                }

                // (A+B)/C --> (A/C + B/C) if the add does not unsigned wrap and
                // A/C and B/C can be folded.
                if matches!(self.node(lhs.scev).kind, Kind::Add(_)) {
                    let a = lhs.scev;
                    if self.get_no_wrap_flags(a, flags::NUW) != 0 {
                        let a_ops = self.operands(a).to_vec();
                        let mut operands: Vec<Use> = Vec::new();
                        for a_op in &a_ops {
                            let op = self.get_udiv_expr(cx, *a_op, rhs);
                            let back = self.get_mul_expr(cx, vec![Use::of(op), rhs], Flags::default(), 0).scev;
                            if matches!(self.node(op).kind, Kind::UDiv(..)) || back != a_op.scev {
                                break;
                            }
                            operands.push(Use::of(op));
                        }
                        if operands.len() == a_ops.len() {
                            return self.get_add_expr(cx, operands, Flags::default(), 0).scev;
                        }
                    }
                }

                // ((N - M) + (M * A)) / N --> ((N - 1) + (M * A)) / N
                // This is an idiom for rounding A up to the next multiple of N,
                // where A is aready known to be a multiple of M. In this case,
                // instcombine can see that some low bits of the added constant
                // are unused, so can clear them, but we want to canonicalise to
                // set the low bits. This makes the pattern easier to match,
                // without needing to check for known bits in A*M.
                let n = rhsc;
                if let Some((n_minus_m_scev, mul)) = self.match_binary(lhs.scev, Type::Add, 0)
                    && let Some(n_minus_m) = self.match_apint(n_minus_m_scev)
                    && let Some((m_scev, a)) = self.match_binary(mul, Type::Mul, 0)
                    && let Some(m) = self.match_apint(m_scev)
                    && n.is_power_of_2()
                    && m.is_power_of_2()
                    && m.ult(n)
                    && n_minus_m == n.sub(m)
                {
                    let n_minus_1 = self.get_constant(n.sub(Ap::one(n.width)));
                    let m_const = self.get_constant(m);
                    let prod = self.get_mul_expr(cx, vec![Use::of(m_const), Use::of(a)], Flags::default(), 0);
                    let sum = self.get_add_expr(cx, vec![Use::of(n_minus_1), prod], Flags::default(), 0);
                    return self.get_udiv_expr(cx, sum, rhs);
                }

                // Fold if both operands are constant.
                if let Some(lhsc) = self.as_constant(lhs.scev) {
                    return self.get_constant(lhsc.udiv(rhsc));
                }
            }
        }

        // ((-C + (C smax %x)) /u %x) evaluates to zero, for any positive
        // constant C.
        if let Some((neg_c_scev, smax)) = self.match_binary(lhs.scev, Type::Add, 0)
            && let Some(neg_c) = self.match_apint(neg_c_scev)
            && let Some((m0, m1)) = self.match_binary(smax, Type::SMax, 0)
        {
            // m_scev_SMax is commutable: the constant and the specific
            // operand in either order.
            let found = [(m0, m1), (m1, m0)]
                .into_iter()
                .find_map(|(c, specific)| self.match_apint(c).filter(|_| specific == rhs.scev));
            if let Some(c) = found
                && neg_c.is_negative()
                && !neg_c.is_signed_min()
                && c == neg_c.neg()
            {
                return self.get_zero(cx, self.type_of(lhs.scev));
            }
        }

        // (%a * %b)<nuw> / %b -> %a
        if matches!(self.node(lhs.scev).kind, Kind::Mul(_)) && self.get_no_wrap_flags(lhs.scev, flags::NUW) != 0 {
            let operands = self.operands(lhs.scev).to_vec();
            for (i, op) in operands.iter().enumerate() {
                if *op == rhs {
                    let mut new_operands: Vec<Use> = operands[..i].to_vec();
                    new_operands.extend_from_slice(&operands[i + 1..]);
                    return self.get_mul_expr(cx, new_operands, Flags::default(), 0).scev;
                }
            }
        }

        // TODO: Generalize to handle any common factors.
        // udiv (mul nuw a, vscale), (mul nuw b, vscale) --> udiv a, b
        // (DEVIATE 2: no vscale.)

        self.get_or_create_udiv_expr(lhs, rhs)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3677
    /// `getUDivExactExpr`.
    pub fn get_udiv_exact_expr(
        &mut self,
        cx: &Cx,
        lhs: Use,
        rhs: Use,
    ) -> Scev {
        // Currently there is no exact specific logic.
        self.get_udiv_expr(cx, lhs, rhs)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3685
    /// `getAddRecExpr(Start, Step, L, Flags)`: an add recurrence for the loop,
    /// simplified as much as possible.
    pub fn get_add_rec_expr_two(
        &mut self,
        cx: &Cx,
        start: Use,
        step: Use,
        l: BlockId,
        f: Flags,
    ) -> Use {
        let mut operands: Vec<Use> = vec![start];
        if let Kind::AddRec(step_operands, step_loop) = self.node(step.scev).kind.clone()
            && step_loop == l
        {
            operands.extend(step_operands);
            // The use flags describe the two-operand recurrence, not the
            // flattened one built here, so drop them just like the
            // expression's NUW/NSW.
            return self.get_add_rec_expr(cx, operands, l, Flags::expr(f.expr & flags::NW));
        }

        operands.push(step);
        self.get_add_rec_expr(cx, operands, l, f)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3704
    /// `getAddRecExpr(Operands, L, Flags)`: an add recurrence for the loop,
    /// simplified as much as possible.
    pub fn get_add_rec_expr(
        &mut self,
        cx: &Cx,
        mut operands: Vec<Use>,
        l: BlockId,
        nw_flags: Flags,
    ) -> Use {
        let mut f = nw_flags.expr;
        let use_flags = nw_flags.use_;
        debug_assert!(use_flags & !(flags::NUW | flags::NSW) == 0, "only nuw or nsw allowed");
        if operands.len() == 1 {
            return operands[0];
        }
        #[cfg(debug_assertions)]
        {
            let ety = self.get_effective_scev_type(cx, self.type_of(operands[0].scev));
            for op in &operands[1..] {
                debug_assert!(
                    self.get_effective_scev_type(cx, self.type_of(op.scev)) == ety,
                    "SCEVAddRecExpr operand types don't match!"
                );
                debug_assert!(!self.type_of(op.scev).is_pointer(), "Step must be integer");
            }
        }

        #[cfg(debug_assertions)]
        let orig_operands = operands.clone();

        if self.match_zero(operands.last().expect("operands").scev) {
            operands.pop();
            // {X,+,0}  -->  X
            return self.get_add_rec_expr(cx, operands, l, Flags::default());
        }

        // It's tempting to want to call getConstantMaxBackedgeTakenCount count
        // here and use that information to infer NUW and NSW flags. However,
        // computing a BE count requires calling getAddRecExpr, so we may not
        // yet have a meaningful BE count at this point (and if we don't, we'd
        // be stuck with a SCEVCouldNotCompute as the cached BE count).

        f = self.strengthen_no_wrap_flags(cx, Type::AddRec, &operands, f);

        // Canonicalize nested AddRecs in by nesting them in order of loop
        // depth.
        if let Kind::AddRec(nested_operands, nested_loop) = self.node(operands[0].scev).kind.clone() {
            let nested_ar = operands[0].scev;
            let outer = super::disposition::loop_of(cx, l);
            let nested = super::disposition::loop_of(cx, nested_loop);
            let reorder = if outer.contains_loop(Some(nested)) {
                outer.get_loop_depth() < nested.get_loop_depth()
            } else {
                !nested.contains_loop(Some(outer)) && cx.tree.dominates(l, nested_loop)
            };
            if reorder {
                let mut nested_operands: Vec<Use> = nested_operands;
                operands[0] = Use::of(self.addrec_start(nested_ar));
                // AddRecs require their operands be loop-invariant with respect
                // to their loops. Don't perform this transformation if it would
                // break this requirement.
                let all_invariant = operands.clone().iter().all(|op| self.is_loop_invariant(cx, op.scev, Some(l)));

                if all_invariant {
                    // Create a recurrence for the outer loop with the same step
                    // size.
                    //
                    // The outer recurrence keeps its NW flag but only keeps
                    // NUW/NSW if the inner recurrence has the same property.
                    let outer_flags = f & (flags::NW | self.get_no_wrap_flags(nested_ar, flags::MASK));

                    nested_operands[0] = self.get_add_rec_expr(cx, operands.clone(), l, Flags::expr(outer_flags));
                    let all_invariant =
                        nested_operands.clone().iter().all(|op| self.is_loop_invariant(cx, op.scev, Some(nested_loop)));

                    if all_invariant {
                        // Ok, both add recurrences are valid after the
                        // transformation.
                        //
                        // The inner recurrence keeps its NW flag but only keeps
                        // NUW/NSW if the outer recurrence has the same
                        // property.
                        let inner_flags = self.get_no_wrap_flags(nested_ar, flags::MASK) & (flags::NW | f);
                        return self.get_add_rec_expr(cx, nested_operands, nested_loop, Flags::expr(inner_flags));
                    }
                }
                // Reset Operands to its original state.
                operands[0] = Use::of(nested_ar);
            }
        }

        // Okay, it looks like we really DO need an addrec expr. Check to see if
        // we already have one, otherwise create a new one.
        #[cfg(debug_assertions)]
        debug_assert!(
            use_flags == flags::NONE || orig_operands == operands,
            "Tried to add SCEVUse flags after operands changed"
        );
        let s = self.get_or_create_add_rec_expr(cx, &operands, l, f);
        self.with_use_flags(s, use_flags)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4582
    /// `getNegativeSCEV`: -V = -1*V.
    pub fn get_negative_scev(
        &mut self,
        cx: &Cx,
        v: Scev,
        f: u8,
    ) -> Scev {
        if let Kind::Constant(value) = self.node(v).kind {
            return self.get_constant(value.neg());
        }

        let ty = self.type_of(v);
        let ty = self.get_effective_scev_type(cx, ty);
        let minus_one = self.get_minus_one(cx, ty);
        self.get_mul_expr(cx, vec![Use::of(v), Use::of(minus_one)], Flags::expr(f), 0).scev
    }

    /// `getMinusOne(Ty)`.
    pub fn get_minus_one(
        &mut self,
        cx: &Cx,
        ty: Ty,
    ) -> Scev {
        self.get_constant_of_type(cx, ty, -1)
    }

    /// `MatchNotExpr`: A, if `expr` computes ~A.
    fn match_not_expr(
        &self,
        expr: Scev,
    ) -> Option<Scev> {
        let (all_ones, mul) = self.match_binary(expr, Type::Add, 0)?;
        if !self.match_all_ones(all_ones) {
            return None;
        }
        let (all_ones, mul_op) = self.match_binary(mul, Type::Mul, 0)?;
        self.match_all_ones(all_ones).then_some(mul_op)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4603
    /// `getNotSCEV`: ~V = -1-V.
    pub fn get_not_scev(
        &mut self,
        cx: &Cx,
        v: Scev,
    ) -> Scev {
        debug_assert!(!self.type_of(v).is_pointer(), "Can't negate pointer");

        if let Kind::Constant(value) = self.node(v).kind {
            return self.get_constant(value.not());
        }

        // Fold ~(u|s)(min|max)(~x, ~y) to (u|s)(max|min)(x, y)
        if matches!(
            self.node(v).kind,
            Kind::UMax(_) | Kind::SMax(_) | Kind::UMin(_) | Kind::SMin(_)
        ) {
            let mut matched_operands: Vec<Use> = Vec::new();
            let mut all = true;
            for operand in self.operands(v).to_vec() {
                match self.match_not_expr(operand.scev) {
                    Some(matched) => matched_operands.push(Use::of(matched)),
                    None => {
                        all = false;
                        break;
                    }
                }
            }
            if all {
                let negated = match self.node(v).kind {
                    Kind::SMax(_) => Type::SMin,
                    Kind::SMin(_) => Type::SMax,
                    Kind::UMax(_) => Type::UMin,
                    Kind::UMin(_) => Type::UMax,
                    _ => unreachable!("Not a min or max SCEV type!"),
                };
                return self.get_min_max_expr(cx, negated, matched_operands);
            }
        }

        if let Some(&made) = self.not_cache.get(&v) {
            return made;
        }
        let ty = self.get_effective_scev_type(cx, self.type_of(v));
        let minus_one = self.get_minus_one(cx, ty);
        let made = self.get_minus_scev(cx, Use::of(minus_one), Use::of(v), flags::NONE, 0);
        self.not_cache.insert(v, made);
        made
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4632
    /// `removePointerBase`: `p` with its pointer base replaced by zero.
    pub fn remove_pointer_base(
        &mut self,
        cx: &Cx,
        p: Scev,
    ) -> Scev {
        debug_assert!(self.type_of(p).is_pointer());

        if let Kind::AddRec(_, header) = self.node(p).kind {
            // The base of an AddRec is the first operand.
            let mut ops: Vec<Use> = self.operands(p).to_vec();
            ops[0] = Use::of(self.remove_pointer_base(cx, ops[0].scev));
            // Don't try to transfer nowrap flags for now. We could in some
            // cases (for example, if pointer operand of the AddRec is a
            // SCEVUnknown).
            return self.get_add_rec_expr(cx, ops, header, Flags::default()).scev;
        }
        if matches!(self.node(p).kind, Kind::Add(_)) {
            // The base of an Add is the pointer operand.
            let mut ops: Vec<Use> = self.operands(p).to_vec();
            let mut ptr_op: Option<usize> = None;
            for (at, add_op) in ops.iter().enumerate() {
                if self.type_of(add_op.scev).is_pointer() {
                    debug_assert!(ptr_op.is_none(), "Cannot have multiple pointer ops");
                    ptr_op = Some(at);
                }
            }
            let at = ptr_op.expect("a pointer operand");
            ops[at] = Use::of(self.remove_pointer_base(cx, ops[at].scev));
            // Don't try to transfer nowrap flags for now. We could in some
            // cases (for example, if the pointer operand of the Add is a
            // SCEVUnknown).
            return self.get_add_expr(cx, ops, Flags::default(), 0).scev;
        }
        // Any other expression must be a pointer base.
        self.get_zero(cx, self.type_of(p))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4662
    /// `getMinusSCEV`: LHS - RHS, which is LHS + (-1)*RHS in SCEV.
    pub fn get_minus_scev(
        &mut self,
        cx: &Cx,
        mut lhs: Use,
        mut rhs: Use,
        f: u8,
        depth: u32,
    ) -> Scev {
        // Fast path: X - X --> 0.
        if lhs == rhs {
            return self.get_zero(cx, self.type_of(lhs.scev));
        }

        // If we subtract two pointers with different pointer bases, bail.
        // Eventually, we're going to add an assertion to getMulExpr that we
        // can't multiply by a pointer.
        if self.type_of(rhs.scev).is_pointer() {
            if !self.type_of(lhs.scev).is_pointer()
                || self.get_pointer_base(lhs.scev) != self.get_pointer_base(rhs.scev)
            {
                return self.get_could_not_compute();
            }
            lhs = Use::of(self.remove_pointer_base(cx, lhs.scev));
            rhs = Use::of(self.remove_pointer_base(cx, rhs.scev));
        }

        // We represent LHS - RHS as LHS + (-1)*RHS. This transformation makes
        // it so that we cannot make much use of NUW.
        let mut add_flags = flags::NONE;
        let rhs_range_min = self.get_signed_range(cx, rhs.scev).get_signed_min();
        let bits = self.get_type_size_in_bits(cx, self.type_of(rhs.scev));
        let rhs_is_not_min_signed = !Ap::new(bits, rhs_range_min).is_signed_min();
        if f & flags::NSW == flags::NSW {
            // Let M be the minimum representable signed value. Then (-1)*RHS
            // signed-wraps if and only if RHS is M. That can happen even for a
            // NSW subtraction because e.g. (-1)*M signed-wraps even though -1 -
            // M does not. So to transfer NSW from LHS - RHS to LHS + (-1)*RHS,
            // we need to prove that RHS != M.
            //
            // If LHS is non-negative and we know that LHS - RHS does not
            // signed-wrap, then RHS cannot be M. So we can rule out signed-wrap
            // either by proving that RHS > M or that LHS >= 0.
            if rhs_is_not_min_signed || self.is_known_non_negative(cx, lhs.scev) {
                add_flags = flags::NSW;
            }
        }

        // FIXME: Find a correct way to transfer NSW to (-1)*M when LHS - RHS is
        // NSW and LHS >= 0.
        //
        // The difficulty here is that the NSW flag may have been proven
        // relative to a loop that is to be found in a recurrence in LHS and not
        // in RHS. Applying NSW to (-1)*M may then let the NSW have a larger
        // scope than intended.
        let neg_flags = if rhs_is_not_min_signed { flags::NSW } else { flags::NONE };

        let negated = self.get_negative_scev(cx, rhs.scev, neg_flags);
        self.get_add_expr(cx, vec![lhs, Use::of(negated)], Flags::expr(add_flags), depth).scev
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4826
    /// `getPointerBase`.
    pub fn get_pointer_base(
        &self,
        v: Scev,
    ) -> Scev {
        // A pointer operand may evaluate to a nonpointer expression, such as
        // null.
        if !self.type_of(v).is_pointer() {
            return v;
        }

        let mut v = v;
        loop {
            match self.node(v).kind {
                Kind::AddRec(..) => v = self.addrec_start(v),
                Kind::Add(_) => {
                    let mut ptr_op: Option<Scev> = None;
                    for add_op in self.operands(v) {
                        if self.type_of(add_op.scev).is_pointer() {
                            debug_assert!(ptr_op.is_none(), "Cannot have multiple pointer ops");
                            ptr_op = Some(add_op.scev);
                        }
                    }
                    v = ptr_op.expect("Must have pointer op");
                }
                // Not something we can look further into.
                _ => return v,
            }
        }
    }
}

/// `APIntOps::GreatestCommonDivisor`.
fn gcd_ap(
    a: Ap,
    b: Ap,
) -> Ap {
    let (mut x, mut y) = (a.bits, b.bits);
    while y != 0 {
        (x, y) = (y, x % y);
    }
    Ap::new(a.width, x)
}
