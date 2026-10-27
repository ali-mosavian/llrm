//! The cast folds: `getTruncateExpr`, `getZeroExtendExpr`, `getSignExtendExpr`,
//! `getAnyExtendExpr` and the `getTruncateOr...` family, with the extension of
//! an add recurrence's start and the overflow limits they use.

use std::borrow::Cow;

use llrm_mir::constantrange::ConstantRange;

use super::ap::Ap;
use super::backedge::ExitCountKind;
use super::expr::{Kind, Scev, Ty, Type, Use, flags};
use super::{Cx, Flags, Key, KeyLike, ScalarEvolution};

/// `scalar-evolution-max-cast-depth`.
const MAX_CAST_DEPTH: u32 = 8;

/// Which extension `ExtendOpTraits` is for.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Extend {
    Sign,
    Zero,
}

impl Extend {
    /// `ExtendOpTraits<..>::WrapType`.
    fn wrap_type(self) -> u8 {
        match self {
            Extend::Sign => flags::NSW,
            Extend::Zero => flags::NUW,
        }
    }
}

impl ScalarEvolution {
    /// The unique node for a cast of `op` to `ty` of kind `tag`, made if there
    /// is none: `new (SCEVAllocator) SCEV...Expr`.
    fn cast_node(
        &mut self,
        tag: Type,
        op: Use,
        ty: Ty,
    ) -> Scev {
        let one = [op];
        let key = Key { tag, ops: Cow::Borrowed(&one), ty: Some(ty), constant: None, value: None, header: None };
        let kind = || match tag {
            Type::Truncate => Kind::Truncate(op),
            Type::ZeroExtend => Kind::ZeroExtend(op),
            Type::SignExtend => Kind::SignExtend(op),
            Type::PtrToAddr => Kind::PtrToAddr(op),
            _ => unreachable!("not a cast"),
        };
        let size = self.compute_expression_size(&[op]);
        self.intern(key, kind, Some(ty), size)
    }

    /// `UniqueSCEVs.lookup` of a cast.
    fn lookup_cast(
        &self,
        tag: Type,
        op: Use,
        ty: Ty,
    ) -> Option<Scev> {
        let one = [op];
        let key = Key { tag, ops: Cow::Borrowed(&one), ty: Some(ty), constant: None, value: None, header: None };
        self.unique.get(&key as &dyn KeyLike).copied()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1114
    /// `ScalarEvolution::getTruncateExpr`.
    pub fn get_truncate_expr(
        &mut self,
        cx: &Cx,
        op: Use,
        ty: Ty,
        depth: u32,
    ) -> Scev {
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(op.scev)) > self.get_type_size_in_bits(cx, ty),
            "This is not a truncating conversion!"
        );
        debug_assert!(!self.type_of(op.scev).is_pointer(), "Can't truncate pointer!");
        let ty = self.get_effective_scev_type(cx, ty);

        if let Some(found) = self.lookup_cast(Type::Truncate, op, ty) {
            return found;
        }

        // Fold if the operand is constant.
        if let Kind::Constant(value) = self.node(op.scev).kind {
            return self.get_constant(value.trunc(self.get_type_size_in_bits(cx, ty)));
        }

        // trunc(trunc(x)) --> trunc(x)
        if let Kind::Truncate(inner) = self.node(op.scev).kind {
            return self.get_truncate_expr(cx, inner, ty, depth + 1);
        }

        // trunc(sext(x)) --> sext(x) if widening or trunc(x) if narrowing
        if let Kind::SignExtend(inner) = self.node(op.scev).kind {
            return self.get_truncate_or_sign_extend(cx, inner.scev, ty, depth + 1);
        }

        // trunc(zext(x)) --> zext(x) if widening or trunc(x) if narrowing
        if let Kind::ZeroExtend(inner) = self.node(op.scev).kind {
            return self.get_truncate_or_zero_extend(cx, inner.scev, ty, depth + 1);
        }

        if depth > MAX_CAST_DEPTH {
            return self.cast_node(Type::Truncate, op, ty);
        }

        // trunc(x1 + ... + xN) --> trunc(x1) + ... + trunc(xN) and
        // trunc(x1 * ... * xN) --> trunc(x1) * ... * trunc(xN),
        // if after transforming we have at most one truncate, not counting
        // truncates that replace other casts.
        if matches!(self.node(op.scev).kind, Kind::Add(_) | Kind::Mul(_)) {
            let comm_ops: Vec<Use> = self.operands(op.scev).to_vec();
            let mut operands: Vec<Use> = Vec::new();
            let mut num_truncs = 0;
            for comm_op in comm_ops.iter() {
                if num_truncs >= 2 {
                    break;
                }
                let s = self.get_truncate_expr(cx, *comm_op, ty, depth + 1);
                if !matches!(
                    self.node(comm_op.scev).kind,
                    Kind::Truncate(_) | Kind::ZeroExtend(_) | Kind::SignExtend(_)
                ) && matches!(self.node(s).kind, Kind::Truncate(_))
                {
                    num_truncs += 1;
                }
                operands.push(Use::of(s));
            }
            if num_truncs < 2 {
                if matches!(self.node(op.scev).kind, Kind::Add(_)) {
                    return self.get_add_expr(cx, operands, Flags::default(), 0).scev;
                }
                return self.get_mul_expr(cx, operands, Flags::default(), 0).scev;
            }
            // Although we checked in the beginning that ID is not in the
            // cache, it is possible that during recursion and different
            // modification ID was inserted into the cache. So if we find it,
            // just return it.
            if let Some(found) = self.lookup_cast(Type::Truncate, op, ty) {
                return found;
            }
        }

        // If the input value is a chrec scev, truncate the chrec's operands.
        if let Kind::AddRec(_, header) = self.node(op.scev).kind {
            let mut operands: Vec<Use> = Vec::new();
            for operand in self.operands(op.scev).to_vec() {
                operands.push(Use::of(self.get_truncate_expr(cx, operand, ty, depth + 1)));
            }
            return self.get_add_rec_expr(cx, operands, header, Flags::default()).scev;
        }

        // Return zero if truncating to known zeros.
        let min_trailing_zeros = self.get_min_trailing_zeros(cx, op.scev, None);
        if min_trailing_zeros >= self.get_type_size_in_bits(cx, ty) {
            return self.get_zero(cx, ty);
        }

