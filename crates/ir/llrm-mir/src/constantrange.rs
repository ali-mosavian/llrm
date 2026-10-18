//! `ConstantRange`: the integers from `lower` up to, not including, `upper`,
//! wrapping at the width.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` (`llvm/lib/IR/ConstantRange.cpp`,
//! `llvm/include/llvm/IR/ConstantRange.h`): same operations, same order of
//! cases, each function linking the line it translates. `lower == upper` is
//! the full set when it is the maximum value and the empty set when it is zero.
//! An `APInt` is a `u128` and its width ([`crate::apint`]).
//!
//! DEVIATE: widths above 128 are refused: MIR parses
//! `i1` to `i128` and no wider (`mir_holds_no_integer_wider_than_128` in
//! `knownbits_tests.rs` fails if that changes). `multiply` runs at twice the
//! width, as LLVM does, in a [`U256`](crate::apint::U256) above 64 bits.
//! Translated, not deviated: the `samesign` form of `makeAllowedICmpRegion`
//! (MIR has the `samesign` icmp flag), and `getConstantRangeFromMetadata` on
//! the `(low, high)` pairs of `!range` (MIR carries several; the `MDNode` is
//! not needed). `binaryOp(BinaryOperator)` takes the flags as arguments.
//!
//! # Example
//! `[250, 5)` at 8 bits holds 250..=255 and 0..=4; `is_wrapped_set` is true and
//! `get_unsigned_min` is 0.
//!
//! # See also
//! - Used by: [`crate::valuetracking::compute_constant_range`]

use std::cmp::{max, min};

use crate::apint::*;
use crate::knownbits::Bits;
use crate::opcode::{BinaryOp, CastOp, IntPredicate};

/// `OverflowingBinaryOperator::NoUnsignedWrap`, also `TruncInst`'s.
pub const NO_UNSIGNED_WRAP: u32 = 1;
/// `OverflowingBinaryOperator::NoSignedWrap`, also `TruncInst`'s.
pub const NO_SIGNED_WRAP: u32 = 2;

/// `llvm::ConstantRange`: `[lower, upper)` modulo `2^width`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Range<W: Word = u128> {
    pub lower: W,
    pub upper: W,
    pub width: u32,
}

/// LLVM's `ConstantRange`, of a MIR integer (`i1` to `i128`).
pub type ConstantRange = Range<u128>;

/// `ConstantRange::PreferredRangeType`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreferredRangeType {
    Smallest,
    Unsigned,
    Signed,
}

/// `ConstantRange::OverflowResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverflowResult {
    AlwaysOverflowsLow,
    AlwaysOverflowsHigh,
    MayOverflow,
    NeverOverflows,
}

/// The intrinsics `ConstantRange::intrinsic` supports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RangeIntrinsic {
    UAddSat,
    USubSat,
    SAddSat,
    SSubSat,
    UMin,
    UMax,
    SMin,
    SMax,
    Abs,
    Ctlz,
    Cttz,
    Ctpop,
}

fn add<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    a.wrapping_add(b) & mask::<W>(width)
}

fn sub<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    a.wrapping_sub(b) & mask::<W>(width)
}

fn neg<W: Word>(
    a: W,
    width: u32,
) -> W {
    W::ZERO.wrapping_sub(a) & mask::<W>(width)
}

fn is_strictly_positive<W: Word>(
    v: W,
    width: u32,
) -> bool {
    !is_negative(v, width) && !v.is_zero()
}

fn smin_val<W: Word>(width: u32) -> W {
    sign_bit::<W>(width)
}

fn smax_val<W: Word>(width: u32) -> W {
    mask::<W>(width) >> 1
}

fn smax_of_two<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    if slt(a, b, width) { b } else { a }
}

fn smin_of_two<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    if slt(b, a, width) { b } else { a }
}

impl<W: Word> Range<W> {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L46>
    pub fn new(
        width: u32,
        lower: W,
        upper: W,
    ) -> Self {
        assert!((1..=W::BITS).contains(&width), "ConstantRange: width {width} is outside 1..={}", W::BITS);
        let (lower, upper) = (lower & mask::<W>(width), upper & mask::<W>(width));
        assert!(
            lower != upper || lower == mask::<W>(width) || lower == W::ZERO,
            "Lower == Upper, but they aren't min or max value!"
        );
        Self { lower, upper, width }
    }

    /// `ConstantRange(APInt Value)`: the single element.
    pub fn single(
        width: u32,
        value: W,
    ) -> Self {
        Self::new(width, value, add(value, W::ONE, width))
    }

    /// `Self::getFull`.
    pub fn full(width: u32) -> Self {
        Self { lower: mask::<W>(width), upper: mask::<W>(width), width }
    }

