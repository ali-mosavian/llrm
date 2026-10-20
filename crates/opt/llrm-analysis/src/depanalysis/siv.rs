//! The single-index-variable tests: ZIV, strong, weak-crossing, weak-zero
//! and exact SIV, and the exact RDIV test.
//!
//! # How it works
//! Each test takes the two subscripts of one dimension, in the form the
//! classification fixed (`c1 + a1*i` and `c2 + a2*i`), and either disproves a
//! dependence (returns true) or narrows `result`: a distance, a direction at
//! the level of the loop, a runtime assumption. See the comment on each for
//! the equation it solves. `exact_test_impl` is the one routine behind the
//! exact SIV and the exact RDIV tests (Banerjee, "Dependence Analysis for
//! Supercomputing", Algorithm 6.2.1, case 2.5).
//!
//! # Example
//! `a[i + 2]` against `a[i]`: strong SIV, `Delta = 2`, `Coeff = 1`, distance 2,
//! direction `<`.

use llrm_mir::opcode::{BinaryOp, IntPredicate};

use super::overflow::Safe;
use super::{ComparePredicate, DependenceInfo, FullDependence, TestType, dir};
use crate::aa::AAResults;
use crate::scev::ap::Ap;
use crate::scev::backedge::ExitCountKind;
use crate::scev::expr::{Kind, Scev, Ty, Use};
use crate::scev::{Cx, ScalarEvolution};

/// `A >s B`.
pub(crate) fn sgt(
    a: Ap,
    b: Ap,
) -> bool {
    b.slt(a)
}

/// `A >=s B`.
pub(crate) fn sge(
    a: Ap,
    b: Ap,
) -> bool {
    b.sle(a)
}

/// `APInt::abs`.
pub(crate) fn abs(a: Ap) -> Ap {
    if a.is_negative() { a.neg() } else { a }
}

/// `(start, step, header)` of an affine add recurrence.
pub(crate) fn add_rec_parts(
    se: &mut ScalarEvolution,
    cx: &Cx,
    add_rec: Scev,
) -> (Scev, Scev, llrm_mir::module::BlockId) {
    let Kind::AddRec(operands, header) = se.node(add_rec).kind.clone() else { unreachable!("an add recurrence") };
    let step = se.get_step_recurrence(cx, add_rec);
    (operands[0].scev, step, header)
}

/// `minusSCEVNoSignedOverflow`: `a - b` if it cannot wrap in a signed sense.
pub(crate) fn minus_scev_no_signed_overflow(
    se: &mut ScalarEvolution,
    cx: &Cx,
    a: Scev,
    b: Scev,
) -> Option<Scev> {
    if se.will_not_overflow(cx, BinaryOp::Sub, true, a, b, None) {
        return Some(se.get_minus_scev(cx, Use::of(a), Use::of(b), 0, 0));
    }
    None
}

/// `findGCD` (Kirch's algorithm, Wolfe 1989, program 2.1): the gcd of `am`
/// and `bm` and a solution of `a x - b y = gcd(a, b)`. `Some(true)` if the
/// gcd does not divide `delta`, so there is no dependence; `None` if the
/// computation cannot proceed.
pub(crate) fn find_gcd(
    bits: u32,
    am: Ap,
    bm: Ap,
    delta: Ap,
) -> Option<(bool, Ap, Ap, Ap)> {
    let (mut a0, mut a1) = (Ap::one(bits), Ap::zero(bits));
    let (mut b0, mut b1) = (Ap::zero(bits), Ap::one(bits));

    // APInt::abs will overflow. In that case, bail out early.
    if am.is_signed_min() || bm.is_signed_min() {
        return None;
    }

    let mut g0 = abs(am);
    let mut g1 = abs(bm);
    if g1.is_zero() {
        return None;
    }
    let mut q = g0.sdiv(g1);
    let mut r = g0.srem(g1);
    while !r.is_zero() {
        let a2 = a0.sub(q.mul(a1));
        a0 = a1;
        a1 = a2;
        let b2 = b0.sub(q.mul(b1));
        b0 = b1;
        b1 = b2;
        g0 = g1;
        g1 = r;
        q = g0.sdiv(g1);
        r = g0.srem(g1);
    }
    let g = g1;
    let x = if am.is_negative() { a1.neg() } else { a1 };
    let y = if bm.is_negative() { b1 } else { b1.neg() };

    // Make sure gcd divides Delta.
    if !delta.srem(g).is_zero() {
        return Some((true, g, x, y)); // gcd doesn't divide Delta, no dependence
    }
    Some((false, g, x, y))
}

