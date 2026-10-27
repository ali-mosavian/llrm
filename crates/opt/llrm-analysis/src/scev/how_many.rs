//! How many times a comparison holds: `howFarToZero`, `howFarToNonZero`,
//! `howManyLessThans`, `howManyGreaterThans`, and what they solve with.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ScalarEvolution.cpp`. The exit
//! condition `X != Y` is `X - Y != 0`, solved as a linear equation modulo
//! `2^BW` (`SolveLinEquationWithOverflow`); `X < Y` of an add recurrence is
//! `ceil((max(Y, Start) - Start) / Stride)`, with the cases where the
//! recurrence may wrap proved away or guarded by the loop's entry condition.
//! Also here: `BinomialCoefficient` and `SCEVAddRecExpr::evaluateAtIteration`,
//! `getNumIterationsInRange`, `willNotOverflow` and the range helpers they
//! share.
//!
//! # Deviations
//! - Predicates (DEVIATE 1 of `docs/architecture/scev-port.md`): no
//!   `AllowPredicates`, so `SolveLinEquationWithOverflow` gives up where it
//!   would add a predicate.
//! - D-T3 (temporary,
//!   [#2327](https://github.com/ali-mosavian/llrm/issues/2327)):
//!   `BinomialCoefficient` computes at `W + T` bits (`T` the factors of two in
//!   `K!`, plus one), and `Ap` holds 128: past that the coefficient is
//!   CouldNotCompute.
//! - Quadratic recurrences (`quadratic.rs`, DEVIATE Q1): coefficients up to 41
//!   bits; wider, the count is CouldNotCompute, as LLVM's is where its solver
//!   finds no root.
//!
//! # Example
//! `{0,+,1} < %n` is taken `(-1 + %n)` times.

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::module::BlockId;
use llrm_mir::opcode::{BinaryOp, IntPredicate};

use super::ap::Ap;
use super::expr::{Kind, Scev, Ty, Use, flags};
use super::trip::ExitLimit;
use super::{Cx, Flags, ScalarEvolution};

/// `APInt::multiplicativeInverse`: the inverse of an odd number modulo `2^w`,
/// by Newton's method.
fn multiplicative_inverse(a: Ap) -> Ap {
    debug_assert!(a.bits & 1 == 1, "multiplicative inverse is only defined for odd numbers!");
    let two = Ap::new(a.width, 2);
    let mut factor = a;
    loop {
        let t = a.mul(factor);
        if t.is_one() {
            return factor;
        }
        factor = factor.mul(two.sub(t));
    }
}

/// `APInt::getOneBitSet`.
fn one_bit_set(
    width: u32,
    bit: u32,
) -> Ap {
    Ap::new(width, 1u128 << bit)
}

