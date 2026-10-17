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

use crate::apint::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bits<W: Word = u128> {
    pub zero: W,
    pub one: W,
    pub width: u32,
}

/// LLVM's `KnownBits`, of a MIR integer (`i1` to `i128`).
pub type KnownBits = Bits<u128>;

impl<W: Word> Bits<W> {
    fn raw(
        width: u32,
        zero: W,
        one: W,
    ) -> Self {
        Self { zero: zero & mask(width), one: one & mask(width), width }
    }

    /// The same bits in another word; they fit it by construction.
    pub fn to_word<V: Word>(&self) -> Bits<V> {
        Bits { zero: V::from_u128(self.zero.low_u128()), one: V::from_u128(self.one.low_u128()), width: self.width }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L41>
    pub fn unknown(width: u32) -> Self {
        assert!((1..=W::BITS).contains(&width), "KnownBits: width {width} is outside 1..={}", W::BITS);
        Self { zero: W::ZERO, one: W::ZERO, width }
    }

    /// Both masks at a width, as LLVM's private `KnownBits(Zero, One)`.
    pub fn from_masks(
        width: u32,
        zero: W,
        one: W,
    ) -> Self {
        assert!((1..=W::BITS).contains(&width), "KnownBits: width {width} is outside 1..={}", W::BITS);
        Self::raw(width, zero, one)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L369>
    pub fn constant(
        width: u32,
        value: W,
    ) -> Self {
        Self::from_masks(width, !value, value)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L51>
    pub fn has_conflict(&self) -> bool {
        self.zero & self.one != W::ZERO
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L54>
    pub fn is_constant(&self) -> bool {
        self.zero ^ self.one == mask(self.width) && !self.has_conflict()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L58>
    pub fn constant_value(&self) -> W {
        assert!(self.is_constant(), "Can only get value when all bits are known");
        self.one
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L64>
    pub fn is_unknown(&self) -> bool {
        self.zero == W::ZERO && self.one == W::ZERO
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L67>
    pub fn is_sign_unknown(&self) -> bool {
        !bit(self.zero, self.width - 1) && !bit(self.one, self.width - 1)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L72>
    pub fn reset_all(&mut self) {
        self.zero = W::ZERO;
        self.one = W::ZERO;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L64>
    pub fn is_zero(&self) -> bool {
        self.zero == mask(self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L78>
    pub fn is_all_ones(&self) -> bool {
        self.one == mask(self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L84>
    pub fn set_all_zero(&mut self) {
        self.zero = mask(self.width);
        self.one = W::ZERO;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L90>
    pub fn set_all_ones(&mut self) {
        self.zero = W::ZERO;
        self.one = mask(self.width);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L97>
    pub fn set_all_conflict(&mut self) {
        self.zero = mask(self.width);
        self.one = mask(self.width);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L103>
    pub fn is_negative(&self) -> bool {
        bit(self.one, self.width - 1)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L106>
    pub fn is_non_negative(&self) -> bool {
        bit(self.zero, self.width - 1)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L109>
    pub fn is_non_zero(&self) -> bool {
        self.one != W::ZERO
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L112>
    pub fn is_strictly_positive(&self) -> bool {
        self.is_non_negative() && self.is_non_zero()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L117>
    pub fn is_non_positive(&self) -> bool {
        !sgt(self.signed_max_value(), W::ZERO, self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L120>
    pub fn make_negative(&mut self) {
        self.one |= sign_bit::<W>(self.width);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L125>
    pub fn make_non_negative(&mut self) {
        self.zero |= sign_bit::<W>(self.width);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L130>
    pub fn min_value(&self) -> W {
        self.one
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L136>
    pub fn signed_min_value(&self) -> W {
        let min = self.one;
        if bit(self.zero, self.width - 1) { min } else { min | sign_bit::<W>(self.width) }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L146>
    pub fn max_value(&self) -> W {
        !self.zero & mask(self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L117>
    pub fn signed_max_value(&self) -> W {
        let max = self.max_value();
        if bit(self.one, self.width - 1) { max } else { max & !sign_bit::<W>(self.width) }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L162>
    pub fn is_even(&self) -> bool {
        bit(self.zero, 0)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L165>
    pub fn trunc(
        &self,
        width: u32,
    ) -> Self {
        Self::from_masks(width, self.zero, self.one)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L171>
    pub fn anyext(
        &self,
        width: u32,
    ) -> Self {
        Self::from_masks(width, self.zero, self.one)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L172>
    pub fn zext(
        &self,
        width: u32,
    ) -> Self {
        Self::from_masks(width, set_range(self.zero, self.width, width), self.one)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L184>
    pub fn sext(
        &self,
        width: u32,
    ) -> Self {
        Self::from_masks(width, sext(self.zero, self.width, width), sext(self.one, self.width, width))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L190>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L200>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L210>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L159>
    pub fn trunc_ssat(
        &self,
        width: u32,
    ) -> Self {
        let input = self.width;
        let min_in_range = sext(sign_bit::<W>(width), width, input);
        let max_in_range = sext(mask::<W>(width) >> 1, width, input);
        let (input_min, input_max) = (self.signed_min_value(), self.signed_max_value());
        let (sge, sle) = (|a: W, b: W| !slt(a, b, input), |a: W, b: W| !sgt(a, b, input));
        let mut known = Self::unknown(width);
        let top = width - 1;
        if sge(input_min, min_in_range) && sle(input_max, max_in_range) {
            known = self.trunc(width);
        } else if slt(input_max, min_in_range, input) {
            known = Self::constant(width, sign_bit::<W>(width));
        } else if sgt(input_min, max_in_range, input) {
            known = Self::constant(width, mask::<W>(width) >> 1);
        } else if !is_negative(input_min, input) {
            known.zero = sign_bit::<W>(width);
            known.one = with_bit(self.one & mask(width), top, false);
        } else if is_negative(input_max, input) {
            known.zero = with_bit(self.zero & mask(width), top, false);
            known.one = sign_bit::<W>(width);
        } else {
            let (zero_lower, one_lower) = (self.zero & mask(width), self.one & mask(width));
            let min_sat = Self::constant(width, sign_bit::<W>(width));
            let max_sat = Self::constant(width, mask::<W>(width) >> 1);
            if sle(input_max, max_in_range) {
                known.zero = zero_lower & min_sat.zero;
                known.one = one_lower & min_sat.one;
            } else if sge(input_min, min_in_range) {
                known.zero = zero_lower & max_sat.zero;
                known.one = one_lower & max_sat.one;
            } else {
                known.zero = zero_lower & min_sat.zero & max_sat.zero;
                known.one = one_lower & min_sat.one & max_sat.one;
            }
        }
        known
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L225>
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
            known.zero = W::ZERO;
            known.one = self.one & mask(width);
        } else if input_max <= max_in_range {
            known.zero = self.zero & mask(width);
            known.one = W::ZERO;
        } else {
            known.zero = W::ZERO;
            known.one = W::ZERO;
        }
        known
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L262>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L280>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L233>
    pub fn insert_bits(
        &mut self,
        sub: &Self,
        position: u32,
    ) {
        let hole = mask::<W>(sub.width) << position;
        self.zero = (self.zero & !hole) | (sub.zero << position);
        self.one = (self.one & !hole) | (sub.one << position);
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L239>
    pub fn extract_bits(
        &self,
        count: u32,
        position: u32,
    ) -> Self {
        Self::from_masks(count, lshr(self.zero, position), lshr(self.one, position))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L247>
    pub fn concat(
        &self,
        low: &Self,
    ) -> Self {
        let width = self.width + low.width;
        Self::from_masks(width, self.zero << low.width | low.zero, self.one << low.width | low.one)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L297>
    pub fn make_ge(
        &self,
        value: W,
    ) -> Self {
        let n = countl_one(self.zero | value, self.width);
        let masked = value & !mask::<W>(self.width - n);
        Self { zero: self.zero, one: self.one | masked, width: self.width }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L256>
    pub fn count_min_trailing_zeros(&self) -> u32 {
        countr_one(self.zero, self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L259>
    pub fn count_min_trailing_ones(&self) -> u32 {
        countr_one(self.one, self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L262>
    pub fn count_min_leading_zeros(&self) -> u32 {
        countl_one(self.zero, self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L265>
    pub fn count_min_leading_ones(&self) -> u32 {
        countl_one(self.one, self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L269>
    pub fn count_min_sign_bits(&self) -> u32 {
        if self.is_non_negative() {
            self.count_min_leading_zeros()
        } else if self.is_negative() {
            self.count_min_leading_ones()
        } else {
            1
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L283>
    pub fn count_max_significant_bits(&self) -> u32 {
        self.width - self.count_min_sign_bits() + 1
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L288>
    pub fn count_max_trailing_zeros(&self) -> u32 {
        countr_zero(self.one, self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L291>
    pub fn count_max_trailing_ones(&self) -> u32 {
        countr_zero(self.zero, self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L294>
    pub fn count_max_leading_zeros(&self) -> u32 {
        countl_zero(self.one, self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L297>
    pub fn count_max_leading_ones(&self) -> u32 {
        countl_zero(self.zero, self.width)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L300>
    pub fn count_min_population(&self) -> u32 {
        popcount(self.one)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L303>
    pub fn count_max_population(&self) -> u32 {
        self.width - popcount(self.zero)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L310>
    pub fn count_max_active_bits(&self) -> u32 {
        self.width - self.count_min_leading_zeros()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L325>
    pub fn intersect_with(
        &self,
        rhs: &Self,
    ) -> Self {
        Self { zero: self.zero & rhs.zero, one: self.one & rhs.one, width: self.width }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L335>
    pub fn union_with(
        &self,
        rhs: &Self,
    ) -> Self {
        Self { zero: self.zero | rhs.zero, one: self.one | rhs.one, width: self.width }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L340>
    pub fn have_no_common_bits_set(
        lhs: &Self,
        rhs: &Self,
    ) -> bool {
        (lhs.zero | rhs.zero) == mask(lhs.width)
    }

    /// Flip the range of values: `[-0x80000000, 0x7FFFFFFF]` <-> `[0,
    /// 0xFFFFFFFF]`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L22>
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
        let possible_sum_zero =
            lhs.max_value().wrapping_add(rhs.max_value()).wrapping_add(W::from_u32(u32::from(!carry_zero))) & m;
        let possible_sum_one =
            lhs.min_value().wrapping_add(rhs.min_value()).wrapping_add(W::from_u32(u32::from(carry_one))) & m;
        let carry_known_zero = !(possible_sum_zero ^ lhs.zero ^ rhs.zero) & m;
        let carry_known_one = (possible_sum_one ^ lhs.one ^ rhs.one) & m;
        let known = (lhs.zero | lhs.one) & (rhs.zero | rhs.one) & (carry_known_zero | carry_known_one);
        Self { zero: !possible_sum_zero & known, one: possible_sum_one & known, width }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L54>
    pub fn compute_for_add_carry(
        lhs: &Self,
        rhs: &Self,
        carry: &Self,
    ) -> Self {
        assert_eq!(carry.width, 1, "Carry must be 1-bit");
        Self::compute_for_add_carry_bits(lhs, rhs, carry.zero != W::ZERO, carry.one != W::ZERO)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L61>
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
                out.zero |= sign_bit::<W>(width);
            }
            if bit(max_val, width - 1) {
                let num = countl_zero(max_val & mask(width - 1), width - 1);
                out.zero = set_range(out.zero, width - 1 - num, width - 1);
                out.one |= sign_bit::<W>(width);
            }
        }
        if out.has_conflict() {
            out.set_all_zero();
        }
        out
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L147>
    pub fn compute_for_sub_borrow(
        lhs: &Self,
        rhs: Self,
        borrow: &Self,
    ) -> Self {
        assert_eq!(borrow.width, 1, "Borrow must be 1-bit");
        let swapped = Self { zero: rhs.one, one: rhs.zero, width: rhs.width };
        Self::compute_for_add_carry_bits(lhs, &swapped, borrow.one != W::ZERO, borrow.zero != W::ZERO)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L361>
    pub fn add(
        lhs: &Self,
        rhs: &Self,
        nsw: bool,
        nuw: bool,
        self_add: bool,
    ) -> Self {
        if self_add {
            assert_eq!(lhs, rhs, "Expected matching knownbits");
            return Self::shl(lhs, &Self::constant(8, W::ONE), nuw, nsw, true);
        }
        Self::compute_for_add_sub(true, nsw, nuw, lhs, rhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L376>
    pub fn sub(
        lhs: &Self,
        rhs: &Self,
        nsw: bool,
        nuw: bool,
    ) -> Self {
        Self::compute_for_add_sub(false, nsw, nuw, lhs, rhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L876>
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
                unsigned_lhs.one &= !sign_bit::<W>(width);
                unsigned_lhs.zero |= sign_bit::<W>(width);
                unsigned_rhs.one &= !sign_bit::<W>(width);
                unsigned_rhs.zero |= sign_bit::<W>(width);
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
                if lhs.is_negative() { sign_bit::<W>(width) } else { mask::<W>(width) >> 1 }
            } else if add {
                mask::<W>(width)
            } else {
                W::ZERO
            };
            res.one = c;
            res.zero = !c & mask(width);
            return res;
        }
        if is_signed {
            if may_pos_clamp {
                res.zero &= !mask::<W>(width - 1);
            }
            if may_neg_clamp {
                res.one &= !mask::<W>(width - 1);
            }
        } else if add {
            res.zero = W::ZERO;
        } else {
            res.one = W::ZERO;
        }
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1033>
    pub fn sadd_sat(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::compute_for_sat_add_sub(true, true, lhs, rhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1036>
    pub fn ssub_sat(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::compute_for_sat_add_sub(false, true, lhs, rhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1039>
    pub fn uadd_sat(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::compute_for_sat_add_sub(true, false, lhs, rhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1042>
    pub fn usub_sat(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::compute_for_sat_add_sub(false, false, lhs, rhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1072>
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
        res.zero = set_high(W::ZERO, width, lead_z);
        res.zero |= !bottom_known & mask(result_bits_known);
        res.one = bottom_known & mask(result_bits_known);

        if no_undef_self_multiply {
            let two_tz_p1 = 2 * trail_zero_lhs + 1;
            if two_tz_p1 < width {
                res.zero |= W::ONE << two_tz_p1;
            }
            if trail_zero_lhs < width && bit(lhs.one, trail_zero_lhs) {
                let two_tz_p2 = two_tz_p1 + 1;
                if two_tz_p2 < width {
                    res.zero |= W::ONE << two_tz_p2;
                }
            }
        }
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1191>
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
            known.one |= W::ONE;
        }
        let min_tz = lhs.count_min_trailing_zeros() as i64 - rhs.count_max_trailing_zeros() as i64;
        let max_tz = lhs.count_max_trailing_zeros() as i64 - rhs.count_min_trailing_zeros() as i64;
        if min_tz >= 0 {
            known.zero |= mask::<W>(min_tz as u32);
            if min_tz == max_tz {
                known.one |= W::ONE << (min_tz as u32);
            }
        } else if max_tz < 0 {
            known.set_all_zero();
        }
        if known.has_conflict() {
            known.set_all_zero();
        }
        known
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1321>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1334>
    pub fn urem(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        if lhs.max_value() < rhs.min_value() {
            return *lhs;
        }
        let mut known = Self::rem_get_low_bits(lhs, rhs);
        let max_res = min(lhs.max_value(), rhs.max_value().wrapping_sub(W::ONE) & mask(lhs.width));
        known.zero = set_high(known.zero, lhs.width, countl_zero(max_res, lhs.width));
        known
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1353>
    pub fn srem(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        let mut known = Self::rem_get_low_bits(lhs, rhs);
        if rhs.is_constant() && is_power_of_two(rhs.constant_value()) {
            let low_bits = rhs.constant_value().wrapping_sub(W::ONE);
            if lhs.is_non_negative() || low_bits & !lhs.zero == W::ZERO {
                known.zero |= !low_bits & mask(width);
            }
            if lhs.is_negative() && low_bits & lhs.one != W::ZERO {
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L309>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L327>
    pub fn umin(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let flip = |v: &Self| Self { zero: v.one, one: v.zero, width: v.width };
        flip(&Self::umax(&flip(lhs), &flip(rhs)))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L333>
    pub fn smax(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::flip_sign_bit(&Self::umax(&Self::flip_sign_bit(lhs), &Self::flip_sign_bit(rhs)))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L337>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L350>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L369>
    pub fn abds(
        mut lhs: Self,
        mut rhs: Self,
    ) -> Self {
        if !slt(lhs.signed_min_value(), rhs.signed_max_value(), lhs.width) {
            return Self::compute_for_add_sub(false, false, false, &lhs, &rhs);
        }
        if !slt(rhs.signed_min_value(), lhs.signed_max_value(), lhs.width) {
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L400>
    fn max_shift_amount(
        max_value: W,
        value_width: u32,
        width: u32,
    ) -> u32 {
        if width.is_power_of_two() {
            let bits = min(width.ilog2(), value_width);
            (max_value & mask::<W>(bits)).low_u128() as u32
        } else {
            min(max_value, W::from_u32(width - 1)).low_u128() as u32
        }
    }

    /// The amounts of `rhs` a loop over `min..=max` may take.
    fn shift_amount_possible(
        rhs: &Self,
        amount: u32,
    ) -> bool {
        let zero_mask = rhs.zero.low_u128() as u32;
        let one_mask = rhs.one.low_u128() as u32;
        zero_mask & amount == 0 && (one_mask | amount) == amount
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L410>
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
        let mut min_shift = min(rhs.min_value(), W::from_u32(width)).low_u128() as u32;
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
                known.one |= sign_bit::<W>(width);
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L494>
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
        let mut min_shift = min(rhs.min_value(), W::from_u32(width)).low_u128() as u32;
        if min_shift == 0 && shamt_non_zero {
            min_shift = 1;
        }
        if lhs.is_unknown() {
            known.zero = set_high(W::ZERO, width, min_shift);
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L550>
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
        let mut min_shift = min(rhs.min_value(), W::from_u32(width)).low_u128() as u32;
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L621>
    pub fn clmul(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        let mut res = Self::constant(width, clmul(lhs.min_value(), rhs.min_value(), width));
        let clmul_or = |a: W, b: W| {
            let mut result = W::ZERO;
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L648>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L682>
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
                let range = set_range(W::ZERO, m0, m1 + 1) & mask::<W>(width);
                if range & !val.one != W::ZERO {
                    res.one = with_bit(res.one, i, false);
                }
                if range & !val.zero != W::ZERO {
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L716>
    pub fn eq(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        if lhs.is_constant() && rhs.is_constant() {
            return Some(lhs.constant_value() == rhs.constant_value());
        }
        if lhs.one & rhs.zero != W::ZERO || rhs.one & lhs.zero != W::ZERO {
            return Some(false);
        }
        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L724>
    pub fn ne(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::eq(lhs, rhs).map(|equal| !equal)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L730>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L740>
    pub fn uge(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::ugt(rhs, lhs).map(|greater| !greater)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L746>
    pub fn ult(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::ugt(rhs, lhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L750>
    pub fn ule(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::uge(rhs, lhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L754>
    pub fn sgt(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        let w = lhs.width;
        if !sgt(lhs.signed_max_value(), rhs.signed_min_value(), w) {
            return Some(false);
        }
        if sgt(lhs.signed_min_value(), rhs.signed_max_value(), w) {
            return Some(true);
        }
        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L764>
    pub fn sge(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::sgt(rhs, lhs).map(|greater| !greater)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L770>
    pub fn slt(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::sgt(rhs, lhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L774>
    pub fn sle(
        lhs: &Self,
        rhs: &Self,
    ) -> Option<bool> {
        Self::sge(rhs, lhs)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1382>
    pub fn and(
        &self,
        rhs: &Self,
    ) -> Self {
        Self { zero: self.zero | rhs.zero, one: self.one & rhs.one, width: self.width }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1390>
    pub fn or(
        &self,
        rhs: &Self,
    ) -> Self {
        Self { zero: self.zero & rhs.zero, one: self.one | rhs.one, width: self.width }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1398>
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L538>
    pub fn shl_by(
        &self,
        amount: u32,
    ) -> Self {
        Self::raw(self.width, shl_or_zero(self.zero, amount), shl_or_zero(self.one, amount))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L545>
    pub fn lshr_by(
        &self,
        amount: u32,
    ) -> Self {
        Self::raw(self.width, lshr(self.zero, amount), lshr(self.one, amount))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L778>
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
                tmp.one |= W::ONE << self.count_min_trailing_zeros();
            }
            abs = Self::compute_for_add_sub(false, int_min_is_poison, false, &Self::constant(width, W::ZERO), &tmp);
            if int_min_is_poison && tmp.count_min_population() == 1 && tmp.count_max_population() != 1 {
                tmp.one &= !sign_bit::<W>(width);
                tmp.zero |= sign_bit::<W>(width);
                abs.one = set_range(abs.one, width - tmp.count_min_leading_zeros(), width - 1);
            }
        } else {
            let max_tz = self.count_max_trailing_zeros();
            let min_tz = self.count_min_trailing_zeros();
            abs.zero = mask(min_tz);
            if max_tz == min_tz && max_tz < width {
                abs.one |= W::ONE << max_tz;
            }
            if int_min_is_poison || (self.one != W::ZERO && self.one != sign_bit::<W>(width)) {
                abs.one &= !sign_bit::<W>(width);
                abs.zero |= sign_bit::<W>(width);
            }
        }
        abs
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L836>
    pub fn reduce_add(
        &self,
        num_elts: u32,
    ) -> Self {
        let width = self.width;
        if num_elts == 0 {
            return Self::unknown(width);
        }
        if self.is_constant() {
            return Self::constant(width, W::from_u32(num_elts).wrapping_mul(self.constant_value()) & mask(width));
        }
        let mut result = Self::unknown(width);
        let lost_bits = if num_elts <= 1 { 0 } else { 32 - (num_elts - 1).leading_zeros() };
        if self.is_non_negative() {
            let leading_zeros = self.count_min_leading_zeros();
            let leading_zeros = if leading_zeros > lost_bits { leading_zeros - lost_bits } else { 0 };
            result.zero = set_high(W::ZERO, width, leading_zeros);
        } else if self.is_negative() {
            let leading_ones = self.count_min_leading_ones();
            let leading_ones = if leading_ones > lost_bits { leading_ones - lost_bits } else { 0 };
            result.one = set_high(W::ZERO, width, leading_ones);
        }
        result
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1407>
    pub fn blsi(&self) -> Self {
        let width = self.width;
        let mut known = Self { zero: self.zero, one: W::ZERO, width };
        let max = self.count_max_trailing_zeros();
        known.zero = set_range(known.zero, min(max + 1, width), width);
        let min_tz = self.count_min_trailing_zeros();
        if max == min_tz && max < width {
            known.one |= W::ONE << max;
        }
        known
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1418>
    pub fn blsmsk(&self) -> Self {
        let width = self.width;
        let mut known = Self::unknown(width);
        let max = self.count_max_trailing_zeros();
        known.zero = set_range(known.zero, min(max + 1, width), width);
        let min_tz = self.count_min_trailing_zeros();
        known.one = mask(min(min_tz + 1, width));
        known
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1428>
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

impl Bits<u128> {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1227>
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
        let sdiv = |num: u128, denom: u128| {
            (signed(num, width).wrapping_div(signed(denom, width)) as u128) & mask::<u128>(width)
        };
        let mut res: Option<u128> = None;
        if lhs.is_negative() && rhs.is_negative() {
            let denom = rhs.signed_max_value();
            let num = lhs.signed_min_value();
            res = Some(if num == sign_bit::<u128>(width) && denom == mask::<u128>(width) {
                mask::<u128>(width) >> 1
            } else {
                sdiv(num, denom)
            });
        } else if lhs.is_negative() && rhs.is_non_negative() {
            let neg_max = lhs.signed_max_value().wrapping_neg() & mask::<u128>(width);
            if exact || neg_max >= rhs.signed_max_value() {
                let denom = rhs.signed_min_value();
                let num = lhs.signed_min_value();
                res = Some(if denom == 0 { num } else { sdiv(num, denom) });
            }
        } else if lhs.is_strictly_positive() && rhs.is_negative() {
            let neg_min = rhs.signed_min_value().wrapping_neg() & mask::<u128>(width);
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

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1283>
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
            known.zero &= !mask::<u128>(different + 1);
            known.one &= !mask::<u128>(different + 1);
        }
        Self::div_compute_low_bit(known, lhs, rhs, exact)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L609>
    pub fn fshl(
        lhs: &Self,
        rhs: &Self,
        amt: u128,
    ) -> Self {
        let w = lhs.width;
        Self::raw(w, fshl(lhs.zero, rhs.zero, amt, w), fshl(lhs.one, rhs.one, amt, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L615>
    pub fn fshr(
        lhs: &Self,
        rhs: &Self,
        amt: u128,
    ) -> Self {
        let w = lhs.width;
        Self::raw(w, fshr(lhs.zero, rhs.zero, amt, w), fshr(lhs.one, rhs.one, amt, w))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L559>
    pub fn byte_swap(&self) -> Self {
        let swap = |v: u128| v.swap_bytes() >> (128 - self.width);
        Self::raw(self.width, swap(self.zero), swap(self.one))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/KnownBits.h#L563>
    pub fn reverse_bits(&self) -> Self {
        let reverse = |v: u128| v.reverse_bits() >> (128 - self.width);
        Self::raw(self.width, reverse(self.zero), reverse(self.one))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1175>
    pub fn mulhs(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        assert_eq!(width, rhs.width, "Operand mismatch");
        let wide_lhs = lhs.to_word::<U256>().sext(2 * width);
        let wide_rhs = rhs.to_word::<U256>().sext(2 * width);
        Bits::<U256>::mul(&wide_lhs, &wide_rhs, false).extract_bits(width, width).to_word()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1183>
    pub fn mulhu(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        let width = lhs.width;
        assert_eq!(width, rhs.width, "Operand mismatch");
        let wide_lhs = lhs.to_word::<U256>().zext(2 * width);
        let wide_rhs = rhs.to_word::<U256>().zext(2 * width);
        Bits::<U256>::mul(&wide_lhs, &wide_rhs, false).extract_bits(width, width).to_word()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1046>
    fn avg_compute_u(
        lhs: &Self,
        rhs: &Self,
        ceil: bool,
    ) -> Self {
        let width = lhs.width;
        let (wide_lhs, wide_rhs) = (lhs.to_word::<U256>().zext(width + 1), rhs.to_word::<U256>().zext(width + 1));
        Bits::<U256>::compute_for_add_carry_bits(&wide_lhs, &wide_rhs, !ceil, ceil).extract_bits(width, 1).to_word()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1056>
    pub fn avg_floor_s(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::flip_sign_bit(&Self::avg_floor_u(&Self::flip_sign_bit(lhs), &Self::flip_sign_bit(rhs)))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1060>
    pub fn avg_floor_u(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::avg_compute_u(lhs, rhs, false)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1064>
    pub fn avg_ceil_s(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::flip_sign_bit(&Self::avg_ceil_u(&Self::flip_sign_bit(lhs), &Self::flip_sign_bit(rhs)))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1068>
    pub fn avg_ceil_u(
        lhs: &Self,
        rhs: &Self,
    ) -> Self {
        Self::avg_compute_u(lhs, rhs, true)
    }
}

fn shl_or_zero<W: Word>(
    v: W,
    by: u32,
) -> W {
    if by >= W::BITS { W::ZERO } else { v << by }
}

/// `SignBitsOps::rot`: the sign bits left in a rotated value.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/KnownBits.cpp#L1450>
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