/// `floorOfQuotient`.
fn floor_of_quotient(
    oa: Safe,
    ob: Safe,
) -> Safe {
    if !oa.is_valid() || !ob.is_valid() {
        return Safe::INVALID;
    }
    let (a, b) = (oa.value(), ob.value());
    if b.is_zero() {
        return Safe::INVALID;
    }
    let q = a.sdiv(b);
    let r = a.srem(b);
    if r.is_zero() {
        return Safe::of(q);
    }
    let zero = Ap::zero(a.width);
    if (sgt(a, zero) && sgt(b, zero)) || (a.slt(zero) && b.slt(zero)) {
        return Safe::of(q);
    }
    Safe::of(q).sub_int(1)
}

/// `ceilingOfQuotient`.
fn ceiling_of_quotient(
    oa: Safe,
    ob: Safe,
) -> Safe {
    if !oa.is_valid() || !ob.is_valid() {
        return Safe::INVALID;
    }
    let (a, b) = (oa.value(), ob.value());
    if b.is_zero() {
        return Safe::INVALID;
    }
    let q = a.sdiv(b);
    let r = a.srem(b);
    if r.is_zero() {
        return Safe::of(q);
    }
    let zero = Ap::zero(a.width);
    if (sgt(a, zero) && sgt(b, zero)) || (a.slt(zero) && b.slt(zero)) {
        return Safe::of(q).add_int(1);
    }
    Safe::of(q)
}

/// `inferDomainOfAffine`: given an affine expression `A*k + B` for an
/// arbitrary integer `k`, the possible range of `k` from the known range of
/// the expression, `0 <= A*k + B <= UB`. For positive `A`: `k >= ceil(-B / A)`
/// and `k <= floor((UB - B) / A)`; the signs flip for negative `A`.
///
/// Preconditions: `A` is non-zero, and `A*k + B` and `UB` are non-negative.
fn infer_domain_of_affine(
    a: Safe,
    b: Safe,
    ub: Safe,
) -> (Safe, Safe) {
    debug_assert!(a.is_valid() && b.is_valid(), "A and B must be available");
    debug_assert!(!a.value().is_zero(), "A must be non-zero");
    let (tl, tu);
    if sgt(a.value(), Ap::zero(a.value().width)) {
        tl = ceiling_of_quotient(b.neg(), a);
        // New bound check - modification to Banerjee's e3 check.
        tu = floor_of_quotient(ub.sub(b), a);
    } else {
        tu = floor_of_quotient(b.neg(), a);
        // New bound check - modification to Banerjee's e3 check.
        tl = ceiling_of_quotient(ub.sub(b), a);
    }
    (tl, tu)
}

/// `isRemainderZero`: whether the divisor evenly divides the dividend.
fn is_remainder_zero(
    dividend: Ap,
    divisor: Ap,
) -> bool {
    !divisor.is_zero() && dividend.srem(divisor).is_zero()
}

