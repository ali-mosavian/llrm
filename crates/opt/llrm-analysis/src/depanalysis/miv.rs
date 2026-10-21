//! The multiple-index-variable tests: GCD and Banerjee.
//!
//! # How it works
//! `gcd_miv_test` asks whether the gcd of the constant coefficients of all
//! the loops divides the difference of the constant terms, then tries to
//! disprove each loop's `=` direction by moving that loop's coefficient out
//! of the gcd (Wolfe, "High Performance Compilers for Parallel Computing",
//! p. 235). It copes with symbolic terms, so it backs up the SIV tests on
//! linearized subscripts.
//!
//! `banerjee_miv_test` evaluates the subscript difference at the vertices of
//! the constraint polytope of each direction and refines the direction vector
//! hierarchically (`explore_directions`). It is off by default in the pin.
//!
//! # Deviations from LLVM
//! - D1: see [`super`]: Banerjee runs in `2 * B + L + 1` bits, and an `Ap` is
//!   at most 128, so a wider test is not run (it answers "no information").
//!
//! # Example
//! `[3*i + 2*j]` against `[i' + 2*j' - 1]`: the gcd is 1 and divides the
//! difference, but with `i = i'` the pair becomes `[2*i + 2*j]` against
//! `[2*j' - 1]`, which no integer solves: the `=` direction for `i` goes.

use llrm_mir::opcode::IntPredicate;

use super::siv::{abs, add_rec_parts, minus_scev_no_signed_overflow};
use super::{DependenceInfo, FullDependence, Levels, TestType, dir};
use crate::aa::AAResults;
use crate::scev::ap::Ap;
use crate::scev::backedge::ExitCountKind;
use crate::scev::expr::{Kind, Scev, Ty, Use};
use crate::scev::{Flags, ScalarEvolution};

/// `APIntOps::GreatestCommonDivisor`: the unsigned gcd; `gcd(0, b) = b`.
fn greatest_common_divisor(
    a: Ap,
    b: Ap,
) -> Ap {
    let (mut x, mut y) = (a.bits, b.bits);
    while y != 0 {
        let t = x % y;
        x = y;
        y = t;
    }
    Ap::new(a.width, x)
}

/// `getConstantCoefficient`: a constant, or the constant first operand of a
/// product that cannot overflow in a signed sense (`(10 * X * Y)<nsw>` gives
/// 10; without `nsw` the product may overflow and not be a multiple of 10).
fn get_constant_coefficient(
    se: &ScalarEvolution,
    expr: Scev,
) -> Option<Ap> {
    if let Some(constant) = se.as_constant(expr) {
        return Some(constant);
    }
    if let Kind::Mul(operands) = &se.node(expr).kind
        && let Some(constant) = se.as_constant(operands[0].scev)
        && se.node(expr).flags & crate::scev::expr::flags::NSW != 0
    {
        return Some(constant);
    }
    None
}

/// `BanerjeeInterval`: a closed signed interval of the values of part of the
/// Banerjee subscript-difference expression. A missing `lower` is -infinity, a
/// missing `upper` +infinity; both finite and `lower >s upper` is empty.
#[derive(Clone, Copy, Debug)]
struct Interval {
    lower: Option<Scev>,
    upper: Option<Scev>,
}

/// `CoefficientInfo`.
#[derive(Clone, Copy, Debug)]
struct CoefficientInfo {
    src_coeff: Scev,
    dst_coeff: Scev,
    max_iter_index: Option<Scev>,
}

/// `BoundInfo`: the bounds by direction (indexed by the direction bits).
#[derive(Clone, Copy, Debug)]
struct BoundInfo {
    upper: [Option<Scev>; 8],
    lower: [Option<Scev>; 8],
    direction: u8,
    dir_set: u8,
}

impl BoundInfo {
    fn new() -> Self {
        Self { upper: [None; 8], lower: [None; 8], direction: dir::ALL, dir_set: dir::NONE }
    }
}

