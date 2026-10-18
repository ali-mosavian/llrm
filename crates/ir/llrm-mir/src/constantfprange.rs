//! `ConstantFPRange`: the floating-point values from `lower` up to and
//! including `upper`, and perhaps a quiet and a signaling NaN.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` (`llvm/lib/IR/ConstantFPRange.cpp`,
//! `llvm/include/llvm/IR/ConstantFPRange.h`): same operations, same order of
//! cases. Each function links the line it translates. The set is
//! `[lower, upper] U (may_be_qnan ? QNaN : {}) U (may_be_snan ? SNaN : {})`;
//! `[+inf, -inf]` is the empty set, -0 is below +0, and a range does not wrap.
//! The bounds are [`ApFloat`]s of one [`Semantics`](crate::apfloat::Semantics).
//!
//! DEVIATE: `print` and `dump` are not translated: they print the bounds in
//! decimal (`APFloat::toString`), which [`crate::apfloat`] does not have
//! (ConstantFPRange.cpp:337-362). A predicate is MIR's [`FloatPredicate`]; its
//! bits are LLVM's numbering of `FCmpInst::Predicate` (`fcmp_predicate_bits`),
//! which `fcmpPredExcludesEqual` reads. `isOrdered` and `isUnordered` of
//! `CmpInst` are here, as private functions. `ConstantFPRange.h` declares
//! `KnownFPClass` and uses it nowhere. The private `getEmpty()`, `getFull()`
//! and `makeFull()` have no caller and are not translated.
//!
//! # Example
//! `makeAllowedFCmpRegion(ole, [2, 5])` is `[-inf, 5]`; `[0, 0]` does not
//! contain -0.
//!
//! # See also
//! - Uses: [`crate::apfloat`], [`crate::fpclass`]

use crate::apfloat::*;
use crate::fpclass::*;
use crate::opcode::FloatPredicate;

/// `FCmpInst::Predicate`'s number, as LLVM has it.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/InstrTypes.h#L742>
pub fn fcmp_predicate_bits(pred: FloatPredicate) -> u32 {
    match pred {
        FloatPredicate::False => 0,
        FloatPredicate::Oeq => 1,
        FloatPredicate::Ogt => 2,
        FloatPredicate::Oge => 3,
        FloatPredicate::Olt => 4,
        FloatPredicate::Ole => 5,
        FloatPredicate::One => 6,
        FloatPredicate::Ord => 7,
        FloatPredicate::Uno => 8,
        FloatPredicate::Ueq => 9,
        FloatPredicate::Ugt => 10,
        FloatPredicate::Uge => 11,
        FloatPredicate::Ult => 12,
        FloatPredicate::Ule => 13,
        FloatPredicate::Une => 14,
        FloatPredicate::True => 15,
    }
}

/// Whether the predicate is true only for ordered operands.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L3997>
fn is_ordered(pred: FloatPredicate) -> bool {
    use FloatPredicate::*;
    matches!(pred, Oeq | One | Ogt | Olt | Oge | Ole | Ord)
}

/// Whether the predicate is true for unordered operands.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L4006>
fn is_unordered(pred: FloatPredicate) -> bool {
    use FloatPredicate::*;
    matches!(pred, Ueq | Une | Ugt | Ult | Uge | Ule | Uno)
}

/// A range of floating-point values.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L37>
#[derive(Clone, Copy, Debug)]
pub struct ConstantFpRange {
    lower: ApFloat,
    upper: ApFloat,
    may_be_qnan: bool,
    may_be_snan: bool,
}