impl<A: AAResults> DependenceInfo<'_, A> {
    /// `isDependenceTestEnabled`.
    pub(crate) fn is_dependence_test_enabled(
        &self,
        test: TestType,
    ) -> bool {
        if self.options.enable_test == TestType::All {
            return true;
        }
        // The Banerjee test is disabled by default because of correctness
        // issues, but can be enabled with -da-enable-dependence-test.
        if self.options.enable_test == TestType::Default {
            return test != TestType::BanerjeeMiv;
        }
        self.options.enable_test == test
    }

    /// `collectUpperBound`: the loop's backedge-taken count in `ty`, none
    /// where it is not loop-invariant.
    pub(crate) fn collect_upper_bound(
        &self,
        se: &mut ScalarEvolution,
        header: llrm_mir::module::BlockId,
        ty: Ty,
    ) -> Option<Scev> {
        if se.has_loop_invariant_backedge_taken_count(self.cx, header) {
            let ub = se.get_backedge_taken_count(self.cx, header, ExitCountKind::Exact);
            return Some(se.get_truncate_or_zero_extend(self.cx, ub, ty, 0));
        }
        None
    }

    /// `collectNonNegativeConstantUpperBound`.
    pub(crate) fn collect_non_negative_constant_upper_bound(
        &self,
        se: &mut ScalarEvolution,
        header: llrm_mir::module::BlockId,
        ty: Ty,
    ) -> Option<Ap> {
        let ub = self.collect_upper_bound(se, header, ty)?;
        let constant = se.as_constant(ub)?;
        if constant.is_negative() { None } else { Some(constant) }
    }

    /// `testZIV`: a pair of loop-invariant subscripts `[c1]` and `[c2]` is
    /// provably dependent when equal, provably independent when different, and
    /// assumed dependent otherwise. True if the dependence is disproved.
    pub(crate) fn test_ziv(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
    ) -> bool {
        if se.is_known_predicate_scev(self.cx, IntPredicate::Eq, src, dst) {
            return false; // provably dependent
        }
        if se.is_known_predicate_scev(self.cx, IntPredicate::Ne, src, dst) {
            return true; // provably independent
        }
        false // possibly dependent
    }

    /// `strongSIVtest` (Practical Dependence Testing, section 4.2.1): `[c1 +
    /// a*i]` against `[c2 + a*i]`. A dependence needs `d = (c1 - c2)/a` to be
    /// an integer with `abs(d) <= U`; `<` if `d > 0`, `=` if `d = 0`, `>` if
    /// `d < 0`. True if the dependence is disproved.
    pub(crate) fn strong_siv_test(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        level: u32,
        result: &mut FullDependence,
        under_runtime_assumptions: bool,
    ) -> bool {
        if !self.is_dependence_test_enabled(TestType::StrongSiv) {
            return false;
        }
        let cx = self.cx;
        let (src_const, coeff, _) = add_rec_parts(se, cx, src);
        let (dst_const, dst_coeff, _) = add_rec_parts(se, cx, dst);
        debug_assert_eq!(coeff, dst_coeff, "Expecting same coefficient in Strong SIV test");
        debug_assert!(0 < level && level <= self.common_levels, "level out of range");
        let level = (level - 1) as usize;

        let Some(delta) = minus_scev_no_signed_overflow(se, cx, src_const, dst_const) else { return false };

        // Can we compute distance?
        if let (Some(const_delta), Some(const_coeff)) = (se.as_constant(delta), se.as_constant(coeff)) {
            if const_coeff.is_zero() {
                return false;
            }
            let distance = const_delta.sdiv(const_coeff);
            let remainder = const_delta.srem(const_coeff);
            // Make sure Coeff divides Delta exactly.
            if !remainder.is_zero() {
                // Coeff doesn't divide Distance, no dependence.
                return true;
            }
            result.dv[level].distance = Some(se.get_constant(distance));
            let zero = Ap::zero(distance.width);
            if sgt(distance, zero) {
                result.dv[level].direction &= dir::LT;
            } else if distance.slt(zero) {
                result.dv[level].direction &= dir::GT;
            } else {
                result.dv[level].direction &= dir::EQ;
            }
        } else if se.is_zero_scev(delta) {
            // Check if coefficient could be zero. If so, 0/0 is undefined and
            // we cannot conclude that only same-iteration dependencies exist.
            // When coeff=0, all iterations access the same location.
            if !se.is_known_non_zero(cx, coeff) {
                // Cannot prove at compile time, would need runtime assumption.
                if under_runtime_assumptions {
                    let zero = se.get_zero(cx, se.type_of(coeff));
                    result.assumptions.add(ComparePredicate { pred: IntPredicate::Ne, lhs: coeff, rhs: zero });
                } else {
                    // Cannot add runtime assumptions, this test cannot handle
                    // this case. Let more complex tests try.
                    return false;
                }
            }
            // Since 0/X == 0 (where X is known non-zero or assumed non-zero).
            result.dv[level].distance = Some(delta);
            result.dv[level].direction &= dir::EQ;
        } else {
            if se.is_one_scev(coeff) {
                result.dv[level].distance = Some(delta); // since X/1 == X
            }

            // Maybe we can get a useful direction. "MaybeZero" reads
            // !isKnownNonZero.
            let delta_maybe_zero = !se.is_known_non_zero(cx, delta);
            let delta_maybe_positive = !se.is_known_non_positive(cx, delta);
            let delta_maybe_negative = !se.is_known_non_negative(cx, delta);
            let coeff_maybe_positive = !se.is_known_non_positive(cx, coeff);
            let coeff_maybe_negative = !se.is_known_non_negative(cx, coeff);
            let mut new_direction = dir::NONE;
            if (delta_maybe_positive && coeff_maybe_positive) || (delta_maybe_negative && coeff_maybe_negative) {
                new_direction = dir::LT;
            }
            if delta_maybe_zero {
                new_direction |= dir::EQ;
            }
            if (delta_maybe_negative && coeff_maybe_positive) || (delta_maybe_positive && coeff_maybe_negative) {
                new_direction |= dir::GT;
            }
            result.dv[level].direction &= new_direction;
        }
        false
    }

    /// `weakCrossingSIVtest` (section 4.2.2): `[c1 + a*i]` against `[c2 -
    /// a*i]`, solved at `i = (c2 - c1)/2a`: no dependence if that is below 0
    /// or above the upper bound, distance 0 at the ends, all directions if it
    /// is integral, `<>` if the fraction is a half. True if disproved.
    pub(crate) fn weak_crossing_siv_test(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        level: u32,
        result: &mut FullDependence,
    ) -> bool {
        if !self.is_dependence_test_enabled(TestType::WeakCrossingSiv) {
            return false;
        }
        let cx = self.cx;
        let (src_const, coeff, _) = add_rec_parts(se, cx, src);
        let (dst_const, _, _) = add_rec_parts(se, cx, dst);
        debug_assert!(0 < level && level <= self.common_levels, "Level out of range");
        let level = (level - 1) as usize;
        let Some(delta) = minus_scev_no_signed_overflow(se, cx, dst_const, src_const) else { return false };

        let Some(const_coeff) = se.as_constant(coeff) else { return false };
        let Some(const_delta) = se.as_constant(delta) else { return false };

        let src_range = se.get_signed_range(cx, src);
        let dst_range = se.get_signed_range(cx, dst);
        if src_range
            .intersect_with(&dst_range, llrm_mir::constantrange::PreferredRangeType::Smallest)
            .is_single_element()
        {
            // The ranges touch at exactly one value (i = i' = 0 or i = i' =
            // BTC).
            result.dv[level].direction &= !dir::LT;
            result.dv[level].direction &= !dir::GT;
            if result.dv[level].direction == 0 {
                return true;
            }
            result.dv[level].distance = Some(se.get_zero(cx, se.type_of(delta)));
            return false;
        }

        // Check that Coeff divides Delta.
        if const_coeff.is_zero() {
            return false;
        }
        let distance = const_delta.sdiv(const_coeff);
        let remainder = const_delta.srem(const_coeff);
        if !remainder.is_zero() {
            // Coeff doesn't divide Delta, no dependence.
            return true;
        }

        // If 2*Coeff doesn't divide Delta, then the equal direction isn't
        // possible.
        if distance.bits & 1 != 0 {
            // Equal direction isn't possible.
            result.dv[level].direction &= !dir::EQ;
        }
        false
    }

    /// `exactSIVtest`: `[c1 + a1*i]` against `[c2 + a2*i]` for constants `a1`
    /// and `a2`, solved exactly (Banerjee, case 2.5). Slower than the
    /// specialised tests; extended to return all the dependences between the
    /// destination and the source. True if disproved.
    pub(crate) fn exact_siv_test(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        level: u32,
        result: &mut FullDependence,
    ) -> bool {
        if !self.is_dependence_test_enabled(TestType::ExactSiv) {
            return false;
        }
        debug_assert!(0 < level && level <= self.common_levels, "Level out of range");
        self.exact_test_impl(se, src, dst, result, Some((level - 1) as usize))
    }

    /// `weakZeroSIVtestImpl`.
    fn weak_zero_siv_test_impl(
        &self,
        se: &mut ScalarEvolution,
        add_rec: Scev,
        constant: Scev,
        level: usize,
        result: &mut FullDependence,
    ) -> bool {
        let cx = self.cx;
        let (ar_const, ar_coeff, header) = add_rec_parts(se, cx, add_rec);

        if constant == ar_const && se.is_known_non_zero(cx, ar_coeff) {
            if (level as u32) < self.common_levels {
                result.dv[level].direction &= dir::LE;
            }
            return false; // dependences caused by first iteration
        }

        let Some(delta) = minus_scev_no_signed_overflow(se, cx, constant, ar_const) else { return false };
        let Some(const_coeff) = se.as_constant(ar_coeff) else { return false };

        if let Some(upper_bound) = self.collect_upper_bound(se, header, se.type_of(delta)) {
            let overlap_at_last = (|| {
                if !se.is_known_non_zero(cx, ar_coeff) {
                    return false;
                }
                let last = se.evaluate_at_iteration(cx, add_rec, upper_bound);
                last == constant
            })();
            if overlap_at_last {
                // Dependences caused by last iteration.
                if (level as u32) < self.common_levels {
                    result.dv[level].direction &= dir::GE;
                }
                return false;
            }
        }

        // If ARCoeff doesn't divide Delta, then no dependence.
        if let Some(const_delta) = se.as_constant(delta)
            && !is_remainder_zero(const_delta, const_coeff)
        {
            return true;
        }
        false
    }

    /// `weakZeroSrcSIVtest` (section 4.2.2): `[c1]` against `[c2 + a*i]`,
    /// `i = (c1 - c2)/a`: no dependence if not an integer or outside `0..=UB`;
    /// `>=` if `i = 0`, `<=` if `i = UB`. True if disproved.
    pub(crate) fn weak_zero_src_siv_test(
        &self,
        se: &mut ScalarEvolution,
        src_const: Scev,
        dst: Scev,
        level: u32,
        result: &mut FullDependence,
    ) -> bool {
        if !self.is_dependence_test_enabled(TestType::WeakZeroSiv) {
            return false;
        }
        debug_assert!(0 < level && level <= self.max_levels, "Level out of range");
        let level = (level - 1) as usize;

        // We have analyzed a dependence from Src to Dst, so `result` may
        // represent a dependence in that direction. weakZeroSIVtestImpl
        // analyzes a dependence from Dst to SrcConst: negate the current
        // result before passing it, and back after.
        result.negate(se, self.cx);
        let disproved = self.weak_zero_siv_test_impl(se, dst, src_const, level, result);
        result.negate(se, self.cx);
        disproved
    }

    /// `weakZeroDstSIVtest` (section 4.2.2): `[c1 + a*i]` against `[c2]`,
    /// `i = (c2 - c1)/a`: `<=` if `i = 0`, `>=` if `i = UB`. True if
    /// disproved.
    pub(crate) fn weak_zero_dst_siv_test(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst_const: Scev,
        level: u32,
        result: &mut FullDependence,
    ) -> bool {
        if !self.is_dependence_test_enabled(TestType::WeakZeroSiv) {
            return false;
        }
        debug_assert!(0 < level && level <= self.src_levels, "Level out of range");
        self.weak_zero_siv_test_impl(se, src, dst_const, (level - 1) as usize, result)
    }

    /// `exactRDIVtest`: `[c1 + a*i]` against `[c2 + b*j]` for two induction
    /// variables and constants `a`, `b`. True if any possible dependence is
    /// disproved.
    pub(crate) fn exact_rdiv_test(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        result: &mut FullDependence,
    ) -> bool {
        if !self.is_dependence_test_enabled(TestType::ExactRdiv) {
            return false;
        }
        self.exact_test_impl(se, src, dst, result, None)
    }

    /// `exactTestImpl`.
    fn exact_test_impl(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        result: &mut FullDependence,
        level: Option<usize>,
    ) -> bool {
        let cx = self.cx;
        let (src_const, src_coeff, src_header) = add_rec_parts(se, cx, src);
        let (dst_const, dst_coeff, dst_header) = add_rec_parts(se, cx, dst);

        let Some(delta) = minus_scev_no_signed_overflow(se, cx, dst_const, src_const) else { return false };
        let (Some(const_delta), Some(const_src_coeff), Some(const_dst_coeff)) =
            (se.as_constant(delta), se.as_constant(src_coeff), se.as_constant(dst_coeff))
        else {
            return false;
        };

        // Find the gcd.
        let (am, bm, cm) = (const_src_coeff, const_dst_coeff, const_delta);
        let bits = am.width;
        // GCD calculation failed so we cannot proceed with this test.
        let Some((disproved, g, x, y)) = find_gcd(bits, am, bm, cm) else { return false };
        if disproved {
            // gcd doesn't divide Delta, no dependence.
            return true;
        }

        // Since SCEV construction seems to normalize, LM = 0.
        let delta_ty = se.type_of(delta);
        let src_um = self.collect_non_negative_constant_upper_bound(se, src_header, delta_ty);
        let dst_um = self.collect_non_negative_constant_upper_bound(se, dst_header, delta_ty);

        let tc = Safe::of(cm.sdiv(g));
        let tx = Safe::of(x).mul(tc);
        let ty = Safe::of(y).mul(tc);
        if !tc.is_valid() || !tx.is_valid() || !ty.is_valid() {
            return false;
        }

        let tb = bm.sdiv(g);
        let ta = am.sdiv(g);

        // At this point, we have the following equations:
        //
        //   TA*i - TB*j = TC
        //
        // Also, we know that the all pairs of (i, j) can be expressed as:
        //
        //   (TX + k*TB, TY + k*TA)
        //
        // where k is an arbitrary integer.
        let (tl0, tu0) = infer_domain_of_affine(Safe::of(tb), tx, Safe(src_um));
        let (tl1, tu1) = infer_domain_of_affine(Safe::of(ta), ty, Safe(dst_um));

        let get_max_or_min = |v0: Safe, v1: Safe, is_min: bool| -> Option<Ap> {
            match (v0.0, v1.0) {
                (Some(a), Some(b)) => Some(if is_min { a.smin(b) } else { a.smax(b) }),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            }
        };

        let opt_tl = get_max_or_min(tl0, tl1, false);
        let opt_tu = get_max_or_min(tu0, tu1, true);
        let (Some(tl), Some(tu)) = (opt_tl, opt_tu) else { return false };

        if sgt(tl, tu) {
            return true;
        }

        let Some(level) = level else { return false };
        debug_assert!(src_um == dst_um, "Expecting same upper bound for Src and Dst");

        // Explore directions.
        let mut new_direction = dir::NONE;
        let (oty, otx, ota, otb, otl, otu) = (ty, tx, Safe::of(ta), Safe::of(tb), Safe::of(tl), Safe::of(tu));
        // NOTE: It's unclear whether these calculations can overflow. At the
        // moment, we conservatively assume they can.
        let (lower_distance, upper_distance);
        if sgt(ta, tb) {
            lower_distance = oty.sub(otx).add(ota.sub(otb).mul(otl));
            upper_distance = oty.sub(otx).add(ota.sub(otb).mul(otu));
        } else {
            lower_distance = oty.sub(otx).add(ota.sub(otb).mul(otu));
            upper_distance = oty.sub(otx).add(ota.sub(otb).mul(otl));
        }

        if !lower_distance.is_valid() || !upper_distance.is_valid() {
            return false;
        }
        let (lower, upper) = (lower_distance.value(), upper_distance.value());
        let zero = Ap::zero(lower.width);

        if lower.sle(zero) && sge(upper, zero) {
            new_direction |= dir::EQ;
        }
        if lower.slt(zero) {
            new_direction |= dir::GT;
        }
        if sgt(upper, zero) {
            new_direction |= dir::LT;
        }

        // Finished.
        result.dv[level].direction &= new_direction;
        result.dv[level].direction == dir::NONE
    }

    /// `testSIV`: `[c1 + a1*i]` against `[c2 - a2*i]` for constants. While the
    /// exact SIV test solves them all, the simpler tests are cheaper and
    /// sometimes more precise. Returns whether the dependence is disproved
    /// and the level of the loop.
    pub(crate) fn test_siv(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        result: &mut FullDependence,
        under_runtime_assumptions: bool,
    ) -> (bool, u32) {
        let cx = self.cx;
        let src_add_rec = matches!(se.node(src).kind, Kind::AddRec(..));
        let dst_add_rec = matches!(se.node(dst).kind, Kind::AddRec(..));
        if src_add_rec && dst_add_rec {
            let (_, src_coeff, src_header) = add_rec_parts(se, cx, src);
            let (_, dst_coeff, _) = add_rec_parts(se, cx, dst);
            let level = self.map_src_loop(src_header);
            let mut disproven = false;
            if src_coeff == dst_coeff {
                disproven = self.strong_siv_test(se, src, dst, level, result, under_runtime_assumptions);
            } else if src_coeff == se.get_negative_scev(cx, dst_coeff, 0) {
                disproven = self.weak_crossing_siv_test(se, src, dst, level, result);
            }
            return (disproven || self.exact_siv_test(se, src, dst, level, result), level);
        }
        if src_add_rec {
            let (_, _, src_header) = add_rec_parts(se, cx, src);
            let level = self.map_src_loop(src_header);
            return (self.weak_zero_dst_siv_test(se, src, dst, level, result), level);
        }
        if dst_add_rec {
            let (_, _, dst_header) = add_rec_parts(se, cx, dst);
            let level = self.map_dst_loop(dst_header);
            return (self.weak_zero_src_siv_test(se, src, dst, level, result), level);
        }
        unreachable!("SIV test expected at least one AddRec")
    }

    /// `testRDIV`: `[c1 + a1*i]` against `[c2 + a2*j]`, solved by an
    /// adaptation of the exact SIV test (no distance or direction to talk of),
    /// then by the GCD test. True if disproved.
    pub(crate) fn test_rdiv(
        &self,
        se: &mut ScalarEvolution,
        src: Scev,
        dst: Scev,
        result: &mut FullDependence,
    ) -> bool {
        debug_assert!(
            matches!(se.node(src).kind, Kind::AddRec(..)) && matches!(se.node(dst).kind, Kind::AddRec(..)),
            "Unexpected non-addrec input"
        );
        self.exact_rdiv_test(se, src, dst, result) || self.gcd_miv_test(se, src, dst, result)
    }
}