impl<A: AAResults> DependenceInfo<'_, A> {
    /// `accumulateCoefficientsGCD`: adds the constant coefficients of the loops
    /// other than `cur_loop` in `expr` to the running gcd, and puts
    /// `cur_loop`'s coefficient in `cur_loop_coeff`. Returns the constant
    /// term, or none where a coefficient has no constant factor.
    fn accumulate_coefficients_gcd(
        &self,
        se: &mut ScalarEvolution,
        expr: Scev,
        cur_loop: Option<llrm_mir::module::BlockId>,
        cur_loop_coeff: &mut Scev,
        running_gcd: &mut Ap,
    ) -> Option<Scev> {
        let mut expr = expr;
        loop {
            let Kind::AddRec(_, header) = se.node(expr).kind.clone() else {
                debug_assert!(se.is_loop_invariant(self.cx, expr, cur_loop), "Expected loop invariant expression");
                return Some(expr);
            };
            let (start, step, _) = add_rec_parts(se, self.cx, expr);
            if Some(header) == cur_loop {
                *cur_loop_coeff = step;
            } else {
                // If the coefficient is the product of a constant and other
                // stuff, we can use the constant in the GCD computation.
                let const_coeff = get_constant_coefficient(se, step)?;
                *running_gcd = greatest_common_divisor(*running_gcd, abs(const_coeff));
            }
            expr = start;
        }
    }

    /// `gcdMIVtest` (Wolfe, p. 235): the gcd of the coefficients must divide
    /// the difference of the constants. Symbolic loop-invariant terms are
    /// handled, so `[10*i + 5*N*j + 15*M + 6]` works: the test is "a common
    /// divisor" rather than "the greatest". Can disprove the `=` direction for
    /// one or more loops. True if any possible dependence is disproved.
    pub(crate) fn gcd_miv_test(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        result: &mut FullDependence,
    ) -> bool {
        if !self.is_dependence_test_enabled(TestType::GcdMiv) {
            return false;
        }
        let cx = self.cx;
        let bit_width = se.get_type_size_in_bits(cx, se.type_of(src));
        let mut running_gcd = Ap::zero(bit_width);

        let mut dummy = src;
        let Some(src_const) = self.accumulate_coefficients_gcd(se, src, None, &mut dummy, &mut running_gcd) else {
            return false;
        };
        let Some(dst_const) = self.accumulate_coefficients_gcd(se, dst, None, &mut dummy, &mut running_gcd) else {
            return false;
        };

        let Some(delta) = minus_scev_no_signed_overflow(se, cx, dst_const, src_const) else { return false };
        let Some(const_delta) = se.as_constant(delta) else { return false };
        if const_delta.is_zero() {
            return false;
        }
        if running_gcd.is_zero() {
            return false;
        }
        if !const_delta.srem(running_gcd).is_zero() {
            return true;
        }

        // Try to disprove equal directions. For example, given a subscript
        // pair [3*i + 2*j] and [i' + 2*j' - 1], the code above can't disprove
        // the dependence because the GCD = 1. So we consider what happen if i
        // = i' and what happens if j = j'. If i = i', we can simplify the
        // subscript to [2*i + 2*j] and [2*j' - 1], which is infeasible, so we
        // can disallow the = direction for the i level. Setting j = j' doesn't
        // help matters, so we end up with a direction vector of [<>, *].
        let mut coefficients = src;
        while let Kind::AddRec(_, cur_loop) = se.node(coefficients).kind.clone() {
            let (start, step, _) = add_rec_parts(se, cx, coefficients);
            coefficients = start;
            running_gcd = Ap::zero(bit_width);
            let mut src_coeff = step;
            let mut dst_coeff = se.get_zero(cx, se.type_of(src_coeff));

            if self.accumulate_coefficients_gcd(se, src, Some(cur_loop), &mut src_coeff, &mut running_gcd).is_none()
                || self.accumulate_coefficients_gcd(se, dst, Some(cur_loop), &mut dst_coeff, &mut running_gcd).is_none()
            {
                return false;
            }

            let Some(delta) = minus_scev_no_signed_overflow(se, cx, dst_coeff, src_coeff) else { continue };
            // If the coefficient is the product of a constant and other stuff,
            // we can use the constant in the GCD computation.
            let Some(const_coeff) = get_constant_coefficient(se, delta) else {
                // The difference of the two coefficients might not be a
                // product or constant, in which case we give up on this
                // direction.
                continue;
            };
            running_gcd = greatest_common_divisor(running_gcd, abs(const_coeff));
            if !running_gcd.is_zero() && !const_delta.srem(running_gcd).is_zero() {
                let level = self.map_src_loop(cur_loop);
                result.dv[(level - 1) as usize].direction &= !dir::EQ;
            }
        }
        false
    }

    // ---- Banerjee ---------------------------------------------------------

    /// `BanerjeeInterval::isEmpty`.
    fn interval_is_empty(
        &self,
        se: &mut ScalarEvolution,
        interval: Interval,
    ) -> bool {
        match (interval.lower, interval.upper) {
            (Some(lower), Some(upper)) => se.is_known_predicate_scev(self.cx, IntPredicate::Sgt, lower, upper),
            _ => false,
        }
    }

    /// `addIntervals`: a missing endpoint propagates the infinity.
    fn add_intervals(
        &self,
        se: &mut ScalarEvolution,
        a: Interval,
        b: Interval,
    ) -> Interval {
        let add = |se: &mut ScalarEvolution, x: Option<Scev>, y: Option<Scev>| match (x, y) {
            (Some(x), Some(y)) => {
                Some(se.get_add_expr(self.cx, vec![Use::of(x), Use::of(y)], Flags::default(), 0).scev)
            }
            _ => None,
        };
        Interval { lower: add(se, a.lower, b.lower), upper: add(se, a.upper, b.upper) }
    }

    /// `intersectIntervals`: both inputs contain the feasible values, so their
    /// intersection does too and may give tighter one-sided bounds.
    fn intersect_intervals(
        &self,
        se: &mut ScalarEvolution,
        a: Interval,
        b: Interval,
    ) -> Interval {
        let mut lower = a.lower;
        let mut upper = a.upper;
        if let Some(b_lower) = b.lower {
            lower = Some(match lower {
                Some(lower) => se.get_smax_expr(self.cx, vec![Use::of(lower), Use::of(b_lower)]),
                None => b_lower,
            });
        }
        if let Some(b_upper) = b.upper {
            upper = Some(match upper {
                Some(upper) => se.get_smin_expr(self.cx, vec![Use::of(upper), Use::of(b_upper)]),
                None => b_upper,
            });
        }
        Interval { lower, upper }
    }

    /// `constantInterval`.
    fn constant_interval(c: Scev) -> Interval {
        Interval { lower: Some(c), upper: Some(c) }
    }

    /// `emptyInterval`: the canonical empty interval [1, 0].
    fn empty_interval(
        &self,
        se: &mut ScalarEvolution,
        ty: Ty,
    ) -> Interval {
        Interval { lower: Some(se.get_one(self.cx, ty)), upper: Some(se.get_zero(self.cx, ty)) }
    }

    /// `signedRangeInterval`: the range of `Coeff * X` for `Lower <=s X <=s
    /// Upper`. The width proof in `banerjee_miv_test` guarantees the
    /// multiplications do not wrap.
    fn signed_range_interval(
        &self,
        se: &mut ScalarEvolution,
        coeff: Scev,
        lower: Scev,
        upper: Scev,
    ) -> Interval {
        let cx = self.cx;
        let lower_value = se.get_mul_expr(cx, vec![Use::of(coeff), Use::of(lower)], Flags::default(), 0).scev;
        let upper_value = se.get_mul_expr(cx, vec![Use::of(coeff), Use::of(upper)], Flags::default(), 0).scev;
        Interval {
            lower: Some(se.get_smin_expr(cx, vec![Use::of(lower_value), Use::of(upper_value)])),
            upper: Some(se.get_smax_expr(cx, vec![Use::of(lower_value), Use::of(upper_value)])),
        }
    }

    /// `variableInterval`: the range of `Coeff * X` for `0 <= X <= Upper`; a
    /// missing `upper` is an unbounded non-negative `X`.
    fn variable_interval(
        &self,
        se: &mut ScalarEvolution,
        coeff: Scev,
        upper: Option<Scev>,
    ) -> Interval {
        let cx = self.cx;
        let zero = se.get_zero(cx, se.type_of(coeff));
        if se.is_zero_scev(coeff) {
            return Self::constant_interval(zero);
        }
        let Some(upper) = upper else {
            if se.is_known_negative(cx, coeff) {
                return Interval { lower: None, upper: Some(zero) };
            }
            if se.is_known_non_negative(cx, coeff) {
                return Interval { lower: Some(zero), upper: None };
            }
            return Interval { lower: None, upper: None };
        };
        self.signed_range_interval(se, coeff, zero, upper)
    }

    /// `strictDirectionIntervalWithUnknownUpperBound`: any finite one-sided
    /// bound on `A * SrcIndex - B * DstIndex` for a strict direction when no
    /// upper bound is known for the non-negative normalized indices. For
    /// `SrcIndex < DstIndex`, `DstIndex = SrcIndex + D` with `D >= 1` gives `(A
    /// - B) * SrcIndex + (-B) * D`: bounded below by `-B` if `A - B >= 0` and
    /// `B <= 0`, above by `-B` if `A - B <= 0` and `B >= 0`. For `SrcIndex >
    /// DstIndex`, `SrcIndex = DstIndex + D` gives `(A - B) * DstIndex + A * D`:
    /// below by `A` if `A - B >= 0` and `A >= 0`, above by `A` if `A - B <= 0`
    /// and `A <= 0`.
    fn strict_direction_interval_with_unknown_upper_bound(
        &self,
        se: &mut ScalarEvolution,
        a_coeff: Scev,
        b_coeff: Scev,
        direction: u8,
    ) -> Interval {
        let cx = self.cx;
        let delta_coeff = se.get_minus_scev(cx, Use::of(a_coeff), Use::of(b_coeff), 0, 0);
        match direction {
            dir::LT => {
                let boundary = se.get_negative_scev(cx, b_coeff, 0);
                let mut lower = None;
                let mut upper = None;
                if se.is_known_non_negative(cx, delta_coeff) && se.is_known_non_positive(cx, b_coeff) {
                    lower = Some(boundary);
                }
                if se.is_known_non_positive(cx, delta_coeff) && se.is_known_non_negative(cx, b_coeff) {
                    upper = Some(boundary);
                }
                Interval { lower, upper }
            }
            dir::GT => {
                let mut lower = None;
                let mut upper = None;
                if se.is_known_non_negative(cx, a_coeff) && se.is_known_non_negative(cx, delta_coeff) {
                    lower = Some(a_coeff);
                }
                if se.is_known_non_positive(cx, a_coeff) && se.is_known_non_positive(cx, delta_coeff) {
                    upper = Some(a_coeff);
                }
                Interval { lower, upper }
            }
            _ => unreachable!("unexpected direction"),
        }
    }

    /// `intervalFromValues`: the smallest closed interval holding all `values`.
    fn interval_from_values(
        &self,
        se: &mut ScalarEvolution,
        values: &[Scev],
    ) -> Interval {
        let mut lower = values[0];
        let mut upper = values[0];
        for &value in &values[1..] {
            lower = se.get_smin_expr(self.cx, vec![Use::of(lower), Use::of(value)]);
            upper = se.get_smax_expr(self.cx, vec![Use::of(upper), Use::of(value)]);
        }
        Interval { lower: Some(lower), upper: Some(upper) }
    }

    /// `evaluateSubscriptDifference`: one loop level's contribution `A *
    /// SrcIndex - B * DstIndex` to the source-minus-destination difference.
    fn evaluate_subscript_difference(
        &self,
        se: &mut ScalarEvolution,
        a: Scev,
        src_index: Scev,
        b: Scev,
        dst_index: Scev,
    ) -> Scev {
        let cx = self.cx;
        let left = se.get_mul_expr(cx, vec![Use::of(a), Use::of(src_index)], Flags::default(), 0).scev;
        let right = se.get_mul_expr(cx, vec![Use::of(b), Use::of(dst_index)], Flags::default(), 0).scev;
        se.get_minus_scev(cx, Use::of(left), Use::of(right), 0, 0)
    }

    /// `getBanerjeeBaseType`: the widest type of a subscript or of an exact
    /// backedge-taken count of one of its recurrences.
    fn get_banerjee_base_type(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
    ) -> Ty {
        let cx = self.cx;
        let mut base_type = se.get_wider_type(cx, se.type_of(src), se.type_of(dst));
        for mut subscript in [src, dst] {
            while let Kind::AddRec(_, header) = se.node(subscript).kind.clone() {
                let max_iter_index = se.get_backedge_taken_count(cx, header, ExitCountKind::Exact);
                if !matches!(se.node(max_iter_index).kind, Kind::CouldNotCompute) {
                    base_type = se.get_wider_type(cx, base_type, se.type_of(max_iter_index));
                }
                subscript = se.addrec_start(subscript);
            }
        }
        base_type
    }

    /// `banerjeeMIVtest` (Wolfe's extreme value test, section 2.5.2): the
    /// formulae simplified for normalized loops (`L_k = 0`, `N_k = 1`) are
    /// evaluated at the vertices of the constraint polytope of each direction
    /// (e.g. `(0, 1)`, `(0, U)`, `(U - 1, U)` for `<`), in a type wide enough
    /// that no intermediate wraps. Loop bounds are backedge-taken counts; a
    /// single-iteration loop has bound 0, making `<` and `>` impossible.
    /// Unknown endpoints are infinities. True if disproved.
    pub(crate) fn banerjee_miv_test(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        loops: &Levels,
        result: &mut FullDependence,
    ) -> bool {
        if !self.is_dependence_test_enabled(TestType::BanerjeeMiv) {
            return false;
        }
        let cx = self.cx;
        let base_type = self.get_banerjee_base_type(se, src, dst);
        let base_bits = se.get_type_size_in_bits(cx, base_type);
        // Let B be the maximum bit width among the source and destination
        // subscripts and the exact backedge-taken counts of the loops
        // appearing in their recurrences, L = MaxLevels. |C*I| < 2^(2B-1);
        // one level's contribution |A*I-B*J| < 2^(2B); the accumulated bound
        // across L levels is less than 2^(2B+L); one more bit holds the sign.
        let wide_bits = 2 * base_bits + self.max_levels + 1;
        // D1: an `Ap` has at most 128 bits.
        if wide_bits > 128 {
            return false;
        }
        let wide_type = Ty::Int(wide_bits);
        let zero = se.get_zero(cx, wide_type);

        let empty = CoefficientInfo { src_coeff: zero, dst_coeff: zero, max_iter_index: None };
        let mut ci = vec![empty; self.max_levels as usize + 1];
        debug_assert!(loops.len() > self.max_levels as usize, "loop bit vector is too small");
        debug_assert!(result.levels >= self.common_levels, "direction vector is too small for common levels");

        let a0 = self.collect_coeff_info(se, src, true, wide_type, &mut ci);
        let b0 = self.collect_coeff_info(se, dst, false, wide_type, &mut ci);
        let delta = se.get_minus_scev(cx, Use::of(b0), Use::of(a0), 0, 0);

        let mut bound = vec![BoundInfo::new(); self.max_levels as usize + 1];
        for k in 1..=self.max_levels as usize {
            self.find_bounds_all(se, &ci, &mut bound, k);
            self.find_bounds_lt(se, &ci, &mut bound, k);
            self.find_bounds_eq(se, &ci, &mut bound, k);
            self.find_bounds_gt(se, &ci, &mut bound, k);
        }

        if !self.test_bounds(se, dir::ALL, 0, &mut bound, delta) {
            return true;
        }

        let new_deps = self.explore_directions(se, 1, &mut bound, loops, delta, result);
        if new_deps == 0 {
            return true;
        }

        for k in 1..=self.common_levels {
            if !loops.get(k) {
                continue;
            }
            let old = result.dv[(k - 1) as usize].direction;
            result.dv[(k - 1) as usize].direction = old & bound[k as usize].dir_set;
            if result.dv[(k - 1) as usize].direction == 0 {
                return true;
            }
        }
        false
    }

    /// `collectCoeffInfo`: the coefficient of `subscript` at each loop level.
    /// Each level has one maximum iteration index shared by the source and
    /// destination coefficients. Every value and the returned constant term
    /// are extended to the wide analysis type.
    fn collect_coeff_info(
        &self,
        se: &mut ScalarEvolution,
        subscript: Scev,
        src_flag: bool,
        wide_type: Ty,
        ci: &mut [CoefficientInfo],
    ) -> Scev {
        let cx = self.cx;
        let mut subscript = subscript;
        let mut seen_levels = Levels::new(self.max_levels as usize + 1);
        while let Kind::AddRec(_, header) = se.node(subscript).kind.clone() {
            let k = if src_flag { self.map_src_loop(header) } else { self.map_dst_loop(header) };
            debug_assert!(k > 0 && k <= self.max_levels, "invalid mapped loop level");
            debug_assert!(!seen_levels.get(k), "duplicate recurrence for loop level");
            seen_levels.set(k);

            let (start, step, _) = add_rec_parts(se, cx, subscript);
            let coeff = se.get_noop_or_sign_extend(cx, step, wide_type);
            if src_flag {
                ci[k as usize].src_coeff = coeff;
            } else {
                ci[k as usize].dst_coeff = coeff;
            }

            let max_iter_index = se.get_backedge_taken_count(cx, header, ExitCountKind::Exact);
            if !matches!(se.node(max_iter_index).kind, Kind::CouldNotCompute) {
                // A backedge-taken count is semantically an unsigned,
                // non-negative iteration index. When its signed
                // non-negativity is also known, sign extension preserves more
                // symbolic relationships. Otherwise use zero extension.
                ci[k as usize].max_iter_index = Some(if se.is_known_non_negative(cx, max_iter_index) {
                    se.get_noop_or_sign_extend(cx, max_iter_index, wide_type)
                } else {
                    se.get_noop_or_zero_extend(cx, max_iter_index, wide_type)
                });
            }

            subscript = start;
        }
        se.get_noop_or_sign_extend(cx, subscript, wide_type)
    }

    /// `getLowerBound`: the sum of the selected lower bounds, none where one
    /// is missing.
    fn get_lower_bound(
        &self,
        se: &mut ScalarEvolution,
        bound: &[BoundInfo],
    ) -> Option<Scev> {
        let mut sum = bound[1].lower[bound[1].direction as usize];
        let mut k = 2;
        while sum.is_some() && k <= self.max_levels as usize {
            let next = bound[k].lower[bound[k].direction as usize]?;
            sum = Some(se.get_add_expr(self.cx, vec![Use::of(sum?), Use::of(next)], Flags::default(), 0).scev);
            k += 1;
        }
        sum
    }

    /// `getUpperBound`.
    fn get_upper_bound(
        &self,
        se: &mut ScalarEvolution,
        bound: &[BoundInfo],
    ) -> Option<Scev> {
        let mut sum = bound[1].upper[bound[1].direction as usize];
        let mut k = 2;
        while sum.is_some() && k <= self.max_levels as usize {
            let next = bound[k].upper[bound[k].direction as usize]?;
            sum = Some(se.get_add_expr(self.cx, vec![Use::of(sum?), Use::of(next)], Flags::default(), 0).scev);
            k += 1;
        }
        sum
    }

    /// `testBounds`: false when the selected bounds are proven infeasible, true
    /// when they may be feasible or ScalarEvolution cannot decide.
    fn test_bounds(
        &self,
        se: &mut ScalarEvolution,
        dir_kind: u8,
        level: usize,
        bound: &mut [BoundInfo],
        delta: Scev,
    ) -> bool {
        bound[level].direction = dir_kind;
        for k in 1..=self.max_levels as usize {
            let direction = bound[k].direction as usize;
            let interval = Interval { lower: bound[k].lower[direction], upper: bound[k].upper[direction] };
            if self.interval_is_empty(se, interval) {
                return false;
            }
        }
        if let Some(lower) = self.get_lower_bound(se, bound)
            && se.is_known_predicate_scev(self.cx, IntPredicate::Sgt, lower, delta)
        {
            return false;
        }
        if let Some(upper) = self.get_upper_bound(se, bound)
            && se.is_known_predicate_scev(self.cx, IntPredicate::Sgt, delta, upper)
        {
            return false;
        }
        true
    }

    /// `exploreDirections`: hierarchically expands the direction-vector search
    /// space.
    fn explore_directions(
        &self,
        se: &mut ScalarEvolution,
        level: u32,
        bound: &mut [BoundInfo],
        loops: &Levels,
        delta: Scev,
        result: &FullDependence,
    ) -> u32 {
        if self.common_levels > self.options.miv_max_level_threshold {
            // Number of common levels exceeded the threshold: MIV direction
            // exploration is terminated.
            for k in 1..=self.common_levels {
                if loops.get(k) {
                    bound[k as usize].dir_set = dir::ALL;
                }
            }
            return 1;
        }

        if level > self.common_levels {
            for k in 1..=self.common_levels {
                if loops.get(k) {
                    bound[k as usize].dir_set |= bound[k as usize].direction;
                }
            }
            return 1;
        }

        if !loops.get(level) {
            return self.explore_directions(se, level + 1, bound, loops, delta, result);
        }

        let mut new_deps = 0;
        let old_directions = result.dv[(level - 1) as usize].direction;
        for direction in [dir::LT, dir::EQ, dir::GT] {
            if old_directions & direction == 0 {
                continue;
            }
            if self.test_bounds(se, direction, level as usize, bound, delta) {
                new_deps += self.explore_directions(se, level + 1, bound, loops, delta, result);
            }
        }
        bound[level as usize].direction = dir::ALL;
        new_deps
    }

    /// `findBoundsALL`: the bounds of level `k` for `*`. The contribution is
    /// `F_k(i, j) = A_k i - B_k j`, unrelated `i`, `j`: the feasible domain is
    /// the rectangle `[0, U_A] x [0, U_B]`, whose extrema are its corners.
    fn find_bounds_all(
        &self,
        se: &mut ScalarEvolution,
        ci: &[CoefficientInfo],
        bound: &mut [BoundInfo],
        k: usize,
    ) {
        let src_interval = self.variable_interval(se, ci[k].src_coeff, ci[k].max_iter_index);
        let negated = se.get_negative_scev(self.cx, ci[k].dst_coeff, 0);
        let dst_interval = self.variable_interval(se, negated, ci[k].max_iter_index);
        let interval = self.add_intervals(se, src_interval, dst_interval);
        bound[k].lower[dir::ALL as usize] = interval.lower;
        bound[k].upper[dir::ALL as usize] = interval.upper;
    }

    /// `findBoundsEQ`: `i = j`, so `F_k(i, i) = (A_k - B_k) i` over `[0,
    /// min(U_A, U_B)]`; the extrema are the two endpoints.
    fn find_bounds_eq(
        &self,
        se: &mut ScalarEvolution,
        ci: &[CoefficientInfo],
        bound: &mut [BoundInfo],
        k: usize,
    ) {
        let difference = se.get_minus_scev(self.cx, Use::of(ci[k].src_coeff), Use::of(ci[k].dst_coeff), 0, 0);
        let interval = self.variable_interval(se, difference, ci[k].max_iter_index);
        bound[k].lower[dir::EQ as usize] = interval.lower;
        bound[k].upper[dir::EQ as usize] = interval.upper;
    }

    /// `findBoundsLT`: for a known common upper bound `U`, the feasible domain
    /// is `0 <= i < j <= U`, vertices `(0, 1)`, `(0, U)`, `(U - 1, U)`. If `U`
    /// is zero the domain is empty; with no known bound, any finite one-sided
    /// bound that can be proved.
    fn find_bounds_lt(
        &self,
        se: &mut ScalarEvolution,
        ci: &[CoefficientInfo],
        bound: &mut [BoundInfo],
        k: usize,
    ) {
        let cx = self.cx;
        let (a_coeff, b_coeff, max_iter_index) = (ci[k].src_coeff, ci[k].dst_coeff, ci[k].max_iter_index);
        let mut interval = self.strict_direction_interval_with_unknown_upper_bound(se, a_coeff, b_coeff, dir::LT);
        if let Some(max_iter_index) = max_iter_index {
            let ty = se.type_of(a_coeff);
            if se.is_zero_scev(max_iter_index) {
                interval = self.empty_interval(se, ty);
            } else {
                let zero = se.get_zero(cx, ty);
                let one = se.get_one(cx, ty);
                let max_minus_one = se.get_minus_scev(cx, Use::of(max_iter_index), Use::of(one), 0, 0);
                let values = [
                    self.evaluate_subscript_difference(se, a_coeff, zero, b_coeff, one),
                    self.evaluate_subscript_difference(se, a_coeff, zero, b_coeff, max_iter_index),
                    self.evaluate_subscript_difference(se, a_coeff, max_minus_one, b_coeff, max_iter_index),
                ];
                let from_values = self.interval_from_values(se, &values);
                interval = self.intersect_intervals(se, interval, from_values);
            }
        }
        bound[k].lower[dir::LT as usize] = interval.lower;
        bound[k].upper[dir::LT as usize] = interval.upper;
    }

    /// `findBoundsGT`: `0 <= j < i <= U`, vertices `(1, 0)`, `(U, 0)`, `(U, U -
    /// 1)`.
    fn find_bounds_gt(
        &self,
        se: &mut ScalarEvolution,
        ci: &[CoefficientInfo],
        bound: &mut [BoundInfo],
        k: usize,
    ) {
        let cx = self.cx;
        let (a_coeff, b_coeff, max_iter_index) = (ci[k].src_coeff, ci[k].dst_coeff, ci[k].max_iter_index);
        let mut interval = self.strict_direction_interval_with_unknown_upper_bound(se, a_coeff, b_coeff, dir::GT);
        if let Some(max_iter_index) = max_iter_index {
            let ty = se.type_of(a_coeff);
            if se.is_zero_scev(max_iter_index) {
                interval = self.empty_interval(se, ty);
            } else {
                let zero = se.get_zero(cx, ty);
                let one = se.get_one(cx, ty);
                let max_minus_one = se.get_minus_scev(cx, Use::of(max_iter_index), Use::of(one), 0, 0);
                let values = [
                    self.evaluate_subscript_difference(se, a_coeff, one, b_coeff, zero),
                    self.evaluate_subscript_difference(se, a_coeff, max_iter_index, b_coeff, zero),
                    self.evaluate_subscript_difference(se, a_coeff, max_iter_index, b_coeff, max_minus_one),
                ];
                let from_values = self.interval_from_values(se, &values);
                interval = self.intersect_intervals(se, interval, from_values);
            }
        }
        bound[k].lower[dir::GT as usize] = interval.lower;
        bound[k].upper[dir::GT as usize] = interval.upper;
    }

    /// `testMIV`: GCD, then Banerjee; true if the dependence is disproved.
    pub(crate) fn test_miv(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        loops: &Levels,
        result: &mut FullDependence,
    ) -> bool {
        self.gcd_miv_test(se, src, dst, result) || self.banerjee_miv_test(se, src, dst, loops, result)
    }
}