/// We treat that -0 is less than 0 here.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L61>
pub(crate) fn strict_compare(
    lhs: &ApFloat,
    rhs: &ApFloat,
) -> CmpResult {
    assert!(!lhs.is_nan() && !rhs.is_nan(), "Unordered compare");
    if lhs.is_zero() && rhs.is_zero() {
        if lhs.is_negative() == rhs.is_negative() {
            return CmpResult::Equal;
        }
        return if lhs.is_negative() { CmpResult::LessThan } else { CmpResult::GreaterThan };
    }
    lhs.compare(rhs)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L72>
fn is_non_canonical_empty_set(
    lower: &ApFloat,
    upper: &ApFloat,
) -> bool {
    strict_compare(lower, upper) == CmpResult::GreaterThan && !(lower.is_infinity() && upper.is_infinity())
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L77>
fn canonicalize_range(
    lower: &mut ApFloat,
    upper: &mut ApFloat,
) {
    if is_non_canonical_empty_set(lower, upper) {
        *lower = ApFloat::get_inf(lower.get_semantics(), false);
        *upper = ApFloat::get_inf(upper.get_semantics(), true);
    }
}

/// Return true for ULT/UGT/OLT/OGT.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L113>
fn fcmp_pred_excludes_equal(pred: FloatPredicate) -> bool {
    fcmp_predicate_bits(pred) & 1 == 0
}

/// Return [-inf, V) or [-inf, V].
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L118>
fn make_less_than(
    mut v: ApFloat,
    pred: FloatPredicate,
) -> ConstantFpRange {
    let sem = v.get_semantics();
    if fcmp_pred_excludes_equal(pred) {
        if v.is_neg_infinity() {
            return ConstantFpRange::get_empty(sem);
        }
        v.next(true);
    }
    ConstantFpRange::get_non_nan_range(ApFloat::get_inf(sem, true), v)
}

/// Return (V, +inf] or [V, +inf].
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L130>
fn make_greater_than(
    mut v: ApFloat,
    pred: FloatPredicate,
) -> ConstantFpRange {
    let sem = v.get_semantics();
    if fcmp_pred_excludes_equal(pred) {
        if v.is_pos_infinity() {
            return ConstantFpRange::get_empty(sem);
        }
        v.next(false);
    }
    ConstantFpRange::get_non_nan_range(v, ApFloat::get_inf(sem, false))
}

/// Make sure that +0/-0 are both included in the range.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L142>
fn extend_zero_if_equal(
    cr: &ConstantFpRange,
    pred: FloatPredicate,
) -> ConstantFpRange {
    if fcmp_pred_excludes_equal(pred) {
        return *cr;
    }
    let mut lower = *cr.get_lower();
    let mut upper = *cr.get_upper();
    if lower.is_pos_zero() {
        lower = ApFloat::get_zero(lower.get_semantics(), true);
    }
    if upper.is_neg_zero() {
        upper = ApFloat::get_zero(upper.get_semantics(), false);
    }
    ConstantFpRange::new(lower, upper, cr.contains_qnan(), cr.contains_snan())
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L157>
fn set_nan_field(
    cr: &ConstantFpRange,
    pred: FloatPredicate,
) -> ConstantFpRange {
    let contains_nan = is_unordered(pred);
    ConstantFpRange::new(*cr.get_lower(), *cr.get_upper(), contains_nan, contains_nan)
}

impl ConstantFpRange {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L18>
    fn make_empty(&mut self) {
        let sem = self.lower.get_semantics();
        self.lower = ApFloat::get_inf(sem, false);
        self.upper = ApFloat::get_inf(sem, true);
        self.may_be_qnan = false;
        self.may_be_snan = false;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L34>
    pub fn is_nan_only(&self) -> bool {
        self.lower.is_pos_infinity() && self.upper.is_neg_infinity()
    }

    /// Initialize a full or empty set for the specified semantics.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L38>
    fn with_semantics(
        sem: Sem,
        is_full_set: bool,
    ) -> ConstantFpRange {
        ConstantFpRange {
            lower: ApFloat::get_inf(sem, is_full_set),
            upper: ApFloat::get_inf(sem, !is_full_set),
            may_be_qnan: is_full_set,
            may_be_snan: is_full_set,
        }
    }

    /// Initialize a range to hold the single specified value.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L46>
    pub fn from_value(value: &ApFloat) -> ConstantFpRange {
        let mut range = ConstantFpRange {
            lower: ApFloat::get_zero(value.get_semantics(), false),
            upper: ApFloat::get_zero(value.get_semantics(), false),
            may_be_qnan: false,
            may_be_snan: false,
        };
        if value.is_nan() {
            range.make_empty();
            let is_snan = value.is_signaling();
            range.may_be_qnan = !is_snan;
            range.may_be_snan = is_snan;
        } else {
            range.lower = *value;
            range.upper = *value;
            range.may_be_qnan = false;
            range.may_be_snan = false;
        }
        range
    }

    /// Initialize a range of values explicitly. If `lower` is greater than
    /// `upper`, use the canonical form `[Inf, -Inf]`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L84>
    pub fn new(
        lower: ApFloat,
        upper: ApFloat,
        may_be_qnan: bool,
        may_be_snan: bool,
    ) -> ConstantFpRange {
        assert!(lower.get_semantics() == upper.get_semantics(), "Should only use the same semantics");
        assert!(!is_non_canonical_empty_set(&lower, &upper), "Non-canonical form");
        ConstantFpRange { lower, upper, may_be_qnan, may_be_snan }
    }

    /// Create empty constant range with the given semantics.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L69>
    pub fn get_empty(sem: Sem) -> ConstantFpRange {
        ConstantFpRange::with_semantics(sem, false)
    }

    /// Create full constant range with the given semantics.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L74>
    pub fn get_full(sem: Sem) -> ConstantFpRange {
        ConstantFpRange::with_semantics(sem, true)
    }

    /// Helper for (-inf, inf) to represent all finite values.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L93>
    pub fn get_finite(sem: Sem) -> ConstantFpRange {
        ConstantFpRange::new(ApFloat::get_largest(sem, true), ApFloat::get_largest(sem, false), false, false)
    }

    /// Helper for [-inf, inf] to represent all non-NaN values.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L106>
    pub fn get_non_nan(sem: Sem) -> ConstantFpRange {
        ConstantFpRange::new(ApFloat::get_inf(sem, true), ApFloat::get_inf(sem, false), false, false)
    }

    /// Create a range which doesn't contain NaNs.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L85>
    pub fn get_non_nan_range(
        lower: ApFloat,
        upper: ApFloat,
    ) -> ConstantFpRange {
        ConstantFpRange::new(lower, upper, false, false)
    }

    /// Create a range which may contain NaNs.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L91>
    pub fn get_may_be_nan(
        lower: ApFloat,
        upper: ApFloat,
    ) -> ConstantFpRange {
        ConstantFpRange::new(lower, upper, true, true)
    }

    /// Create a range which only contains NaNs.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L99>
    pub fn get_nan_only(
        sem: Sem,
        may_be_qnan: bool,
        may_be_snan: bool,
    ) -> ConstantFpRange {
        ConstantFpRange::new(ApFloat::get_inf(sem, false), ApFloat::get_inf(sem, true), may_be_qnan, may_be_snan)
    }

    /// The smallest range such that all values that may satisfy the predicate
    /// with any value in `other` are in it.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L165>
    pub fn make_allowed_fcmp_region(
        pred: FloatPredicate,
        other: &ConstantFpRange,
    ) -> ConstantFpRange {
        if other.is_empty_set() {
            return *other;
        }
        if other.contains_nan() && is_unordered(pred) {
            return ConstantFpRange::get_full(other.get_semantics());
        }
        if other.is_nan_only() && is_ordered(pred) {
            return ConstantFpRange::get_empty(other.get_semantics());
        }

        match pred {
            FloatPredicate::True => ConstantFpRange::get_full(other.get_semantics()),
            FloatPredicate::False => ConstantFpRange::get_empty(other.get_semantics()),
            FloatPredicate::Ord => ConstantFpRange::get_non_nan(other.get_semantics()),
            FloatPredicate::Uno => ConstantFpRange::get_nan_only(other.get_semantics(), true, true),
            FloatPredicate::Oeq | FloatPredicate::Ueq => set_nan_field(&extend_zero_if_equal(other, pred), pred),
            FloatPredicate::One | FloatPredicate::Une => {
                if let Some(single_element) = other.get_single_element(true) {
                    let sem = single_element.get_semantics();
                    if single_element.is_pos_infinity() {
                        return set_nan_field(
                            &ConstantFpRange::get_non_nan_range(
                                ApFloat::get_inf(sem, true),
                                ApFloat::get_largest(sem, false),
                            ),
                            pred,
                        );
                    }
                    if single_element.is_neg_infinity() {
                        return set_nan_field(
                            &ConstantFpRange::get_non_nan_range(
                                ApFloat::get_largest(sem, true),
                                ApFloat::get_inf(sem, false),
                            ),
                            pred,
                        );
                    }
                }
                if pred == FloatPredicate::One {
                    ConstantFpRange::get_non_nan(other.get_semantics())
                } else {
                    ConstantFpRange::get_full(other.get_semantics())
                }
            }
            FloatPredicate::Olt | FloatPredicate::Ole | FloatPredicate::Ult | FloatPredicate::Ule => {
                set_nan_field(&extend_zero_if_equal(&make_less_than(*other.get_upper(), pred), pred), pred)
            }
            FloatPredicate::Ogt | FloatPredicate::Oge | FloatPredicate::Ugt | FloatPredicate::Uge => {
                set_nan_field(&extend_zero_if_equal(&make_greater_than(*other.get_lower(), pred), pred), pred)
            }
        }
    }

    /// The largest range such that all values in it satisfy the predicate with
    /// all values contained within `other`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L223>
    pub fn make_satisfying_fcmp_region(
        pred: FloatPredicate,
        other: &ConstantFpRange,
    ) -> ConstantFpRange {
        if other.is_empty_set() {
            return ConstantFpRange::get_full(other.get_semantics());
        }
        if other.contains_nan() && is_ordered(pred) {
            return ConstantFpRange::get_empty(other.get_semantics());
        }
        if other.is_nan_only() && is_unordered(pred) {
            return ConstantFpRange::get_full(other.get_semantics());
        }

        match pred {
            FloatPredicate::True => ConstantFpRange::get_full(other.get_semantics()),
            FloatPredicate::False => ConstantFpRange::get_empty(other.get_semantics()),
            FloatPredicate::Ord => ConstantFpRange::get_non_nan(other.get_semantics()),
            FloatPredicate::Uno => ConstantFpRange::get_nan_only(other.get_semantics(), true, true),
            FloatPredicate::Oeq | FloatPredicate::Ueq => set_nan_field(
                &if other.is_single_element(true) || ((other.classify() & !FC_NAN) == FC_ZERO) {
                    extend_zero_if_equal(other, pred)
                } else {
                    ConstantFpRange::get_empty(other.get_semantics())
                },
                pred,
            ),
            FloatPredicate::One | FloatPredicate::Une => ConstantFpRange::get_empty(other.get_semantics()),
            FloatPredicate::Olt | FloatPredicate::Ole | FloatPredicate::Ult | FloatPredicate::Ule => {
                set_nan_field(&extend_zero_if_equal(&make_less_than(*other.get_lower(), pred), pred), pred)
            }
            FloatPredicate::Ogt | FloatPredicate::Oge | FloatPredicate::Ugt | FloatPredicate::Uge => {
                set_nan_field(&extend_zero_if_equal(&make_greater_than(*other.get_upper(), pred), pred), pred)
            }
        }
    }

    /// The exact range of `{ x : fcmp pred x other }`, or `None` if it is not
    /// a range.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L270>
    pub fn make_exact_fcmp_region(
        pred: FloatPredicate,
        other: &ApFloat,
    ) -> Option<ConstantFpRange> {
        if (pred == FloatPredicate::Une || pred == FloatPredicate::One) && !other.is_nan() {
            return None;
        }
        Some(ConstantFpRange::make_satisfying_fcmp_region(pred, &ConstantFpRange::from_value(other)))
    }

    /// Does the predicate hold between ranges this and `other`? False does not
    /// mean that the inverse predicate holds.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L278>
    pub fn fcmp(
        &self,
        pred: FloatPredicate,
        other: &ConstantFpRange,
    ) -> bool {
        ConstantFpRange::make_satisfying_fcmp_region(pred, other).contains_range(self)
    }

    /// The lower value for this range.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L139>
    pub fn get_lower(&self) -> &ApFloat {
        &self.lower
    }

    /// The upper value for this range.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L142>
    pub fn get_upper(&self) -> &ApFloat {
        &self.upper
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L144>
    pub fn contains_nan(&self) -> bool {
        self.may_be_qnan || self.may_be_snan
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L145>
    pub fn contains_qnan(&self) -> bool {
        self.may_be_qnan
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L146>
    pub fn contains_snan(&self) -> bool {
        self.may_be_snan
    }

    /// The semantics of this range.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L150>
    pub fn get_semantics(&self) -> Sem {
        self.lower.get_semantics()
    }

    /// Whether this set contains all of the elements possible for this
    /// data-type.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L283>
    pub fn is_full_set(&self) -> bool {
        self.lower.is_neg_infinity() && self.upper.is_pos_infinity() && self.may_be_qnan && self.may_be_snan
    }

    /// Whether this set contains no members.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L288>
    pub fn is_empty_set(&self) -> bool {
        self.lower.is_pos_infinity() && self.upper.is_neg_infinity() && !self.may_be_qnan && !self.may_be_snan
    }

    /// Whether the specified value is in the set.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L293>
    pub fn contains(
        &self,
        val: &ApFloat,
    ) -> bool {
        assert!(self.get_semantics() == val.get_semantics(), "Should only use the same semantics");

        if val.is_nan() {
            return if val.is_signaling() { self.may_be_snan } else { self.may_be_qnan };
        }
        strict_compare(&self.lower, val) != CmpResult::GreaterThan
            && strict_compare(val, &self.upper) != CmpResult::GreaterThan
    }

    /// Whether the other range is a subset of this one.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L303>
    pub fn contains_range(
        &self,
        cr: &ConstantFpRange,
    ) -> bool {
        assert!(self.get_semantics() == cr.get_semantics(), "Should only use the same semantics");

        if cr.may_be_qnan && !self.may_be_qnan {
            return false;
        }

        if cr.may_be_snan && !self.may_be_snan {
            return false;
        }

        strict_compare(&self.lower, &cr.lower) != CmpResult::GreaterThan
            && strict_compare(&cr.upper, &self.upper) != CmpResult::GreaterThan
    }

    /// The single element of the set, if it has exactly one. If `excludes_nan`
    /// is true, the non-NaN single element.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L317>
    pub fn get_single_element(
        &self,
        excludes_nan: bool,
    ) -> Option<&ApFloat> {
        if !excludes_nan && (self.may_be_snan || self.may_be_qnan) {
            return None;
        }
        if self.lower.bitwise_is_equal(&self.upper) { Some(&self.lower) } else { None }
    }

    /// Whether this set contains exactly one member (one non-NaN member if
    /// `excludes_nan`).
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L172>
    pub fn is_single_element(
        &self,
        excludes_nan: bool,
    ) -> bool {
        self.get_single_element(excludes_nan).is_some()
    }

    /// True if the sign bit of all values in this range is 1, false if it is 0
    /// for all, else `None`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L323>
    pub fn get_sign_bit(&self) -> Option<bool> {
        if !self.may_be_snan && !self.may_be_qnan && self.lower.is_negative() == self.upper.is_negative() {
            return Some(self.lower.is_negative());
        }
        None
    }

    /// The `FPClassTest` which will return true for the value.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L337>
    pub fn classify(&self) -> FpClassTest {
        let mut mask = FC_NONE.0;
        if self.may_be_snan {
            mask |= FC_SNAN.0;
        }
        if self.may_be_qnan {
            mask |= FC_QNAN.0;
        }
        if !self.is_nan_only() {
            let lower_mask = self.lower.classify();
            let upper_mask = self.upper.classify();
            assert!(lower_mask.0 <= upper_mask.0, "Range is nan-only.");
            // Set all bits from log2(LowerMask) to log2(UpperMask).
            mask |= (upper_mask.0 << 1) - lower_mask.0;
        }
        FpClassTest(mask)
    }

    /// The intersection of this range with another range.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L381>
    pub fn intersect_with(
        &self,
        cr: &ConstantFpRange,
    ) -> ConstantFpRange {
        assert!(self.get_semantics() == cr.get_semantics(), "Should only use the same semantics");
        let mut new_lower = maxnum(&self.lower, &cr.lower);
        let mut new_upper = minnum(&self.upper, &cr.upper);
        canonicalize_range(&mut new_lower, &mut new_upper);
        ConstantFpRange::new(new_lower, new_upper, self.may_be_qnan & cr.may_be_qnan, self.may_be_snan & cr.may_be_snan)
    }

    /// The smallest range that includes the elements of both sets.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L391>
    pub fn union_with(
        &self,
        cr: &ConstantFpRange,
    ) -> ConstantFpRange {
        assert!(self.get_semantics() == cr.get_semantics(), "Should only use the same semantics");
        ConstantFpRange::new(
            minnum(&self.lower, &cr.lower),
            maxnum(&self.upper, &cr.upper),
            self.may_be_qnan | cr.may_be_qnan,
            self.may_be_snan | cr.may_be_snan,
        )
    }

    /// Calculate absolute value range.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L398>
    pub fn abs(&self) -> ConstantFpRange {
        if self.is_nan_only() {
            return *self;
        }
        // Check if the range is all non-negative or all non-positive.
        if self.lower.is_negative() == self.upper.is_negative() {
            if self.lower.is_negative() {
                return self.negate();
            }
            return *self;
        }
        // The range contains both positive and negative values.
        let new_lower = ApFloat::get_zero(self.get_semantics(), false);
        let new_upper = maxnum(&self.lower.neg(), &self.upper);
        ConstantFpRange::new(new_lower, new_upper, self.may_be_qnan, self.may_be_snan)
    }

    /// Calculate range of negated values.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L414>
    pub fn negate(&self) -> ConstantFpRange {
        ConstantFpRange::new(self.upper.neg(), self.lower.neg(), self.may_be_qnan, self.may_be_snan)
    }

    /// Get the range without NaNs.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantFPRange.h#L212>
    pub fn get_without_nan(&self) -> ConstantFpRange {
        ConstantFpRange::new(self.lower, self.upper, false, false)
    }

    /// Get the range without infinities.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L435>
    pub fn get_without_inf(&self) -> ConstantFpRange {
        if self.is_nan_only() {
            return *self;
        }
        let mut new_lower = self.lower;
        let mut new_upper = self.upper;
        let (mut unused_pos, mut unused_neg) = (false, false);
        remove_inf(&mut new_lower, &mut new_upper, &mut unused_pos, &mut unused_neg);
        canonicalize_range(&mut new_lower, &mut new_upper);
        ConstantFpRange::new(new_lower, new_upper, self.may_be_qnan, self.may_be_snan)
    }

    /// A new range in the specified format with the specified rounding mode.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L448>
    pub fn cast(
        &self,
        dst_sem: Sem,
        rm: RoundingMode,
    ) -> ConstantFpRange {
        let mut new_lower = self.lower;
        let mut new_upper = self.upper;
        // For conservative, return full range if conversion is invalid.
        if new_lower.convert(dst_sem, rm).0 == OP_INVALID_OP || new_lower.is_nan() {
            return ConstantFpRange::get_full(dst_sem);
        }
        if new_upper.convert(dst_sem, rm).0 == OP_INVALID_OP || new_upper.is_nan() {
            return ConstantFpRange::get_full(dst_sem);
        }
        ConstantFpRange::new(new_lower, new_upper, self.may_be_qnan || self.may_be_snan, false)
    }

    /// The possible values resulting from an addition of a value in this range
    /// and a value in `other`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L465>
    pub fn add(
        &self,
        other: &ConstantFpRange,
    ) -> ConstantFpRange {
        let mut res_may_be_qnan = ((self.may_be_qnan || self.may_be_snan) && !other.is_empty_set())
            || ((other.may_be_qnan || other.may_be_snan) && !self.is_empty_set());
        if self.is_nan_only() || other.is_nan_only() {
            return ConstantFpRange::get_nan_only(self.get_semantics(), res_may_be_qnan, false);
        }
        let (mut lhs_has_neg_inf, mut lhs_has_pos_inf) = (false, false);
        let (mut lhs_lower, mut lhs_upper) = (self.lower, self.upper);
        let lhs_finite_is_non_empty =
            remove_inf(&mut lhs_lower, &mut lhs_upper, &mut lhs_has_pos_inf, &mut lhs_has_neg_inf);
        let (mut rhs_has_neg_inf, mut rhs_has_pos_inf) = (false, false);
        let (mut rhs_lower, mut rhs_upper) = (other.lower, other.upper);
        let rhs_finite_is_non_empty =
            remove_inf(&mut rhs_lower, &mut rhs_upper, &mut rhs_has_pos_inf, &mut rhs_has_neg_inf);
        // -inf + +inf = QNaN
        res_may_be_qnan |= (lhs_has_neg_inf && rhs_has_pos_inf) || (lhs_has_pos_inf && rhs_has_neg_inf);
        // +inf + finite/+inf = +inf, -inf + finite/-inf = -inf
        let has_neg_inf = (lhs_has_neg_inf && (rhs_finite_is_non_empty || rhs_has_neg_inf))
            || (rhs_has_neg_inf && (lhs_finite_is_non_empty || lhs_has_neg_inf));
        let has_pos_inf = (lhs_has_pos_inf && (rhs_finite_is_non_empty || rhs_has_pos_inf))
            || (rhs_has_pos_inf && (lhs_finite_is_non_empty || lhs_has_pos_inf));
        if lhs_finite_is_non_empty && rhs_finite_is_non_empty {
            let new_lower = if has_neg_inf {
                ApFloat::get_inf(lhs_lower.get_semantics(), true)
            } else {
                lhs_lower.plus(&rhs_lower)
            };
            let new_upper = if has_pos_inf {
                ApFloat::get_inf(lhs_upper.get_semantics(), false)
            } else {
                lhs_upper.plus(&rhs_upper)
            };
            return ConstantFpRange::new(new_lower, new_upper, res_may_be_qnan, false);
        }
        // If both HasNegInf and HasPosInf are false, the non-NaN part is
        // empty. We just return the canonical form [+inf, -inf] for the empty
        // non-NaN set.
        ConstantFpRange::new(
            ApFloat::get_inf(self.lower.get_semantics(), has_neg_inf),
            ApFloat::get_inf(self.upper.get_semantics(), !has_pos_inf),
            res_may_be_qnan,
            false,
        )
    }

    /// The possible values resulting from a subtraction.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L506>
    pub fn sub(
        &self,
        other: &ConstantFpRange,
    ) -> ConstantFpRange {
        // fsub X, Y = fadd X, (fneg Y)
        self.add(&other.negate())
    }

    /// Flush denormal values to zero according to the specified mode. For
    /// dynamic mode, we return the union of all possible results.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L511>
    pub fn flush_denormals(
        &mut self,
        mode: DenormalModeKind,
    ) {
        if mode == DenormalModeKind::Ieee {
            return;
        }
        let class = self.classify();
        if !class.any(FC_SUBNORMAL) {
            return;
        }

        let sem = self.get_semantics();
        // PreserveSign: PosSubnormal -> PosZero, NegSubnormal -> NegZero
        // PositiveZero: PosSubnormal -> PosZero, NegSubnormal -> PosZero
        // Dynamic:      PosSubnormal -> PosZero, NegSubnormal ->
        // NegZero/PosZero
        let zero_lower_negative = mode != DenormalModeKind::PositiveZero && class.any(FC_NEG_SUBNORMAL);
        let zero_upper_negative = mode == DenormalModeKind::PreserveSign && !class.any(FC_POS_SUBNORMAL);
        assert!(zero_lower_negative || !zero_upper_negative, "ZeroLower is greater than ZeroUpper.");
        self.lower = minnum(&self.lower, &ApFloat::get_zero(sem, zero_lower_negative));
        self.upper = maxnum(&self.upper, &ApFloat::get_zero(sem, zero_upper_negative));
    }

    /// The possible values resulting from a multiplication.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L572>
    pub fn mul(
        &self,
        other: &ConstantFpRange,
    ) -> ConstantFpRange {
        let sem = self.get_semantics();
        let mut res_may_be_qnan = ((self.may_be_qnan || self.may_be_snan) && !other.is_empty_set())
            || ((other.may_be_qnan || other.may_be_snan) && !self.is_empty_set());
        if self.is_nan_only() || other.is_nan_only() {
            return ConstantFpRange::get_nan_only(sem, res_may_be_qnan, false);
        }
        let (mut lhs_neg, mut lhs_pos, mut rhs_neg, mut rhs_pos) = (None, None, None, None);
        split_pos_neg(&self.lower, &self.upper, &mut lhs_neg, &mut lhs_pos);
        split_pos_neg(&other.lower, &other.upper, &mut rhs_neg, &mut rhs_pos);
        let mut res_lower = ApFloat::get_inf(sem, false);
        let mut res_upper = ApFloat::get_inf(sem, true);
        let mut update = |lhs: &Option<SameSignRange>, rhs: &Option<SameSignRange>, negative: bool| {
            let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
                return;
            };
            // 0 * inf = QNaN
            res_may_be_qnan |= lhs.has_zero && rhs.has_inf;
            res_may_be_qnan |= rhs.has_zero && lhs.has_inf;
            // NonZero * inf = inf
            if (lhs.has_inf && rhs.has_non_zero) || (rhs.has_inf && lhs.has_non_zero) {
                *(if negative { &mut res_lower } else { &mut res_upper }) = ApFloat::get_inf(sem, negative);
            }
            // Finite * Finite
            if let (Some(lhs_finite), Some(rhs_finite)) = (&lhs.finite_part, &rhs.finite_part) {
                let new_lower = lhs_finite.0.times(&rhs_finite.0);
                let new_upper = lhs_finite.1.times(&rhs_finite.1);
                if negative {
                    res_lower = minnum(&res_lower, &new_upper.neg());
                    res_upper = maxnum(&res_upper, &new_lower.neg());
                } else {
                    res_lower = minnum(&res_lower, &new_lower);
                    res_upper = maxnum(&res_upper, &new_upper);
                }
            }
        };
        update(&lhs_neg, &rhs_neg, false);
        update(&lhs_neg, &rhs_pos, true);
        update(&lhs_pos, &rhs_neg, true);
        update(&lhs_pos, &rhs_pos, false);
        ConstantFpRange::new(res_lower, res_upper, res_may_be_qnan, false)
    }

    /// The possible values resulting from a division.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L614>
    pub fn div(
        &self,
        other: &ConstantFpRange,
    ) -> ConstantFpRange {
        let sem = self.get_semantics();
        let mut res_may_be_qnan = ((self.may_be_qnan || self.may_be_snan) && !other.is_empty_set())
            || ((other.may_be_qnan || other.may_be_snan) && !self.is_empty_set());
        if self.is_nan_only() || other.is_nan_only() {
            return ConstantFpRange::get_nan_only(sem, res_may_be_qnan, false);
        }
        let (mut lhs_neg, mut lhs_pos, mut rhs_neg, mut rhs_pos) = (None, None, None, None);
        split_pos_neg(&self.lower, &self.upper, &mut lhs_neg, &mut lhs_pos);
        split_pos_neg(&other.lower, &other.upper, &mut rhs_neg, &mut rhs_pos);
        let mut res_lower = ApFloat::get_inf(sem, false);
        let mut res_upper = ApFloat::get_inf(sem, true);
        let mut update = |lhs: &Option<SameSignRange>, rhs: &Option<SameSignRange>, negative: bool| {
            let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
                return;
            };
            // inf / inf = QNaN 0 / 0 = QNaN
            res_may_be_qnan |= lhs.has_inf && rhs.has_inf;
            res_may_be_qnan |= lhs.has_zero && rhs.has_zero;
            // It is not straightforward to infer HasNonZeroFinite = HasFinite
            // && HasNonZero. By definitions we have:
            //   HasFinite = HasNonZeroFinite || HasZero
            //   HasNonZero = HasNonZeroFinite || HasInf
            // Since the range is contiguous, if both HasFinite and HasNonZero
            // are true, HasNonZeroFinite must be true.
            let lhs_has_non_zero_finite = lhs.finite_part.is_some() && lhs.has_non_zero;
            let rhs_has_non_zero_finite = rhs.finite_part.is_some() && rhs.has_non_zero;
            // inf / Finite = inf FiniteNonZero / 0 = inf
            if (lhs.has_inf && rhs.finite_part.is_some()) || (lhs_has_non_zero_finite && rhs.has_zero) {
                *(if negative { &mut res_lower } else { &mut res_upper }) = ApFloat::get_inf(sem, negative);
            }
            // Finite / inf = 0
            if lhs.finite_part.is_some() && rhs.has_inf {
                let zero = ApFloat::get_zero(sem, negative);
                res_lower = minnum(&res_lower, &zero);
                res_upper = maxnum(&res_upper, &zero);
            }
            // Finite / FiniteNonZero
            if let (Some(lhs_finite), true) = (&lhs.finite_part, rhs_has_non_zero_finite) {
                let rhs_finite = rhs.finite_part.as_ref().expect("a non-zero finite part");
                assert!(!rhs_finite.1.is_zero(), "Divisor should be non-zero.");
                let new_lower = lhs_finite.0.divided_by(&rhs_finite.1);
                let new_upper = lhs_finite
                    .1
                    .divided_by(
                        &if rhs_finite.0.is_zero() {
                            ApFloat::get_smallest(sem, false)
                        } else {
                            rhs_finite.0
                        },
                    );
                if negative {
                    res_lower = minnum(&res_lower, &new_upper.neg());
                    res_upper = maxnum(&res_upper, &new_lower.neg());
                } else {
                    res_lower = minnum(&res_lower, &new_lower);
                    res_upper = maxnum(&res_upper, &new_upper);
                }
            }
        };
        update(&lhs_neg, &rhs_neg, false);
        update(&lhs_neg, &rhs_pos, true);
        update(&lhs_pos, &rhs_neg, true);
        update(&lhs_pos, &rhs_pos, false);
        ConstantFpRange::new(res_lower, res_upper, res_may_be_qnan, false)
    }
}

impl PartialEq for ConstantFpRange {
    /// Whether this range is equal to another range.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L329>
    fn eq(
        &self,
        cr: &ConstantFpRange,
    ) -> bool {
        assert!(self.get_semantics() == cr.get_semantics(), "Should only use the same semantics");
        if self.may_be_snan != cr.may_be_snan || self.may_be_qnan != cr.may_be_qnan {
            return false;
        }
        self.lower.bitwise_is_equal(&cr.lower) && self.upper.bitwise_is_equal(&cr.upper)
    }
}

/// Return true if the finite part is not empty after removing infinities.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L419>
fn remove_inf(
    lower: &mut ApFloat,
    upper: &mut ApFloat,
    has_pos_inf: &mut bool,
    has_neg_inf: &mut bool,
) -> bool {
    assert!(strict_compare(lower, upper) != CmpResult::GreaterThan, "Non-NaN part is empty.");
    let sem = lower.get_semantics();
    if lower.is_neg_infinity() {
        *lower = ApFloat::get_largest(sem, true);
        *has_neg_inf = true;
    }
    if upper.is_pos_infinity() {
        *upper = ApFloat::get_largest(sem, false);
        *has_pos_inf = true;
    }
    strict_compare(lower, upper) != CmpResult::GreaterThan
}

/// A contiguous range of values sharing the same sign.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L533>
struct SameSignRange {
    has_zero: bool,
    has_non_zero: bool,
    has_inf: bool,
    /// The lower and upper bounds of the range (inclusive). The sign is
    /// dropped and infinities are excluded.
    finite_part: Option<(ApFloat, ApFloat)>,
}

impl SameSignRange {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L541>
    fn new(
        lower: &ApFloat,
        upper: &ApFloat,
    ) -> SameSignRange {
        let has_zero = lower.is_zero();
        let has_non_zero = !upper.is_zero();
        let has_inf = upper.is_infinity();
        assert!(!lower.is_negative() && !upper.is_negative(), "The sign should be dropped.");
        assert!(strict_compare(lower, upper) != CmpResult::GreaterThan, "Empty set.");
        let finite_part = if !lower.is_infinity() {
            Some((*lower, if has_inf { ApFloat::get_largest(lower.get_semantics(), false) } else { *upper }))
        } else {
            None
        };
        SameSignRange { has_zero, has_non_zero, has_inf, finite_part }
    }
}

/// Split the range into positive and negative components.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFPRange.cpp#L555>
fn split_pos_neg(
    lower: &ApFloat,
    upper: &ApFloat,
    neg_part: &mut Option<SameSignRange>,
    pos_part: &mut Option<SameSignRange>,
) {
    assert!(strict_compare(lower, upper) != CmpResult::GreaterThan, "Non-NaN part is empty.");
    if lower.is_negative() == upper.is_negative() {
        if lower.is_negative() {
            *neg_part = Some(SameSignRange::new(&upper.abs(), &lower.abs()));
        } else {
            *pos_part = Some(SameSignRange::new(lower, upper));
        }
        return;
    }
    let sem = lower.get_semantics();
    *neg_part = Some(SameSignRange::new(&ApFloat::get_zero(sem, false), &lower.abs()));
    *pos_part = Some(SameSignRange::new(&ApFloat::get_zero(sem, false), upper));
}