impl ScalarEvolution {
    /// `s` is an add recurrence of the loop headed by `header`.
    pub fn is_add_rec_of(
        &self,
        s: Scev,
        header: BlockId,
    ) -> bool {
        self.addrec_header(s) == Some(header)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L2531
    /// `getRangeMax(S, IsSigned, Invert)`.
    pub fn get_range_max(
        &mut self,
        cx: &Cx,
        s: Scev,
        is_signed: bool,
        invert: bool,
    ) -> Ap {
        if invert {
            return self.get_range_min(cx, s, is_signed, false).not();
        }
        if is_signed { self.get_signed_range_max(cx, s) } else { self.get_unsigned_range_max(cx, s) }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L2524
    /// `getRangeMin(S, IsSigned, Invert)`.
    pub fn get_range_min(
        &mut self,
        cx: &Cx,
        s: Scev,
        is_signed: bool,
        invert: bool,
    ) -> Ap {
        if invert {
            return self.get_range_max(cx, s, is_signed, false).not();
        }
        if is_signed { self.get_signed_range_min(cx, s) } else { self.get_unsigned_range_min(cx, s) }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10413
    /// `stripInjectiveFunctions`.
    fn strip_injective_functions(
        &self,
        s: Scev,
    ) -> Scev {
        match self.node(s).kind {
            Kind::ZeroExtend(operand) | Kind::SignExtend(operand) => self.strip_injective_functions(operand.scev),
            _ => s,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13322
    /// `getUDivCeilSCEV`.
    pub fn get_udiv_ceil_scev(
        &mut self,
        cx: &Cx,
        n: Scev,
        d: Scev,
    ) -> Scev {
        // umin(N, 1) + floor((N - umin(N, 1)) / D)
        // This is equivalent to "1 + floor((N - 1) / D)" for N != 0. The umin
        // expression fixes the case of N=0.
        let one = self.get_one(cx, self.type_of(n));
        let min_n_one = self.get_umin_expr(cx, vec![Use::of(n), Use::of(one)], false);
        let n_minus_one = self.get_minus_scev(cx, Use::of(n), Use::of(min_n_one), 0, 0);
        let quotient = self.get_udiv_expr(cx, Use::of(n_minus_one), Use::of(d));
        self.get_add_expr(cx, vec![Use::of(min_n_one), Use::of(quotient)], Flags::default(), 0).scev
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13298
    /// `canIVOverflowOnLT`.
    fn can_iv_overflow_on_lt(
        &mut self,
        cx: &Cx,
        rhs: Scev,
        stride: Scev,
        is_signed: bool,
        invert: bool,
    ) -> bool {
        debug_assert!(self.is_known_positive(cx, stride), "Positive stride expected!");

        let bit_width = self.get_type_size_in_bits(cx, self.type_of(rhs));
        let one = self.get_one(cx, self.type_of(stride));
        let stride_minus_one = self.get_minus_scev(cx, Use::of(stride), Use::of(one), 0, 0);

        if is_signed {
            let max_rhs = self.get_range_max(cx, rhs, true, invert);
            let max_value = Ap::signed_max(bit_width);
            let max_stride_minus_one = self.get_signed_range_max(cx, stride_minus_one);

            // SMaxRHS + SMaxStrideMinusOne > SMaxValue => overflow!
            return max_value.sub(max_stride_minus_one).slt(max_rhs);
        }

        let max_rhs = self.get_range_max(cx, rhs, false, invert);
        let max_value = Ap::all_ones(bit_width);
        let max_stride_minus_one = self.get_unsigned_range_max(cx, stride_minus_one);

        // UMaxRHS + UMaxStrideMinusOne > UMaxValue => overflow!
        max_value.sub(max_stride_minus_one).ult(max_rhs)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13332
    /// `computeMaxBECountForLT`.
    fn compute_max_be_count_for_lt(
        &mut self,
        cx: &Cx,
        start: Scev,
        stride: Scev,
        end: Scev,
        bit_width: u32,
        is_signed: bool,
        invert: bool,
    ) -> Scev {
        // The logic in this function assumes we can represent a positive
        // stride. If we can't, the backedge-taken count must be zero.
        if is_signed && bit_width == 1 {
            return self.get_zero(cx, self.type_of(stride));
        }

        // This code below only been closely audited for negative strides in
        // the unsigned comparison case, it may be correct for signed
        // comparison, but that needs to be established.
        if is_signed && self.is_known_negative(cx, stride) {
            return self.get_could_not_compute();
        }

        // Calculate the maximum backedge count based on the range of values
        // permitted by Start, End, and Stride. If Invert is true, both Start
        // and End need inverting. Stride was already negated by the caller.
        let min_start = self.get_range_min(cx, start, is_signed, invert);

        let min_stride =
            if is_signed { self.get_signed_range_min(cx, stride) } else { self.get_unsigned_range_min(cx, stride) };

        // We assume either the stride is positive, or the backedge-taken
        // count is zero. So force StrideForMaxBECount to be at least one.
        let one = Ap::new(bit_width, 1);
        let stride_for_max_be_count = if is_signed { one.smax(min_stride) } else { one.umax(min_stride) };

        let max_value = if is_signed { Ap::signed_max(bit_width) } else { Ap::all_ones(bit_width) };
        let limit = max_value.sub(stride_for_max_be_count.sub(one));

        // Although End can be a MAX expression we estimate MaxEnd considering
        // only the case End = RHS of the loop termination condition. This is
        // safe because in the other case (End - Start) is zero, leading to a
        // zero maximum backedge taken count.
        let mut max_end = self.get_range_max(cx, end, is_signed, invert);
        max_end = if is_signed { max_end.smin(limit) } else { max_end.umin(limit) };

        // MaxBECount = ceil((max(MaxEnd, MinStart) - MinStart) / Stride)
        max_end = if is_signed { max_end.smax(min_start) } else { max_end.umax(min_start) };

        let mut delta = max_end.sub(min_start);

        // Try to refine Delta in case End - Start (or Start - End if Invert)
        // gives a tighter bound after folding.
        let delta_expr = if invert {
            self.get_minus_scev(cx, Use::of(start), Use::of(end), 0, 0)
        } else {
            self.get_minus_scev(cx, Use::of(end), Use::of(start), 0, 0)
        };
        delta = delta.umin(self.get_unsigned_range_max(cx, delta_expr));

        let numerator = self.get_constant(delta);
        let denominator = self.get_constant(stride_for_max_be_count);
        self.get_udiv_ceil_scev(cx, numerator, denominator)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10434
    /// `SolveLinEquationWithOverflow`: the minimum unsigned root of
    /// `A * X = B (mod 2^BW)`, or CouldNotCompute if there is none.
    fn solve_lin_equation_with_overflow(
        &mut self,
        cx: &Cx,
        a: Ap,
        b: Scev,
        header: BlockId,
        predicates: Option<&mut Vec<super::wrap_predicate::Predicate>>,
    ) -> Scev {
        let bw = a.width;
        debug_assert!(bw == self.get_type_size_in_bits(cx, self.type_of(b)));
        debug_assert!(!a.is_zero(), "A must be non-zero.");

        // 1. D = gcd(A, N)
        //
        // The gcd of A and N may have only one prime factor: 2. The number of
        // trailing zeros in A is its multiplicity.
        let mult2 = a.countr_zero();
        // D = 2^Mult2

        // 2. Check if B is divisible by D.
        //
        // B is divisible by D if and only if the multiplicity of prime factor
        // 2 for B is not less than multiplicity of this prime factor for D.
        let mut min_tz = self.get_min_trailing_zeros(cx, b, None);
        // Try again with the terminator of the loop predecessor for
        // context-specific result, if MinTZ s too small.
        if min_tz < mult2
            && let Some(terminator) = cx
                .loops
                .get_loop_for(header)
                .and_then(|one| one.get_loop_predecessor(cx.function))
                .and_then(|predecessor| cx.function.terminator(predecessor))
        {
            min_tz = self.get_min_trailing_zeros(cx, b, Some(terminator));
        }
        if min_tz < mult2 {
            // Check if we can prove there's no remainder using URem.
            let divisor = self.get_constant(one_bit_set(bw, mult2));
            let urem = self.get_urem_expr(cx, Use::of(b), Use::of(divisor));
            let zero = self.get_zero(cx, self.type_of(b));
            if !self.is_known_predicate_scev(cx, IntPredicate::Eq, urem, zero) {
                // Try to add a predicate ensuring B is a multiple of
                // 1 << Mult2.
                let Some(predicates) = predicates else { return self.get_could_not_compute() };
                // Avoid adding a predicate that is known to be false.
                if self.is_known_predicate_scev(cx, IntPredicate::Ne, urem, zero) {
                    return self.get_could_not_compute();
                }
                predicates.push(self.get_equal_predicate(urem, zero));
            }
        }

        // 3. Compute I: the multiplicative inverse of (A / D) in arithmetic
        // modulo (N / D).
        //
        // If D == 1, (N / D) == N == 2^BW, so we need one extra bit to
        // represent (N / D) in general. The inverse itself always fits into BW
        // bits, though, so we immediately truncate it.
        let ad = a.lshr(mult2).trunc(bw - mult2); // AD = A / D
        let inverse = multiplicative_inverse(ad).zext(bw);

        // 4. Compute the minimum unsigned root of the equation:
        // I * (B / D) mod (N / D)
        // To simplify the computation, we factor out the divide by D:
        // (I * B mod N) / D
        let d = self.get_constant(one_bit_set(bw, mult2));
        let i = self.get_constant(inverse);
        let product = self.get_mul_expr(cx, vec![Use::of(b), Use::of(i)], Flags::default(), 0).scev;
        self.get_udiv_exact_expr(cx, Use::of(product), Use::of(d))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10764
    /// `howFarToZero`: the number of times the backedge of the loop is taken
    /// while `V != 0`, `V` being an expression of the loop.
    pub(super) fn how_far_to_zero(
        &mut self,
        cx: &Cx,
        v: Scev,
        header: BlockId,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        // This is only used for loops with a "x != y" exit test. The exit
        // condition is now expressed as a single expression, V = x-y. So the
        // exit test is effectively V != 0. We know and take advantage of the
        // fact that this expression only being used in a comparison by zero
        // context.

        // If the value is a constant
        if let Some(c) = self.match_apint(v) {
            // If the value is already zero, the branch will execute zero
            // times.
            if c.is_zero() {
                return ExitLimit::exact(self, v);
            }
            // Otherwise it will loop infinitely.
            return ExitLimit::could_not_compute(self);
        }

        let mut predicates: Vec<super::wrap_predicate::Predicate> = Vec::new();
        let mut add_rec = self.strip_injective_functions(v);
        if !self.is_add_rec_of(add_rec, header) && allow_predicates {
            // Try to make this an AddRec using runtime tests, in the first X
            // iterations of this loop, where X is the SCEV expression found by
            // the algorithm below.
            if let Some(converted) = self.convert_scev_to_add_rec_with_predicates(cx, v, header, &mut predicates) {
                add_rec = converted;
            }
        }
        if !self.is_add_rec_of(add_rec, header) {
            return ExitLimit::could_not_compute(self);
        }

        // If this is a quadratic (3-term) AddRec {L,+,M,+,N}, find the roots
        // of the quadratic equation to solve it.
        if self.is_quadratic(add_rec) && self.type_of(add_rec).is_integer() {
            // We can only use this value if the chrec ends up with an exact
            // zero value at this index. When solving for "X*X != 5", for
            // example, we should not accept a root of 2.
            if let Some((root, width)) = self.solve_quadratic_add_rec_exact(cx, add_rec) {
                let r = self.get_constant(Ap::new(width, root as u128));
                return ExitLimit::new(self, r, r, r, false);
            }
            return ExitLimit::could_not_compute(self);
        }

        // Otherwise we can only handle this if it is affine.
        if !self.is_affine(add_rec) {
            return ExitLimit::could_not_compute(self);
        }

        // If this is an affine expression, the execution count of this branch
        // is the minimum unsigned root of the following equation:
        //
        //     Start + Step*N = 0 (mod 2^BW)
        //
        // equivalent to:
        //
        //             Step*N = -Start (mod 2^BW)
        //
        // where BW is the common bit width of Start and Step.

        // Get the initial value for the loop.
        let parent = cx.loops.get_loop_for(header).and_then(|one| one.get_parent_loop()).map(|one| one.get_header());
        let start = self.addrec_start(add_rec);
        let start = self.get_scev_at_scope(cx, start, parent);
        let step = self.operands(add_rec)[1].scev;
        let step = self.get_scev_at_scope(cx, step, parent);

        if !self.is_loop_invariant(cx, step, Some(header)) {
            return ExitLimit::could_not_compute(self);
        }

        // Specialize step for this loop so we get context sensitive facts
        // below.
        let guards = super::guards::LoopGuards::collect(self, cx, header);
        let step_wlg = self.apply_loop_guards_with(cx, step, &guards);

        // For positive steps (counting up until unsigned overflow):
        //   N = -Start/Step (as unsigned)
        // For negative steps (counting down to zero):
        //   N = Start/-Step
        // First compute the unsigned distance from zero in the direction of
        // Step.
        let count_down = self.is_known_negative(cx, step_wlg);
        if !count_down && !self.is_known_non_negative(cx, step_wlg) {
            return ExitLimit::could_not_compute(self);
        }

        let distance = if count_down { start } else { self.get_negative_scev(cx, start, 0) };
        // Handle unitary steps, which cannot wraparound.
        // 1*N = -Start; -1*N = Start (mod 2^BW), so:
        //   N = Distance (as unsigned)

        if self.match_one(step) || self.match_all_ones(step) {
            let guarded_distance = self.apply_loop_guards_with(cx, distance, &guards);
            let mut max_be_count = self.get_unsigned_range_max(cx, guarded_distance);
            max_be_count = max_be_count.umin(self.get_unsigned_range_max(cx, distance));

            // When a loop like "for (int i = 0; i != n; ++i) { /* body */ }" is
            // rotated, we end up with a loop whose backedge-taken count is
            // n - 1. Detect this case, and see if we can improve the bound.
            //
            // Explicitly handling this here is necessary because
            // getUnsignedRange isn't context-sensitive; it doesn't know that
            // we only care about the range inside the loop.
            let zero = self.get_zero(cx, self.type_of(distance));
            let one = self.get_one(cx, self.type_of(distance));
            let distance_plus_one =
                self.get_add_expr(cx, vec![Use::of(distance), Use::of(one)], Flags::default(), 0).scev;
            if self.is_loop_entry_guarded_by_cond_pred(cx, header, IntPredicate::Ne, distance_plus_one, zero) {
                // If Distance + 1 doesn't overflow, we can compute the maximum
                // distance as "unsigned_max(Distance + 1) - 1". Also apply the
                // loop guards to Distance + 1; the range of Distance itself
                // may be a wrapped set even when the guards bound Distance + 1
                // tightly.
                let guarded = self.apply_loop_guards_with(cx, distance_plus_one, &guards);
                let max =
                    self.get_unsigned_range_max(cx, guarded).umin(self.get_unsigned_range_max(cx, distance_plus_one));
                max_be_count = max_be_count.umin(max.sub(Ap::one(max.width)));
            }
            let constant_max = self.get_constant(max_be_count);
            return ExitLimit::new(self, distance, constant_max, distance, false).with_predicates(predicates);
        }

        // If the condition controls loop exit (the loop exits only if the
        // expression is true) and the addition is no-wrap we can use unsigned
        // divide to compute the backedge count. In this case, the step may not
        // divide the distance, but we don't care because if the condition is
        // "missed" the loop will have undefined behavior due to wrapping.
        if controls_only_exit
            && self.get_no_wrap_flags(add_rec, flags::NW) != 0
            && self.loop_has_no_abnormal_exits(cx, header)
        {
            // If the stride is zero and the start is non-zero, the loop must be
            // infinite. In C++, most loops are finite by assumption, in which
            // case the step being zero implies UB must execute if the loop is
            // entered.
            if !(self.loop_is_finite_by_assumption(cx, header) && self.is_known_non_zero(cx, start))
                && !self.is_known_non_zero(cx, step_wlg)
            {
                return ExitLimit::could_not_compute(self);
            }

            let divisor = if count_down { self.get_negative_scev(cx, step, 0) } else { step };
            let exact = self.get_udiv_expr(cx, Use::of(distance), Use::of(divisor));
            let mut constant_max = self.get_could_not_compute();
            if exact != self.get_could_not_compute() {
                let guarded = self.apply_loop_guards_with(cx, exact, &guards);
                let max_int = self.get_unsigned_range_max(cx, guarded);
                let exact_max = self.get_unsigned_range_max(cx, exact);
                constant_max = self.get_constant(max_int.umin(exact_max));
            }
            let symbolic_max = if exact == self.get_could_not_compute() { constant_max } else { exact };
            return ExitLimit::new(self, exact, constant_max, symbolic_max, false).with_predicates(predicates);
        }

        // Solve the general equation.
        let Some(step_c) = self.match_apint(step) else { return ExitLimit::could_not_compute(self) };
        if step_c.is_zero() {
            return ExitLimit::could_not_compute(self);
        }
        let negative_start = self.get_negative_scev(cx, start, 0);
        let e = self.solve_lin_equation_with_overflow(
            cx,
            step_c,
            negative_start,
            header,
            allow_predicates.then_some(&mut predicates),
        );

        let mut m = e;
        if e != self.get_could_not_compute() {
            let guarded = self.apply_loop_guards_with(cx, e, &guards);
            let max_with_guards = self.get_unsigned_range_max(cx, guarded);
            let e_max = self.get_unsigned_range_max(cx, e);
            m = self.get_constant(max_with_guards.umin(e_max));
        }
        let s = if e == self.get_could_not_compute() { m } else { e };
        ExitLimit::new(self, e, m, s, false).with_predicates(predicates)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10921
    /// `howFarToNonZero`.
    pub(super) fn how_far_to_non_zero(
        &mut self,
        cx: &Cx,
        v: Scev,
        _header: BlockId,
    ) -> ExitLimit {
        // Loops that look like: while (X == 0) are very strange indeed. We
        // don't handle them yet except for the trivial case. This could be
        // expanded in the future as needed.

        // If the value is a constant, check to see if it is known to be
        // non-zero already. If so, the backedge will execute zero times.
        if let Some(c) = self.match_apint(v) {
            if !c.is_zero() {
                let zero = self.get_zero(cx, self.type_of(v));
                return ExitLimit::exact(self, zero);
            }
            // Otherwise it will loop infinitely.
            return ExitLimit::could_not_compute(self);
        }

        // We could implement others, but I really doubt anyone writes loops
        // like this, and if they did, they would already be constant folded.
        ExitLimit::could_not_compute(self)
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13388
    /// `howManyLessThans`: the number of times the backedge is taken while
    /// `LHS < RHS`, `LHS` being an add recurrence of the loop.
    pub(super) fn how_many_less_thans(
        &mut self,
        cx: &Cx,
        lhs: Scev,
        mut rhs: Scev,
        header: BlockId,
        is_signed: bool,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        let mut predicates: Vec<super::wrap_predicate::Predicate> = Vec::new();
        let mut predicated_iv = false;
        let mut iv = self.is_add_rec_of(lhs, header).then_some(lhs);
        if iv.is_none()
            && let Kind::ZeroExtend(zext) = self.node(lhs).kind
        {
            let ar = zext.scev;
            if self.is_add_rec_of(ar, header) && self.is_affine(ar) {
                let can_prove_nuw = |se: &mut ScalarEvolution| {
                    // We can use the comparison to infer no-wrap flags only if
                    // it fully controls the loop exit.
                    if !controls_only_exit {
                        return false;
                    }

                    if !se.is_loop_invariant(cx, rhs, Some(header)) {
                        return false;
                    }

                    let step = se.get_step_recurrence(cx, ar);
                    if !se.is_known_non_zero(cx, step) {
                        // We need the sequence defined by AR to strictly
                        // increase in the unsigned integer domain for the
                        // logic below to hold.
                        return false;
                    }

                    let inner_bit_width = se.get_type_size_in_bits(cx, se.type_of(ar));
                    let outer_bit_width = se.get_type_size_in_bits(cx, se.type_of(rhs));
                    // If RHS <=u Limit, then there must exist a value V in the
                    // sequence defined by AR (e.g. {Start,+,Step}) such that
                    // V >u RHS, and V <=u UINT_MAX. Thus, we must exit the
                    // loop before unsigned overflow occurs. This limit also
                    // implies that a signed comparison (in the wide bitwidth)
                    // is equivalent to an unsigned comparison as the high bits
                    // on both sides must be zero.
                    let stride_max = se.get_unsigned_range_max(cx, step);
                    let limit = Ap::all_ones(inner_bit_width).sub(stride_max.sub(Ap::one(inner_bit_width)));
                    let limit = limit.zext(outer_bit_width);
                    let guarded = se.apply_loop_guards(cx, rhs, header);
                    se.get_unsigned_range_max(cx, guarded).ule(limit)
                };
                let mut f = self.get_no_wrap_flags(ar, flags::MASK);
                if f & flags::NUW == 0 && can_prove_nuw(self) {
                    f |= flags::NUW;
                }

                self.set_no_wrap_flags(ar, f);
                if self.get_no_wrap_flags(ar, flags::NUW) != 0 {
                    // Emulate what getZeroExtendExpr would have done during
                    // construction if we'd been able to infer the fact just
                    // above at that time.
                    let step = self.get_step_recurrence(cx, ar);
                    let ty = self.type_of(lhs);
                    let start = self.get_extend_add_rec_start(cx, super::cast::Extend::Zero, ar, ty, 0);
                    let wide_step = self.get_zero_extend_expr(cx, Use::of(step), ty, 0);
                    let nw_flags = self.get_no_wrap_flags(ar, flags::MASK);
                    let s = self
                        .get_add_rec_expr_two(cx, Use::of(start), Use::of(wide_step), header, Flags::expr(nw_flags))
                        .scev;
                    iv = self.is_add_rec_of(s, header).then_some(s);
                }
            }
        }

        if iv.is_none() && allow_predicates {
            // Try to make this an AddRec using runtime tests, in the first X
            // iterations of this loop, where X is the SCEV expression found by
            // the algorithm below.
            iv = self.convert_scev_to_add_rec_with_predicates(cx, lhs, header, &mut predicates);
            predicated_iv = true;
        }

        // Avoid weird loops
        let Some(iv) = iv.filter(|&iv| self.is_add_rec_of(iv, header) && self.is_affine(iv)) else {
            return ExitLimit::could_not_compute(self);
        };

        // A precondition of this method is that the condition being analyzed
        // reaches an exiting branch which dominates the latch. Given that, we
        // can assume that an increment which violates the nowrap specification
        // and produces poison must cause undefined behavior when the resulting
        // poison value is branched upon and thus we can conclude that the
        // backedge is taken no more often than would be required to produce
        // that poison value. Note that a well defined loop can exit on the
        // iteration which violates the nowrap specification if there is
        // another exit (either explicit or implicit/exceptional) which causes
        // the loop to execute before the exiting instruction we're analyzing
        // would trigger UB.
        let wrap_type = if is_signed { flags::NSW } else { flags::NUW };
        let no_wrap = controls_only_exit && self.get_no_wrap_flags(iv, wrap_type) != 0;
        let cond = if is_signed { IntPredicate::Slt } else { IntPredicate::Ult };

        let mut stride = self.get_step_recurrence(cx, iv);
        let mut guarded_stride = stride;

        // Whether the IV may reach the maximum value before the exit is taken.
        let mut iv_may_overflow = true;

        let mut positive_stride = self.is_known_positive(cx, stride);
        // A dominating guard may prove the stride positive.
        if !positive_stride {
            let loop_guarded_stride = self.apply_loop_guards(cx, stride, header);
            if self.is_known_positive(cx, loop_guarded_stride) {
                guarded_stride = loop_guarded_stride;
                positive_stride = true;
                // Encode the context-sensitive stride > 0 fact into the
                // expression.
                let one = self.get_one(cx, self.type_of(stride));
                stride = self.get_umax_expr(cx, vec![Use::of(stride), Use::of(one)]);
            }
        }

        // Avoid negative or zero stride values.
        if !positive_stride {
            // We can compute the correct backedge taken count for loops with
            // unknown strides if we can prove that the loop is not an infinite
            // loop with side effects. Here's the loop structure we are trying
            // to handle -
            //
            // i = start
            // do {
            //   A[i] = i;
            //   i += s;
            // } while (i < end);
            //
            // The backedge taken count for such loops is evaluated as -
            // (max(end, start + stride) - start - 1) /u stride
            //
            // The additional preconditions that we need to check to prove
            // correctness of the above formula is as follows -
            //
            // a) IV is either nuw or nsw depending upon signedness (indicated
            //    by the NoWrap flag).
            // b) the loop is guaranteed to be finite (e.g. is mustprogress and
            //    has no side effects within the loop)
            // c) loop has a single static exit (with no abnormal exits)
            //
            // Precondition a) implies that if the stride is negative, this is
            // a single trip loop. The backedge taken count formula reduces to
            // zero in this case.
            //
            // Precondition b) and c) combine to imply that if rhs is invariant
            // in L, then a zero stride means the backedge can't be taken
            // without executing undefined behavior.
            //
            // The positive stride case is the same as isKnownPositive(Stride)
            // returning true (original behavior of the function).
            if predicated_iv
                || !no_wrap
                || !self.loop_is_finite_by_assumption(cx, header)
                || !self.loop_has_no_abnormal_exits(cx, header)
            {
                return ExitLimit::could_not_compute(self);
            }

            if !self.is_known_non_zero(cx, stride) {
                // If we have a step of zero, and RHS isn't invariant in L, we
                // don't know if it might eventually be greater than start and
                // if so, on which iteration. We can't even produce a useful
                // upper bound.
                if !self.is_loop_invariant(cx, rhs, Some(header)) {
                    return ExitLimit::could_not_compute(self);
                }

                // We allow a potentially zero stride, but we need to divide by
                // stride below. Since the loop can't be infinite and this
                // check must control the sole exit, we can infer the exit must
                // be taken on the first iteration (e.g. backedge count = 0) if
                // the stride is zero. Given that, we know the numerator in the
                // divides below must be zero, so we can pick an arbitrary
                // non-zero value for the denominator (e.g. stride) and produce
                // the right result.
                // FIXME (LLVM): Handle the case where Stride is poison?
                //
                // Proof by contradiction. Suppose the stride were zero. If we
                // can prove that the backedge *is* taken on the first
                // iteration, then since we know this condition controls the
                // sole exit, we must have an infinite loop. We can't have a
                // (well defined) infinite loop per check just above. Note: the
                // (Start - Stride) term is used to get the start' term from
                // (start' + stride,+,stride). Remember that we only care about
                // the result of this expression when stride == 0 at runtime.
                let iv_start = self.addrec_start(iv);
                let start_if_zero = self.get_minus_scev(cx, Use::of(iv_start), Use::of(stride), 0, 0);
                let would_zero_stride_be_ub =
                    self.is_loop_entry_guarded_by_cond_pred(cx, header, cond, start_if_zero, rhs);
                if !would_zero_stride_be_ub {
                    let one = self.get_one(cx, self.type_of(stride));
                    stride = self.get_umax_expr(cx, vec![Use::of(stride), Use::of(one)]);
                }
            }
        } else {
            // Avoid proven overflow cases: this will ensure that the backedge
            // taken count will not generate any unsigned overflow.
            iv_may_overflow = self.can_iv_overflow_on_lt(cx, rhs, guarded_stride, is_signed, false);
            if iv_may_overflow && !no_wrap {
                return ExitLimit::could_not_compute(self);
            }
        }

        // On all paths just preceeding, we established the following
        // invariant: IV can be assumed not to overflow up to and including the
        // exiting iteration. We proved this in one of two ways:
        //   1) We can show overflow doesn't occur before the exiting iteration
        //      1a) canIVOverflowOnLT, and b) step of one
        //   2) We can show that if overflow occurs, the loop must execute UB
        //      before any possible exit.
        // Note that we have not yet proved RHS invariant (in general).

        let mut start = self.addrec_start(iv);

        // Preserve pointer-typed Start/RHS to pass to isLoopEntryGuardedByCond.
        // If we convert to integers, isLoopEntryGuardedByCond will miss some
        // cases. Use integer-typed versions for actual computation; we can't
        // subtract pointers in general.
        let orig_start = start;
        let orig_rhs = rhs;
        if self.type_of(start).is_pointer() {
            start = self.get_ptr_to_addr_expr(cx, start);
            if start == self.get_could_not_compute() {
                return ExitLimit::exact(self, start);
            }
        }
        if self.type_of(rhs).is_pointer() {
            rhs = self.get_ptr_to_addr_expr(cx, rhs);
            if rhs == self.get_could_not_compute() {
                return ExitLimit::exact(self, rhs);
            }
        }

        let unknown = self.get_could_not_compute();
        let mut end: Option<Scev> = None;
        let mut be_count = unknown;
        let mut be_count_if_backedge_taken = unknown;
        if !self.is_loop_invariant(cx, rhs, Some(header)) {
            if positive_stride && self.is_add_rec_of(rhs, header) && self.get_no_wrap_flags(rhs, flags::MASK) != 0 {
                // The structure of loop we are trying to calculate backedge
                // count of:
                //
                //  left = left_start
                //  right = right_start
                //
                //  while(left < right){
                //    ... do something here ...
                //    left += s1; // stride of left is s1 (s1 > 0)
                //    right += s2; // stride of right is s2 (s2 < 0)
                //  }
                //
                let rhs_start = self.addrec_start(rhs);
                let rhs_stride = self.get_step_recurrence(cx, rhs);

                // If Stride - RHSStride is positive and does not overflow, we
                // can write backedge count as ->
                //    ceil((End - Start) /u (Stride - RHSStride))
                //    Where, End = max(RHSStart, Start)

                // Check if RHSStride < 0 and Stride - RHSStride will not
                // overflow.
                if self.is_known_negative(cx, rhs_stride)
                    && self.will_not_overflow(cx, BinaryOp::Sub, true, stride, rhs_stride, None)
                {
                    let denominator = self.get_minus_scev(cx, Use::of(stride), Use::of(rhs_stride), 0, 0);
                    if self.is_known_positive(cx, denominator) {
                        let e = if is_signed {
                            self.get_smax_expr(cx, vec![Use::of(rhs_start), Use::of(start)])
                        } else {
                            self.get_umax_expr(cx, vec![Use::of(rhs_start), Use::of(start)])
                        };
                        end = Some(e);

                        // We can do this because End >= Start, as
                        // End = max(RHSStart, Start)
                        let delta = self.get_minus_scev(cx, Use::of(e), Use::of(start), 0, 0);

                        be_count = self.get_udiv_ceil_scev(cx, delta, denominator);
                        let to_start = self.get_minus_scev(cx, Use::of(rhs_start), Use::of(start), 0, 0);
                        be_count_if_backedge_taken = self.get_udiv_ceil_scev(cx, to_start, denominator);
                    }
                }
            }
        } else {
            // Let End = max(RHS,Start). We use the expression
            // (End-Start)/Stride to describe the backedge count: if
            // the backedge is taken at least once then End is RHS,
            // and if not End is Start so we get a backedge count of
            // zero.
            //
            // AddingStrideMinusOneMayOverflow has the following preconditions:
            //
            // 1. If IsSigned, Start <=s End; otherwise, Start <=u End
            // 2. The index variable doesn't overflow.
            //
            // Therefore, we know N exists such that
            // (Start + Stride * N) >= End, and computing "(Start + Stride * N)"
            // doesn't overflow.
            //
            // Using this information, try to prove whether the addition in
            // "(End - Start) + (Stride - 1)" has unsigned overflow.
            //
            // If the IV cannot overflow, RHS is at least Stride - 1 below the
            // maximum value, so the distance End - Start is at most
            // UMAX - (Stride - 1) and the (Stride - 1) addition below cannot
            // overflow.
            let one = self.get_one(cx, self.type_of(stride));
            let adding_stride_minus_one_may_overflow = iv_may_overflow && {
                if self.is_known_to_be_a_power_of_two(cx, stride, false, false) {
                    // Suppose Stride is a power of two, and Start/End are
                    // unsigned integers. By the preconditions of this
                    // function, "(Start + Stride * N) >= End" and this doesn't
                    // overflow, so End - Start <= Stride * N <= UMAX - Start
                    // <= UMAX; Stride * N is a multiple of Stride, so
                    // End - Start <= UMAX - (UMAX mod Stride), and
                    // UMAX mod Stride == Stride - 1 for a power of two. So
                    // (End - Start) + (Stride - 1) <= UMAX: the addition
                    // doesn't have unsigned overflow. A similar proof works if
                    // we treat Start/End as signed values.
                    false
                } else {
                    let stride_minus_one = self.get_minus_scev(cx, Use::of(stride), Use::of(one), 0, 0);
                    // If Start is equal to Stride, (End - Start) + (Stride - 1)
                    // == End - 1. If !IsSigned, 0 <u Stride == Start <=u End;
                    // so 0 <u End - 1 <u End. If IsSigned, 0 <s Stride == Start
                    // <=s End; so 0 <s End - 1 <s End. If Start is equal to
                    // Stride - 1, (End - Start) + Stride - 1 == End.
                    !(start == stride || start == stride_minus_one)
                }
            };

            let orig_start_minus_stride = self.get_minus_scev(cx, Use::of(orig_start), Use::of(stride), 0, 0);
            debug_assert!(self.is_available_at_loop_entry(cx, orig_start_minus_stride, header), "Must be!");
            debug_assert!(self.is_available_at_loop_entry(cx, orig_start, header), "Must be!");
            debug_assert!(self.is_available_at_loop_entry(cx, orig_rhs, header), "Must be!");
            // Can we prove Start - Stride < RHS, and either Start - Stride <
            // Start or (via !AddingStrideMinusOneMayOverflow) that
            // (RHS - Start) + (Stride - 1) does not overflow?
            if (!adding_stride_minus_one_may_overflow
                || self.is_loop_entry_guarded_by_cond_pred(cx, header, cond, orig_start_minus_stride, orig_start))
                && self.is_loop_entry_guarded_by_cond_pred(cx, header, cond, orig_start_minus_stride, orig_rhs)
            {
                // In this case, we can use a refined formula for computing
                // backedge taken count. The general formula remains:
                //   "End-Start /uceiling Stride"
                // We want to use the alternate formula:
                //   "((RHS - 1) - (Start - Stride)) /u Stride"
                // (The case analysis showing these are equivalent under the
                // preconditions is in LLVM's comment at this place.)
                let minus_one = self.get_minus_one(cx, self.type_of(stride));
                let rhs_minus_one =
                    self.get_add_expr(cx, vec![Use::of(rhs), Use::of(minus_one)], Flags::default(), 0).scev;
                let start_minus_stride = self.get_minus_scev(cx, Use::of(start), Use::of(stride), 0, 0);
                let numerator = self.get_minus_scev(cx, Use::of(rhs_minus_one), Use::of(start_minus_stride), 0, 0);
                be_count = self.get_udiv_expr(cx, Use::of(numerator), Use::of(stride));
            }

            if be_count == unknown {
                let can_prove_rhs_greater_than_equal_start = |se: &mut ScalarEvolution| {
                    let cond_ge = if is_signed { IntPredicate::Sge } else { IntPredicate::Uge };
                    let guarded_rhs = se.apply_loop_guards(cx, orig_rhs, header);
                    let guarded_start = se.apply_loop_guards(cx, orig_start, header);

                    if se.is_loop_entry_guarded_by_cond_pred(cx, header, cond_ge, orig_rhs, orig_start)
                        || se.is_known_predicate_scev(cx, cond_ge, guarded_rhs, guarded_start)
                    {
                        return true;
                    }

                    // (RHS > Start - 1) implies RHS >= Start.
                    // * "RHS >= Start" is trivially equivalent to "RHS > Start
                    //   - 1" if "Start - 1" doesn't overflow.
                    // * For signed comparison, if Start - 1 does overflow, it's
                    //   equal to INT_MAX, and "RHS >s INT_MAX" is trivially
                    //   false.
                    // * For unsigned comparison, if Start - 1 does overflow,
                    //   it's equal to UINT_MAX, and "RHS >u UINT_MAX" is
                    //   trivially false.
                    //
                    // FIXME (LLVM): Should isLoopEntryGuardedByCond do this for
                    // us?
                    let cond_gt = if is_signed { IntPredicate::Sgt } else { IntPredicate::Ugt };
                    let minus_one = se.get_minus_one(cx, se.type_of(orig_start));
                    let start_minus_one =
                        se.get_add_expr(cx, vec![Use::of(orig_start), Use::of(minus_one)], Flags::default(), 0).scev;
                    se.is_loop_entry_guarded_by_cond_pred(cx, header, cond_gt, orig_rhs, start_minus_one)
                };

                // If we know that RHS >= Start in the context of loop, then we
                // know that max(RHS, Start) = RHS at this point.
                let e;
                if can_prove_rhs_greater_than_equal_start(self) {
                    e = rhs;
                } else {
                    // If RHS < Start, the backedge will be taken zero times.
                    // So in general, we can write the backedge-taken count as:
                    //
                    //     RHS >= Start ? ceil(RHS - Start) / Stride : 0
                    //
                    // We convert it to the following to make it more convenient
                    // for SCEV:
                    //
                    //     ceil(max(RHS, Start) - Start) / Stride
                    e = if is_signed {
                        self.get_smax_expr(cx, vec![Use::of(rhs), Use::of(start)])
                    } else {
                        self.get_umax_expr(cx, vec![Use::of(rhs), Use::of(start)])
                    };

                    // See what would happen if we assume the backedge is taken.
                    // This is used to compute MaxBECount.
                    let to_start = self.get_minus_scev(cx, Use::of(rhs), Use::of(start), 0, 0);
                    be_count_if_backedge_taken = self.get_udiv_ceil_scev(cx, to_start, stride);
                }
                end = Some(e);

                let delta = self.get_minus_scev(cx, Use::of(e), Use::of(start), 0, 0);
                if !adding_stride_minus_one_may_overflow {
                    // floor((D + (S - 1)) / S)
                    // We prefer this formulation if it's legal because it's
                    // fewer operations.
                    let stride_minus_one = self.get_minus_scev(cx, Use::of(stride), Use::of(one), 0, 0);
                    let sum = self
                        .get_add_expr(cx, vec![Use::of(delta), Use::of(stride_minus_one)], Flags::default(), 0)
                        .scev;
                    be_count = self.get_udiv_expr(cx, Use::of(sum), Use::of(stride));
                } else {
                    be_count = self.get_udiv_ceil_scev(cx, delta, stride);
                }
            }
        }
        let _ = end;

        let constant_max_be_count;
        let mut max_or_zero = false;
        if self.match_apint(be_count).is_some() {
            constant_max_be_count = be_count;
        } else if self.match_apint(be_count_if_backedge_taken).is_some() {
            // If we know exactly how many times the backedge will be taken if
            // it's taken at least once, then the backedge count will either be
            // that or zero.
            constant_max_be_count = be_count_if_backedge_taken;
            max_or_zero = true;
        } else {
            let bits = self.get_type_size_in_bits(cx, self.type_of(lhs));
            constant_max_be_count = self.compute_max_be_count_for_lt(cx, start, stride, rhs, bits, is_signed, false);
        }
        let mut constant_max_be_count = constant_max_be_count;

        if constant_max_be_count == unknown && be_count != unknown {
            let max = self.get_unsigned_range_max(cx, be_count);
            constant_max_be_count = self.get_constant(max);
        }

        let symbolic_max_be_count = if be_count == unknown { constant_max_be_count } else { be_count };
        ExitLimit::new(self, be_count, constant_max_be_count, symbolic_max_be_count, max_or_zero)
            .with_predicates(predicates)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13849
    /// `howManyGreaterThans`: the number of times the backedge is taken while
    /// `LHS > RHS`.
    pub(super) fn how_many_greater_thans(
        &mut self,
        cx: &Cx,
        lhs: Scev,
        mut rhs: Scev,
        header: BlockId,
        is_signed: bool,
        controls_only_exit: bool,
        allow_predicates: bool,
    ) -> ExitLimit {
        let mut predicates: Vec<super::wrap_predicate::Predicate> = Vec::new();
        // We handle only IV > Invariant
        if !self.is_loop_invariant(cx, rhs, Some(header)) {
            return ExitLimit::could_not_compute(self);
        }

        let mut iv = self.is_add_rec_of(lhs, header).then_some(lhs);
        if iv.is_none() && allow_predicates {
            // Try to make this an AddRec using runtime tests, in the first X
            // iterations of this loop, where X is the SCEV expression found by
            // the algorithm below.
            iv = self.convert_scev_to_add_rec_with_predicates(cx, lhs, header, &mut predicates);
        }

        // Avoid weird loops
        let Some(iv) = iv.filter(|&iv| self.is_add_rec_of(iv, header) && self.is_affine(iv)) else {
            return ExitLimit::could_not_compute(self);
        };

        let wrap_type = if is_signed { flags::NSW } else { flags::NUW };
        let no_wrap = controls_only_exit && self.get_no_wrap_flags(iv, wrap_type) != 0;
        let cond = if is_signed { IntPredicate::Sgt } else { IntPredicate::Ugt };

        let step = self.get_step_recurrence(cx, iv);
        let stride = self.get_negative_scev(cx, step, 0);

        // Avoid negative or zero stride values
        if !self.is_known_positive(cx, stride) {
            return ExitLimit::could_not_compute(self);
        }

        // Avoid proven overflow cases: this will ensure that the backedge
        // taken count will not generate any unsigned overflow. Relaxed
        // no-overflow conditions exploit NoWrapFlags, allowing to optimize in
        // presence of undefined behaviors like the case of C language.
        let mut may_add_overflow = false;
        let mut start = self.addrec_start(iv);
        let mut end = rhs;
        if !self.match_one(stride) && self.can_iv_overflow_on_lt(cx, rhs, stride, is_signed, true) {
            if !no_wrap {
                return ExitLimit::could_not_compute(self);
            }
            may_add_overflow = true;
        }

        let start_plus_stride = self.get_add_expr(cx, vec![Use::of(start), Use::of(stride)], Flags::default(), 0).scev;
        if !self.is_loop_entry_guarded_by_cond_pred(cx, header, cond, start_plus_stride, rhs) {
            // If we know that Start >= RHS in the context of loop, then we know
            // that min(RHS, Start) = RHS at this point.
            let cond_ge = if is_signed { IntPredicate::Sge } else { IntPredicate::Uge };
            if self.is_loop_entry_guarded_by_cond_pred(cx, header, cond_ge, start, rhs) {
                end = rhs;
            } else {
                end = if is_signed {
                    self.get_smin_expr(cx, vec![Use::of(rhs), Use::of(start)])
                } else {
                    self.get_umin_expr(cx, vec![Use::of(rhs), Use::of(start)], false)
                };
            }
        }

        if self.type_of(start).is_pointer() {
            debug_assert!(
                self.type_of(end).is_pointer() && self.type_of(rhs).is_pointer(),
                "Start, End and RHS all must be pointers"
            );
            start = self.get_ptr_to_addr_expr(cx, start);
            if start == self.get_could_not_compute() {
                return ExitLimit::exact(self, start);
            }

            end = self.get_ptr_to_addr_expr(cx, end);
            if end == self.get_could_not_compute() {
                return ExitLimit::exact(self, end);
            }

            rhs = self.get_ptr_to_addr_expr(cx, rhs);
            if rhs == self.get_could_not_compute() {
                return ExitLimit::exact(self, rhs);
            }
        }

        let delta = self.get_minus_scev(cx, Use::of(start), Use::of(end), 0, 0);
        let be_count;
        if may_add_overflow {
            // The ceiling division instead needs Start >= End, so that
            // (Start - End) is the exact unsigned distance between them.
            let cond_ge = if is_signed { IntPredicate::Sge } else { IntPredicate::Uge };
            if !self.is_loop_entry_guarded_by_cond_pred(cx, header, cond_ge, start, end) {
                return ExitLimit::could_not_compute(self);
            }
            be_count = self.get_udiv_ceil_scev(cx, delta, stride);
        } else {
            // Compute ((Start - End) + (Stride - 1)) / Stride, if the IV cannot
            // overflow as it requires fewer operations.
            let one = self.get_one(cx, self.type_of(stride));
            let stride_minus_one = self.get_minus_scev(cx, Use::of(stride), Use::of(one), 0, 0);
            let sum = self.get_add_expr(cx, vec![Use::of(delta), Use::of(stride_minus_one)], Flags::default(), 0).scev;
            be_count = self.get_udiv_expr(cx, Use::of(sum), Use::of(stride));
        }

        // "IV > RHS" is analyzed as the equivalent "~IV < ~RHS"; Stride is
        // already the negated step.
        let mut constant_max_be_count = if self.match_apint(be_count).is_some() {
            be_count
        } else {
            let bits = self.get_type_size_in_bits(cx, self.type_of(lhs));
            self.compute_max_be_count_for_lt(cx, start, stride, rhs, bits, is_signed, true)
        };

        if constant_max_be_count == self.get_could_not_compute() {
            constant_max_be_count = be_count;
        }
        let symbolic_max_be_count =
            if be_count == self.get_could_not_compute() { constant_max_be_count } else { be_count };

        ExitLimit::new(self, be_count, constant_max_be_count, symbolic_max_be_count, false).with_predicates(predicates)
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11290
    /// `ScalarEvolution::isKnownToBeAPowerOfTwo(S, OrZero, OrNegative)`.
    pub fn is_known_to_be_a_power_of_two(
        &mut self,
        cx: &Cx,
        s: Scev,
        or_zero: bool,
        or_negative: bool,
    ) -> bool {
        let non_recursive = |se: &ScalarEvolution, s: Scev| match se.as_constant(s) {
            Some(c) => {
                // `isNegatedPowerOf2`: negative, and the ones then the zeros
                // fill the width.
                let negated_power_of_2 = c.is_negative() && c.not().countl_zero() + c.countr_zero() == c.width;
                c.is_power_of_2() || (or_negative && negated_power_of_2)
            }
            // vscale is a power-of-two.
            None => false,
        };

        if non_recursive(self, s) {
            return true;
        }

        let Kind::Mul(operands) = self.node(s).kind.clone() else { return false };
        operands.iter().all(|op| non_recursive(self, op.scev)) && (or_zero || self.is_known_non_zero(cx, s))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2372
    /// `getStrengthenedNoWrapFlagsFromBinOp`: the `(nuw, nsw)` the no-wrap
    /// proofs give the add, sub, mul or shl by a constant `inst`, with the ones
    /// it has; none if it gains none. (`UseContextForNoWrapFlagInference` is
    /// on, its `cl::opt` default: `inst` is the context.)
    pub fn get_strengthened_no_wrap_flags_from_bin_op(
        &mut self,
        cx: &Cx,
        inst: llrm_mir::module::InstId,
    ) -> Option<(bool, bool)> {
        use llrm_mir::context::ConstantKind;
        use llrm_mir::opcode::{Flags as IrFlags, Opcode};
        let instruction = cx.function.instruction(inst);
        let Opcode::Binary(mut opcode) = instruction.opcode else { return None };
        let (has_nuw, has_nsw) = (instruction.flags.contains(IrFlags::NUW), instruction.flags.contains(IrFlags::NSW));
        // It cannot be done any better.
        if has_nuw && has_nsw {
            return None;
        }
        let (mut nuw, mut nsw) = (has_nuw, has_nsw);
        let mut deduced = false;

        let lhs = self.get_scev(cx, instruction.operands[0]);
        let mut rhs = self.get_scev(cx, instruction.operands[1]);

        let mut can_use_nsw = true;
        if opcode == BinaryOp::Shl {
            // Treat `shl %a, C` as `mul %a, 1 << C`.
            let llrm_mir::module::Operand::Constant(constant) = instruction.operands[1] else { return None };
            let found = cx.context.get(constant);
            let ConstantKind::Int(amount) = found.kind else { return None };
            let bit_width = cx.context.types.int_bits(found.ty)?;
            if amount >= u128::from(bit_width) {
                return None;
            }
            // NSW only transfers if the shift amount is < BitWidth - 1, as
            // INT_MIN * -1 overflows.
            can_use_nsw = amount < u128::from(bit_width - 1);
            opcode = BinaryOp::Mul;
            rhs = self.get_constant(Ap::new(bit_width, 1u128 << amount));
        } else if !matches!(opcode, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) {
            return None;
        }

        if !has_nuw && self.will_not_overflow(cx, opcode, false, lhs, rhs, Some(inst)) {
            nuw = true;
            deduced = true;
        }
        if can_use_nsw && !has_nsw && self.will_not_overflow(cx, opcode, true, lhs, rhs, Some(inst)) {
            nsw = true;
            deduced = true;
        }
        deduced.then_some((nuw, nsw))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2299
    /// `willNotOverflow`: whether `ext(LHS op RHS) == ext(LHS) op ext(RHS)`.
    pub fn will_not_overflow(
        &mut self,
        cx: &Cx,
        bin_op: BinaryOp,
        signed: bool,
        lhs: Scev,
        rhs: Scev,
        ctx_i: Option<llrm_mir::module::InstId>,
    ) -> bool {
        let operation = |se: &mut ScalarEvolution, l: Scev, r: Scev| -> Scev {
            match bin_op {
                BinaryOp::Add => se.get_add_expr(cx, vec![Use::of(l), Use::of(r)], Flags::default(), 0).scev,
                BinaryOp::Sub => se.get_minus_scev(cx, Use::of(l), Use::of(r), 0, 0),
                BinaryOp::Mul => se.get_mul_expr(cx, vec![Use::of(l), Use::of(r)], Flags::default(), 0).scev,
                other => unreachable!("Unsupported binary op {other:?}"),
            }
        };
        let extension = |se: &mut ScalarEvolution, s: Scev, ty: Ty| -> Scev {
            if signed {
                se.get_sign_extend_expr(cx, Use::of(s), ty, 0)
            } else {
                se.get_zero_extend_expr(cx, Use::of(s), ty, 0)
            }
        };

        // Check ext(LHS op RHS) == ext(LHS) op ext(RHS)
        let Ty::Int(narrow) = self.type_of(lhs) else { return false };
        let wide_ty = Ty::Int(narrow * 2);

        let combined = operation(self, lhs, rhs);
        let a = extension(self, combined, wide_ty);
        let lhsb = extension(self, lhs, wide_ty);
        let rhsb = extension(self, rhs, wide_ty);
        let b = operation(self, lhsb, rhsb);
        if a == b {
            return true;
        }
        // Can we use context to prove the fact we need?
        if ctx_i.is_none() {
            return false;
        }
        // TODO (LLVM): Support mul.
        if bin_op == BinaryOp::Mul {
            return false;
        }
        let Some(c) = self.match_apint(rhs) else { return false };
        // TODO (LLVM): Lift this limitation.
        let num_bits = c.width;
        let is_sub = bin_op == BinaryOp::Sub;
        let is_negative_const = signed && c.is_negative();
        // Compute the direction and magnitude by which we need to check
        // overflow.
        let overflow_down = is_sub ^ is_negative_const;
        let mut magnitude = c;
        if is_negative_const {
            if c == Ap::signed_min(num_bits) {
                // TODO (LLVM): SINT_MIN on inversion gives the same negative
                // value, we don't want to deal with that.
                return false;
            }
            magnitude = c.neg();
        }

        let pred = if signed { IntPredicate::Sle } else { IntPredicate::Ule };
        if overflow_down {
            // To avoid overflow down, we need to make sure that
            // MIN + Magnitude <= LHS.
            let min = if signed { Ap::signed_min(num_bits) } else { Ap::zero(num_bits) };
            let limit = self.get_constant(min.add(magnitude));
            self.is_known_predicate_at(cx, pred, limit, lhs, ctx_i)
        } else {
            // To avoid overflow up, we need to make sure that
            // LHS <= MAX - Magnitude.
            let max = if signed { Ap::signed_max(num_bits) } else { Ap::all_ones(num_bits) };
            let limit = self.get_constant(max.sub(magnitude));
            self.is_known_predicate_at(cx, pred, lhs, limit, ctx_i)
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L868
    /// `BinomialCoefficient(It, K, SE, ResultTy)`: `BC(It, K)` at the width of
    /// `result_ty`, `K > 0`.
    fn binomial_coefficient(
        &mut self,
        cx: &Cx,
        it: Scev,
        k: u32,
        result_ty: Ty,
    ) -> Scev {
        // Handle the simplest case efficiently.
        if k == 1 {
            return self.get_truncate_or_zero_extend(cx, it, result_ty, 0);
        }

        // (LLVM's comment follows: BC(It, K) = It*(It-1)*...*(It-K+1)/K! is
        // computed as the product at W + T bits, divided by 2^T, then
        // multiplied by the inverse of the odd part of K!.)

        // Protection from insane SCEVs; this bound is conservative, but it
        // probably doesn't matter.
        if k > 1000 {
            return self.get_could_not_compute();
        }

        let w = self.get_type_size_in_bits(cx, result_ty);

        // Calculate K! / 2^T and T; we divide out the factors of two before
        // multiplying for calculating K! / 2^T to avoid overflow. Other
        // overflow doesn't matter because we only care about the bottom W bits
        // of the result.
        let mut odd_factorial = Ap::new(w, 1);
        let mut t: u32 = 1;
        for i in 3..=k {
            let two_factors = i.trailing_zeros();
            t += two_factors;
            odd_factorial = odd_factorial.mul(Ap::new(w, u128::from(i >> two_factors)));
        }

        // We need at least W + T bits for the multiplication step.
        let calculation_bits = w + t;
        if calculation_bits > 128 {
            // D-T3.
            return self.get_could_not_compute();
        }

        // Calculate 2^T, at width T+W.
        let div_factor = one_bit_set(calculation_bits, t);

        // Calculate the multiplicative inverse of K! / 2^T; this
        // multiplication factor will perform the exact division by K! / 2^T.
        let multiply_factor = multiplicative_inverse(odd_factorial);

        // Calculate the product, at width T+W.
        let calculation_ty = Ty::Int(calculation_bits);
        let mut dividend = self.get_truncate_or_zero_extend(cx, it, calculation_ty, 0);
        for i in 1..k {
            let it_ty = self.type_of(it);
            let c = self.get_constant_of_type(cx, it_ty, i128::from(i));
            let s = self.get_minus_scev(cx, Use::of(it), Use::of(c), 0, 0);
            let wide = self.get_truncate_or_zero_extend(cx, s, calculation_ty, 0);
            dividend = self.get_mul_expr(cx, vec![Use::of(dividend), Use::of(wide)], Flags::default(), 0).scev;
        }

        // Divide by 2^T.
        let divisor = self.get_constant(div_factor);
        let div_result = self.get_udiv_expr(cx, Use::of(dividend), Use::of(divisor));

        // Truncate the result, and divide by K! / 2^T.
        let factor = self.get_constant(multiply_factor);
        let truncated = self.get_truncate_or_zero_extend(cx, div_result, result_ty, 0);
        self.get_mul_expr(cx, vec![Use::of(factor), Use::of(truncated)], Flags::default(), 0).scev
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L986
    /// `SCEVAddRecExpr::evaluateAtIteration(Operands, It, SE, UseFlags)`: the
    /// chain `{A,+,B,+,C,...}` at iteration `it`, as
    /// `A*BC(It,0) + B*BC(It,1) + C*BC(It,2) + ...`.
    pub fn evaluate_at_iteration_ops(
        &mut self,
        cx: &Cx,
        operands: &[Use],
        it: Scev,
        use_flags: u8,
    ) -> Use {
        debug_assert!(!operands.is_empty());
        debug_assert!(operands.len() == 2 || use_flags == 0, "use-specific flags only supported for affine AddRecs");
        let result_ty = self.type_of(operands[0].scev);
        let mut result = Use::of(operands[0].scev);
        for (i, operand) in operands.iter().enumerate().skip(1) {
            // The computation is correct in the face of overflow provided that
            // the multiplication is performed _after_ the evaluation of the
            // binomial coefficient.
            let coeff = self.binomial_coefficient(cx, it, i as u32, result_ty);
            if coeff == self.get_could_not_compute() {
                return Use::of(coeff);
            }
            let flags = Flags { expr: 0, use_: use_flags };
            let mul = self.get_mul_expr(cx, vec![Use::of(operand.scev), Use::of(coeff)], flags, 0);
            result = self.get_add_expr(cx, vec![result, mul], flags, 0);
        }
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L981
    /// `SCEVAddRecExpr::evaluateAtIteration(It, SE)`.
    pub fn evaluate_at_iteration(
        &mut self,
        cx: &Cx,
        add_rec: Scev,
        it: Scev,
    ) -> Scev {
        let operands = self.operands(add_rec).to_vec();
        self.evaluate_at_iteration_ops(cx, &operands, it, 0).scev
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13952
    /// `SCEVAddRecExpr::getNumIterationsInRange`: how many iterations of the
    /// recurrence stay in `range`.
    pub fn get_num_iterations_in_range(
        &mut self,
        cx: &Cx,
        add_rec: Scev,
        range: &ConstantRange,
    ) -> Scev {
        if range.is_full_set() {
            // Infinite loop.
            return self.get_could_not_compute();
        }

        let Kind::AddRec(operands, header) = self.node(add_rec).kind.clone() else { unreachable!("an add recurrence") };
        let ty = self.type_of(add_rec);

        // If the start is a non-zero constant, shift the range to simplify
        // things.
        if let Some(start) = self.as_constant(operands[0].scev)
            && !start.is_zero()
        {
            let mut shifted_operands = operands.clone();
            shifted_operands[0] = Use::of(self.get_zero(cx, self.type_of(operands[0].scev)));
            let nw = self.get_no_wrap_flags(add_rec, flags::NW);
            let shifted = self.get_add_rec_expr(cx, shifted_operands, header, Flags::expr(nw)).scev;
            if self.is_add_rec_of(shifted, header) {
                return self.get_num_iterations_in_range(cx, shifted, &range.subtract(start.bits));
            }
            // This is strange and shouldn't happen.
            return self.get_could_not_compute();
        }

        // The only time we can solve this is when we have all constant
        // indices. Otherwise, we cannot determine the overflow conditions.
        if !operands.iter().all(|op| self.as_constant(op.scev).is_some()) {
            return self.get_could_not_compute();
        }

        // Okay at this point we know that all elements of the chrec are
        // constants and that the start element is zero.

        // First check to see if the range contains zero. If not, the first
        // iteration exits.
        let bit_width = self.get_type_size_in_bits(cx, ty);
        if !range.contains(0) {
            return self.get_zero(cx, ty);
        }

        if operands.len() == 2 {
            // If this is an affine expression then we have this situation:
            //   Solve {0,+,A} in Range  ===  Ax in Range

            // We know that zero is in the range. If A is positive then we know
            // that the upper value of the range must be the first possible
            // exit value. If A is negative then the lower of the range is the
            // last possible loop value. Also note that we already checked for a
            // full range.
            let a = self.as_constant(operands[1].scev).expect("a constant");
            let end = if a.sext_value() >= 1 {
                Ap::new(bit_width, range.upper).sub(Ap::one(bit_width))
            } else {
                Ap::new(bit_width, range.lower)
            };

            // The exit value should be (End+A)/A.
            let exit_val = end.add(a).udiv(a);
            let exit_value = self.get_constant(exit_val);

            // Evaluate at the exit value. If we really did fall out of the
            // valid range, then we computed our trip count, otherwise wrap
            // around or other things must have happened.
            let val = self.evaluate_at_iteration(cx, add_rec, exit_value);
            let val = self.as_constant(val).expect("Evaluation of SCEV at constant didn't fold correctly?");
            if range.contains(val.bits) {
                // Something strange happened.
                return self.get_could_not_compute();
            }

            return exit_value;
        }

        if operands.len() == 3
            && let Some((x, width)) = self.solve_quadratic_add_rec_range(cx, add_rec, range)
        {
            return self.get_constant(Ap::new(width, x as u128));
        }

        self.get_could_not_compute()
    }

    /// `evaluateAtIteration` of `add_rec` at the constant `x` (of `width`
    /// bits), as a number, if it folds to one.
    fn evaluate_chrec_at_constant(
        &mut self,
        cx: &Cx,
        add_rec: Scev,
        x: i128,
        width: u32,
    ) -> Option<Ap> {
        let it = self.get_constant(Ap::new(width, x as u128));
        let value = self.evaluate_at_iteration(cx, add_rec, it);
        self.as_constant(value)
    }

    /// `GetQuadraticEquation` of `{L,+,M,+,N}`: the equation, the multiplier
    /// and the coefficients' bit width, if all are constants.
    fn quadratic_equation_of(
        &self,
        cx: &Cx,
        add_rec: Scev,
    ) -> Option<(i128, i128, i128, i128, u32)> {
        let operands = self.operands(add_rec);
        let (l, m, n) = (
            self.as_constant(operands[0].scev)?,
            self.as_constant(operands[1].scev)?,
            self.as_constant(operands[2].scev)?,
        );
        let bit_width = l.width;
        let _ = cx;
        let (a, b, c, t) =
            super::quadratic::quadratic_equation(l.sext_value(), m.sext_value(), n.sext_value(), bit_width)?;
        Some((a, b, c, t, bit_width))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10603
    /// `SolveQuadraticAddRecExact`: the least `n >= 0` at which the quadratic
    /// recurrence is exactly zero, with the width of the solution.
    fn solve_quadratic_add_rec_exact(
        &mut self,
        cx: &Cx,
        add_rec: Scev,
    ) -> Option<(i128, u32)> {
        let (a, b, c, _, bit_width) = self.quadratic_equation_of(cx, add_rec)?;
        let (x, width) = super::quadratic::solve_quadratic_equation_wrap(a, b, c, bit_width + 1, bit_width + 1)?;
        let value = self.evaluate_chrec_at_constant(cx, add_rec, x, width)?;
        if !value.is_zero() {
            return None;
        }
        super::quadratic::trunc_if_possible(Some((x, width)), bit_width)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10636
    /// `SolveQuadraticAddRecRange`: the least `n` at which the quadratic
    /// recurrence `{0,+,M,+,N}` leaves `range` while `n - 1` is in it.
    fn solve_quadratic_add_rec_range(
        &mut self,
        cx: &Cx,
        add_rec: Scev,
        range: &ConstantRange,
    ) -> Option<(i128, u32)> {
        use super::quadratic::{min_optional, solve_quadratic_equation_wrap as solve, trunc_if_possible};
        let (a, b, _, m, bit_width) = self.quadratic_equation_of(cx, add_rec)?;

        // The solution is "unknown" (no answer yet) or known, possibly absent.
        let solve_for_boundary = |this: &mut Self, bound: i128| -> (Option<(i128, u32)>, bool) {
            // The quadratic equation's multiplier.
            let Some(bound) = bound.checked_mul(m) else { return (None, false) };
            // Signed overflow and unsigned overflow: pick the lower solution.
            let so = if bit_width > 1 { solve(a, b, -bound, bit_width + 1, bit_width) } else { None };
            let uo = solve(a, b, -bound, bit_width + 1, bit_width + 1);
            let leaves_range = |this: &mut Self, (x, width): (i128, u32)| -> Option<bool> {
                let v0 = this.evaluate_chrec_at_constant(cx, add_rec, x, width)?;
                if range.contains(v0.bits) {
                    return Some(false);
                }
                // X should be at least 1, so X-1 is non-negative.
                let v1 = this.evaluate_chrec_at_constant(cx, add_rec, x - 1, width)?;
                Some(range.contains(v1.bits))
            };
            // No solution found does not mean none exists.
            let (Some(so), Some(uo)) = (so, uo) else { return (None, false) };
            // Check the smaller value first to see if it leaves the range.
            let min = min_optional(Some(so), Some(uo)).expect("both exist");
            if leaves_range(this, min) == Some(true) {
                return (Some(min), true);
            }
            let max = if min == so { uo } else { so };
            if leaves_range(this, max) == Some(true) {
                return (Some(max), true);
            }
            // Solutions were found, but were eliminated.
            (None, true)
        };

        // The lower bound is inclusive: subtract 1 to represent the exiting
        // value.
        let lower = Ap::new(bit_width, range.lower).sext_value() - 1;
        let upper = Ap::new(bit_width, range.upper).sext_value();
        let (sl, sl_known) = solve_for_boundary(self, lower);
        let (su, su_known) = solve_for_boundary(self, upper);
        // If any of the solutions was unknown, no meaningful conclusion.
        if !sl_known || !su_known {
            return None;
        }
        trunc_if_possible(min_optional(sl, su), bit_width)
    }
}