        // The cast wasn't folded; create an explicit cast node.
        self.cast_node(Type::Truncate, op, ty)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1214
    /// `getSignedOverflowLimitForStep`: the predicate and limit, or none.
    pub(super) fn get_signed_overflow_limit_for_step(
        &mut self,
        cx: &Cx,
        step: Scev,
    ) -> Option<(llrm_mir::opcode::IntPredicate, Scev)> {
        use llrm_mir::opcode::IntPredicate as P;
        let bits = self.get_type_size_in_bits(cx, self.type_of(step));
        if self.is_known_positive(cx, step) {
            let max = self.get_signed_range(cx, step).get_signed_max();
            let limit = Ap::signed_min(bits).sub(Ap::new(bits, max));
            return Some((P::Slt, self.get_constant(limit)));
        }
        if self.is_known_negative(cx, step) {
            let min = self.get_signed_range(cx, step).get_signed_min();
            let limit = Ap::signed_max(bits).sub(Ap::new(bits, min));
            return Some((P::Sgt, self.get_constant(limit)));
        }
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1234
    /// `getUnsignedOverflowLimitForStep`.
    pub(super) fn get_unsigned_overflow_limit_for_step(
        &mut self,
        cx: &Cx,
        step: Scev,
    ) -> (llrm_mir::opcode::IntPredicate, Scev) {
        let bits = self.get_type_size_in_bits(cx, self.type_of(step));
        let max = self.get_unsigned_range(cx, step).get_unsigned_max();
        let limit = Ap::zero(bits).sub(Ap::new(bits, max));
        (llrm_mir::opcode::IntPredicate::Ult, self.get_constant(limit))
    }

    /// `ExtendOpTraits<..>::getOverflowLimitForStep`.
    fn get_overflow_limit_for_step(
        &mut self,
        cx: &Cx,
        extend: Extend,
        step: Scev,
    ) -> Option<(llrm_mir::opcode::IntPredicate, Scev)> {
        match extend {
            Extend::Sign => self.get_signed_overflow_limit_for_step(cx, step),
            Extend::Zero => Some(self.get_unsigned_overflow_limit_for_step(cx, step)),
        }
    }

    /// `ExtendOpTraits<..>::GetExtendExpr`.
    fn get_extend_expr(
        &mut self,
        cx: &Cx,
        extend: Extend,
        op: Scev,
        ty: Ty,
        depth: u32,
    ) -> Scev {
        match extend {
            Extend::Sign => self.get_sign_extend_expr(cx, Use::of(op), ty, depth),
            Extend::Zero => self.get_zero_extend_expr(cx, Use::of(op), ty, depth),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1306
    /// `getPreStartForExtend`: the start of `ar` less its step, if that is
    /// provably the same recurrence a step before.
    fn get_pre_start_for_extend(
        &mut self,
        cx: &Cx,
        extend: Extend,
        ar: Scev,
        depth: u32,
    ) -> Option<Scev> {
        let wrap_type = extend.wrap_type();

        let Kind::AddRec(_, l) = self.node(ar).kind else { unreachable!("an add recurrence") };
        let start = self.addrec_start(ar);
        let step = self.get_step_recurrence(cx, ar);

        // Check for a simple looking step prior to loop entry.
        if !matches!(self.node(start).kind, Kind::Add(_)) {
            return None;
        }
        let sa = start;

        // Create an AddExpr for "PreStart" after subtracting Step. Full SCEV
        // subtraction is expensive. For this purpose, perform a quick and dirty
        // difference, by checking for Step in the operand list. Note, that SA
        // might have repeated ops, like %a + %a + ..., so only remove one.
        let mut diff_ops: Vec<Use> = self.operands(sa).to_vec();
        if let Some(at) = diff_ops.iter().position(|it| it.scev == step) {
            diff_ops.remove(at);
        }

        if diff_ops.len() == self.operands(sa).len() {
            return None;
        }

        // Try to prove `WrapType` (SCEV::FlagNSW or SCEV::FlagNUW) on
        // `PreStart` + `Step`:

        // 1. NSW/NUW flags on the step increment.
        let pre_start_flags = self.get_no_wrap_flags(sa, flags::MASK) & flags::NUW;
        let pre_start = self.get_add_expr(cx, diff_ops, Flags::expr(pre_start_flags), 0).scev;
        let pre_ar = self.get_add_rec_expr(cx, vec![Use::of(pre_start), Use::of(step)], l, Flags::default()).scev;
        let pre_ar = matches!(self.node(pre_ar).kind, Kind::AddRec(..)).then_some(pre_ar);

        // "{S,+,X} is <nsw>/<nuw>" and "the backedge is taken at least once"
        // implies "S+X does not sign/unsign-overflow".
        let be_count = self.get_backedge_taken_count(cx, l, ExitCountKind::Exact);
        if let Some(pre_ar) = pre_ar
            && self.get_no_wrap_flags(pre_ar, wrap_type) != 0
            && !matches!(self.node(be_count).kind, Kind::CouldNotCompute)
            && self.is_known_positive(cx, be_count)
        {
            return Some(pre_start);
        }

        // 2. Direct overflow check on the step operation's expression.
        let bit_width = self.get_type_size_in_bits(cx, self.type_of(ar));
        let wide_ty = Ty::Int(bit_width * 2);
        let wide_pre = self.get_extend_expr(cx, extend, pre_start, wide_ty, depth);
        let wide_step = self.get_extend_expr(cx, extend, step, wide_ty, depth);
        let operand_extended_start =
            self.get_add_expr(cx, vec![Use::of(wide_pre), Use::of(wide_step)], Flags::default(), 0).scev;
        if self.get_extend_expr(cx, extend, start, wide_ty, depth) == operand_extended_start {
            if let Some(pre_ar) = pre_ar
                && self.get_no_wrap_flags(ar, wrap_type) != 0
            {
                // If we know `AR` == {`PreStart`+`Step`,+,`Step`} is `WrapType`
                // (FlagNSW or FlagNUW) and that `PreStart` + `Step` is
                // `WrapType` too, then `PreAR` == {`PreStart`,+,`Step`} is also
                // `WrapType`. Cache this fact.
                self.set_no_wrap_flags(pre_ar, wrap_type);
            }
            return Some(pre_start);
        }

        // 3. Loop precondition.
        let overflow_limit = self.get_overflow_limit_for_step(cx, extend, step);
        if let Some((pred, limit)) = overflow_limit
            && self.is_loop_entry_guarded_by_cond_pred(cx, l, pred, pre_start, limit)
        {
            return Some(pre_start);
        }

        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1383
    /// `getExtendAddRecStart`: the normalized zero or sign extended expression
    /// for this AddRec's Start.
    pub(super) fn get_extend_add_rec_start(
        &mut self,
        cx: &Cx,
        extend: Extend,
        ar: Scev,
        ty: Ty,
        depth: u32,
    ) -> Scev {
        let Some(pre_start) = self.get_pre_start_for_extend(cx, extend, ar, depth) else {
            let start = self.addrec_start(ar);
            return self.get_extend_expr(cx, extend, start, ty, depth);
        };
        let step = self.get_step_recurrence(cx, ar);
        let step = self.get_extend_expr(cx, extend, step, ty, depth);
        let pre_start = self.get_extend_expr(cx, extend, pre_start, ty, depth);
        self.get_add_expr(cx, vec![Use::of(step), Use::of(pre_start)], Flags::default(), 0).scev
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1430
    /// `proveNoWrapByVaryingStart`: whether `{start,+,step}` does not wrap
    /// because a nearby `{start-delta,+,step}` we already have does not.
    fn prove_no_wrap_by_varying_start(
        &mut self,
        cx: &Cx,
        extend: Extend,
        start: Scev,
        step: Scev,
        l: llrm_mir::module::BlockId,
    ) -> bool {
        let wrap_type = extend.wrap_type();

        // We restrict `Start` to a constant to prevent SCEV from spending too
        // much time here. It is correct (but more expensive) to continue with a
        // non-constant `Start` and do a general SCEV subtraction to compute
        // `PreStart` below.
        let Some(start_ai) = self.as_constant(start) else { return false };

        for delta in [-2_i128, -1, 1, 2] {
            let pre_start = self.get_constant(start_ai.sub(Ap::from_signed(start_ai.width, delta)));

            let pair = [Use::of(pre_start), Use::of(step)];
            let key = Key {
                tag: Type::AddRec,
                ops: Cow::Borrowed(&pair),
                ty: None,
                constant: None,
                value: None,
                header: Some(l),
            };
            let pre_ar = self.unique.get(&key as &dyn KeyLike).copied();

            // Give up if we don't already have the add recurrence we need
            // because actually constructing an add recurrence is relatively
            // expensive.
            if let Some(pre_ar) = pre_ar
                && self.get_no_wrap_flags(pre_ar, wrap_type) != 0
            {
                // proves (2)
                let delta_s = self.get_constant_of_type(cx, self.type_of(start), delta);
                let limit = self.get_overflow_limit_for_step(cx, extend, delta_s);
                if let Some((pred, limit)) = limit
                    && self.is_known_predicate_scev(cx, pred, pre_ar, limit)
                {
                    // proves (1)
                    return true;
                }
            }
        }

        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1477
    /// `extractConstantWithoutWrapping` for an add: the integer D of
    /// `(C + x + y + ...)` such that `D + (C - D + x + y + ...)` does not wrap
    /// and the trailing zeros of the rest are most.
    fn extract_constant_without_wrapping_add(
        &mut self,
        cx: &Cx,
        constant_term: Ap,
        whole_add_expr: Scev,
    ) -> Ap {
        let c = constant_term;
        let bit_width = c.width;
        // Find number of trailing zeros of (x + y + ...) w/o the C first:
        let mut tz = bit_width;
        let operands = self.operands(whole_add_expr).to_vec();
        for operand in operands.iter().skip(1) {
            if tz == 0 {
                break;
            }
            tz = tz.min(self.get_min_trailing_zeros(cx, operand.scev, None));
        }
        if tz != 0 {
            // Set D to be as many least significant bits of C as possible while
            // still guaranteeing that adding D to (C - D + x + y + ...) won't
            // cause a wrap:
            return if tz < bit_width { c.trunc(tz).zext(bit_width) } else { c };
        }
        Ap::zero(bit_width)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1498
    /// `extractConstantWithoutWrapping` for an affine add recurrence
    /// `{C,+,x}`.
    fn extract_constant_without_wrapping_addrec(
        &mut self,
        cx: &Cx,
        constant_start: Ap,
        step: Scev,
    ) -> Ap {
        let bit_width = constant_start.width;
        let tz = self.get_min_trailing_zeros(cx, step, None);
        if tz != 0 {
            return if tz < bit_width { constant_start.trunc(tz).zext(bit_width) } else { constant_start };
        }
        Ap::zero(bit_width)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1509
    /// `insertFoldCacheEntry`.
    fn insert_fold_cache_entry(
        &mut self,
        id: (Type, Use, Ty),
        s: Scev,
    ) {
        if let Some(previous) = self.fold_cache.insert(id, s) {
            // Remove FoldCacheUser entry for ID when replacing an existing
            // FoldCache entry.
            let user_ids = self.fold_cache_user.entry(previous).or_default();
            debug_assert!(user_ids.iter().filter(|one| **one == id).count() == 1, "unexpected duplicates in UserIDs");
            if let Some(at) = user_ids.iter().position(|one| *one == id) {
                user_ids.swap_remove(at);
            }
        }
        self.fold_cache_user.entry(s).or_default().push(id);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1531
    /// `ScalarEvolution::getZeroExtendExpr`.
    pub fn get_zero_extend_expr(
        &mut self,
        cx: &Cx,
        op: Use,
        ty: Ty,
        depth: u32,
    ) -> Scev {
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(op.scev)) < self.get_type_size_in_bits(cx, ty),
            "This is not an extending conversion!"
        );
        debug_assert!(!self.type_of(op.scev).is_pointer(), "Can't extend pointer!");
        let ty = self.get_effective_scev_type(cx, ty);

        let id = (Type::ZeroExtend, op, ty);
        if let Some(&found) = self.fold_cache.get(&id) {
            return found;
        }

        let s = self.get_zero_extend_expr_impl(cx, op, ty, depth);
        if !matches!(self.node(s).kind, Kind::ZeroExtend(_)) {
            self.insert_fold_cache_entry(id, s);
        }
        s
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1550
    /// `ScalarEvolution::getZeroExtendExprImpl`.
    pub fn get_zero_extend_expr_impl(
        &mut self,
        cx: &Cx,
        op: Use,
        ty: Ty,
        depth: u32,
    ) -> Scev {
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(op.scev)) < self.get_type_size_in_bits(cx, ty),
            "This is not an extending conversion!"
        );
        debug_assert!(!self.type_of(op.scev).is_pointer(), "Can't extend pointer!");

        // Fold if the operand is constant.
        if let Kind::Constant(value) = self.node(op.scev).kind {
            return self.get_constant(value.zext(self.get_type_size_in_bits(cx, ty)));
        }

        // zext(zext(x)) --> zext(x)
        if let Kind::ZeroExtend(inner) = self.node(op.scev).kind {
            return self.get_zero_extend_expr(cx, inner, ty, depth + 1);
        }

        // If the operand is an affine AddRec with the no-unsigned-wrap flag,
        // the zero-extension distributes over the recurrence.
        let affine = self.match_affine_add_rec(op.scev);
        if depth <= MAX_CAST_DEPTH
            && let Some((_, step, l)) = affine
        {
            let ar = op.scev;
            if self.get_no_wrap_flags(ar, flags::NUW) != 0 {
                let start = self.get_extend_add_rec_start(cx, Extend::Zero, ar, ty, depth + 1);
                let step = self.get_zero_extend_expr(cx, Use::of(step), ty, depth + 1);
                let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                return self.get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags)).scev;
            }
        }

        // Before doing any expensive analysis, check to see if we've already
        // computed a SCEV for this Op and Ty.
        if let Some(found) = self.lookup_cast(Type::ZeroExtend, op, ty) {
            return found;
        }
        if depth > MAX_CAST_DEPTH {
            return self.cast_node(Type::ZeroExtend, op, ty);
        }

        // zext(trunc(x)) --> zext(x) or x or trunc(x)
        if let Kind::Truncate(trunc_operand) = self.node(op.scev).kind {
            // It's possible the bits taken off by the truncate were all zero
            // bits. If so, we should be able to simplify this further.
            let x = trunc_operand.scev;
            let cr: ConstantRange = self.get_unsigned_range(cx, x);
            let trunc_bits = self.get_type_size_in_bits(cx, self.type_of(op.scev));
            let new_bits = self.get_type_size_in_bits(cx, ty);
            if cr.truncate(trunc_bits, 0).zero_extend(new_bits).contains_range(&cr.zext_or_trunc(new_bits)) {
                return self.get_truncate_or_zero_extend(cx, x, ty, depth);
            }
        }

        // If the input value is a chrec scev, and we can prove that the value
        // did not overflow the old, smaller, value, we can zero extend all of
        // the operands (often constants). This allows analysis of something
        // like this: for (unsigned char X = 0; X < 100; ++X) { int Y = X; }
        if let Some((start, step, l)) = affine {
            let ar = op.scev;
            let bit_width = self.get_type_size_in_bits(cx, self.type_of(ar));

            // The no-unsigned-wrap case is handled before the uniquing lookup
            // above.

            // Check whether the backedge-taken count is SCEVCouldNotCompute.
            // Note that this serves two purposes: It filters out loops that
            // are simply not analyzable, and it covers the case where this
            // code is being called from within backedge-taken count analysis,
            // such that attempting to ask for the backedge-taken count would
            // likely result in infinite recursion. In the later case, the
            // analysis code will cope with a conservative value, and it will
            // take care to purge that value once it has finished.
            let max_be_count = self.get_backedge_taken_count(cx, l, ExitCountKind::ConstantMaximum);
            if !matches!(self.node(max_be_count).kind, Kind::CouldNotCompute) {
                // Manually compute the final value for AR, checking for
                // overflow.

                // Check whether the backedge-taken count can be losslessly
                // casted to the addrec's type. The count is always unsigned.
                let casted_max_be_count =
                    self.get_truncate_or_zero_extend(cx, max_be_count, self.type_of(start), depth);
                let recasted_max_be_count =
                    self.get_truncate_or_zero_extend(cx, casted_max_be_count, self.type_of(max_be_count), depth);
                if max_be_count == recasted_max_be_count {
                    let wide_ty = Ty::Int(bit_width * 2);
                    // Check whether Start+Step*MaxBECount has no unsigned
                    // overflow.
                    let zmul = self
                        .get_mul_expr(
                            cx,
                            vec![Use::of(casted_max_be_count), Use::of(step)],
                            Flags::default(),
                            depth + 1,
                        )
                        .scev;
                    let sum =
                        self.get_add_expr(cx, vec![Use::of(start), Use::of(zmul)], Flags::default(), depth + 1).scev;
                    let zadd = self.get_zero_extend_expr(cx, Use::of(sum), wide_ty, depth + 1);
                    let wide_start = self.get_zero_extend_expr(cx, Use::of(start), wide_ty, depth + 1);
                    let wide_max_be_count =
                        self.get_zero_extend_expr(cx, Use::of(casted_max_be_count), wide_ty, depth + 1);
                    let wide_step = self.get_zero_extend_expr(cx, Use::of(step), wide_ty, depth + 1);
                    let product = self
                        .get_mul_expr(
                            cx,
                            vec![Use::of(wide_max_be_count), Use::of(wide_step)],
                            Flags::default(),
                            depth + 1,
                        )
                        .scev;
                    let operand_extended_add = self
                        .get_add_expr(cx, vec![Use::of(wide_start), Use::of(product)], Flags::default(), depth + 1)
                        .scev;
                    if zadd == operand_extended_add {
                        // Cache knowledge of AR NUW, which is propagated to
                        // this AddRec.
                        self.set_no_wrap_flags(ar, flags::NUW);
                        // Return the expression with the addrec on the outside.
                        let start = self.get_extend_add_rec_start(cx, Extend::Zero, ar, ty, depth + 1);
                        let step = self.get_zero_extend_expr(cx, Use::of(step), ty, depth + 1);
                        let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                        return self
                            .get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags))
                            .scev;
                    }
                    // Similar to above, only this time treat the step value as
                    // signed. This covers loops that count down.
                    let wide_step = self.get_sign_extend_expr(cx, Use::of(step), wide_ty, depth + 1);
                    let product = self
                        .get_mul_expr(
                            cx,
                            vec![Use::of(wide_max_be_count), Use::of(wide_step)],
                            Flags::default(),
                            depth + 1,
                        )
                        .scev;
                    let operand_extended_add = self
                        .get_add_expr(cx, vec![Use::of(wide_start), Use::of(product)], Flags::default(), depth + 1)
                        .scev;
                    if zadd == operand_extended_add {
                        // Cache knowledge of AR NW, which is propagated to this
                        // AddRec. Negative step causes unsigned wrap, but it
                        // still can't self-wrap.
                        self.set_no_wrap_flags(ar, flags::NW);
                        // Return the expression with the addrec on the outside.
                        let start = self.get_extend_add_rec_start(cx, Extend::Zero, ar, ty, depth + 1);
                        let step = self.get_sign_extend_expr(cx, Use::of(step), ty, depth + 1);
                        let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                        return self
                            .get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags))
                            .scev;
                    }
                }
            }

            // Normally, in the cases we can prove no-overflow via a backedge
            // guarding condition, we can also compute a backedge taken count
            // for the loop. The exceptions are assumptions and guards present
            // in the loop -- SCEV is not great at exploiting these to compute
            // max backedge taken counts, but can still use these to prove lack
            // of overflow. Use this fact to avoid doing extra work that may not
            // pay off.
            if !matches!(self.node(max_be_count).kind, Kind::CouldNotCompute)
                || self.has_guards(cx)
                || self.has_assumptions(cx)
            {
                let new_flags = self.prove_no_unsigned_wrap_via_induction(cx, ar);
                self.set_no_wrap_flags(ar, new_flags);
                if self.get_no_wrap_flags(ar, flags::NUW) != 0 {
                    // Same as nuw case above - duplicated here to avoid a
                    // compile time issue. It's not clear that the order of
                    // checks does matter, but it's one of two issue possible
                    // causes for a change which was reverted. Be conservative
                    // for the moment.
                    let start = self.get_extend_add_rec_start(cx, Extend::Zero, ar, ty, depth + 1);
                    let step = self.get_zero_extend_expr(cx, Use::of(step), ty, depth + 1);
                    let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                    return self
                        .get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags))
                        .scev;
                }

                // For a negative step, we can extend the operands iff doing so
                // only traverses values in the range zext([0,UINT_MAX]).
                if self.is_known_negative(cx, step) {
                    let min = self.get_signed_range(cx, step).get_signed_min();
                    let n = self.get_constant(Ap::all_ones(bit_width).sub(Ap::new(bit_width, min)));
                    if self.is_loop_backedge_guarded_by_cond_pred(cx, l, llrm_mir::opcode::IntPredicate::Ugt, ar, n)
                        || self.is_known_on_every_iteration(cx, llrm_mir::opcode::IntPredicate::Ugt, ar, n)
                    {
                        // Cache knowledge of AR NW, which is propagated to this
                        // AddRec. Negative step causes unsigned wrap, but it
                        // still can't self-wrap.
                        self.set_no_wrap_flags(ar, flags::NW);
                        // Return the expression with the addrec on the outside.
                        let start = self.get_extend_add_rec_start(cx, Extend::Zero, ar, ty, depth + 1);
                        let step = self.get_sign_extend_expr(cx, Use::of(step), ty, depth + 1);
                        let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                        return self
                            .get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags))
                            .scev;
                    }
                }
            }

            // zext({C,+,Step}) --> (zext(D) + zext({C-D,+,Step}))<nuw><nsw>
            // if D + (C - D + Step * n) could be proven to not unsigned wrap
            // where D maximizes the number of trailing zeros of (C - D + Step *
            // n)
            if let Some(c) = self.as_constant(start) {
                let d = self.extract_constant_without_wrapping_addrec(cx, c, step);
                if !d.is_zero() {
                    let d_const = self.get_constant(d);
                    let szext_d = self.get_zero_extend_expr(cx, Use::of(d_const), ty, depth);
                    let residual_start = self.get_constant(c.sub(d));
                    let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                    let sresidual = self
                        .get_add_rec_expr(cx, vec![Use::of(residual_start), Use::of(step)], l, Flags::expr(ar_flags))
                        .scev;
                    let szext_r = self.get_zero_extend_expr(cx, Use::of(sresidual), ty, depth + 1);
                    return self
                        .get_add_expr(
                            cx,
                            vec![Use::of(szext_d), Use::of(szext_r)],
                            Flags::expr(flags::NSW | flags::NUW),
                            depth + 1,
                        )
                        .scev;
                }
            }

            if self.prove_no_wrap_by_varying_start(cx, Extend::Zero, start, step, l) {
                self.set_no_wrap_flags(ar, flags::NUW);
                let start = self.get_extend_add_rec_start(cx, Extend::Zero, ar, ty, depth + 1);
                let step = self.get_zero_extend_expr(cx, Use::of(step), ty, depth + 1);
                let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                return self.get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags)).scev;
            }
        }

        // zext(A % B) --> zext(A) % zext(B)
        if let Some((lhs, rhs)) = self.match_urem(cx, op.scev) {
            let lhs = self.get_zero_extend_expr(cx, Use::of(lhs), ty, depth + 1);
            let rhs = self.get_zero_extend_expr(cx, Use::of(rhs), ty, depth + 1);
            return self.get_urem_expr(cx, Use::of(lhs), Use::of(rhs));
        }

        // zext(A / B) --> zext(A) / zext(B).
        if let Kind::UDiv(div_lhs, div_rhs) = self.node(op.scev).kind {
            let lhs = self.get_zero_extend_expr(cx, div_lhs, ty, depth + 1);
            let rhs = self.get_zero_extend_expr(cx, div_rhs, ty, depth + 1);
            return self.get_udiv_expr(cx, Use::of(lhs), Use::of(rhs));
        }

        if matches!(self.node(op.scev).kind, Kind::Add(_)) {
            let sa = op.scev;
            // zext((A + B + ...)<nuw>) --> (zext(A) + zext(B) + ...)<nuw>
            if self.get_no_wrap_flags(sa, flags::NUW) != 0 {
                // If the addition does not unsign overflow then we can, by
                // definition, commute the zero extension with the addition
                // operation.
                let mut ops: Vec<Use> = Vec::new();
                for operand in self.operands(sa).to_vec() {
                    ops.push(Use::of(self.get_zero_extend_expr(cx, operand, ty, depth + 1)));
                }
                return self.get_add_expr(cx, ops, Flags::expr(flags::NUW), depth + 1).scev;
            }

            // zext (C + A)<nsw> -> (sext(C) + sext(A))<nsw> if zext (C +
            // A)<nsw> >=s 0. Currently the non-negative check is done manually,
            // as isKnownNonNegative is too expensive.
            if self.get_no_wrap_flags(sa, flags::NSW) != 0
                && let Some((c_scev, smax)) = self.match_binary(sa, Type::Add, 0)
                && let Some(c) = self.match_apint(c_scev)
                && let Some((m0, m1)) = self.match_binary(smax, Type::SMax, 0)
                && let Some(c2) = self.match_apint(m0).or_else(|| self.match_apint(m1))
                && c.is_negative()
                && !c.is_signed_min()
                && !c2.slt(abs_ap(c))
            {
                let ops = self.operands(sa).to_vec();
                let a0 = self.get_sign_extend_expr(cx, ops[0], ty, depth + 1);
                let a1 = self.get_sign_extend_expr(cx, ops[1], ty, depth + 1);
                return self.get_add_expr(cx, vec![Use::of(a0), Use::of(a1)], Flags::expr(flags::NSW), depth + 1).scev;
            }

            // zext(C + x + y + ...) --> (zext(D) + zext((C - D) + x + y + ...))
            // if D + (C - D + x + y + ...) could be proven to not unsigned wrap
            // where D maximizes the number of trailing zeros of (C - D + x + y
            // + ...)
            //
            // Often address arithmetics contain expressions like
            // (zext (add (shl X, C1), C2)), for instance, (zext (5 + (4 * X))).
            // This transformation is useful while proving that such
            // expressions are equal or differ by a small constant amount, see
            // LoadStoreVectorizer pass.
            let first = self.operands(sa)[0].scev;
            if let Some(c) = self.as_constant(first) {
                let d = self.extract_constant_without_wrapping_add(cx, c, sa);
                if !d.is_zero() {
                    let d_const = self.get_constant(d);
                    let szext_d = self.get_zero_extend_expr(cx, Use::of(d_const), ty, depth);
                    let neg_d = self.get_constant(d.neg());
                    let sresidual =
                        self.get_add_expr(cx, vec![Use::of(neg_d), Use::of(sa)], Flags::default(), depth).scev;
                    let szext_r = self.get_zero_extend_expr(cx, Use::of(sresidual), ty, depth + 1);
                    return self
                        .get_add_expr(
                            cx,
                            vec![Use::of(szext_d), Use::of(szext_r)],
                            Flags::expr(flags::NSW | flags::NUW),
                            depth + 1,
                        )
                        .scev;
                }
            }
        }

        if matches!(self.node(op.scev).kind, Kind::Mul(_)) {
            let sm = op.scev;
            // zext((A * B * ...)<nuw>) --> (zext(A) * zext(B) * ...)<nuw>
            if self.get_no_wrap_flags(sm, flags::NUW) != 0 {
                // If the multiply does not unsign overflow then we can, by
                // definition, commute the zero extension with the multiply
                // operation.
                let mut ops: Vec<Use> = Vec::new();
                for operand in self.operands(sm).to_vec() {
                    ops.push(Use::of(self.get_zero_extend_expr(cx, operand, ty, depth + 1)));
                }
                return self.get_mul_expr(cx, ops, Flags::expr(flags::NUW), depth + 1).scev;
            }

            // zext(2^K * (trunc X to iN)) to iM ->
            // 2^K * (zext(trunc X to i{N-K}) to iM)<nuw>
            //
            // Proof:
            //
            //     zext(2^K * (trunc X to iN)) to iM
            //   = zext((trunc X to iN) << K) to iM
            //   = zext((trunc X to i{N-K}) << K)<nuw> to iM
            //     (because shl removes the top K bits)
            //   = zext((2^K * (trunc X to i{N-K}))<nuw>) to iM
            //   = (2^K * (zext(trunc X to i{N-K}) to iM))<nuw>.
            //
            if let Some((c_scev, trunc)) = self.match_binary(sm, Type::Mul, 0)
                && let Some(c) = self.match_apint(c_scev)
                && let Some(trunc_rhs) = self.match_unary(trunc, Type::Truncate)
                && c.is_power_of_2()
            {
                let ops = self.operands(sm).to_vec();
                let new_trunc_bits = self.get_type_size_in_bits(cx, self.type_of(ops[1].scev)) - c.countr_zero();
                let new_trunc_ty = Ty::Int(new_trunc_bits);
                let lhs = self.get_zero_extend_expr(cx, ops[0], ty, 0);
                let truncated = self.get_truncate_expr(cx, Use::of(trunc_rhs), new_trunc_ty, 0);
                let rhs = self.get_zero_extend_expr(cx, Use::of(truncated), ty, 0);
                return self
                    .get_mul_expr(cx, vec![Use::of(lhs), Use::of(rhs)], Flags::expr(flags::NUW), depth + 1)
                    .scev;
            }
        }

        // zext(umin(x, y)) -> umin(zext(x), zext(y))
        // zext(umax(x, y)) -> umax(zext(x), zext(y))
        if matches!(self.node(op.scev).kind, Kind::UMin(_) | Kind::UMax(_)) {
            let mut operands: Vec<Use> = Vec::new();
            for operand in self.operands(op.scev).to_vec() {
                operands.push(Use::of(self.get_zero_extend_expr(cx, operand, ty, 0)));
            }
            if matches!(self.node(op.scev).kind, Kind::UMin(_)) {
                return self.get_umin_expr(cx, operands, false);
            }
            return self.get_umax_expr(cx, operands);
        }

        // zext(umin_seq(x, y)) -> umin_seq(zext(x), zext(y))
        if matches!(self.node(op.scev).kind, Kind::SequentialUMin(_)) {
            let mut operands: Vec<Use> = Vec::new();
            for operand in self.operands(op.scev).to_vec() {
                operands.push(Use::of(self.get_zero_extend_expr(cx, operand, ty, 0)));
            }
            return self.get_umin_expr(cx, operands, true);
        }

        // The cast wasn't folded; create an explicit cast node. Recompute the
        // insert position, as it may have been invalidated.
        self.cast_node(Type::ZeroExtend, op, ty)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1883
    /// `ScalarEvolution::getSignExtendExpr`.
    pub fn get_sign_extend_expr(
        &mut self,
        cx: &Cx,
        op: Use,
        ty: Ty,
        depth: u32,
    ) -> Scev {
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(op.scev)) < self.get_type_size_in_bits(cx, ty),
            "This is not an extending conversion!"
        );
        debug_assert!(!self.type_of(op.scev).is_pointer(), "Can't extend pointer!");
        let ty = self.get_effective_scev_type(cx, ty);

        let id = (Type::SignExtend, op, ty);
        if let Some(&found) = self.fold_cache.get(&id) {
            return found;
        }

        let s = self.get_sign_extend_expr_impl(cx, op, ty, depth);
        if !matches!(self.node(s).kind, Kind::SignExtend(_)) {
            self.insert_fold_cache_entry(id, s);
        }
        s
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1902
    /// `ScalarEvolution::getSignExtendExprImpl`.
    pub fn get_sign_extend_expr_impl(
        &mut self,
        cx: &Cx,
        op: Use,
        ty: Ty,
        depth: u32,
    ) -> Scev {
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(op.scev)) < self.get_type_size_in_bits(cx, ty),
            "This is not an extending conversion!"
        );
        debug_assert!(!self.type_of(op.scev).is_pointer(), "Can't extend pointer!");

        // Fold if the operand is constant.
        if let Kind::Constant(value) = self.node(op.scev).kind {
            return self.get_constant(value.sext(self.get_type_size_in_bits(cx, ty)));
        }

        // sext(sext(x)) --> sext(x)
        if let Kind::SignExtend(inner) = self.node(op.scev).kind {
            return self.get_sign_extend_expr(cx, inner, ty, depth + 1);
        }

        // sext(zext(x)) --> zext(x)
        if let Kind::ZeroExtend(inner) = self.node(op.scev).kind {
            return self.get_zero_extend_expr(cx, inner, ty, depth + 1);
        }

        // If the operand is an affine AddRec with the no-signed-wrap flag, the
        // sign-extension distributes over the recurrence.
        let affine = self.match_affine_add_rec(op.scev);
        if depth <= MAX_CAST_DEPTH
            && let Some((_, step, l)) = affine
        {
            let ar = op.scev;
            if self.get_no_wrap_flags(ar, flags::NSW) != 0 {
                let start = self.get_extend_add_rec_start(cx, Extend::Sign, ar, ty, depth + 1);
                let step = self.get_sign_extend_expr(cx, Use::of(step), ty, depth + 1);
                let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                return self.get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags)).scev;
            }
        }

        // Before doing any expensive analysis, check to see if we've already
        // computed a SCEV for this Op and Ty.
        if let Some(found) = self.lookup_cast(Type::SignExtend, op, ty) {
            return found;
        }
        // Limit recursion depth.
        if depth > MAX_CAST_DEPTH {
            return self.cast_node(Type::SignExtend, op, ty);
        }

        // sext(trunc(x)) --> sext(x) or x or trunc(x)
        if let Kind::Truncate(trunc_operand) = self.node(op.scev).kind {
            // It's possible the bits taken off by the truncate were all sign
            // bits. If so, we should be able to simplify this further.
            let x = trunc_operand.scev;
            let cr: ConstantRange = self.get_signed_range(cx, x);
            let trunc_bits = self.get_type_size_in_bits(cx, self.type_of(op.scev));
            let new_bits = self.get_type_size_in_bits(cx, ty);
            if cr.truncate(trunc_bits, 0).sign_extend(new_bits).contains_range(&cr.sext_or_trunc(new_bits)) {
                return self.get_truncate_or_sign_extend(cx, x, ty, depth);
            }
        }

        if matches!(self.node(op.scev).kind, Kind::Add(_)) {
            let sa = op.scev;
            // sext((A + B + ...)<nsw>) --> (sext(A) + sext(B) + ...)<nsw>
            if self.get_no_wrap_flags(sa, flags::NSW) != 0 {
                // If the addition does not sign overflow then we can, by
                // definition, commute the sign extension with the addition
                // operation.
                let mut ops: Vec<Use> = Vec::new();
                for operand in self.operands(sa).to_vec() {
                    ops.push(Use::of(self.get_sign_extend_expr(cx, operand, ty, depth + 1)));
                }
                return self.get_add_expr(cx, ops, Flags::expr(flags::NSW), depth + 1).scev;
            }

            // sext(C + x + y + ...) --> (sext(D) + sext((C - D) + x + y + ...))
            // if D + (C - D + x + y + ...) could be proven to not signed wrap
            // where D maximizes the number of trailing zeros of (C - D + x + y
            // + ...)
            //
            // For instance, this will bring two seemingly different
            // expressions:
            //     1 + sext(5 + 20 * %x + 24 * %y)  and
            //         sext(6 + 20 * %x + 24 * %y)
            // to the same form:
            //     2 + sext(4 + 20 * %x + 24 * %y)
            let first = self.operands(sa)[0].scev;
            if let Some(c) = self.as_constant(first) {
                let d = self.extract_constant_without_wrapping_add(cx, c, sa);
                if !d.is_zero() {
                    let d_const = self.get_constant(d);
                    let ssext_d = self.get_sign_extend_expr(cx, Use::of(d_const), ty, depth);
                    let neg_d = self.get_constant(d.neg());
                    let sresidual =
                        self.get_add_expr(cx, vec![Use::of(neg_d), Use::of(sa)], Flags::default(), depth).scev;
                    let ssext_r = self.get_sign_extend_expr(cx, Use::of(sresidual), ty, depth + 1);
                    return self
                        .get_add_expr(
                            cx,
                            vec![Use::of(ssext_d), Use::of(ssext_r)],
                            Flags::expr(flags::NSW | flags::NUW),
                            depth + 1,
                        )
                        .scev;
                }
            }
        }
        // If the input value is a chrec scev, and we can prove that the value
        // did not overflow the old, smaller, value, we can sign extend all of
        // the operands (often constants). This allows analysis of something
        // like this: for (signed char X = 0; X < 100; ++X) { int Y = X; }
        if let Some((start, step, l)) = affine {
            let ar = op.scev;
            let bit_width = self.get_type_size_in_bits(cx, self.type_of(ar));

            // The no-signed-wrap case is handled before the uniquing lookup
            // above.

            // Check whether the backedge-taken count is SCEVCouldNotCompute.
            // (See the comment in getZeroExtendExprImpl.)
            let max_be_count = self.get_backedge_taken_count(cx, l, ExitCountKind::ConstantMaximum);
            if !matches!(self.node(max_be_count).kind, Kind::CouldNotCompute) {
                // Manually compute the final value for AR, checking for
                // overflow.

                // Check whether the backedge-taken count can be losslessly
                // casted to the addrec's type. The count is always unsigned.
                let casted_max_be_count =
                    self.get_truncate_or_zero_extend(cx, max_be_count, self.type_of(start), depth);
                let recasted_max_be_count =
                    self.get_truncate_or_zero_extend(cx, casted_max_be_count, self.type_of(max_be_count), depth);
                if max_be_count == recasted_max_be_count {
                    let wide_ty = Ty::Int(bit_width * 2);
                    // Check whether Start+Step*MaxBECount has no signed
                    // overflow.
                    let smul = self
                        .get_mul_expr(
                            cx,
                            vec![Use::of(casted_max_be_count), Use::of(step)],
                            Flags::default(),
                            depth + 1,
                        )
                        .scev;
                    let sum =
                        self.get_add_expr(cx, vec![Use::of(start), Use::of(smul)], Flags::default(), depth + 1).scev;
                    let sadd = self.get_sign_extend_expr(cx, Use::of(sum), wide_ty, depth + 1);
                    let wide_start = self.get_sign_extend_expr(cx, Use::of(start), wide_ty, depth + 1);
                    let wide_max_be_count =
                        self.get_zero_extend_expr(cx, Use::of(casted_max_be_count), wide_ty, depth + 1);
                    let wide_step = self.get_sign_extend_expr(cx, Use::of(step), wide_ty, depth + 1);
                    let product = self
                        .get_mul_expr(
                            cx,
                            vec![Use::of(wide_max_be_count), Use::of(wide_step)],
                            Flags::default(),
                            depth + 1,
                        )
                        .scev;
                    let operand_extended_add = self
                        .get_add_expr(cx, vec![Use::of(wide_start), Use::of(product)], Flags::default(), depth + 1)
                        .scev;
                    if sadd == operand_extended_add {
                        // Cache knowledge of AR NSW, which is propagated to
                        // this AddRec.
                        self.set_no_wrap_flags(ar, flags::NSW);
                        // Return the expression with the addrec on the outside.
                        let start = self.get_extend_add_rec_start(cx, Extend::Sign, ar, ty, depth + 1);
                        let step = self.get_sign_extend_expr(cx, Use::of(step), ty, depth + 1);
                        let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                        return self
                            .get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags))
                            .scev;
                    }
                    // Similar to above, only this time treat the step value as
                    // unsigned. This covers loops that count up with an
                    // unsigned step.
                    let wide_step = self.get_zero_extend_expr(cx, Use::of(step), wide_ty, depth + 1);
                    let product = self
                        .get_mul_expr(
                            cx,
                            vec![Use::of(wide_max_be_count), Use::of(wide_step)],
                            Flags::default(),
                            depth + 1,
                        )
                        .scev;
                    let operand_extended_add = self
                        .get_add_expr(cx, vec![Use::of(wide_start), Use::of(product)], Flags::default(), depth + 1)
                        .scev;
                    if sadd == operand_extended_add {
                        // If AR wraps around then
                        //
                        //    abs(Step) * MaxBECount >
                        // unsigned-max(AR->getType())
                        // => SAdd != OperandExtendedAdd
                        //
                        // Thus (AR is not NW => SAdd != OperandExtendedAdd) <=>
                        // (SAdd == OperandExtendedAdd => AR is NW)

                        self.set_no_wrap_flags(ar, flags::NW);

                        // Return the expression with the addrec on the outside.
                        let start = self.get_extend_add_rec_start(cx, Extend::Sign, ar, ty, depth + 1);
                        let step = self.get_zero_extend_expr(cx, Use::of(step), ty, depth + 1);
                        let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                        return self
                            .get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags))
                            .scev;
                    }
                }
            }

            let new_flags = self.prove_no_signed_wrap_via_induction(cx, ar);
            self.set_no_wrap_flags(ar, new_flags);
            if self.get_no_wrap_flags(ar, flags::NSW) != 0 {
                // Same as nsw case above - duplicated here to avoid a compile
                // time issue. It's not clear that the order of checks does
                // matter, but it's one of two issue possible causes for a
                // change which was reverted. Be conservative for the moment.
                let start = self.get_extend_add_rec_start(cx, Extend::Sign, ar, ty, depth + 1);
                let step = self.get_sign_extend_expr(cx, Use::of(step), ty, depth + 1);
                let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                return self.get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags)).scev;
            }

            // sext({C,+,Step}) --> (sext(D) + sext({C-D,+,Step}))<nuw><nsw>
            // if D + (C - D + Step * n) could be proven to not signed wrap
            // where D maximizes the number of trailing zeros of (C - D + Step *
            // n)
            if let Some(c) = self.as_constant(start) {
                let d = self.extract_constant_without_wrapping_addrec(cx, c, step);
                if !d.is_zero() {
                    let d_const = self.get_constant(d);
                    let ssext_d = self.get_sign_extend_expr(cx, Use::of(d_const), ty, depth);
                    let residual_start = self.get_constant(c.sub(d));
                    let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                    let sresidual = self
                        .get_add_rec_expr(cx, vec![Use::of(residual_start), Use::of(step)], l, Flags::expr(ar_flags))
                        .scev;
                    let ssext_r = self.get_sign_extend_expr(cx, Use::of(sresidual), ty, depth + 1);
                    return self
                        .get_add_expr(
                            cx,
                            vec![Use::of(ssext_d), Use::of(ssext_r)],
                            Flags::expr(flags::NSW | flags::NUW),
                            depth + 1,
                        )
                        .scev;
                }
            }

            if self.prove_no_wrap_by_varying_start(cx, Extend::Sign, start, step, l) {
                self.set_no_wrap_flags(ar, flags::NSW);
                let start = self.get_extend_add_rec_start(cx, Extend::Sign, ar, ty, depth + 1);
                let step = self.get_sign_extend_expr(cx, Use::of(step), ty, depth + 1);
                let ar_flags = self.get_no_wrap_flags(ar, flags::MASK);
                return self.get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], l, Flags::expr(ar_flags)).scev;
            }
        }

        // If the input value is provably positive and we could not simplify
        // away the sext build a zext instead.
        if self.is_known_non_negative(cx, op.scev) {
            return self.get_zero_extend_expr(cx, op, ty, depth + 1);
        }

        // sext(smin(x, y)) -> smin(sext(x), sext(y))
        // sext(smax(x, y)) -> smax(sext(x), sext(y))
        if matches!(self.node(op.scev).kind, Kind::SMin(_) | Kind::SMax(_)) {
            let mut operands: Vec<Use> = Vec::new();
            for operand in self.operands(op.scev).to_vec() {
                operands.push(Use::of(self.get_sign_extend_expr(cx, operand, ty, 0)));
            }
            if matches!(self.node(op.scev).kind, Kind::SMin(_)) {
                return self.get_smin_expr(cx, operands);
            }
            return self.get_smax_expr(cx, operands);
        }

        // The cast wasn't folded; create an explicit cast node. Recompute the
        // insert position, as it may have been invalidated.
        self.cast_node(Type::SignExtend, op, ty)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2148
    /// `ScalarEvolution::getCastExpr`.
    pub fn get_cast_expr(
        &mut self,
        cx: &Cx,
        kind: Type,
        op: Use,
        ty: Ty,
    ) -> Scev {
        match kind {
            Type::Truncate => self.get_truncate_expr(cx, op, ty, 0),
            Type::ZeroExtend => self.get_zero_extend_expr(cx, op, ty, 0),
            Type::SignExtend => self.get_sign_extend_expr(cx, op, ty, 0),
            Type::PtrToAddr => {
                let expr = self.get_ptr_to_addr_expr(cx, op.scev);
                debug_assert!(self.type_of(expr) == ty, "requested type must match");
                expr
            }
            _ => unreachable!("Not a SCEV cast expression!"),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2168
    /// `getAnyExtendExpr`: the operand extended with unspecified bits out to
    /// the given type.
    pub fn get_any_extend_expr(
        &mut self,
        cx: &Cx,
        op: Use,
        ty: Ty,
    ) -> Scev {
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(op.scev)) < self.get_type_size_in_bits(cx, ty),
            "This is not an extending conversion!"
        );
        let ty = self.get_effective_scev_type(cx, ty);

        // Sign-extend negative constants.
        if let Kind::Constant(value) = self.node(op.scev).kind
            && value.is_negative()
        {
            return self.get_sign_extend_expr(cx, op, ty, 0);
        }

        // Peel off a truncate cast.
        if let Kind::Truncate(new_op) = self.node(op.scev).kind {
            if self.get_type_size_in_bits(cx, self.type_of(new_op.scev)) < self.get_type_size_in_bits(cx, ty) {
                return self.get_any_extend_expr(cx, new_op, ty);
            }
            return self.get_truncate_or_noop(cx, new_op.scev, ty);
        }

        // Next try a zext cast. If the cast is folded, use it.
        let zext = self.get_zero_extend_expr(cx, op, ty, 0);
        if !matches!(self.node(zext).kind, Kind::ZeroExtend(_)) {
            return zext;
        }

        // Next try a sext cast. If the cast is folded, use it.
        let sext = self.get_sign_extend_expr(cx, op, ty, 0);
        if !matches!(self.node(sext).kind, Kind::SignExtend(_)) {
            return sext;
        }

        // Force the cast to be folded into the operands of an addrec.
        if let Kind::AddRec(_, header) = self.node(op.scev).kind {
            let mut ops: Vec<Use> = Vec::new();
            for operand in self.operands(op.scev).to_vec() {
                ops.push(Use::of(self.get_any_extend_expr(cx, operand, ty)));
            }
            return self.get_add_rec_expr(cx, ops, header, Flags::expr(flags::NW)).scev;
        }

        // If the expression is obviously signed, use the sext cast value.
        if matches!(self.node(op.scev).kind, Kind::SMax(_)) {
            return sext;
        }

        // Absent any other information, use the zext cast value.
        zext
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4712
    /// `getTruncateOrZeroExtend`.
    pub fn get_truncate_or_zero_extend(
        &mut self,
        cx: &Cx,
        v: Scev,
        ty: Ty,
        depth: u32,
    ) -> Scev {
        let src_ty = self.type_of(v);
        let (src, dst) = (self.get_type_size_in_bits(cx, src_ty), self.get_type_size_in_bits(cx, ty));
        if src == dst {
            return v; // No conversion
        }
        if src > dst {
            return self.get_truncate_expr(cx, Use::of(v), ty, depth);
        }
        self.get_zero_extend_expr(cx, Use::of(v), ty, depth)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4724
    /// `getTruncateOrSignExtend`.
    pub fn get_truncate_or_sign_extend(
        &mut self,
        cx: &Cx,
        v: Scev,
        ty: Ty,
        depth: u32,
    ) -> Scev {
        let src_ty = self.type_of(v);
        let (src, dst) = (self.get_type_size_in_bits(cx, src_ty), self.get_type_size_in_bits(cx, ty));
        if src == dst {
            return v; // No conversion
        }
        if src > dst {
            return self.get_truncate_expr(cx, Use::of(v), ty, depth);
        }
        self.get_sign_extend_expr(cx, Use::of(v), ty, depth)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4736
    /// `getNoopOrZeroExtend`.
    pub fn get_noop_or_zero_extend(
        &mut self,
        cx: &Cx,
        v: Scev,
        ty: Ty,
    ) -> Scev {
        let (src, dst) = (self.get_type_size_in_bits(cx, self.type_of(v)), self.get_type_size_in_bits(cx, ty));
        debug_assert!(src <= dst, "getNoopOrZeroExtend cannot truncate!");
        if src == dst {
            return v; // No conversion
        }
        self.get_zero_extend_expr(cx, Use::of(v), ty, 0)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4747
    /// `getNoopOrSignExtend`.
    pub fn get_noop_or_sign_extend(
        &mut self,
        cx: &Cx,
        v: Scev,
        ty: Ty,
    ) -> Scev {
        let (src, dst) = (self.get_type_size_in_bits(cx, self.type_of(v)), self.get_type_size_in_bits(cx, ty));
        debug_assert!(src <= dst, "getNoopOrSignExtend cannot truncate!");
        if src == dst {
            return v; // No conversion
        }
        self.get_sign_extend_expr(cx, Use::of(v), ty, 0)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4758
    /// `getNoopOrAnyExtend`.
    pub fn get_noop_or_any_extend(
        &mut self,
        cx: &Cx,
        v: Scev,
        ty: Ty,
    ) -> Scev {
        let (src, dst) = (self.get_type_size_in_bits(cx, self.type_of(v)), self.get_type_size_in_bits(cx, ty));
        debug_assert!(src <= dst, "getNoopOrAnyExtend cannot truncate!");
        if src == dst {
            return v; // No conversion
        }
        self.get_any_extend_expr(cx, Use::of(v), ty)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4769
    /// `getTruncateOrNoop`.
    pub fn get_truncate_or_noop(
        &mut self,
        cx: &Cx,
        v: Scev,
        ty: Ty,
    ) -> Scev {
        let (src, dst) = (self.get_type_size_in_bits(cx, self.type_of(v)), self.get_type_size_in_bits(cx, ty));
        debug_assert!(src >= dst, "getTruncateOrNoop cannot extend!");
        if src == dst {
            return v; // No conversion
        }
        self.get_truncate_expr(cx, Use::of(v), ty, 0)
    }
}

/// `C->abs()`.
fn abs_ap(c: Ap) -> Ap {
    if c.is_negative() { c.neg() } else { c }
}
