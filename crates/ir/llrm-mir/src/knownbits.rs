//! Which bits of an integer are known zero and which known one: LLVM's
//! `KnownBits`, the type every ValueTracking query returns.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` (`llvm/lib/Support/KnownBits.cpp`,
//! `llvm/include/llvm/Support/KnownBits.h`): same operations, same order of
//! cases, so a bit LLVM proves is a bit this proves. Each function links the
//! line it translates.
//!
//! An `APInt` is a `u128` and its width: `zero` and `one` hold the low `width`
//! bits and no others. DEVIATE: widths above 128 are refused (`unknown`
//! panics); `mulhs`/`mulhu` need twice the width in `u128`, so they take at
//! most 64 bits, and the averages one more bit than the width. Approved by
//! oversight.
//!
//! # Example
//! `0b1?0?` is `zero = 0b0101, one = 0b1000` at width 4; `add` of it with the
//! constant 2 knows bit 0 of the sum is 0 and bit 1 is not.
//!
//! # See also
//! - Used by: [`crate::valuetracking`]

use std::cmp::{max, min};

use crate::context::mask;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KnownBits {
    pub zero: u128,
    pub one: u128,
    pub width: u32,
}

// The `APInt` operations the translation needs, on `u128` at a width.

/// `v` with its top `count` of `width` bits set.
fn set_high(
    v: u128,
    width: u32,
    count: u32,
) -> u128 {
    v | (mask(width) & !mask(width - min(count, width)))
}

/// `v` with bits `[low, high)` set.
fn set_range(
    v: u128,
    low: u32,
    high: u32,
) -> u128 {
    if high <= low { v } else { v | (mask(high) & !mask(low)) }
}

fn clear_range(
    v: u128,
    low: u32,
    high: u32,
) -> u128 {
    if high <= low { v } else { v & !(mask(high) & !mask(low)) }
}

fn bit(
    v: u128,
    at: u32,
) -> bool {
    at < 128 && v >> at & 1 == 1
}

fn with_bit(
    v: u128,
    at: u32,
    on: bool,
) -> u128 {
    if on { v | 1_u128 << at } else { v & !(1_u128 << at) }
}

fn countl_zero(
    v: u128,
    width: u32,
) -> u32 {
    if v == 0 { width } else { v.leading_zeros() - (128 - width) }
}

fn countl_one(
    v: u128,
    width: u32,
) -> u32 {
    countl_zero(!v & mask(width), width)
}

fn countr_zero(
    v: u128,
    width: u32,
) -> u32 {
    if v == 0 { width } else { v.trailing_zeros() }
}

fn countr_one(
    v: u128,
    width: u32,
) -> u32 {
    countr_zero(!v & mask(width), width)
}

fn popcount(v: u128) -> u32 {
    v.count_ones()
}

fn sign_bit(width: u32) -> u128 {
    1_u128 << (width - 1)
}

fn signed(
    v: u128,
    width: u32,
) -> i128 {
    if width >= 128 { v as i128 } else { ((v << (128 - width)) as i128) >> (128 - width) }
}

fn sext(
    v: u128,
    from: u32,
    to: u32,
) -> u128 {
    signed(v, from) as u128 & mask(to)
}

fn ashr(
    v: u128,
    width: u32,
    by: u32,
) -> u128 {
    (signed(v, width) >> min(by, width - 1)) as u128 & mask(width)
}

fn lshr(
    v: u128,
    by: u32,
) -> u128 {
    if by >= 128 { 0 } else { v >> by }
}

/// `APInt::ushl_ov`: the shift, and whether a set bit left the width.
fn ushl_ov(
    v: u128,
    width: u32,
    by: u32,
) -> (u128, bool) {
    if by >= width {
        return (0, true);
    }
    ((v << by) & mask(width), by > countl_zero(v, width))
}

fn uadd_ov(
    a: u128,
    b: u128,
    width: u32,
) -> (u128, bool) {
    let (sum, over) = a.overflowing_add(b);
    if width >= 128 { (sum, over) } else { (sum & mask(width), over || sum > mask(width)) }
}

fn usub_ov(
    a: u128,
    b: u128,
    width: u32,
) -> (u128, bool) {
    (a.wrapping_sub(b) & mask(width), a < b)
}

fn umul_ov(
    a: u128,
    b: u128,
    width: u32,
) -> (u128, bool) {
    match a.checked_mul(b) {
        Some(product) if product <= mask(width) => (product, false),
        _ => (a.wrapping_mul(b) & mask(width), true),
    }
}

fn smin_of(width: u32) -> i128 {
    if width >= 128 { i128::MIN } else { -(1_i128 << (width - 1)) }
}

fn smax_of(width: u32) -> i128 {
    if width >= 128 { i128::MAX } else { (1_i128 << (width - 1)) - 1 }
}

fn saturated(
    sum: Option<i128>,
    width: u32,
    negative: bool,
) -> u128 {
    let value = match sum {
        Some(s) if s < smin_of(width) => smin_of(width),
        Some(s) if s > smax_of(width) => smax_of(width),
        Some(s) => s,
        None if negative => smin_of(width),
        None => smax_of(width),
    };
    value as u128 & mask(width)
}

fn sadd_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (x, y) = (signed(a, width), signed(b, width));
    saturated(x.checked_add(y), width, x < 0)
}

fn ssub_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (x, y) = (signed(a, width), signed(b, width));
    saturated(x.checked_sub(y), width, x < 0)
}

fn uadd_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (sum, over) = uadd_ov(a, b, width);
    if over { mask(width) } else { sum }
}

fn usub_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (diff, over) = usub_ov(a, b, width);
    if over { 0 } else { diff }
}

/// `APIntOps::fshl`: the top `width` bits of `(a:b) << (amt mod width)`.
fn fshl(
    a: u128,
    b: u128,
    amt: u128,
    width: u32,
) -> u128 {
    let by = (amt % u128::from(width)) as u32;
    if by == 0 { a } else { ((a << by) | lshr(b, width - by)) & mask(width) }
}

/// `APIntOps::fshr`: the low `width` bits of `(a:b) >> (amt mod width)`.
fn fshr(
    a: u128,
    b: u128,
    amt: u128,
    width: u32,
) -> u128 {
    let by = (amt % u128::from(width)) as u32;
    if by == 0 { b } else { ((a << (width - by)) | lshr(b, by)) & mask(width) }
}

fn clmul(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    (0..width).filter(|i| bit(b, *i)).fold(0, |acc, i| acc ^ (a << i)) & mask(width)
}

fn rotate_amount(
    amt: u128,
    width: u32,
) -> u32 {
    (amt % u128::from(width)) as u32
}