    /// `Self::getEmpty`.
    pub fn empty(width: u32) -> Self {
        Self { lower: W::ZERO, upper: W::ZERO, width }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantRange.h#L85>
    pub fn non_empty(
        width: u32,
        lower: W,
        upper: W,
    ) -> Self {
        if lower & mask::<W>(width) == upper & mask::<W>(width) {
            Self::full(width)
        } else {
            Self::new(width, lower, upper)
        }
    }

    /// `isFullSet`.
    pub fn is_full_set(&self) -> bool {
        self.lower == self.upper && self.upper == mask::<W>(self.width)
    }

    /// `isEmptySet`.
    pub fn is_empty_set(&self) -> bool {
        self.lower == self.upper && self.upper == W::ZERO
    }

    /// The same range in another word; the bounds fit it by construction.
    pub fn to_word<V: Word>(&self) -> Range<V> {
        Range {
            lower: V::from_u128(self.lower.low_u128()),
            upper: V::from_u128(self.upper.low_u128()),
            width: self.width,
        }
    }

    fn w(&self) -> u32 {
        self.width
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L61>
    pub fn from_known_bits(
        known: &Bits<W>,
        is_signed: bool,
    ) -> Self {
        let width = known.width;
        if known.has_conflict() {
            return Self::empty(width);
        }
        if known.is_unknown() {
            return Self::full(width);
        }
        if !is_signed || known.is_negative() || known.is_non_negative() {
            return Self::new(width, known.min_value(), add(known.max_value(), W::ONE, width));
        }
        let lower = known.min_value() | sign_bit::<W>(width);
        let upper = known.max_value() & !sign_bit::<W>(width);
        Self::new(width, lower, add(upper, W::ONE, width))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L81>
    pub fn to_known_bits(&self) -> Bits<W> {
        if self.is_empty_set() {
            return Bits::<W>::unknown(self.width);
        }
        let (min_value, max_value) = (self.get_unsigned_min(), self.get_unsigned_max());
        let mut known = Bits::<W>::constant(self.width, min_value);
        if min_value != max_value {
            let different = self.width - 1 - countl_zero(min_value ^ max_value, self.width);
            known.zero &= !mask::<W>(different + 1);
            known.one &= !mask::<W>(different + 1);
        }
        known
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L99>
    pub fn split_pos_neg(&self) -> (Self, Self) {
        let w = self.width;
        let pos_filter = if w == 1 { Self::empty(w) } else { Self::new(w, W::ONE, smin_val(w)) };
        let neg_filter = Self::new(w, smin_val(w), W::ZERO);
        (
            self.intersect_with(&pos_filter, PreferredRangeType::Smallest),
            self.intersect_with(&neg_filter, PreferredRangeType::Smallest),
        )
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L110>
    pub fn make_allowed_icmp_region(
        pred: IntPredicate,
        cr: &Self,
    ) -> Self {
        if cr.is_empty_set() {
            return *cr;
        }
        let w = cr.width;
        match pred {
            IntPredicate::Eq => *cr,
            IntPredicate::Ne => {
                if cr.is_single_element() {
                    return Self::new(w, cr.upper, cr.lower);
                }
                Self::full(w)
            }
            IntPredicate::Ult => {
                let umax = cr.get_unsigned_max();
                if umax == W::ZERO {
                    return Self::empty(w);
                }
                Self::new(w, W::ZERO, umax)
            }
            IntPredicate::Slt => {
                let smax = cr.get_signed_max();
                if smax == smin_val(w) {
                    return Self::empty(w);
                }
                Self::new(w, smin_val(w), smax)
            }
            IntPredicate::Ule => Self::non_empty(w, W::ZERO, add(cr.get_unsigned_max(), W::ONE, w)),
            IntPredicate::Sle => Self::non_empty(w, smin_val(w), add(cr.get_signed_max(), W::ONE, w)),
            IntPredicate::Ugt => {
                let umin = cr.get_unsigned_min();
                if umin == mask::<W>(w) {
                    return Self::empty(w);
                }
                Self::new(w, add(umin, W::ONE, w), W::ZERO)
            }
            IntPredicate::Sgt => {
                let smin = cr.get_signed_min();
                if smin == smax_val(w) {
                    return Self::empty(w);
                }
                Self::new(w, add(smin, W::ONE, w), smin_val(w))
            }
            IntPredicate::Uge => Self::non_empty(w, cr.get_unsigned_min(), W::ZERO),
            IntPredicate::Sge => Self::non_empty(w, cr.get_signed_min(), smin_val(w)),
        }
    }

    /// The `CmpPredicate` form: with `samesign` the operands have one sign, so
    /// the region is also the one the signed predicate allows.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L162>
    pub fn make_allowed_icmp_region_samesign(
        pred: IntPredicate,
        samesign: bool,
        cr: &Self,
    ) -> Self {
        let result = Self::make_allowed_icmp_region(pred, cr);
        if !samesign {
            return result;
        }
        let signed_pred = match pred {
            IntPredicate::Ugt => IntPredicate::Sgt,
            IntPredicate::Uge => IntPredicate::Sge,
            IntPredicate::Ult => IntPredicate::Slt,
            IntPredicate::Ule => IntPredicate::Sle,
            other => other,
        };
        result.intersect_with(&Self::make_allowed_icmp_region(signed_pred, cr), PreferredRangeType::Smallest)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L169>
    pub fn make_satisfying_icmp_region(
        pred: IntPredicate,
        cr: &Self,
    ) -> Self {
        Self::make_allowed_icmp_region(pred.inverse(), cr).inverse()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L179>
    pub fn make_exact_icmp_region(
        pred: IntPredicate,
        width: u32,
        value: W,
    ) -> Self {
        Self::make_allowed_icmp_region(pred, &Self::single(width, value))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L190>
    pub fn are_insensitive_to_signedness_of_icmp_predicate(
        cr1: &Self,
        cr2: &Self,
    ) -> bool {
        if cr1.is_empty_set() || cr2.is_empty_set() {
            return true;
        }
        (cr1.is_all_non_negative() && cr2.is_all_non_negative()) || (cr1.is_all_negative() && cr2.is_all_negative())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L199>
    pub fn are_insensitive_to_signedness_of_inverted_icmp_predicate(
        cr1: &Self,
        cr2: &Self,
    ) -> bool {
        if cr1.is_empty_set() || cr2.is_empty_set() {
            return true;
        }
        (cr1.is_all_non_negative() && cr2.is_all_negative()) || (cr1.is_all_negative() && cr2.is_all_non_negative())
    }

    /// `ICmpInst::getFlippedSignednessPredicate` of a relational predicate.
    fn flipped_signedness(pred: IntPredicate) -> IntPredicate {
        match pred {
            IntPredicate::Ugt => IntPredicate::Sgt,
            IntPredicate::Uge => IntPredicate::Sge,
            IntPredicate::Ult => IntPredicate::Slt,
            IntPredicate::Ule => IntPredicate::Sle,
            IntPredicate::Sgt => IntPredicate::Ugt,
            IntPredicate::Sge => IntPredicate::Uge,
            IntPredicate::Slt => IntPredicate::Ult,
            IntPredicate::Sle => IntPredicate::Ule,
            IntPredicate::Eq | IntPredicate::Ne => panic!("Only for relational integer predicates!"),
        }
    }

    /// `None` is `BAD_ICMP_PREDICATE`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L208>
    pub fn get_equivalent_pred_with_flipped_signedness(
        pred: IntPredicate,
        cr1: &Self,
        cr2: &Self,
    ) -> Option<IntPredicate> {
        let flipped = Self::flipped_signedness(pred);
        if Self::are_insensitive_to_signedness_of_icmp_predicate(cr1, cr2) {
            return Some(flipped);
        }
        if Self::are_insensitive_to_signedness_of_inverted_icmp_predicate(cr1, cr2) {
            return Some(flipped.inverse());
        }
        None
    }

    /// `(Pred, RHS, Offset)` such that `x in this` is `Pred (x + Offset), RHS`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L226>
    pub fn get_equivalent_icmp_offset(&self) -> (IntPredicate, W, W) {
        let w = self.width;
        let mut offset = W::ZERO;
        let (pred, rhs);
        if self.is_full_set() || self.is_empty_set() {
            pred = if self.is_empty_set() { IntPredicate::Ult } else { IntPredicate::Uge };
            rhs = W::ZERO;
        } else if let Some(only) = self.get_single_element() {
            pred = IntPredicate::Eq;
            rhs = only;
        } else if let Some(missing) = self.get_single_missing_element() {
            pred = IntPredicate::Ne;
            rhs = missing;
        } else if self.lower == smin_val(w) || self.lower == W::ZERO {
            pred = if self.lower == smin_val(w) { IntPredicate::Slt } else { IntPredicate::Ult };
            rhs = self.upper;
        } else if self.upper == smin_val(w) || self.upper == W::ZERO {
            pred = if self.upper == smin_val(w) { IntPredicate::Sge } else { IntPredicate::Uge };
            rhs = self.lower;
        } else {
            pred = IntPredicate::Ult;
            rhs = sub(self.upper, self.lower, w);
            offset = neg(self.lower, w);
        }
        debug_assert_eq!(Self::make_exact_icmp_region(pred, w, rhs), self.add(&Self::single(w, offset)), "Bad result!");
        (pred, rhs, offset)
    }

    /// `Some((Pred, RHS))` where no offset is needed.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L226>
    pub fn get_equivalent_icmp(&self) -> Option<(IntPredicate, W)> {
        let (pred, rhs, offset) = self.get_equivalent_icmp_offset();
        (offset == W::ZERO).then_some((pred, rhs))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L263>
    pub fn icmp(
        &self,
        pred: IntPredicate,
        other: &Self,
    ) -> bool {
        if self.is_empty_set() || other.is_empty_set() {
            return true;
        }
        let w = self.width;
        match pred {
            IntPredicate::Eq => match (self.get_single_element(), other.get_single_element()) {
                (Some(l), Some(r)) => l == r,
                _ => false,
            },
            IntPredicate::Ne => self.inverse().contains_range(other),
            IntPredicate::Ult => self.get_unsigned_max() < other.get_unsigned_min(),
            IntPredicate::Ule => self.get_unsigned_max() <= other.get_unsigned_min(),
            IntPredicate::Ugt => self.get_unsigned_min() > other.get_unsigned_max(),
            IntPredicate::Uge => self.get_unsigned_min() >= other.get_unsigned_max(),
            IntPredicate::Slt => slt(self.get_signed_max(), other.get_signed_min(), w),
            IntPredicate::Sle => !sgt(self.get_signed_max(), other.get_signed_min(), w),
            IntPredicate::Sgt => sgt(self.get_signed_min(), other.get_signed_max(), w),
            IntPredicate::Sge => !slt(self.get_signed_min(), other.get_signed_max(), w),
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L417>
    pub fn make_mask_not_equal_range(
        width: u32,
        mask_bits: W,
        c: W,
    ) -> Self {
        if mask_bits & c != c {
            return Self::full(width);
        }
        if mask_bits == W::ZERO {
            return Self::empty(width);
        }
        Self::non_empty(width, add(W::ONE << countr_zero(mask_bits, width), c, width), c)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L442>
    pub fn is_wrapped_set(&self) -> bool {
        self.lower > self.upper && self.upper != W::ZERO
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L446>
    pub fn is_upper_wrapped(&self) -> bool {
        self.lower > self.upper
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L450>
    pub fn is_sign_wrapped_set(&self) -> bool {
        sgt(self.lower, self.upper, self.w()) && self.upper != smin_val(self.w())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L454>
    pub fn is_upper_sign_wrapped(&self) -> bool {
        sgt(self.lower, self.upper, self.w())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L459>
    pub fn is_size_strictly_smaller_than(
        &self,
        other: &Self,
    ) -> bool {
        assert_eq!(self.width, other.width);
        if self.is_full_set() {
            return false;
        }
        if other.is_full_set() {
            return true;
        }
        sub(self.upper, self.lower, self.w()) < sub(other.upper, other.lower, self.w())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L469>
    pub fn is_size_larger_than(
        &self,
        max_size: u64,
    ) -> bool {
        if self.is_full_set() {
            return max_size == 0 || mask::<W>(self.width) > W::from_u128(u128::from(max_size - 1));
        }
        sub(self.upper, self.lower, self.w()) > W::from_u128(u128::from(max_size))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L478>
    pub fn is_all_negative(&self) -> bool {
        if self.is_empty_set() {
            return true;
        }
        if self.is_full_set() {
            return false;
        }
        !self.is_upper_sign_wrapped() && !is_strictly_positive(self.upper, self.w())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L488>
    pub fn is_all_non_negative(&self) -> bool {
        !self.is_sign_wrapped_set() && !is_negative(self.lower, self.w())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L493>
    pub fn is_all_positive(&self) -> bool {
        if self.is_empty_set() {
            return true;
        }
        if self.is_full_set() {
            return false;
        }
        !self.is_sign_wrapped_set() && is_strictly_positive(self.lower, self.w())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L503>
    pub fn get_unsigned_max(&self) -> W {
        if self.is_full_set() || self.is_upper_wrapped() {
            return mask::<W>(self.width);
        }
        sub(self.upper, W::ONE, self.w())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L509>
    pub fn get_unsigned_min(&self) -> W {
        if self.is_full_set() || self.is_wrapped_set() {
            return W::ZERO;
        }
        self.lower
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L515>
    pub fn get_signed_max(&self) -> W {
        if self.is_full_set() || self.is_upper_sign_wrapped() {
            return smax_val(self.width);
        }
        sub(self.upper, W::ONE, self.w())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L521>
    pub fn get_signed_min(&self) -> W {
        if self.is_full_set() || self.is_sign_wrapped_set() {
            return smin_val(self.width);
        }
        self.lower
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L527>
    pub fn contains(
        &self,
        v: W,
    ) -> bool {
        if self.lower == self.upper {
            return self.is_full_set();
        }
        if !self.is_upper_wrapped() {
            return self.lower <= v && v < self.upper;
        }
        self.lower <= v || v < self.upper
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L527>
    pub fn contains_range(
        &self,
        other: &Self,
    ) -> bool {
        if self.is_full_set() || other.is_empty_set() {
            return true;
        }
        if self.is_empty_set() || other.is_full_set() {
            return false;
        }
        if !self.is_upper_wrapped() {
            if other.is_upper_wrapped() {
                return false;
            }
            return self.lower <= other.lower && other.upper <= self.upper;
        }
        if !other.is_upper_wrapped() {
            return other.upper <= self.upper || self.lower <= other.lower;
        }
        other.upper <= self.upper && self.lower <= other.lower
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantRange.h#L265>
    pub fn get_single_element(&self) -> Option<W> {
        (self.upper == add(self.lower, W::ONE, self.w())).then_some(self.lower)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantRange.h#L273>
    pub fn get_single_missing_element(&self) -> Option<W> {
        (self.lower == add(self.upper, W::ONE, self.w())).then_some(self.upper)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ConstantRange.h#L280>
    pub fn is_single_element(&self) -> bool {
        self.get_single_element().is_some()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L554>
    pub fn get_active_bits(&self) -> u32 {
        if self.is_empty_set() {
            return 0;
        }
        active_bits(self.get_unsigned_max())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L561>
    pub fn get_min_signed_bits(&self) -> u32 {
        if self.is_empty_set() {
            return 0;
        }
        max(significant_bits(self.get_signed_min(), self.w()), significant_bits(self.get_signed_max(), self.w()))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L569>
    pub fn subtract(
        &self,
        val: W,
    ) -> Self {
        if self.lower == self.upper {
            return *self;
        }
        Self::new(self.width, sub(self.lower, val, self.w()), sub(self.upper, val, self.w()))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L577>
    pub fn difference(
        &self,
        cr: &Self,
    ) -> Self {
        self.intersect_with(&cr.inverse(), PreferredRangeType::Smallest)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L581>
    fn preferred_range(
        cr1: &Self,
        cr2: &Self,
        kind: PreferredRangeType,
    ) -> Self {
        match kind {
            PreferredRangeType::Unsigned => {
                if !cr1.is_wrapped_set() && cr2.is_wrapped_set() {
                    return *cr1;
                }
                if cr1.is_wrapped_set() && !cr2.is_wrapped_set() {
                    return *cr2;
                }
            }
            PreferredRangeType::Signed => {
                if !cr1.is_sign_wrapped_set() && cr2.is_sign_wrapped_set() {
                    return *cr1;
                }
                if cr1.is_sign_wrapped_set() && !cr2.is_sign_wrapped_set() {
                    return *cr2;
                }
            }
            PreferredRangeType::Smallest => {}
        }
        if cr1.is_size_strictly_smaller_than(cr2) { *cr1 } else { *cr2 }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L601>
    pub fn intersect_with(
        &self,
        cr: &Self,
        kind: PreferredRangeType,
    ) -> Self {
        assert_eq!(self.width, cr.width, "ConstantRange types don't agree!");
        let w = self.width;
        let (lower, upper) = (self.lower, self.upper);
        if self.is_empty_set() || cr.is_full_set() {
            return *self;
        }
        if cr.is_empty_set() || self.is_full_set() {
            return *cr;
        }
        if !self.is_upper_wrapped() && cr.is_upper_wrapped() {
            return cr.intersect_with(self, kind);
        }
        if !self.is_upper_wrapped() && !cr.is_upper_wrapped() {
            if lower < cr.lower {
                if upper <= cr.lower {
                    return Self::empty(w);
                }
                if upper < cr.upper {
                    return Self::new(w, cr.lower, upper);
                }
                return *cr;
            }
            if upper < cr.upper {
                return *self;
            }
            if lower < cr.upper {
                return Self::new(w, lower, cr.upper);
            }
            return Self::empty(w);
        }
        if self.is_upper_wrapped() && !cr.is_upper_wrapped() {
            if cr.lower < upper {
                if cr.upper < upper {
                    return *cr;
                }
                if cr.upper <= lower {
                    return Self::new(w, cr.lower, upper);
                }
                return Self::preferred_range(self, cr, kind);
            }
            if cr.lower < lower {
                if cr.upper <= lower {
                    return Self::empty(w);
                }
                return Self::new(w, lower, cr.upper);
            }
            return *cr;
        }
        if cr.upper < upper {
            if cr.lower < upper {
                return Self::preferred_range(self, cr, kind);
            }
            if cr.lower < lower {
                return Self::new(w, lower, cr.upper);
            }
            return *cr;
        }
        if cr.upper <= lower {
            if cr.lower < lower {
                return *self;
            }
            return Self::new(w, cr.lower, upper);
        }
        Self::preferred_range(self, cr, kind)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L707>
    pub fn union_with(
        &self,
        cr: &Self,
        kind: PreferredRangeType,
    ) -> Self {
        assert_eq!(self.width, cr.width, "ConstantRange types don't agree!");
        let w = self.width;
        let (lower, upper) = (self.lower, self.upper);
        if self.is_full_set() || cr.is_empty_set() {
            return *self;
        }
        if cr.is_full_set() || self.is_empty_set() {
            return *cr;
        }
        if !self.is_upper_wrapped() && cr.is_upper_wrapped() {
            return cr.union_with(self, kind);
        }
        if !self.is_upper_wrapped() && !cr.is_upper_wrapped() {
            if cr.upper < lower || upper < cr.lower {
                return Self::preferred_range(&Self::new(w, lower, cr.upper), &Self::new(w, cr.lower, upper), kind);
            }
            let l = if cr.lower < lower { cr.lower } else { lower };
            let u = if sub(cr.upper, W::ONE, w) > sub(upper, W::ONE, w) { cr.upper } else { upper };
            if l == W::ZERO && u == W::ZERO {
                return Self::full(w);
            }
            return Self::new(w, l, u);
        }
        if !cr.is_upper_wrapped() {
            if cr.upper <= upper || cr.lower >= lower {
                return *self;
            }
            if cr.lower <= upper && lower <= cr.upper {
                return Self::full(w);
            }
            if upper < cr.lower && cr.upper < lower {
                return Self::preferred_range(&Self::new(w, lower, cr.upper), &Self::new(w, cr.lower, upper), kind);
            }
            if upper < cr.lower && lower <= cr.upper {
                return Self::new(w, cr.lower, upper);
            }
            assert!(cr.lower <= upper && cr.upper < lower, "Self::unionWith missed a case with one range wrapped");
            return Self::new(w, lower, cr.upper);
        }
        if cr.lower <= upper || lower <= cr.upper {
            return Self::full(w);
        }
        let l = if cr.lower < lower { cr.lower } else { lower };
        let u = if cr.upper > upper { cr.upper } else { upper };
        Self::new(w, l, u)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L781>
    pub fn exact_intersect_with(
        &self,
        cr: &Self,
    ) -> Option<Self> {
        let result = self.intersect_with(cr, PreferredRangeType::Smallest);
        let check = self.inverse().union_with(&cr.inverse(), PreferredRangeType::Smallest).inverse();
        (result == check).then_some(result)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L790>
    pub fn exact_union_with(
        &self,
        cr: &Self,
    ) -> Option<Self> {
        let result = self.union_with(cr, PreferredRangeType::Smallest);
        let check = self.inverse().intersect_with(&cr.inverse(), PreferredRangeType::Smallest).inverse();
        (result == check).then_some(result)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L798>
    pub fn cast_op(
        &self,
        cast: CastOp,
        result_width: u32,
    ) -> Self {
        let bw = self.width;
        match cast {
            CastOp::Trunc => self.truncate(result_width, 0),
            CastOp::SExt => self.sign_extend(result_width),
            CastOp::ZExt => self.zero_extend(result_width),
            CastOp::BitCast => *self,
            CastOp::FPToUI | CastOp::FPToSI => {
                if bw == result_width {
                    *self
                } else {
                    Self::full(result_width)
                }
            }
            CastOp::UIToFP => Self::non_empty(result_width, W::ZERO, add(mask::<W>(bw), W::ONE, result_width)),
            CastOp::SIToFP => {
                let (mut smin, mut smax) = (smin_val(bw), smax_val(bw));
                if result_width > bw {
                    smin = sext(smin, bw, result_width);
                    smax = sext(smax, bw, result_width);
                }
                Self::non_empty(result_width, smin, add(smax, W::ONE, result_width))
            }
            CastOp::FPTrunc
            | CastOp::FPExt
            | CastOp::FPRound
            | CastOp::IntToPtr
            | CastOp::PtrToInt
            | CastOp::AddrSpaceCast => Self::full(result_width),
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L850>
    pub fn zero_extend(
        &self,
        dst: u32,
    ) -> Self {
        if self.is_empty_set() {
            return Self::empty(dst);
        }
        let src = self.width;
        if dst == src {
            return *self;
        }
        assert!(src < dst, "Not a value extension");
        if self.is_full_set() || self.is_upper_wrapped() {
            let lower_ext = if self.upper == W::ZERO { self.lower } else { W::ZERO };
            return Self::new(dst, lower_ext, W::ONE << src);
        }
        Self::new(dst, self.lower, self.upper)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L869>
    pub fn sign_extend(
        &self,
        dst: u32,
    ) -> Self {
        if self.is_empty_set() {
            return Self::empty(dst);
        }
        let src = self.width;
        if dst == src {
            return *self;
        }
        assert!(src < dst, "Not a value extension");
        if self.upper == smin_val(src) {
            return Self::new(dst, sext(self.lower, src, dst), self.upper);
        }
        if self.is_full_set() || self.is_sign_wrapped_set() {
            return Self::new(dst, set_high(W::ZERO, dst, dst - src + 1), add(mask::<W>(src - 1), W::ONE, dst));
        }
        Self::new(dst, sext(self.lower, src, dst), sext(self.upper, src, dst))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L889>
    pub fn truncate(
        &self,
        dst: u32,
        no_wrap_kind: u32,
    ) -> Self {
        if dst == self.width {
            return *self;
        }
        assert!(self.width > dst, "Not a value truncation");
        if self.is_empty_set() {
            return Self::empty(dst);
        }
        if self.is_full_set() {
            return Self::full(dst);
        }
        let bw = self.width;
        let (mut lower_div, mut upper_div) = (self.lower, self.upper);
        let mut union = Self::empty(dst);
        if self.is_upper_wrapped() {
            if active_bits(self.upper) > dst {
                return Self::full(dst);
            }
            if no_wrap_kind & NO_UNSIGNED_WRAP != 0 {
                union = Self::new(dst, W::ZERO, self.upper & mask::<W>(dst));
                upper_div = W::ONE << dst;
            } else {
                if countr_one(self.upper, bw) == dst {
                    return Self::full(dst);
                }
                union = Self::new(dst, mask::<W>(dst), self.upper & mask::<W>(dst));
                upper_div = mask::<W>(bw);
                if lower_div == upper_div {
                    return union;
                }
            }
        }
        if active_bits(lower_div) > dst {
            if no_wrap_kind & NO_UNSIGNED_WRAP != 0 {
                return union;
            }
            let adjust = lower_div & set_range(W::ZERO, dst, bw);
            lower_div = sub(lower_div, adjust, bw);
            upper_div = sub(upper_div, adjust, bw);
        }
        let upper_div_width = active_bits(upper_div);
        if upper_div_width <= dst {
            return Self::new(dst, lower_div & mask::<W>(dst), upper_div & mask::<W>(dst))
                .union_with(&union, PreferredRangeType::Smallest);
        }
        if lower_div != W::ZERO && no_wrap_kind & NO_UNSIGNED_WRAP != 0 {
            return Self::new(dst, lower_div & mask::<W>(dst), W::ZERO)
                .union_with(&union, PreferredRangeType::Smallest);
        }
        if upper_div_width == dst + 1 {
            upper_div &= !(W::ONE << dst);
            if upper_div < lower_div {
                return Self::new(dst, lower_div & mask::<W>(dst), upper_div & mask::<W>(dst))
                    .union_with(&union, PreferredRangeType::Smallest);
            }
        }
        Self::full(dst)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L963>
    pub fn zext_or_trunc(
        &self,
        dst: u32,
    ) -> Self {
        match self.width.cmp(&dst) {
            std::cmp::Ordering::Greater => self.truncate(dst, 0),
            std::cmp::Ordering::Less => self.zero_extend(dst),
            std::cmp::Ordering::Equal => *self,
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L972>
    pub fn sext_or_trunc(
        &self,
        dst: u32,
    ) -> Self {
        match self.width.cmp(&dst) {
            std::cmp::Ordering::Greater => self.truncate(dst, 0),
            std::cmp::Ordering::Less => self.sign_extend(dst),
            std::cmp::Ordering::Equal => *self,
        }
    }

    /// `ops[1]` of `abs`, `ctlz` and `cttz` is the immarg flag as a 1-bit
    /// range.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1078>
    pub fn intrinsic(
        intrinsic: RangeIntrinsic,
        ops: &[Self],
    ) -> Self {
        let flag = |at: usize| {
            let value = ops[at].get_single_element().expect("Must be known (immarg)");
            assert_eq!(ops[at].width, 1, "Must be boolean");
            value != W::ZERO
        };
        match intrinsic {
            RangeIntrinsic::UAddSat => ops[0].uadd_sat(&ops[1]),
            RangeIntrinsic::USubSat => ops[0].usub_sat(&ops[1]),
            RangeIntrinsic::SAddSat => ops[0].sadd_sat(&ops[1]),
            RangeIntrinsic::SSubSat => ops[0].ssub_sat(&ops[1]),
            RangeIntrinsic::UMin => ops[0].umin(&ops[1]),
            RangeIntrinsic::UMax => ops[0].umax(&ops[1]),
            RangeIntrinsic::SMin => ops[0].smin(&ops[1]),
            RangeIntrinsic::SMax => ops[0].smax(&ops[1]),
            RangeIntrinsic::Abs => ops[0].abs(flag(1)),
            RangeIntrinsic::Ctlz => ops[0].ctlz(flag(1)),
            RangeIntrinsic::Cttz => ops[0].cttz(flag(1)),
            RangeIntrinsic::Ctpop => ops[0].ctpop(),
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1124>
    pub fn add(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        if self.is_full_set() || other.is_full_set() {
            return Self::full(w);
        }
        let new_lower = add(self.lower, other.lower, w);
        let new_upper = sub(add(self.upper, other.upper, w), W::ONE, w);
        if new_lower == new_upper {
            return Self::full(w);
        }
        let x = Self::new(w, new_lower, new_upper);
        if x.is_size_strictly_smaller_than(self) || x.is_size_strictly_smaller_than(other) {
            return Self::full(w);
        }
        x
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1143>
    pub fn add_with_no_wrap(
        &self,
        other: &Self,
        no_wrap_kind: u32,
        range_type: PreferredRangeType,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        if self.is_full_set() && other.is_full_set() {
            return Self::full(w);
        }
        let mut result = self.add(other);
        if no_wrap_kind & NO_SIGNED_WRAP != 0 {
            result = result.intersect_with(&self.sadd_sat(other), range_type);
        }
        if no_wrap_kind & NO_UNSIGNED_WRAP != 0 {
            result = result.intersect_with(&self.uadd_sat(other), range_type);
        }
        result
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1171>
    pub fn sub(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        if self.is_full_set() || other.is_full_set() {
            return Self::full(w);
        }
        let new_lower = add(sub(self.lower, other.upper, w), W::ONE, w);
        let new_upper = sub(self.upper, other.lower, w);
        if new_lower == new_upper {
            return Self::full(w);
        }
        let x = Self::new(w, new_lower, new_upper);
        if x.is_size_strictly_smaller_than(self) || x.is_size_strictly_smaller_than(other) {
            return Self::full(w);
        }
        x
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1190>
    pub fn sub_with_no_wrap(
        &self,
        other: &Self,
        no_wrap_kind: u32,
        range_type: PreferredRangeType,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        if self.is_full_set() && other.is_full_set() {
            return Self::full(w);
        }
        let mut result = self.sub(other);
        if no_wrap_kind & NO_SIGNED_WRAP != 0 {
            result = result.intersect_with(&self.ssub_sat(other), range_type);
        }
        if no_wrap_kind & NO_UNSIGNED_WRAP != 0 {
            if self.get_unsigned_max() < other.get_unsigned_min() {
                return Self::empty(w);
            }
            result = result.intersect_with(&self.usub_sat(other), range_type);
        }
        result
    }

    /// `multiply` in a word that holds twice the width.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1220>
    pub fn multiply_at_double_width(
        &self,
        other: &Self,
        no_wrap_kind: u32,
    ) -> Self {
        let bw = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(bw);
        }
        if let Some(c) = self.get_single_element() {
            if c == W::ONE {
                return *other;
            }
            if c == mask::<W>(bw) {
                return Self::single(bw, W::ZERO).sub(other);
            }
        }
        if let Some(c) = other.get_single_element() {
            if c == W::ONE {
                return *self;
            }
            if c == mask::<W>(bw) {
                return Self::single(bw, W::ZERO).sub(self);
            }
        }
        let wide = bw * 2;
        assert!(wide <= W::BITS, "multiply needs a word of {wide} bits");
        let ur;
        if no_wrap_kind & NO_UNSIGNED_WRAP != 0 {
            let (min_mul, min_ov) = umul_ov(self.get_unsigned_min(), other.get_unsigned_min(), bw);
            if min_ov {
                return Self::empty(bw);
            }
            let max_mul = umul_sat(self.get_unsigned_max(), other.get_unsigned_max(), bw);
            ur = Self::non_empty(bw, min_mul, add(max_mul, W::ONE, bw));
        } else {
            let (this_min, this_max) = (self.get_unsigned_min(), self.get_unsigned_max());
            let (other_min, other_max) = (other.get_unsigned_min(), other.get_unsigned_max());
            let result_zext =
                Self::new(wide, this_min.wrapping_mul(other_min), add(this_max.wrapping_mul(other_max), W::ONE, wide));
            ur = result_zext.truncate(bw, 0);
        }
        if no_wrap_kind & NO_SIGNED_WRAP == 0
            && !ur.is_upper_wrapped()
            && (!is_negative(ur.upper, bw) || ur.upper == smin_val(bw))
        {
            return ur;
        }
        let wide_signed = |v: W| sext(v, bw, wide);
        let (this_min, this_max) = (wide_signed(self.get_signed_min()), wide_signed(self.get_signed_max()));
        let (other_min, other_max) = (wide_signed(other.get_signed_min()), wide_signed(other.get_signed_max()));
        let product = |a: W, b: W| a.wrapping_mul(b) & mask::<W>(wide);
        let candidates = [
            product(this_min, other_min),
            product(this_min, other_max),
            product(this_max, other_min),
            product(this_max, other_max),
        ];
        let lowest = candidates.iter().copied().fold(candidates[0], |m, c| smin_of_two(m, c, wide));
        let highest = candidates.iter().copied().fold(candidates[0], |m, c| smax_of_two(m, c, wide));
        let mut result_sext = Self::new(wide, lowest, add(highest, W::ONE, wide));
        if no_wrap_kind & NO_SIGNED_WRAP != 0 {
            result_sext = result_sext.intersect_with(
                &Self::new(wide, wide_signed(smin_val(bw)), add(wide_signed(smax_val(bw)), W::ONE, wide)),
                PreferredRangeType::Smallest,
            );
        }
        let sr = result_sext.truncate(bw, 0);
        let mut result = if ur.is_size_strictly_smaller_than(&sr) { ur } else { sr };
        if no_wrap_kind == (NO_SIGNED_WRAP | NO_UNSIGNED_WRAP)
            && !result.is_all_non_negative()
            && (sgt(self.get_signed_min(), W::ONE, bw) || sgt(other.get_signed_min(), W::ONE, bw))
        {
            result = result.intersect_with(&Self::non_empty(bw, W::ZERO, smin_val(bw)), PreferredRangeType::Smallest);
        }
        result
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1338>
    pub fn smax(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = smax_of_two(self.get_signed_min(), other.get_signed_min(), w);
        let new_u = add(smax_of_two(self.get_signed_max(), other.get_signed_max(), w), W::ONE, w);
        let res = Self::non_empty(w, new_l, new_u);
        if self.is_sign_wrapped_set() || other.is_sign_wrapped_set() {
            return res.intersect_with(&self.union_with(other, PreferredRangeType::Signed), PreferredRangeType::Signed);
        }
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1352>
    pub fn umax(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = max(self.get_unsigned_min(), other.get_unsigned_min());
        let new_u = add(max(self.get_unsigned_max(), other.get_unsigned_max()), W::ONE, w);
        let res = Self::non_empty(w, new_l, new_u);
        if self.is_wrapped_set() || other.is_wrapped_set() {
            return res
                .intersect_with(&self.union_with(other, PreferredRangeType::Unsigned), PreferredRangeType::Unsigned);
        }
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1366>
    pub fn smin(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = smin_of_two(self.get_signed_min(), other.get_signed_min(), w);
        let new_u = add(smin_of_two(self.get_signed_max(), other.get_signed_max(), w), W::ONE, w);
        let res = Self::non_empty(w, new_l, new_u);
        if self.is_sign_wrapped_set() || other.is_sign_wrapped_set() {
            return res.intersect_with(&self.union_with(other, PreferredRangeType::Signed), PreferredRangeType::Signed);
        }
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1380>
    pub fn umin(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = min(self.get_unsigned_min(), other.get_unsigned_min());
        let new_u = add(min(self.get_unsigned_max(), other.get_unsigned_max()), W::ONE, w);
        let res = Self::non_empty(w, new_l, new_u);
        if self.is_wrapped_set() || other.is_wrapped_set() {
            return res
                .intersect_with(&self.union_with(other, PreferredRangeType::Unsigned), PreferredRangeType::Unsigned);
        }
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1569>
    pub fn binary_not(&self) -> Self {
        Self::single(self.width, mask::<W>(self.width)).sub(self)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1597>
    fn estimate_bit_masked_and_lower_bound(
        lhs: &Self,
        rhs: &Self,
    ) -> W {
        let bw = lhs.width;
        if lhs.is_full_set() || rhs.is_full_set() || lhs.is_wrapped_set() || rhs.is_wrapped_set() {
            return W::ZERO;
        }
        let (l_lo, l_hi) = (lhs.lower, sub(lhs.upper, W::ONE, bw));
        let (r_lo, r_hi) = (rhs.lower, sub(rhs.upper, W::ONE, bw));
        let mut mask_bits = !((l_lo ^ l_hi) | (r_lo ^ r_hi) | (l_lo ^ r_lo)) & mask::<W>(bw);
        let leading_ones = countl_one(mask_bits, bw);
        mask_bits &= !mask::<W>(bw - leading_ones);
        let estimate_bound = |a_lo: W, b_lo: W, b_hi: W| {
            let leading_ones = countl_one((b_lo & b_hi) | mask_bits, bw);
            a_lo & !mask::<W>(bw - leading_ones)
        };
        max(estimate_bound(l_lo, r_lo, r_hi), estimate_bound(r_lo, l_lo, l_hi))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1630>
    pub fn binary_and(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let known_bits_range = Self::from_known_bits(&self.to_known_bits().and(&other.to_known_bits()), false);
        let lower_bound = Self::estimate_bit_masked_and_lower_bound(self, other);
        let umin_umax_range =
            Self::non_empty(w, lower_bound, add(min(other.get_unsigned_max(), self.get_unsigned_max()), W::ONE, w));
        known_bits_range.intersect_with(&umin_umax_range, PreferredRangeType::Smallest)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1642>
    pub fn binary_or(
        &self,
        other: &Self,
        is_disjoint: bool,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let known_bits_range = Self::from_known_bits(&self.to_known_bits().or(&other.to_known_bits()), false);
        let upper_bound = neg(Self::estimate_bit_masked_and_lower_bound(&self.binary_not(), &other.binary_not()), w);
        let umax_umin_range = Self::non_empty(w, max(self.get_unsigned_min(), other.get_unsigned_min()), upper_bound);
        let mut result = known_bits_range.intersect_with(&umax_umin_range, PreferredRangeType::Smallest);
        if is_disjoint {
            result = self
                .add_with_no_wrap(other, NO_UNSIGNED_WRAP | NO_SIGNED_WRAP, PreferredRangeType::Smallest)
                .intersect_with(&result, PreferredRangeType::Smallest);
        }
        result
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1672>
    pub fn binary_xor(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        if let (Some(a), Some(b)) = (self.get_single_element(), other.get_single_element()) {
            return Self::single(w, a ^ b);
        }
        if other.get_single_element() == Some(mask::<W>(w)) {
            return self.binary_not();
        }
        if self.get_single_element() == Some(mask::<W>(w)) {
            return other.binary_not();
        }
        let lhs_known = self.to_known_bits();
        let rhs_known = other.to_known_bits();
        let known = lhs_known.xor(&rhs_known);
        let mut cr = Self::from_known_bits(&known, false);
        if w == 1 {
            return cr;
        }
        if !lhs_known.zero & mask::<W>(w) & !rhs_known.one == W::ZERO {
            cr = cr.intersect_with(&other.sub(self), PreferredRangeType::Unsigned);
        } else if !rhs_known.zero & mask::<W>(w) & !lhs_known.one == W::ZERO {
            cr = cr.intersect_with(&self.sub(other), PreferredRangeType::Unsigned);
        }
        cr
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1705>
    pub fn shl(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let (mut min_value, mut max_value) = (self.get_unsigned_min(), self.get_unsigned_max());
        if let Some(rhs) = other.get_single_element() {
            if rhs >= W::from_u32(w) {
                return Self::empty(w);
            }
            let equal_leading_bits = countl_zero(min_value ^ max_value, w);
            if rhs <= W::from_u32(equal_leading_bits) {
                let rhs = rhs.low_u128() as u32;
                return Self::non_empty(w, shl(min_value, w, rhs), add(shl(max_value, w, rhs), W::ONE, w));
            }
            return Self::non_empty(w, W::ZERO, add(set_range(W::ZERO, rhs.low_u128() as u32, w), W::ONE, w));
        }
        let other_max = other.get_unsigned_max();
        if self.is_all_negative() && other_max <= W::from_u32(countl_one(min_value, w)) {
            max_value = shl(max_value, w, limited(other.get_unsigned_min(), w));
            min_value = shl(min_value, w, limited(other_max, w));
            return Self::non_empty(w, min_value, add(max_value, W::ONE, w));
        }
        if other_max > W::from_u32(countl_zero(max_value, w)) {
            return Self::full(w);
        }
        min_value = shl(min_value, w, limited(other.get_unsigned_min(), w));
        max_value = shl(max_value, w, limited(other_max, w));
        Self::non_empty(w, min_value, add(max_value, W::ONE, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1745>
    fn compute_shl_nuw(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let w = lhs.width;
        let lhs_min = lhs.get_unsigned_min();
        let mut rhs_min = limited(rhs.get_unsigned_min(), w);
        let (min_shl, overflow) = ushl_ov(lhs_min, w, rhs_min);
        if overflow {
            return Self::empty(w);
        }
        let lhs_max = lhs.get_unsigned_max();
        let mut rhs_max = limited(rhs.get_unsigned_max(), w);
        let mut max_shl = min_shl;
        let max_sh_amt = countl_zero(lhs_max, w);
        if rhs_min <= max_sh_amt {
            max_shl = shl(lhs_max, w, min(rhs_max, max_sh_amt));
        }
        rhs_min = max(rhs_min, max_sh_amt + 1);
        rhs_max = min(rhs_max, countl_zero(lhs_min, w));
        if rhs_min <= rhs_max {
            max_shl = max(max_shl, set_high(W::ZERO, w, w - rhs_min));
        }
        Self::non_empty(w, min_shl, add(max_shl, W::ONE, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1768>
    fn compute_shl_nsw_with_nneg_lhs(
        w: u32,
        lhs_min: W,
        lhs_max: W,
        mut rhs_min: u32,
        mut rhs_max: u32,
    ) -> Self {
        let (min_shl, overflow) = sshl_ov(lhs_min, w, rhs_min);
        if overflow {
            return Self::empty(w);
        }
        let mut max_shl = min_shl;
        let max_sh_amt = countl_zero(lhs_max, w).wrapping_sub(1);
        if rhs_min <= max_sh_amt {
            max_shl = shl(lhs_max, w, min(rhs_max, max_sh_amt));
        }
        rhs_min = max(rhs_min, max_sh_amt.wrapping_add(1));
        rhs_max = min(rhs_max, countl_zero(lhs_min, w).wrapping_sub(1));
        if rhs_min <= rhs_max {
            max_shl = max(max_shl, set_range(W::ZERO, rhs_min, w - 1));
        }
        Self::non_empty(w, min_shl, add(max_shl, W::ONE, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1789>
    fn compute_shl_nsw_with_neg_lhs(
        w: u32,
        lhs_min: W,
        lhs_max: W,
        mut rhs_min: u32,
        mut rhs_max: u32,
    ) -> Self {
        let (max_shl, overflow) = sshl_ov(lhs_max, w, rhs_min);
        if overflow {
            return Self::empty(w);
        }
        let mut min_shl = max_shl;
        let max_sh_amt = countl_one(lhs_min, w).wrapping_sub(1);
        if rhs_min <= max_sh_amt {
            min_shl = shl(lhs_min, w, min(rhs_max, max_sh_amt));
        }
        rhs_min = max(rhs_min, max_sh_amt.wrapping_add(1));
        rhs_max = min(rhs_max, countl_one(lhs_max, w).wrapping_sub(1));
        if rhs_min <= rhs_max {
            min_shl = sign_bit::<W>(w);
        }
        Self::non_empty(w, min_shl, add(max_shl, W::ONE, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1808>
    fn compute_shl_nsw(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let w = lhs.width;
        let rhs_min = limited(rhs.get_unsigned_min(), w);
        let rhs_max = limited(rhs.get_unsigned_max(), w);
        let (lhs_min, lhs_max) = (lhs.get_signed_min(), lhs.get_signed_max());
        if !is_negative(lhs_min, w) {
            return Self::compute_shl_nsw_with_nneg_lhs(w, lhs_min, lhs_max, rhs_min, rhs_max);
        } else if is_negative(lhs_max, w) {
            return Self::compute_shl_nsw_with_neg_lhs(w, lhs_min, lhs_max, rhs_min, rhs_max);
        }
        Self::compute_shl_nsw_with_nneg_lhs(w, W::ZERO, lhs_max, rhs_min, rhs_max).union_with(
            &Self::compute_shl_nsw_with_neg_lhs(w, lhs_min, mask::<W>(w), rhs_min, rhs_max),
            PreferredRangeType::Signed,
        )
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1826>
    pub fn shl_with_no_wrap(
        &self,
        other: &Self,
        no_wrap_kind: u32,
        range_type: PreferredRangeType,
    ) -> Self {
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(self.width);
        }
        match no_wrap_kind {
            0 => self.shl(other),
            NO_SIGNED_WRAP => Self::compute_shl_nsw(self, other),
            NO_UNSIGNED_WRAP => Self::compute_shl_nuw(self, other),
            3 => Self::compute_shl_nsw(self, other).intersect_with(&Self::compute_shl_nuw(self, other), range_type),
            _ => panic!("Invalid NoWrapKind"),
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1849>
    pub fn lshr(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let by = |v: W, amount: W| lshr(v, limited(amount, w));
        let max_value = add(by(self.get_unsigned_max(), other.get_unsigned_min()), W::ONE, w);
        let min_value = by(self.get_unsigned_min(), other.get_unsigned_max());
        Self::non_empty(w, min_value, max_value)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1859>
    pub fn ashr(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let by = |v: W, amount: W| ashr(v, w, limited(amount, w));
        let pos_max = add(by(self.get_signed_max(), other.get_unsigned_min()), W::ONE, w);
        let pos_min = by(self.get_signed_min(), other.get_unsigned_max());
        let neg_max = add(by(self.get_signed_max(), other.get_unsigned_max()), W::ONE, w);
        let neg_min = by(self.get_signed_min(), other.get_unsigned_min());
        let (min_value, max_value) = if !is_negative(self.get_signed_min(), w) {
            (pos_min, pos_max)
        } else if is_negative(self.get_signed_max(), w) {
            (neg_min, neg_max)
        } else {
            (neg_min, pos_max)
        };
        Self::non_empty(w, min_value, max_value)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1909>
    pub fn uadd_sat(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = uadd_sat(self.get_unsigned_min(), other.get_unsigned_min(), w);
        let new_u = add(uadd_sat(self.get_unsigned_max(), other.get_unsigned_max(), w), W::ONE, w);
        Self::non_empty(w, new_l, new_u)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1918>
    pub fn sadd_sat(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = sadd_sat(self.get_signed_min(), other.get_signed_min(), w);
        let new_u = add(sadd_sat(self.get_signed_max(), other.get_signed_max(), w), W::ONE, w);
        Self::non_empty(w, new_l, new_u)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1927>
    pub fn usub_sat(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = usub_sat(self.get_unsigned_min(), other.get_unsigned_max(), w);
        let new_u = add(usub_sat(self.get_unsigned_max(), other.get_unsigned_min(), w), W::ONE, w);
        Self::non_empty(w, new_l, new_u)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1936>
    pub fn ssub_sat(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = ssub_sat(self.get_signed_min(), other.get_signed_max(), w);
        let new_u = add(ssub_sat(self.get_signed_max(), other.get_signed_min(), w), W::ONE, w);
        Self::non_empty(w, new_l, new_u)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1945>
    pub fn umul_sat(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = umul_sat(self.get_unsigned_min(), other.get_unsigned_min(), w);
        let new_u = add(umul_sat(self.get_unsigned_max(), other.get_unsigned_max(), w), W::ONE, w);
        Self::non_empty(w, new_l, new_u)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1975>
    pub fn ushl_sat(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let new_l = ushl_sat(self.get_unsigned_min(), w, limited(other.get_unsigned_min(), w));
        let new_u = add(ushl_sat(self.get_unsigned_max(), w, limited(other.get_unsigned_max(), w)), W::ONE, w);
        Self::non_empty(w, new_l, new_u)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1984>
    pub fn sshl_sat(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let (min_value, max_value) = (self.get_signed_min(), self.get_signed_max());
        let (sh_amt_min, sh_amt_max) = (other.get_unsigned_min(), other.get_unsigned_max());
        let new_l =
            sshl_sat(min_value, w, limited(if !is_negative(min_value, w) { sh_amt_min } else { sh_amt_max }, w));
        let new_u = add(
            sshl_sat(max_value, w, limited(if is_negative(max_value, w) { sh_amt_min } else { sh_amt_max }, w)),
            W::ONE,
            w,
        );
        Self::non_empty(w, new_l, new_u)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1995>
    pub fn inverse(&self) -> Self {
        if self.is_full_set() {
            return Self::empty(self.width);
        }
        if self.is_empty_set() {
            return Self::full(self.width);
        }
        Self::new(self.width, self.upper, self.lower)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2003>
    pub fn abs(
        &self,
        int_min_is_poison: bool,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() {
            return Self::empty(w);
        }
        if self.is_sign_wrapped_set() {
            let lo = if is_strictly_positive(self.upper, w) || !is_strictly_positive(self.lower, w) {
                W::ZERO
            } else {
                min(self.lower, add(neg(self.upper, w), W::ONE, w))
            };
            return if int_min_is_poison {
                Self::new(w, lo, smin_val(w))
            } else {
                Self::new(w, lo, add(smin_val(w), W::ONE, w))
            };
        }
        let (mut smin, smax) = (self.get_signed_min(), self.get_signed_max());
        if int_min_is_poison && smin == smin_val(w) {
            if smax == smin_val(w) {
                return Self::empty(w);
            }
            smin = add(smin, W::ONE, w);
        }
        if !is_negative(smin, w) {
            return Self::new(w, smin, add(smax, W::ONE, w));
        }
        if is_negative(smax, w) {
            return Self::new(w, neg(smax, w), add(neg(smin, w), W::ONE, w));
        }
        Self::non_empty(w, W::ZERO, add(max(neg(smin, w), smax), W::ONE, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2045>
    pub fn ctlz(
        &self,
        zero_is_poison: bool,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() {
            return Self::empty(w);
        }
        if zero_is_poison && self.contains(W::ZERO) {
            if self.lower == W::ZERO {
                if sub(self.upper, W::ONE, w) == W::ZERO {
                    return Self::empty(w);
                }
                return Self::new(
                    w,
                    W::from_u32(countl_zero(sub(self.upper, W::ONE, w), w)),
                    add(W::from_u32(countl_zero(add(self.lower, W::ONE, w), w)), W::ONE, w),
                );
            } else if sub(self.upper, W::ONE, w) == W::ZERO {
                return Self::new(w, W::ZERO, add(W::from_u32(countl_zero(self.lower, w)), W::ONE, w));
            }
            return Self::new(w, W::ZERO, W::from_u32(w));
        }
        Self::non_empty(
            w,
            W::from_u32(countl_zero(self.get_unsigned_max(), w)),
            add(W::from_u32(countl_zero(self.get_unsigned_min(), w)), W::ONE, w),
        )
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2083>
    fn unsigned_count_trailing_zeros_range(
        w: u32,
        lower: W,
        upper: W,
    ) -> Self {
        if add(lower, W::ONE, w) == upper {
            return Self::single(w, W::from_u32(countr_zero(lower, w)));
        }
        if lower == W::ZERO {
            return Self::new(w, W::ZERO, add(W::from_u32(w), W::ONE, w));
        }
        let lcp_length = countl_zero(lower ^ sub(upper, W::ONE, w), w);
        Self::new(w, W::ZERO, add(W::from_u32(max(w - lcp_length - 1, countr_zero(lower, w))), W::ONE, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2105>
    pub fn cttz(
        &self,
        zero_is_poison: bool,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() {
            return Self::empty(w);
        }
        if zero_is_poison && self.contains(W::ZERO) {
            if self.lower == W::ZERO {
                if self.upper == W::ONE {
                    return Self::empty(w);
                }
                return Self::unsigned_count_trailing_zeros_range(w, W::ONE, self.upper);
            } else if self.upper == W::ONE {
                return Self::unsigned_count_trailing_zeros_range(w, self.lower, W::ZERO);
            }
            let cr1 = Self::unsigned_count_trailing_zeros_range(w, self.lower, W::ZERO);
            let cr2 = Self::unsigned_count_trailing_zeros_range(w, W::ONE, self.upper);
            return cr1.union_with(&cr2, PreferredRangeType::Smallest);
        }
        if self.is_full_set() {
            return Self::non_empty(w, W::ZERO, add(W::from_u32(w), W::ONE, w));
        }
        if !self.is_wrapped_set() {
            return Self::unsigned_count_trailing_zeros_range(w, self.lower, self.upper);
        }
        let cr1 = Self::unsigned_count_trailing_zeros_range(w, self.lower, W::ZERO);
        let cr2 = Self::unsigned_count_trailing_zeros_range(w, W::ZERO, self.upper);
        cr1.union_with(&cr2, PreferredRangeType::Smallest)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2151>
    fn unsigned_pop_count_range(
        w: u32,
        lower: W,
        upper: W,
    ) -> Self {
        if add(lower, W::ONE, w) == upper {
            return Self::single(w, W::from_u32(lower.count_ones()));
        }
        let max_value = sub(upper, W::ONE, w);
        let lcp_length = countl_zero(lower ^ max_value, w);
        let lcp_pop_count = lshr(lower, w - lcp_length).count_ones();
        let min_bits = lcp_pop_count + u32::from(countr_zero(lower, w) < w - lcp_length);
        let max_bits = lcp_pop_count + (w - lcp_length) - u32::from(countr_one(max_value, w) < w - lcp_length);
        Self::new(w, W::from_u32(min_bits), add(W::from_u32(max_bits), W::ONE, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2177>
    pub fn ctpop(&self) -> Self {
        let w = self.width;
        if self.is_empty_set() {
            return Self::empty(w);
        }
        if self.is_full_set() {
            return Self::non_empty(w, W::ZERO, add(W::from_u32(w), W::ONE, w));
        }
        if !self.is_wrapped_set() {
            return Self::unsigned_pop_count_range(w, self.lower, self.upper);
        }
        let cr1 = Self::new(w, W::from_u32(countl_one(self.lower, w)), add(W::from_u32(w), W::ONE, w));
        let cr2 = Self::unsigned_pop_count_range(w, W::ZERO, self.upper);
        cr1.union_with(&cr2, PreferredRangeType::Smallest)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2207>
    pub fn unsigned_add_may_overflow(
        &self,
        other: &Self,
    ) -> OverflowResult {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return OverflowResult::MayOverflow;
        }
        let (min_value, max_value) = (self.get_unsigned_min(), self.get_unsigned_max());
        let (other_min, other_max) = (other.get_unsigned_min(), other.get_unsigned_max());
        if min_value > !other_min & mask::<W>(w) {
            return OverflowResult::AlwaysOverflowsHigh;
        }
        if max_value > !other_max & mask::<W>(w) {
            return OverflowResult::MayOverflow;
        }
        OverflowResult::NeverOverflows
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2223>
    pub fn signed_add_may_overflow(
        &self,
        other: &Self,
    ) -> OverflowResult {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return OverflowResult::MayOverflow;
        }
        let (min_value, max_value) = (self.get_signed_min(), self.get_signed_max());
        let (other_min, other_max) = (other.get_signed_min(), other.get_signed_max());
        let (signed_min, signed_max) = (smin_val(w), smax_val(w));
        if !is_negative(min_value, w) && !is_negative(other_min, w) && sgt(min_value, sub(signed_max, other_min, w), w)
        {
            return OverflowResult::AlwaysOverflowsHigh;
        }
        if is_negative(max_value, w) && is_negative(other_max, w) && slt(max_value, sub(signed_min, other_max, w), w) {
            return OverflowResult::AlwaysOverflowsLow;
        }
        if !is_negative(max_value, w) && !is_negative(other_max, w) && sgt(max_value, sub(signed_max, other_max, w), w)
        {
            return OverflowResult::MayOverflow;
        }
        if is_negative(min_value, w) && is_negative(other_min, w) && slt(min_value, sub(signed_min, other_min, w), w) {
            return OverflowResult::MayOverflow;
        }
        OverflowResult::NeverOverflows
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2253>
    pub fn unsigned_sub_may_overflow(
        &self,
        other: &Self,
    ) -> OverflowResult {
        if self.is_empty_set() || other.is_empty_set() {
            return OverflowResult::MayOverflow;
        }
        let (min_value, max_value) = (self.get_unsigned_min(), self.get_unsigned_max());
        let (other_min, other_max) = (other.get_unsigned_min(), other.get_unsigned_max());
        if max_value < other_min {
            return OverflowResult::AlwaysOverflowsLow;
        }
        if min_value < other_max {
            return OverflowResult::MayOverflow;
        }
        OverflowResult::NeverOverflows
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2269>
    pub fn signed_sub_may_overflow(
        &self,
        other: &Self,
    ) -> OverflowResult {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return OverflowResult::MayOverflow;
        }
        let (min_value, max_value) = (self.get_signed_min(), self.get_signed_max());
        let (other_min, other_max) = (other.get_signed_min(), other.get_signed_max());
        let (signed_min, signed_max) = (smin_val(w), smax_val(w));
        if !is_negative(min_value, w) && is_negative(other_max, w) && sgt(min_value, add(signed_max, other_max, w), w) {
            return OverflowResult::AlwaysOverflowsHigh;
        }
        if is_negative(max_value, w) && !is_negative(other_min, w) && slt(max_value, add(signed_min, other_min, w), w) {
            return OverflowResult::AlwaysOverflowsLow;
        }
        if !is_negative(max_value, w) && is_negative(other_min, w) && sgt(max_value, add(signed_max, other_min, w), w) {
            return OverflowResult::MayOverflow;
        }
        if is_negative(min_value, w) && !is_negative(other_max, w) && slt(min_value, add(signed_min, other_max, w), w) {
            return OverflowResult::MayOverflow;
        }
        OverflowResult::NeverOverflows
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2299>
    pub fn unsigned_mul_may_overflow(
        &self,
        other: &Self,
    ) -> OverflowResult {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return OverflowResult::MayOverflow;
        }
        let (min_value, max_value) = (self.get_unsigned_min(), self.get_unsigned_max());
        let (other_min, other_max) = (other.get_unsigned_min(), other.get_unsigned_max());
        if umul_ov(min_value, other_min, w).1 {
            return OverflowResult::AlwaysOverflowsHigh;
        }
        if umul_ov(max_value, other_max, w).1 {
            return OverflowResult::MayOverflow;
        }
        OverflowResult::NeverOverflows
    }

    /// `getConstantRangeFromMetadata` on the `(low, high)` pairs of `!range`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2334>
    pub fn from_range_pairs(
        width: u32,
        pairs: &[(W, W)],
    ) -> Self {
        assert!(!pairs.is_empty(), "Must have at least one range!");
        let mut cr = Self::new(width, pairs[0].0, pairs[0].1);
        for (low, high) in &pairs[1..] {
            cr = cr.union_with(&Self::new(width, *low, *high), PreferredRangeType::Smallest);
        }
        cr
    }
}

impl Range<u128> {
    /// LLVM multiplies at twice the width: a `u128` holds that up to 64 bits, a
    /// [`U256`] up to 128.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1220>
    pub fn multiply(
        &self,
        other: &Self,
        no_wrap_kind: u32,
    ) -> Self {
        if self.width <= 64 {
            return self.multiply_at_double_width(other, no_wrap_kind);
        }
        self.to_word::<U256>().multiply_at_double_width(&other.to_word(), no_wrap_kind).to_word()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L298>
    fn make_exact_mul_nuw_region(
        width: u32,
        v: u128,
    ) -> Self {
        if v == 0 {
            return Self::full(width);
        }
        Self::non_empty(width, rounding_udiv(0, v, true), add(rounding_udiv(mask::<u128>(width), v, false), 1, width))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L311>
    fn make_exact_mul_nsw_region(
        width: u32,
        v: u128,
    ) -> Self {
        if v == 0 {
            return Self::full(width);
        }
        let (min_value, max_value) = (smin_val(width), smax_val(width));
        if v == mask::<u128>(width) {
            return Self::new(width, neg(max_value, width), min_value);
        }
        let (lower, upper) = if is_negative(v, width) {
            (rounding_sdiv(max_value, v, width, true), rounding_sdiv(min_value, v, width, false))
        } else {
            (rounding_sdiv(min_value, v, width, true), rounding_sdiv(max_value, v, width, false))
        };
        Self::non_empty(width, lower, add(upper, 1, width))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L335>
    pub fn make_guaranteed_no_wrap_region(
        bin_op: BinaryOp,
        other: &Self,
        no_wrap_kind: u32,
    ) -> Self {
        assert!(no_wrap_kind == NO_SIGNED_WRAP || no_wrap_kind == NO_UNSIGNED_WRAP, "NoWrapKind invalid!");
        let is_unsigned = no_wrap_kind == NO_UNSIGNED_WRAP;
        let w = other.width;
        match bin_op {
            BinaryOp::Add => {
                if is_unsigned {
                    return Self::non_empty(w, 0, neg(other.get_unsigned_max(), w));
                }
                let signed_min_val = smin_val(w);
                let (smin, smax) = (other.get_signed_min(), other.get_signed_max());
                Self::non_empty(
                    w,
                    if is_negative(smin, w) { sub(signed_min_val, smin, w) } else { signed_min_val },
                    if is_strictly_positive(smax, w) { sub(signed_min_val, smax, w) } else { signed_min_val },
                )
            }
            BinaryOp::Sub => {
                if is_unsigned {
                    return Self::non_empty(w, other.get_unsigned_max(), 0);
                }
                let signed_min_val = smin_val(w);
                let (smin, smax) = (other.get_signed_min(), other.get_signed_max());
                Self::non_empty(
                    w,
                    if is_strictly_positive(smax, w) { add(signed_min_val, smax, w) } else { signed_min_val },
                    if is_negative(smin, w) { add(signed_min_val, smin, w) } else { signed_min_val },
                )
            }
            BinaryOp::Mul => {
                if is_unsigned {
                    return Self::make_exact_mul_nuw_region(w, other.get_unsigned_max());
                }
                if let Some(c) = other.get_single_element() {
                    return Self::make_exact_mul_nsw_region(w, c);
                }
                Self::make_exact_mul_nsw_region(w, other.get_signed_min()).intersect_with(
                    &Self::make_exact_mul_nsw_region(w, other.get_signed_max()),
                    PreferredRangeType::Smallest,
                )
            }
            BinaryOp::Shl => {
                let sh_amt =
                    other.intersect_with(&Self::new(w, 0, add(u128::from(w - 1), 1, w)), PreferredRangeType::Smallest);
                if sh_amt.is_empty_set() {
                    return Self::full(w);
                }
                let sh_amt_umax = limited(sh_amt.get_unsigned_max(), w);
                if is_unsigned {
                    return Self::non_empty(w, 0, add(lshr(mask::<u128>(w), sh_amt_umax), 1, w));
                }
                Self::non_empty(w, ashr(smin_val(w), w, sh_amt_umax), add(ashr(smax_val(w), w, sh_amt_umax), 1, w))
            }
            _ => panic!("Unsupported binary op"),
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L409>
    pub fn make_exact_no_wrap_region(
        bin_op: BinaryOp,
        width: u32,
        other: u128,
        no_wrap_kind: u32,
    ) -> Self {
        Self::make_guaranteed_no_wrap_region(bin_op, &Self::single(width, other), no_wrap_kind)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L981>
    pub fn binary_op(
        &self,
        bin_op: BinaryOp,
        other: &Self,
    ) -> Self {
        match bin_op {
            BinaryOp::Add | BinaryOp::FAdd => self.add(other),
            BinaryOp::Sub | BinaryOp::FSub => self.sub(other),
            BinaryOp::Mul | BinaryOp::FMul => self.multiply(other, 0),
            BinaryOp::UDiv => self.udiv(other),
            BinaryOp::SDiv => self.sdiv(other),
            BinaryOp::URem => self.urem(other),
            BinaryOp::SRem => self.srem(other),
            BinaryOp::Shl => self.shl(other),
            BinaryOp::LShr => self.lshr(other),
            BinaryOp::AShr => self.ashr(other),
            BinaryOp::And => self.binary_and(other),
            BinaryOp::Or => self.binary_or(other, false),
            BinaryOp::Xor => self.binary_xor(other),
            BinaryOp::FDiv | BinaryOp::FRem => Self::full(self.width),
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1026>
    pub fn overflowing_binary_op(
        &self,
        bin_op: BinaryOp,
        other: &Self,
        no_wrap_kind: u32,
    ) -> Self {
        match bin_op {
            BinaryOp::Add => self.add_with_no_wrap(other, no_wrap_kind, PreferredRangeType::Smallest),
            BinaryOp::Sub => self.sub_with_no_wrap(other, no_wrap_kind, PreferredRangeType::Smallest),
            BinaryOp::Mul => self.multiply(other, no_wrap_kind),
            BinaryOp::Shl => self.shl_with_no_wrap(other, no_wrap_kind, PreferredRangeType::Smallest),
            _ => self.binary_op(bin_op, other),
        }
    }

    /// `binaryOp(const BinaryOperator &)`, the flags given as arguments.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L981>
    pub fn binary_op_with_flags(
        &self,
        bin_op: BinaryOp,
        other: &Self,
        nuw: bool,
        nsw: bool,
        disjoint: bool,
    ) -> Self {
        if matches!(
            bin_op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl
        ) {
            let kind = if nuw { NO_UNSIGNED_WRAP } else { 0 } | if nsw { NO_SIGNED_WRAP } else { 0 };
            return self.overflowing_binary_op(bin_op, other, kind);
        }
        if bin_op == BinaryOp::Or {
            return self.binary_or(other, disjoint);
        }
        self.binary_op(bin_op, other)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1318>
    pub fn smul_fast(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let (min_value, max_value) = (self.get_signed_min(), self.get_signed_max());
        let (other_min, other_max) = (other.get_signed_min(), other.get_signed_max());
        let muls = [
            smul_ov(min_value, other_min, w),
            smul_ov(min_value, other_max, w),
            smul_ov(max_value, other_min, w),
            smul_ov(max_value, other_max, w),
        ];
        if muls.iter().any(|(_, over)| *over) {
            return Self::full(w);
        }
        let values: Vec<u128> = muls.iter().map(|(v, _)| *v).collect();
        let lowest = values.iter().copied().fold(values[0], |m, c| smin_of_two(m, c, w));
        let highest = values.iter().copied().fold(values[0], |m, c| smax_of_two(m, c, w));
        Self::non_empty(w, lowest, add(highest, 1, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1394>
    pub fn udiv(
        &self,
        rhs: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || rhs.is_empty_set() || rhs.get_unsigned_max() == 0 {
            return Self::empty(w);
        }
        let lower = self.get_unsigned_min() / rhs.get_unsigned_max();
        let mut rhs_umin = rhs.get_unsigned_min();
        if rhs_umin == 0 {
            rhs_umin = if rhs.upper == 1 { rhs.lower } else { 1 };
        }
        let upper = add(self.get_unsigned_max() / rhs_umin, 1, w);
        Self::non_empty(w, lower, upper)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1414>
    pub fn sdiv(
        &self,
        rhs: &Self,
    ) -> Self {
        let w = self.width;
        let signed_min = smin_val(w);
        let (pos_l, neg_l) = self.split_pos_neg();
        let (pos_r, neg_r) = rhs.split_pos_neg();
        let smallest = PreferredRangeType::Smallest;

        let mut pos_res = Self::empty(w);
        if !pos_l.is_empty_set() && !pos_r.is_empty_set() {
            pos_res = Self::new(
                w,
                sdiv(pos_l.lower, sub(pos_r.upper, 1, w), w),
                add(sdiv(sub(pos_l.upper, 1, w), pos_r.lower, w), 1, w),
            );
        }
        if !neg_l.is_empty_set() && !neg_r.is_empty_set() {
            let lo = sdiv(sub(neg_l.upper, 1, w), neg_r.lower, w);
            if neg_l.lower == signed_min && neg_r.upper == 0 {
                if neg_r.lower != mask::<u128>(w) {
                    let adj_neg_r_upper = if rhs.lower == mask::<u128>(w) { rhs.upper } else { sub(neg_r.upper, 1, w) };
                    pos_res = pos_res.union_with(
                        &Self::new(w, lo, add(sdiv(neg_l.lower, sub(adj_neg_r_upper, 1, w), w), 1, w)),
                        smallest,
                    );
                }
                if neg_l.upper != add(signed_min, 1, w) {
                    let adj_neg_l_lower =
                        if self.upper == add(signed_min, 1, w) { self.lower } else { add(neg_l.lower, 1, w) };
                    pos_res = pos_res.union_with(
                        &Self::new(w, lo, add(sdiv(adj_neg_l_lower, sub(neg_r.upper, 1, w), w), 1, w)),
                        smallest,
                    );
                }
            } else {
                pos_res = pos_res
                    .union_with(&Self::new(w, lo, add(sdiv(neg_l.lower, sub(neg_r.upper, 1, w), w), 1, w)), smallest);
            }
        }
        let mut neg_res = Self::empty(w);
        if !pos_l.is_empty_set() && !neg_r.is_empty_set() {
            neg_res = Self::new(
                w,
                sdiv(sub(pos_l.upper, 1, w), sub(neg_r.upper, 1, w), w),
                add(sdiv(pos_l.lower, neg_r.lower, w), 1, w),
            );
        }
        if !neg_l.is_empty_set() && !pos_r.is_empty_set() {
            neg_res = neg_res.union_with(
                &Self::new(
                    w,
                    sdiv(neg_l.lower, pos_r.lower, w),
                    add(sdiv(sub(neg_l.upper, 1, w), sub(pos_r.upper, 1, w), w), 1, w),
                ),
                smallest,
            );
        }
        let mut res = neg_res.union_with(&pos_res, PreferredRangeType::Signed);
        if self.contains(0) && (!pos_r.is_empty_set() || !neg_r.is_empty_set()) {
            res = res.union_with(&Self::single(w, 0), smallest);
        }
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1496>
    pub fn urem(
        &self,
        rhs: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || rhs.is_empty_set() || rhs.get_unsigned_max() == 0 {
            return Self::empty(w);
        }
        if let Some(rhs_int) = rhs.get_single_element() {
            if rhs_int == 0 {
                return Self::empty(w);
            }
            if let Some(lhs_int) = self.get_single_element() {
                return Self::single(w, lhs_int % rhs_int);
            }
        }
        if self.get_unsigned_max() < rhs.get_unsigned_min() {
            return *self;
        }
        let upper = add(min(self.get_unsigned_max(), sub(rhs.get_unsigned_max(), 1, w)), 1, w);
        Self::non_empty(w, 0, upper)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1518>
    pub fn srem(
        &self,
        rhs: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || rhs.is_empty_set() {
            return Self::empty(w);
        }
        if let Some(rhs_int) = rhs.get_single_element() {
            if rhs_int == 0 {
                return Self::empty(w);
            }
            if let Some(lhs_int) = self.get_single_element() {
                return Self::single(w, srem(lhs_int, rhs_int, w));
            }
        }
        let abs_rhs = rhs.abs(false);
        let mut min_abs_rhs = abs_rhs.get_unsigned_min();
        let max_abs_rhs = abs_rhs.get_unsigned_max();
        if max_abs_rhs == 0 {
            return Self::empty(w);
        }
        if min_abs_rhs == 0 {
            min_abs_rhs = add(min_abs_rhs, 1, w);
        }
        let (min_lhs, max_lhs) = (self.get_signed_min(), self.get_signed_max());
        if !is_negative(min_lhs, w) {
            if max_lhs < min_abs_rhs {
                return *self;
            }
            let upper = add(min(max_lhs, sub(max_abs_rhs, 1, w)), 1, w);
            return Self::new(w, 0, upper);
        }
        if is_negative(max_lhs, w) {
            if min_lhs > neg(min_abs_rhs, w) {
                return *self;
            }
            let lower = max(min_lhs, add(neg(max_abs_rhs, w), 1, w));
            return Self::new(w, lower, add(0, 1, w));
        }
        let lower = max(min_lhs, add(neg(max_abs_rhs, w), 1, w));
        let upper = add(min(max_lhs, sub(max_abs_rhs, 1, w)), 1, w);
        Self::new(w, lower, upper)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L1954>
    pub fn smul_sat(
        &self,
        other: &Self,
    ) -> Self {
        let w = self.width;
        if self.is_empty_set() || other.is_empty_set() {
            return Self::empty(w);
        }
        let (min_value, max_value) = (self.get_signed_min(), self.get_signed_max());
        let (other_min, other_max) = (other.get_signed_min(), other.get_signed_max());
        let l = [
            smul_sat(min_value, other_min, w),
            smul_sat(min_value, other_max, w),
            smul_sat(max_value, other_min, w),
            smul_sat(max_value, other_max, w),
        ];
        let lowest = l.iter().copied().fold(l[0], |m, c| smin_of_two(m, c, w));
        let highest = l.iter().copied().fold(l[0], |m, c| smax_of_two(m, c, w));
        Self::non_empty(w, lowest, add(highest, 1, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2197>
    pub fn sqrt_floor(&self) -> Self {
        let w = self.width;
        if self.is_empty_set() {
            return Self::empty(w);
        }
        Self::non_empty(w, sqrt_floor(self.get_unsigned_min()), add(sqrt_floor(self.get_unsigned_max()), 1, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantRange.cpp#L2319>
    pub fn print(&self) -> String {
        if self.is_full_set() {
            "full-set".to_owned()
        } else if self.is_empty_set() {
            "empty-set".to_owned()
        } else {
            format!("[{},{})", signed(self.lower, self.width), signed(self.upper, self.width))
        }
    }
}