impl KnownBits {
    fn raw(
        width: u32,
        zero: u128,
        one: u128,
    ) -> Self {
        Self { zero: zero & mask(width), one: one & mask(width), width }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L41>
    pub fn unknown(width: u32) -> Self {
        assert!((1..=128).contains(&width), "KnownBits: width {width} is outside 1..=128");
        Self { zero: 0, one: 0, width }
    }

    /// Both masks at a width, as LLVM's private `KnownBits(Zero, One)`.
    pub fn from_masks(
        width: u32,
        zero: u128,
        one: u128,
    ) -> Self {
        assert!((1..=128).contains(&width), "KnownBits: width {width} is outside 1..=128");
        Self::raw(width, zero, one)
    }

    /// @h:makeConstant
    pub fn constant(
        width: u32,
        value: u128,
    ) -> Self {
        Self::from_masks(width, !value, value)
    }

    /// @h:hasConflict
    pub fn has_conflict(&self) -> bool {
        self.zero & self.one != 0
    }

    /// @h:isConstant
    pub fn is_constant(&self) -> bool {
        self.zero ^ self.one == mask(self.width) && !self.has_conflict()
    }

    /// @h:getConstant
    pub fn constant_value(&self) -> u128 {
        assert!(self.is_constant(), "Can only get value when all bits are known");
        self.one
    }

    /// @h:isUnknown
    pub fn is_unknown(&self) -> bool {
        self.zero == 0 && self.one == 0
    }

    /// @h:isSignUnknown
    pub fn is_sign_unknown(&self) -> bool {
        !bit(self.zero, self.width - 1) && !bit(self.one, self.width - 1)
    }

    /// @h:resetAll
    pub fn reset_all(&mut self) {
        self.zero = 0;
        self.one = 0;
    }

    /// @h:isZero
    pub fn is_zero(&self) -> bool {
        self.zero == mask(self.width)
    }

    /// @h:isAllOnes
    pub fn is_all_ones(&self) -> bool {
        self.one == mask(self.width)
    }

    /// @h:setAllZero
    pub fn set_all_zero(&mut self) {
        self.zero = mask(self.width);
        self.one = 0;
    }

    /// @h:setAllOnes
    pub fn set_all_ones(&mut self) {
        self.zero = 0;
        self.one = mask(self.width);
    }

    /// @h:setAllConflict
    pub fn set_all_conflict(&mut self) {
        self.zero = mask(self.width);
        self.one = mask(self.width);
    }

    /// @h:isNegative
    pub fn is_negative(&self) -> bool {
        bit(self.one, self.width - 1)
    }

    /// @h:isNonNegative
    pub fn is_non_negative(&self) -> bool {
        bit(self.zero, self.width - 1)
    }

    /// @h:isNonZero
    pub fn is_non_zero(&self) -> bool {
        self.one != 0
    }

    /// @h:isStrictlyPositive
    pub fn is_strictly_positive(&self) -> bool {
        self.is_non_negative() && self.is_non_zero()
    }

    /// @h:isNonPositive
    pub fn is_non_positive(&self) -> bool {
        signed(self.signed_max_value(), self.width) <= 0
    }

    /// @h:makeNegative
    pub fn make_negative(&mut self) {
        self.one |= sign_bit(self.width);
    }

    /// @h:makeNonNegative
    pub fn make_non_negative(&mut self) {
        self.zero |= sign_bit(self.width);
    }

    /// @h:getMinValue
    pub fn min_value(&self) -> u128 {
        self.one
    }

    /// @h:getSignedMinValue
    pub fn signed_min_value(&self) -> u128 {
        let min = self.one;
        if bit(self.zero, self.width - 1) { min } else { min | sign_bit(self.width) }
    }

    /// @h:getMaxValue
    pub fn max_value(&self) -> u128 {
        !self.zero & mask(self.width)
    }

    /// @h:getSignedMaxValue
    pub fn signed_max_value(&self) -> u128 {
        let max = self.max_value();
        if bit(self.one, self.width - 1) { max } else { max & !sign_bit(self.width) }
    }

    /// @h:isEven
    pub fn is_even(&self) -> bool {
        bit(self.zero, 0)
    }

    /// @h:trunc
    pub fn trunc(
        &self,
        width: u32,
    ) -> Self {
        Self::from_masks(width, self.zero, self.one)
    }

    /// @h:anyext
    pub fn anyext(
        &self,
        width: u32,
    ) -> Self {
        Self::from_masks(width, self.zero, self.one)
    }

    /// @h:zext
    pub fn zext(
        &self,
        width: u32,
    ) -> Self {
        Self::from_masks(width, set_range(self.zero, self.width, width), self.one)
    }

    /// @h:sext
    pub fn sext(
        &self,
        width: u32,
    ) -> Self {
        Self::from_masks(width, sext(self.zero, self.width, width), sext(self.one, self.width, width))
    }

    /// @h:anyextOrTrunc
    pub fn anyext_or_trunc(
        &self,
        width: u32,
    ) -> Self {
        match width.cmp(&self.width) {
            std::cmp::Ordering::Greater => self.anyext(width),
            std::cmp::Ordering::Less => self.trunc(width),
            std::cmp::Ordering::Equal => *self,
        }
    }

    /// @h:zextOrTrunc
    pub fn zext_or_trunc(
        &self,
        width: u32,
    ) -> Self {
        match width.cmp(&self.width) {
            std::cmp::Ordering::Greater => self.zext(width),
            std::cmp::Ordering::Less => self.trunc(width),
            std::cmp::Ordering::Equal => *self,
        }
    }

    /// @h:sextOrTrunc
    pub fn sext_or_trunc(
        &self,
        width: u32,
    ) -> Self {
        match width.cmp(&self.width) {
            std::cmp::Ordering::Greater => self.sext(width),
            std::cmp::Ordering::Less => self.trunc(width),
            std::cmp::Ordering::Equal => *self,
        }
    }

    /// @cpp:truncSSat
    pub fn trunc_ssat(
        &self,
        width: u32,
    ) -> Self {
        let input = self.width;
        let min_in_range = sext(sign_bit(width), width, input);
        let max_in_range = sext(mask(width) >> 1, width, input);
        let (input_min, input_max) = (signed(self.signed_min_value(), input), signed(self.signed_max_value(), input));
        let (min_in_range, max_in_range) = (signed(min_in_range, input), signed(max_in_range, input));
        let mut known = Self::unknown(width);
        let top = width - 1;
        if input_min >= min_in_range && input_max <= max_in_range {
            known = self.trunc(width);
        } else if input_max < min_in_range {
            known = Self::constant(width, sign_bit(width));
        } else if input_min > max_in_range {
            known = Self::constant(width, mask(width) >> 1);
        } else if input_min >= 0 {
            known.zero = sign_bit(width);
            known.one = with_bit(self.one & mask(width), top, false);
        } else if input_max < 0 {
            known.zero = with_bit(self.zero & mask(width), top, false);
            known.one = sign_bit(width);
        } else {
            let (zero_lower, one_lower) = (self.zero & mask(width), self.one & mask(width));
            let min_sat = Self::constant(width, sign_bit(width));
            let max_sat = Self::constant(width, mask(width) >> 1);
            if input_max <= max_in_range {
                known.zero = zero_lower & min_sat.zero;
                known.one = one_lower & min_sat.one;
            } else if input_min >= min_in_range {
                known.zero = zero_lower & max_sat.zero;
                known.one = one_lower & max_sat.one;
            } else {
                known.zero = zero_lower & min_sat.zero & max_sat.zero;
                known.one = one_lower & min_sat.one & max_sat.one;
            }
        }
        known
    }

    /// @cpp:truncSSatU
    pub fn trunc_ssat_u(
        &self,
        width: u32,
    ) -> Self {
        let input = self.width;
        let max_in_range = mask(width);
        let (input_min, input_max) = (self.signed_min_value(), self.signed_max_value());
        let mut known = Self::unknown(width);
        if self.is_negative() {
            known.set_all_zero();
        } else if !bit(input_min, input - 1) && input_max <= max_in_range {
            known = self.trunc(width);
        } else if !bit(input_min, input - 1) && input_min > max_in_range {
            known.set_all_ones();
        } else if !bit(input_min, input - 1) {
            known.zero = 0;
            known.one = self.one & mask(width);
        } else if input_max <= max_in_range {
            known.zero = self.zero & mask(width);
            known.one = 0;
        } else {
            known.zero = 0;
            known.one = 0;
        }
        known
    }

    /// @cpp:truncUSat
    pub fn trunc_usat(
        &self,
        width: u32,
    ) -> Self {
        let max_in_range = mask(width);
        let mut known = Self::unknown(width);
        if self.max_value() <= max_in_range {
            known = self.trunc(width);
        } else if self.min_value() > max_in_range {
            known.set_all_ones();
        } else {
            known.one = self.one & mask(width);
        }
        known
    }

    /// @cpp:sextInReg
    pub fn sext_in_reg(
        &self,
        src_width: u32,
    ) -> Self {
        assert!(0 < src_width && src_width <= self.width, "Illegal sext-in-register");
        if src_width == self.width {
            return *self;
        }
        let ext = self.width - src_width;
        let one = (self.one << ext) & mask(self.width);
        let zero = (self.zero << ext) & mask(self.width);
        Self::raw(self.width, ashr(zero, self.width, ext), ashr(one, self.width, ext))
    }

    /// @h:insertBits
    pub fn insert_bits(
        &mut self,
        sub: &Self,
        position: u32,
    ) {
        let hole = mask(sub.width) << position;
        self.zero = (self.zero & !hole) | (sub.zero << position);
        self.one = (self.one & !hole) | (sub.one << position);
    }

    /// @h:extractBits
    pub fn extract_bits(
        &self,
        count: u32,
        position: u32,
    ) -> Self {
        Self::from_masks(count, lshr(self.zero, position), lshr(self.one, position))
    }

    /// @h:concat
    pub fn concat(
        &self,
        low: &Self,
    ) -> Self {
        let width = self.width + low.width;
        Self::from_masks(width, self.zero << low.width | low.zero, self.one << low.width | low.one)
    }

    /// @cpp:makeGE
    pub fn make_ge(
        &self,
        value: u128,
    ) -> Self {
        let n = countl_one(self.zero | value, self.width);
        let masked = value & !mask(self.width - n);
        Self { zero: self.zero, one: self.one | masked, width: self.width }
    }

    /// @h:countMinTrailingZeros
    pub fn count_min_trailing_zeros(&self) -> u32 {
        countr_one(self.zero, self.width)
    }

    /// @h:countMinTrailingOnes
    pub fn count_min_trailing_ones(&self) -> u32 {
        countr_one(self.one, self.width)
    }

    /// @h:countMinLeadingZeros
    pub fn count_min_leading_zeros(&self) -> u32 {
        countl_one(self.zero, self.width)
    }

    /// @h:countMinLeadingOnes
    pub fn count_min_leading_ones(&self) -> u32 {
        countl_one(self.one, self.width)
    }

    /// @h:countMinSignBits
    pub fn count_min_sign_bits(&self) -> u32 {
        if self.is_non_negative() {
            self.count_min_leading_zeros()
        } else if self.is_negative() {
            self.count_min_leading_ones()
        } else {
            1
        }
    }

    /// @h:countMaxSignificantBits
    pub fn count_max_significant_bits(&self) -> u32 {
        self.width - self.count_min_sign_bits() + 1
    }

    /// @h:countMaxTrailingZeros
    pub fn count_max_trailing_zeros(&self) -> u32 {
        countr_zero(self.one, self.width)
    }

    /// @h:countMaxTrailingOnes
    pub fn count_max_trailing_ones(&self) -> u32 {
        countr_zero(self.zero, self.width)
    }

    /// @h:countMaxLeadingZeros
    pub fn count_max_leading_zeros(&self) -> u32 {
        countl_zero(self.one, self.width)
    }

    /// @h:countMaxLeadingOnes
    pub fn count_max_leading_ones(&self) -> u32 {
        countl_zero(self.zero, self.width)
    }

    /// @h:countMinPopulation
    pub fn count_min_population(&self) -> u32 {
        popcount(self.one)
    }

    /// @h:countMaxPopulation
    pub fn count_max_population(&self) -> u32 {
        self.width - popcount(self.zero)
    }

    /// @h:countMaxActiveBits
    pub fn count_max_active_bits(&self) -> u32 {
        self.width - self.count_min_leading_zeros()
    }

    /// @h:intersectWith
    pub fn intersect_with(
        &self,
        rhs: &Self,
    ) -> Self {
        Self { zero: self.zero & rhs.zero, one: self.one & rhs.one, width: self.width }
    }

    /// @h:unionWith
    pub fn union_with(
        &self,
        rhs: &Self,
    ) -> Self {
        Self { zero: self.zero | rhs.zero, one: self.one | rhs.one, width: self.width }
    }

    /// @h:haveNoCommonBitsSet
    pub fn have_no_common_bits_set(
        lhs: &Self,
        rhs: &Self,
    ) -> bool {
        (lhs.zero | rhs.zero) == mask(lhs.width)
    }

    /// Flip the range of values: `[-0x80000000, 0x7FFFFFFF]` <-> `[0,
    /// 0xFFFFFFFF]`. @cpp:flipSignBit
    fn flip_sign_bit(val: &Self) -> Self {
        let top = val.width - 1;
        Self {
            zero: with_bit(val.zero, top, bit(val.one, top)),
            one: with_bit(val.one, top, bit(val.zero, top)),
            width: val.width,
        }
    }

    fn compute_for_add_carry_bits(
        lhs: &Self,
        rhs: &Self,
        carry_zero: bool,
        carry_one: bool,
    ) -> Self {
        let width = lhs.width;
        let m = mask(width);
        let possible_sum_zero = lhs.max_value().wrapping_add(rhs.max_value()).wrapping_add(u128::from(!carry_zero)) & m;
        let possible_sum_one = lhs.min_value().wrapping_add(rhs.min_value()).wrapping_add(u128::from(carry_one)) & m;
        let carry_known_zero = !(possible_sum_zero ^ lhs.zero ^ rhs.zero) & m;
        let carry_known_one = (possible_sum_one ^ lhs.one ^ rhs.one) & m;
        let known = (lhs.zero | lhs.one) & (rhs.zero | rhs.one) & (carry_known_zero | carry_known_one);
        Self { zero: !possible_sum_zero & known, one: possible_sum_one & known, width }
    }

    /// @cpp:computeForAddCarry
    pub fn compute_for_add_carry(
        lhs: &Self,
        rhs: &Self,
        carry: &Self,
    ) -> Self {
        assert_eq!(carry.width, 1, "Carry must be 1-bit");
        Self::compute_for_add_carry_bits(lhs, rhs, carry.zero != 0, carry.one != 0)
    }

    /// @cpp:computeForAddSub
    pub fn compute_for_add_sub(
        add: bool,
        nsw: bool,
        nuw: bool,
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        let mut out = Self::unknown(width);
        if lhs.is_unknown() && rhs.is_unknown() {
            return out;
        }
        if !lhs.is_unknown() && !rhs.is_unknown() {
            if add {
                out = Self::compute_for_add_carry_bits(lhs, rhs, true, false);
            } else {
                let not_rhs = Self { zero: rhs.one, one: rhs.zero, width };
                out = Self::compute_for_add_carry_bits(lhs, &not_rhs, false, true);
            }
        }
        if nuw {
            if add {
                let min_val = uadd_sat(lhs.min_value(), rhs.min_value(), width);
                if nsw {
                    let num = countl_one(min_val & mask(width - 1), width - 1);
                    out.one = set_range(out.one, width - 1 - num, width - 1);
                }
                out.one = set_high(out.one, width, countl_one(min_val, width));
            } else {
                let max_val = usub_sat(lhs.max_value(), rhs.min_value(), width);
                if nsw {
                    let num = countl_zero(max_val & mask(width - 1), width - 1);
                    out.zero = set_range(out.zero, width - 1 - num, width - 1);
                }
                out.zero = set_high(out.zero, width, countl_zero(max_val, width));
            }
        }
        if nsw {
            let (min_val, max_val) = if add {
                (
                    sadd_sat(lhs.signed_min_value(), rhs.signed_min_value(), width),
                    sadd_sat(lhs.signed_max_value(), rhs.signed_max_value(), width),
                )
            } else {
                (
                    ssub_sat(lhs.signed_min_value(), rhs.signed_max_value(), width),
                    ssub_sat(lhs.signed_max_value(), rhs.signed_min_value(), width),
                )
            };
            if !bit(min_val, width - 1) {
                let num = countl_one(min_val & mask(width - 1), width - 1);
                out.one = set_range(out.one, width - 1 - num, width - 1);
                out.zero |= sign_bit(width);
            }
            if bit(max_val, width - 1) {
                let num = countl_zero(max_val & mask(width - 1), width - 1);
                out.zero = set_range(out.zero, width - 1 - num, width - 1);
                out.one |= sign_bit(width);
            }
        }
        if out.has_conflict() {
            out.set_all_zero();
        }
        out
    }

    /// @cpp:computeForSubBorrow
    pub fn compute_for_sub_borrow(
        lhs: &Self,
        rhs: Self,
        borrow: &Self,
    ) -> Self {
        assert_eq!(borrow.width, 1, "Borrow must be 1-bit");
        let swapped = Self { zero: rhs.one, one: rhs.zero, width: rhs.width };
        Self::compute_for_add_carry_bits(lhs, &swapped, borrow.one != 0, borrow.zero != 0)
    }

    /// @h:add
    pub fn add(
        lhs: &Self,
        rhs: &Self,
        nsw: bool,
        nuw: bool,
        self_add: bool,
    ) -> Self {
        if self_add {
            assert_eq!(lhs, rhs, "Expected matching knownbits");
            return Self::shl(lhs, &Self::constant(8, 1), nuw, nsw, true);
        }
        Self::compute_for_add_sub(true, nsw, nuw, lhs, rhs)
    }

    /// @h:sub
    pub fn sub(
        lhs: &Self,
        rhs: &Self,
        nsw: bool,
        nuw: bool,
    ) -> Self {
        Self::compute_for_add_sub(false, nsw, nuw, lhs, rhs)
    }

    /// @cpp:computeForSatAddSub
    fn compute_for_sat_add_sub(
        add: bool,
        is_signed: bool,
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        let mut overflow: Option<bool> = None;
        let mut may_neg_clamp = true;
        let mut may_pos_clamp = true;
        if is_signed {
            if add && ((lhs.is_negative() && rhs.is_non_negative()) || (lhs.is_non_negative() && rhs.is_negative())) {
                overflow = Some(false);
            } else if !add
                && ((lhs.is_negative() && rhs.is_negative()) || (lhs.is_non_negative() && rhs.is_non_negative()))
            {
                overflow = Some(false);
            } else {
                let mut unsigned_lhs = *lhs;
                let mut unsigned_rhs = *rhs;
                unsigned_lhs.one &= !sign_bit(width);
                unsigned_lhs.zero |= sign_bit(width);
                unsigned_rhs.one &= !sign_bit(width);
                unsigned_rhs.zero |= sign_bit(width);
                let res = Self::compute_for_add_sub(add, false, false, &unsigned_lhs, &unsigned_rhs);
                if add {
                    if res.is_negative() {
                        may_neg_clamp = false;
                        if lhs.is_non_negative() && rhs.is_non_negative() {
                            overflow = Some(true);
                        }
                    } else if res.is_non_negative() {
                        may_pos_clamp = false;
                        if lhs.is_negative() && rhs.is_negative() {
                            overflow = Some(true);
                        }
                    }
                    if lhs.is_negative() || rhs.is_negative() {
                        may_pos_clamp = false;
                    }
                    if lhs.is_non_negative() || rhs.is_non_negative() {
                        may_neg_clamp = false;
                    }
                } else {
                    if res.is_negative() {
                        may_pos_clamp = false;
                        if lhs.is_negative() && rhs.is_non_negative() {
                            overflow = Some(true);
                        }
                    } else if res.is_non_negative() {
                        may_neg_clamp = false;
                        if lhs.is_non_negative() && rhs.is_negative() {
                            overflow = Some(true);
                        }
                    }
                    if lhs.is_negative() || rhs.is_non_negative() {
                        may_pos_clamp = false;
                    }
                    if lhs.is_non_negative() || rhs.is_negative() {
                        may_neg_clamp = false;
                    }
                }
            }
            if !may_neg_clamp && !may_pos_clamp {
                overflow = Some(false);
            }
        } else if add {
            if !uadd_ov(lhs.max_value(), rhs.max_value(), width).1 {
                overflow = Some(false);
            } else if uadd_ov(lhs.min_value(), rhs.min_value(), width).1 {
                overflow = Some(true);
            }
        } else if !usub_ov(lhs.min_value(), rhs.max_value(), width).1 {
            overflow = Some(false);
        } else if usub_ov(lhs.max_value(), rhs.min_value(), width).1 {
            overflow = Some(true);
        }

        let mut res = Self::compute_for_add_sub(add, is_signed, !is_signed, lhs, rhs);
        if let Some(overflowed) = overflow {
            if !overflowed {
                return res;
            }
            let c = if is_signed {
                assert!(!lhs.is_sign_unknown(), "We somehow know overflow without knowing input sign");
                if lhs.is_negative() { sign_bit(width) } else { mask(width) >> 1 }
            } else if add {
                mask(width)
            } else {
                0
            };
            res.one = c;
            res.zero = !c & mask(width);
            return res;
        }
        if is_signed {
            if may_pos_clamp {
                res.zero &= !mask(width - 1);
            }
            if may_neg_clamp {
                res.one &= !mask(width - 1);
            }
        } else if add {
            res.zero = 0;
        } else {
            res.one = 0;
        }
        res
    }

    /// @cpp:sadd_sat
    pub fn sadd_sat(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::compute_for_sat_add_sub(true, true, lhs, rhs)
    }

    /// @cpp:ssub_sat
    pub fn ssub_sat(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::compute_for_sat_add_sub(false, true, lhs, rhs)
    }

    /// @cpp:uadd_sat
    pub fn uadd_sat(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::compute_for_sat_add_sub(true, false, lhs, rhs)
    }

    /// @cpp:usub_sat
    pub fn usub_sat(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::compute_for_sat_add_sub(false, false, lhs, rhs)
    }

    /// @cpp:avgComputeU
    fn avg_compute_u(
        lhs: &Self,
        rhs: &Self,
        ceil: bool,
    ) -> Self {
        let width = lhs.width;
        let sum = Self::compute_for_add_carry_bits(&lhs.zext(width + 1), &rhs.zext(width + 1), !ceil, ceil);
        sum.extract_bits(width, 1)
    }

    /// @cpp:avgFloorS
    pub fn avg_floor_s(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::flip_sign_bit(&Self::avg_floor_u(&Self::flip_sign_bit(lhs), &Self::flip_sign_bit(rhs)))
    }

    /// @cpp:avgFloorU
    pub fn avg_floor_u(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::avg_compute_u(lhs, rhs, false)
    }

    /// @cpp:avgCeilS
    pub fn avg_ceil_s(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::flip_sign_bit(&Self::avg_ceil_u(&Self::flip_sign_bit(lhs), &Self::flip_sign_bit(rhs)))
    }

    /// @cpp:avgCeilU
    pub fn avg_ceil_u(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::avg_compute_u(lhs, rhs, true)
    }

    /// @cpp:mul
    pub fn mul(
        lhs: &Self,
        rhs: &Self,
        no_undef_self_multiply: bool,
    ) -> Self {
        let width = lhs.width;
        assert_eq!(width, rhs.width, "Operand mismatch");
        assert!(!no_undef_self_multiply || lhs == rhs, "Self multiplication knownbits mismatch");
        let (umax_result, overflow) = umul_ov(lhs.max_value(), rhs.max_value(), width);
        let lead_z = if overflow { 0 } else { countl_zero(umax_result, width) };

        let trail_bits_known_lhs = countr_one(lhs.zero | lhs.one, width);
        let trail_bits_known_rhs = countr_one(rhs.zero | rhs.one, width);
        let trail_zero_lhs = lhs.count_min_trailing_zeros();
        let trail_zero_rhs = rhs.count_min_trailing_zeros();
        let trail_z = trail_zero_lhs + trail_zero_rhs;
        let smallest_operand = min(trail_bits_known_lhs - trail_zero_lhs, trail_bits_known_rhs - trail_zero_rhs);
        let result_bits_known = min(smallest_operand + trail_z, width);

        let bottom_known = lhs.one.wrapping_mul(rhs.one) & mask(width);
        let mut res = Self::unknown(width);
        res.zero = set_high(0, width, lead_z);
        res.zero |= !bottom_known & mask(result_bits_known);
        res.one = bottom_known & mask(result_bits_known);

        if no_undef_self_multiply {
            let two_tz_p1 = 2 * trail_zero_lhs + 1;
            if two_tz_p1 < width {
                res.zero |= 1 << two_tz_p1;
            }
            if trail_zero_lhs < width && bit(lhs.one, trail_zero_lhs) {
                let two_tz_p2 = two_tz_p1 + 1;
                if two_tz_p2 < width {
                    res.zero |= 1 << two_tz_p2;
                }
            }
        }
        res
    }

    /// DEVIATE: at most 64 bits (the wide product is a `u128`).
    /// @cpp:mulhs
    pub fn mulhs(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        assert!(width <= 64 && width == rhs.width, "Operand mismatch");
        Self::mul(&lhs.sext(2 * width), &rhs.sext(2 * width), false).extract_bits(width, width)
    }

    /// DEVIATE: at most 64 bits (the wide product is a `u128`).
    /// @cpp:mulhu
    pub fn mulhu(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        assert!(width <= 64 && width == rhs.width, "Operand mismatch");
        Self::mul(&lhs.zext(2 * width), &rhs.zext(2 * width), false).extract_bits(width, width)
    }

    /// @cpp:divComputeLowBit
    fn div_compute_low_bit(
        mut known: Self,
        lhs: &Self,
        rhs: &Self,
        exact: bool,
    ) -> Self {
        if !exact {
            return known;
        }
        if bit(lhs.one, 0) {
            known.one |= 1;
        }
        let min_tz = lhs.count_min_trailing_zeros() as i64 - rhs.count_max_trailing_zeros() as i64;
        let max_tz = lhs.count_max_trailing_zeros() as i64 - rhs.count_min_trailing_zeros() as i64;
        if min_tz >= 0 {
            known.zero |= mask(min_tz as u32);
            if min_tz == max_tz {
                known.one |= 1_u128 << min_tz;
            }
        } else if max_tz < 0 {
            known.set_all_zero();
        }
        if known.has_conflict() {
            known.set_all_zero();
        }
        known
    }

    /// @cpp:sdiv
    pub fn sdiv(
        lhs: &Self,
        rhs: &Self,
        exact: bool,
    ) -> Self {
        if lhs.is_non_negative() && rhs.is_non_negative() {
            return Self::udiv(lhs, rhs, exact);
        }
        let width = lhs.width;
        let mut known = Self::unknown(width);
        if lhs.is_zero() || rhs.is_zero() {
            known.set_all_zero();
            return known;
        }
        let sdiv =
            |num: u128, denom: u128| (signed(num, width).wrapping_div(signed(denom, width)) as u128) & mask(width);
        let mut res: Option<u128> = None;
        if lhs.is_negative() && rhs.is_negative() {
            let denom = rhs.signed_max_value();
            let num = lhs.signed_min_value();
            res =
                Some(if num == sign_bit(width) && denom == mask(width) { mask(width) >> 1 } else { sdiv(num, denom) });
        } else if lhs.is_negative() && rhs.is_non_negative() {
            let neg_max = lhs.signed_max_value().wrapping_neg() & mask(width);
            if exact || neg_max >= rhs.signed_max_value() {
                let denom = rhs.signed_min_value();
                let num = lhs.signed_min_value();
                res = Some(if denom == 0 { num } else { sdiv(num, denom) });
            }
        } else if lhs.is_strictly_positive() && rhs.is_negative() {
            let neg_min = rhs.signed_min_value().wrapping_neg() & mask(width);
            if exact || lhs.signed_min_value() >= neg_min {
                res = Some(sdiv(lhs.signed_max_value(), rhs.signed_max_value()));
            }
        }
        if let Some(res) = res {
            if !bit(res, width - 1) {
                known.zero = set_high(known.zero, width, countl_zero(res, width));
            } else {
                known.one = set_high(known.one, width, countl_one(res, width));
            }
        }
        Self::div_compute_low_bit(known, lhs, rhs, exact)
    }

    /// @cpp:udiv
    pub fn udiv(
        lhs: &Self,
        rhs: &Self,
        exact: bool,
    ) -> Self {
        let width = lhs.width;
        let mut known = Self::unknown(width);
        if lhs.is_zero() || rhs.is_zero() {
            known.set_all_zero();
            return known;
        }
        let mut min_denom = rhs.min_value();
        if min_denom == 0 {
            min_denom |= 1_u128 << rhs.count_min_trailing_zeros();
        }
        let max_res = lhs.max_value() / min_denom;
        let min_res = lhs.min_value() / rhs.max_value();
        known = Self::constant(width, min_res);
        if min_res != max_res {
            let different = width - 1 - countl_zero(min_res ^ max_res, width);
            known.zero &= !mask(different + 1);
            known.one &= !mask(different + 1);
        }
        Self::div_compute_low_bit(known, lhs, rhs, exact)
    }

    /// @cpp:remGetLowBits
    fn rem_get_low_bits(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        if !rhs.is_zero() && bit(rhs.zero, 0) {
            let low = mask(rhs.count_min_trailing_zeros());
            return Self { zero: lhs.zero & low, one: lhs.one & low, width };
        }
        Self::unknown(width)
    }

    /// @cpp:urem
    pub fn urem(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        if lhs.max_value() < rhs.min_value() {
            return *lhs;
        }
        let mut known = Self::rem_get_low_bits(lhs, rhs);
        let max_res = min(lhs.max_value(), rhs.max_value().wrapping_sub(1) & mask(lhs.width));
        known.zero = set_high(known.zero, lhs.width, countl_zero(max_res, lhs.width));
        known
    }

    /// @cpp:srem
    pub fn srem(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        let mut known = Self::rem_get_low_bits(lhs, rhs);
        if rhs.is_constant() && rhs.constant_value().is_power_of_two() {
            let low_bits = rhs.constant_value() - 1;
            if lhs.is_non_negative() || low_bits & !lhs.zero == 0 {
                known.zero |= !low_bits & mask(width);
            }
            if lhs.is_negative() && low_bits & lhs.one != 0 {
                known.one |= !low_bits & mask(width);
            }
            return known;
        }
        if lhs.is_negative() && known.is_non_zero() {
            known.one = set_high(known.one, width, max(lhs.count_min_leading_ones(), rhs.count_min_sign_bits()));
        } else if lhs.is_non_negative() {
            known.zero = set_high(known.zero, width, max(lhs.count_min_leading_zeros(), rhs.count_min_sign_bits()));
        }
        known
    }

    /// @cpp:umax
    pub fn umax(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        if lhs.min_value() >= rhs.max_value() {
            return *lhs;
        }
        if rhs.min_value() >= lhs.max_value() {
            return *rhs;
        }
        let l = lhs.make_ge(rhs.min_value());
        let r = rhs.make_ge(lhs.min_value());
        l.intersect_with(&r)
    }

    /// @cpp:umin
    pub fn umin(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let flip = |v: &Self| Self { zero: v.one, one: v.zero, width: v.width };
        flip(&Self::umax(&flip(lhs), &flip(rhs)))
    }

    /// @cpp:smax
    pub fn smax(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::flip_sign_bit(&Self::umax(&Self::flip_sign_bit(lhs), &Self::flip_sign_bit(rhs)))
    }

    /// @cpp:smin
    pub fn smin(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let flip = |v: &Self| {
            let top = v.width - 1;
            Self {
                zero: with_bit(v.one, top, bit(v.zero, top)),
                one: with_bit(v.zero, top, bit(v.one, top)),
                width: v.width,
            }
        };
        flip(&Self::umax(&flip(lhs), &flip(rhs)))
    }

    /// @cpp:abdu
    pub fn abdu(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        if lhs.min_value() >= rhs.max_value() {
            return Self::compute_for_add_sub(false, false, false, lhs, rhs);
        }
        if rhs.min_value() >= lhs.max_value() {
            return Self::compute_for_add_sub(false, false, false, rhs, lhs);
        }
        let diff0 = Self::compute_for_add_sub(false, false, true, lhs, rhs);
        let diff1 = Self::compute_for_add_sub(false, false, true, rhs, lhs);
        diff0.intersect_with(&diff1)
    }

    /// @cpp:abds
    pub fn abds(
        mut lhs: Self,
        mut rhs: Self,
    ) -> Self {
        if signed(lhs.signed_min_value(), lhs.width) >= signed(rhs.signed_max_value(), rhs.width) {
            return Self::compute_for_add_sub(false, false, false, &lhs, &rhs);
        }
        if signed(rhs.signed_min_value(), rhs.width) >= signed(lhs.signed_max_value(), lhs.width) {
            return Self::compute_for_add_sub(false, false, false, &rhs, &lhs);
        }
        let top = lhs.width - 1;
        for arg in [&mut lhs, &mut rhs] {
            let tmp = bit(arg.zero, top);
            arg.zero = with_bit(arg.zero, top, bit(arg.one, top));
            arg.one = with_bit(arg.one, top, tmp);
        }
        let diff0 = Self::compute_for_add_sub(false, false, true, &lhs, &rhs);
        let diff1 = Self::compute_for_add_sub(false, false, true, &rhs, &lhs);
        diff0.intersect_with(&diff1)
    }

    /// @cpp:getMaxShiftAmount
    fn max_shift_amount(
        max_value: u128,
        value_width: u32,
        width: u32,
    ) -> u32 {
        if width.is_power_of_two() {
            let bits = min(width.ilog2(), value_width);
            (max_value & mask(bits)) as u32
        } else {
            min(max_value, u128::from(width - 1)) as u32
        }
    }

    /// The amounts of `rhs` a loop over `min..=max` may take.
    fn shift_amount_possible(
        rhs: &Self,
        amount: u32,
    ) -> bool {
        let zero_mask = rhs.zero as u32;
        let one_mask = rhs.one as u32;
        zero_mask & amount == 0 && (one_mask | amount) == amount
    }

    /// @cpp:shl
    pub fn shl(
        lhs: &Self,
        rhs: &Self,
        nuw: bool,
        nsw: bool,
        shamt_non_zero: bool,
    ) -> Self {
        let width = lhs.width;
        let shift_by_const = |lhs: &Self, amount: u32| {
            let mut known = Self::unknown(width);
            let (zero, mut shifted_out_zero) = ushl_ov(lhs.zero, width, amount);
            known.zero = zero | mask(amount);
            let (one, shifted_out_one) = ushl_ov(lhs.one, width, amount);
            known.one = one;
            if nsw {
                if nuw && amount != 0 {
                    shifted_out_zero = true;
                }
                if shifted_out_zero {
                    known.make_non_negative();
                } else if shifted_out_one {
                    known.make_negative();
                }
            }
            known
        };
        let mut known = Self::unknown(width);
        let mut min_shift = min(rhs.min_value(), u128::from(width)) as u32;
        if min_shift == 0 && shamt_non_zero {
            min_shift = 1;
        }
        if lhs.is_unknown() {
            known.zero = mask(min_shift);
            if nuw && nsw && min_shift != 0 {
                known.make_non_negative();
            }
            return known;
        }
        let mut max_shift = Self::max_shift_amount(rhs.max_value(), rhs.width, width);
        if nuw && nsw {
            max_shift = min(max_shift, lhs.count_max_leading_zeros().wrapping_sub(1));
        }
        if nuw {
            max_shift = min(max_shift, lhs.count_max_leading_zeros());
        }
        if nsw {
            max_shift =
                min(max_shift, max(lhs.count_max_leading_zeros(), lhs.count_max_leading_ones()).wrapping_sub(1));
        }
        if min_shift == 0 && max_shift == width - 1 && width.is_power_of_two() {
            known.zero = mask(lhs.count_min_trailing_zeros());
            if lhs.is_all_ones() {
                known.one |= sign_bit(width);
            }
            if nsw {
                if lhs.is_non_negative() {
                    known.make_non_negative();
                }
                if lhs.is_negative() {
                    known.make_negative();
                }
            }
            return known;
        }
        known.set_all_conflict();
        for amount in min_shift..=max_shift {
            if !Self::shift_amount_possible(rhs, amount) {
                continue;
            }
            known = known.intersect_with(&shift_by_const(lhs, amount));
            if known.is_unknown() {
                break;
            }
        }
        if known.has_conflict() {
            known.set_all_zero();
        }
        known
    }

    /// @cpp:lshr
    pub fn lshr(
        lhs: &Self,
        rhs: &Self,
        shamt_non_zero: bool,
        exact: bool,
    ) -> Self {
        let width = lhs.width;
        let shift_by_const = |lhs: &Self, amount: u32| {
            let mut known = Self { zero: lshr(lhs.zero, amount), one: lshr(lhs.one, amount), width };
            known.zero = set_high(known.zero, width, amount);
            known
        };
        let mut known = Self::unknown(width);
        let mut min_shift = min(rhs.min_value(), u128::from(width)) as u32;
        if min_shift == 0 && shamt_non_zero {
            min_shift = 1;
        }
        if lhs.is_unknown() {
            known.zero = set_high(0, width, min_shift);
            return known;
        }
        let mut max_shift = Self::max_shift_amount(rhs.max_value(), rhs.width, width);
        if exact {
            let first_one = lhs.count_max_trailing_zeros();
            if first_one < min_shift {
                known.set_all_zero();
                return known;
            }
            max_shift = min(max_shift, first_one);
        }
        known.set_all_conflict();
        for amount in min_shift..=max_shift {
            if !Self::shift_amount_possible(rhs, amount) {
                continue;
            }
            known = known.intersect_with(&shift_by_const(lhs, amount));
            if known.is_unknown() {
                break;
            }
        }
        if known.has_conflict() {
            known.set_all_zero();
        }
        known
    }

    /// @cpp:ashr
    pub fn ashr(
        lhs: &Self,
        rhs: &Self,
        shamt_non_zero: bool,
        exact: bool,
    ) -> Self {
        let width = lhs.width;
        let shift_by_const = |lhs: &Self, amount: u32| Self {
            zero: ashr(lhs.zero, width, amount),
            one: ashr(lhs.one, width, amount),
            width,
        };
        let mut known = Self::unknown(width);
        let mut min_shift = min(rhs.min_value(), u128::from(width)) as u32;
        if min_shift == 0 && shamt_non_zero {
            min_shift = 1;
        }
        if lhs.is_unknown() {
            if min_shift == width {
                known.set_all_zero();
            }
            return known;
        }
        let mut max_shift = Self::max_shift_amount(rhs.max_value(), rhs.width, width);
        if exact {
            let first_one = lhs.count_max_trailing_zeros();
            if first_one < min_shift {
                known.set_all_zero();
                return known;
            }
            max_shift = min(max_shift, first_one);
        }
        known.set_all_conflict();
        for amount in min_shift..=max_shift {
            if !Self::shift_amount_possible(rhs, amount) {
                continue;
            }
            known = known.intersect_with(&shift_by_const(lhs, amount));
            if known.is_unknown() {
                break;
            }
        }
        if known.has_conflict() {
            known.set_all_zero();
        }
        known
    }

    /// @cpp:fshl
    pub fn fshl(
        lhs: &Self,
        rhs: &Self,
        amt: u128,
    ) -> Self {
        let w = lhs.width;
        Self::raw(w, fshl(lhs.zero, rhs.zero, amt, w), fshl(lhs.one, rhs.one, amt, w))
    }

    /// @cpp:fshr
    pub fn fshr(
        lhs: &Self,
        rhs: &Self,
        amt: u128,
    ) -> Self {
        let w = lhs.width;
        Self::raw(w, fshr(lhs.zero, rhs.zero, amt, w), fshr(lhs.one, rhs.one, amt, w))
    }

    /// @cpp:clmul
    pub fn clmul(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        let mut res = Self::constant(width, clmul(lhs.min_value(), rhs.min_value(), width));
        let clmul_or = |a: u128, b: u128| {
            let mut result = 0_u128;
            let active = width - countl_zero(b, width);
            for i in 0..min(active, width - countr_zero(a, width)) {
                if bit(b, i) {
                    result |= (a << i) & mask(width);
                }
            }
            result
        };
        let known = !(clmul_or(!lhs.zero & !lhs.one & mask(width), !rhs.zero & mask(width))
            | clmul_or(!lhs.zero & mask(width), !rhs.zero & !rhs.one & mask(width)))
            & mask(width);
        res.zero &= known;
        res.one &= known;
        res
    }

    /// @cpp:pext
    pub fn pext(
        val: &Self,
        mask_bits: &Self,
    ) -> Self {
        let width = val.width;
        let mut res = Self::unknown(width);
        res.set_all_conflict();
        let (mut m0, mut m1) = (0_u32, 0_u32);
        for i in 0..width {
            if !bit(mask_bits.zero, i) {
                if !bit(val.zero, i) {
                    res.zero = clear_range(res.zero, m0, m1 + 1);
                }
                if !bit(val.one, i) {
                    res.one = clear_range(res.one, m0, m1 + 1);
                }
            }
            if bit(mask_bits.one, i) {
                m0 += 1;
                m1 += 1;
            } else if !bit(mask_bits.zero, i) {
                m1 += 1;
            }
        }
        res.one = clear_range(res.one, m0, width);
        res
    }

    /// @cpp:pdep
    pub fn pdep(
        val: &Self,
        mask_bits: &Self,
    ) -> Self {
        let width = val.width;
        let mut res = Self::unknown(width);
        res.set_all_conflict();
        let (mut m0, mut m1) = (0_u32, 0_u32);
        for i in 0..width {
            if !bit(mask_bits.one, i) {
                res.one = with_bit(res.one, i, false);
            }
            if !bit(mask_bits.zero, i) {
                let range = set_range(0, m0, m1 + 1) & mask(width);
                if range & !val.one != 0 {
                    res.one = with_bit(res.one, i, false);
                }
                if range & !val.zero != 0 {
                    res.zero = with_bit(res.zero, i, false);
                }
            }
            if bit(mask_bits.one, i) {
                m0 += 1;
                m1 += 1;
            } else if !bit(mask_bits.zero, i) {
                m1 += 1;
            }
        }
        res
    }

    /// @cpp:eq
    pub fn eq(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        if lhs.is_constant() && rhs.is_constant() {
            return Some(lhs.constant_value() == rhs.constant_value());
        }
        if lhs.one & rhs.zero != 0 || rhs.one & lhs.zero != 0 {
            return Some(false);
        }
        None
    }

    /// @cpp:ne
    pub fn ne(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::eq(lhs, rhs).map(|equal| !equal)
    }

    /// @cpp:ugt
    pub fn ugt(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        if lhs.max_value() <= rhs.min_value() {
            return Some(false);
        }
        if lhs.min_value() > rhs.max_value() {
            return Some(true);
        }
        None
    }

    /// @cpp:uge
    pub fn uge(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::ugt(rhs, lhs).map(|greater| !greater)
    }

    /// @cpp:ult
    pub fn ult(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::ugt(rhs, lhs)
    }

    /// @cpp:ule
    pub fn ule(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::uge(rhs, lhs)
    }

    /// @cpp:sgt
    pub fn sgt(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        let w = lhs.width;
        if signed(lhs.signed_max_value(), w) <= signed(rhs.signed_min_value(), w) {
            return Some(false);
        }
        if signed(lhs.signed_min_value(), w) > signed(rhs.signed_max_value(), w) {
            return Some(true);
        }
        None
    }

    /// @cpp:sge
    pub fn sge(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::sgt(rhs, lhs).map(|greater| !greater)
    }

    /// @cpp:slt
    pub fn slt(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::sgt(rhs, lhs)
    }

    /// @cpp:sle
    pub fn sle(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::sge(rhs, lhs)
    }

    /// @cpp:operator&=
    pub fn and(
        &self,
        rhs: &Self,
    ) -> Self {
        Self { zero: self.zero | rhs.zero, one: self.one & rhs.one, width: self.width }
    }

    /// @cpp:operator|=
    pub fn or(
        &self,
        rhs: &Self,
    ) -> Self {
        Self { zero: self.zero & rhs.zero, one: self.one | rhs.one, width: self.width }
    }

    /// @cpp:operator^=
    pub fn xor(
        &self,
        rhs: &Self,
    ) -> Self {
        Self {
            zero: (self.zero & rhs.zero) | (self.one & rhs.one),
            one: (self.zero & rhs.one) | (self.one & rhs.zero),
            width: self.width,
        }
    }

    /// @h:operator<<=
    pub fn shl_by(
        &self,
        amount: u32,
    ) -> Self {
        Self::raw(self.width, lshr_or_zero_shl(self.zero, amount), lshr_or_zero_shl(self.one, amount))
    }

    /// @h:operator>>=
    pub fn lshr_by(
        &self,
        amount: u32,
    ) -> Self {
        Self::raw(self.width, lshr(self.zero, amount), lshr(self.one, amount))
    }

    /// @cpp:abs
    pub fn abs(
        &self,
        int_min_is_poison: bool,
    ) -> Self {
        let width = self.width;
        if self.is_non_negative() {
            return *self;
        }
        let mut abs = Self::unknown(width);
        if self.is_negative() {
            let mut tmp = *self;
            if int_min_is_poison && popcount(self.zero) + 2 == width {
                tmp.one |= 1_u128 << self.count_min_trailing_zeros();
            }
            abs = Self::compute_for_add_sub(false, int_min_is_poison, false, &Self::constant(width, 0), &tmp);
            if int_min_is_poison && tmp.count_min_population() == 1 && tmp.count_max_population() != 1 {
                tmp.one &= !sign_bit(width);
                tmp.zero |= sign_bit(width);
                abs.one = set_range(abs.one, width - tmp.count_min_leading_zeros(), width - 1);
            }
        } else {
            let max_tz = self.count_max_trailing_zeros();
            let min_tz = self.count_min_trailing_zeros();
            abs.zero = mask(min_tz);
            if max_tz == min_tz && max_tz < width {
                abs.one |= 1_u128 << max_tz;
            }
            if int_min_is_poison || (self.one != 0 && self.one != sign_bit(width)) {
                abs.one &= !sign_bit(width);
                abs.zero |= sign_bit(width);
            }
        }
        abs
    }

    /// @cpp:reduceAdd
    pub fn reduce_add(
        &self,
        num_elts: u32,
    ) -> Self {
        let width = self.width;
        if num_elts == 0 {
            return Self::unknown(width);
        }
        if self.is_constant() {
            return Self::constant(width, u128::from(num_elts).wrapping_mul(self.constant_value()) & mask(width));
        }
        let mut result = Self::unknown(width);
        let lost_bits = if num_elts <= 1 { 0 } else { 32 - (num_elts - 1).leading_zeros() };
        if self.is_non_negative() {
            let leading_zeros = self.count_min_leading_zeros();
            let leading_zeros = if leading_zeros > lost_bits { leading_zeros - lost_bits } else { 0 };
            result.zero = set_high(0, width, leading_zeros);
        } else if self.is_negative() {
            let leading_ones = self.count_min_leading_ones();
            let leading_ones = if leading_ones > lost_bits { leading_ones - lost_bits } else { 0 };
            result.one = set_high(0, width, leading_ones);
        }
        result
    }

    /// @h:byteSwap
    pub fn byte_swap(&self) -> Self {
        let swap = |v: u128| v.swap_bytes() >> (128 - self.width);
        Self::raw(self.width, swap(self.zero), swap(self.one))
    }

    /// @h:reverseBits
    pub fn reverse_bits(&self) -> Self {
        let reverse = |v: u128| v.reverse_bits() >> (128 - self.width);
        Self::raw(self.width, reverse(self.zero), reverse(self.one))
    }

    /// @cpp:blsi
    pub fn blsi(&self) -> Self {
        let width = self.width;
        let mut known = Self { zero: self.zero, one: 0, width };
        let max = self.count_max_trailing_zeros();
        known.zero = set_range(known.zero, min(max + 1, width), width);
        let min_tz = self.count_min_trailing_zeros();
        if max == min_tz && max < width {
            known.one |= 1_u128 << max;
        }
        known
    }

    /// @cpp:blsmsk
    pub fn blsmsk(&self) -> Self {
        let width = self.width;
        let mut known = Self::unknown(width);
        let max = self.count_max_trailing_zeros();
        known.zero = set_range(known.zero, min(max + 1, width), width);
        let min_tz = self.count_min_trailing_zeros();
        known.one = mask(min(min_tz + 1, width));
        known
    }

    /// @cpp:print
    pub fn print(&self) -> String {
        (0..self.width)
            .rev()
            .map(|n| match (bit(self.zero, n), bit(self.one, n)) {
                (true, true) => '!',
                (true, false) => '0',
                (false, true) => '1',
                (false, false) => '?',
            })
            .collect()
    }
}

fn lshr_or_zero_shl(
    v: u128,
    by: u32,
) -> u128 {
    if by >= 128 { 0 } else { v << by }
}

/// `SignBitsOps::rot`: the sign bits left in a rotated value.
/// @cpp:rot
pub fn rot_sign_bits(
    src_sign_bits: u32,
    width: u32,
    rot_amount: Option<u128>,
    rotate_right: bool,
) -> u32 {
    if src_sign_bits == width {
        return width;
    }
    let Some(amount) = rot_amount else { return 1 };
    let mut amt = rotate_amount(amount, width);
    if rotate_right {
        amt = (width - amt) % width;
    }
    if src_sign_bits > amt + 1 { src_sign_bits - amt } else { 1 }
}
