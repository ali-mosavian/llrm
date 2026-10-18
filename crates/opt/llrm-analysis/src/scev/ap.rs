//! `APInt` for ScalarEvolution: an integer of 1 to 128 bits, the bits in the
//! low `width` of a `u128`.
//!
//! Only what the SCEV translation reads; widths past 128 are not SCEVable
//! (DEVIATE 7 of the SCEV port plan: no MIR integer type in the
//! corpus is wider, and `ConstantRange` stops there too).

use llrm_mir::context::{mask, signed};

/// An `APInt` of `width` bits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ap {
    pub width: u32,
    pub bits: u128,
}

impl Ap {
    /// `APInt(width, value)`: `value` modulo `2^width`.
    pub fn new(
        width: u32,
        value: u128,
    ) -> Self {
        debug_assert!((1..=128).contains(&width), "width {width}");
        Self { width, bits: value & mask(width) }
    }

    /// `APInt(width, value, isSigned)` of a signed `value`.
    pub fn from_signed(
        width: u32,
        value: i128,
    ) -> Self {
        Self::new(width, value as u128)
    }

    pub fn zero(width: u32) -> Self {
        Self::new(width, 0)
    }

    pub fn one(width: u32) -> Self {
        Self::new(width, 1)
    }

    pub fn all_ones(width: u32) -> Self {
        Self::new(width, u128::MAX)
    }

    /// `APInt::getSignedMaxValue`.
    pub fn signed_max(width: u32) -> Self {
        Self::new(width, mask(width) >> 1)
    }

    /// `APInt::getSignedMinValue`.
    pub fn signed_min(width: u32) -> Self {
        Self::new(width, 1u128 << (width - 1))
    }

    pub fn is_zero(&self) -> bool {
        self.bits == 0
    }

    pub fn is_one(&self) -> bool {
        self.bits == 1
    }

    pub fn is_all_ones(&self) -> bool {
        self.bits == mask(self.width)
    }

    /// `isNegative`: the sign bit.
    pub fn is_negative(&self) -> bool {
        self.bits >> (self.width - 1) & 1 == 1
    }

    pub fn is_non_negative(&self) -> bool {
        !self.is_negative()
    }

    /// `isStrictlyPositive`.
    pub fn is_strictly_positive(&self) -> bool {
        self.is_non_negative() && !self.is_zero()
    }

    pub fn is_signed_min(&self) -> bool {
        self.bits == 1u128 << (self.width - 1)
    }

    /// `getSExtValue`.
    pub fn sext_value(&self) -> i128 {
        signed(self.bits, self.width)
    }

    /// `getZExtValue`.
    pub fn zext_value(&self) -> u128 {
        self.bits
    }

    /// `isPowerOf2`.
    pub fn is_power_of_2(&self) -> bool {
        self.bits.count_ones() == 1
    }

    /// `countr_zero`.
    pub fn countr_zero(&self) -> u32 {
        if self.bits == 0 { self.width } else { self.bits.trailing_zeros() }
    }

    /// `countl_zero`.
    pub fn countl_zero(&self) -> u32 {
        self.bits.leading_zeros() - (128 - self.width)
    }

    /// `getActiveBits`.
    pub fn active_bits(&self) -> u32 {
        self.width - self.countl_zero()
    }

    /// `getSignificantBits`: the width of the smallest signed type holding it.
    pub fn significant_bits(&self) -> u32 {
        let sign_run = if self.is_negative() {
            (!self.bits & mask(self.width)).leading_zeros()
        } else {
            self.bits.leading_zeros()
        };
        self.width - (sign_run - (128 - self.width)) + 1
    }

    pub fn add(
        &self,
        other: Self,
    ) -> Self {
        Self::new(self.width, self.bits.wrapping_add(other.bits))
    }

    pub fn sub(
        &self,
        other: Self,
    ) -> Self {
        Self::new(self.width, self.bits.wrapping_sub(other.bits))
    }

    pub fn mul(
        &self,
        other: Self,
    ) -> Self {
        Self::new(self.width, self.bits.wrapping_mul(other.bits))
    }

    pub fn neg(&self) -> Self {
        Self::new(self.width, self.bits.wrapping_neg())
    }

    /// `operator~`.
    pub fn not(&self) -> Self {
        Self::new(self.width, !self.bits)
    }

    pub fn and(
        &self,
        other: Self,
    ) -> Self {
        Self::new(self.width, self.bits & other.bits)
    }

    pub fn udiv(
        &self,
        other: Self,
    ) -> Self {
        Self::new(self.width, self.bits / other.bits)
    }

    pub fn urem(
        &self,
        other: Self,
    ) -> Self {
        Self::new(self.width, self.bits % other.bits)
    }

    pub fn sdiv(
        &self,
        other: Self,
    ) -> Self {
        Self::from_signed(self.width, self.sext_value().wrapping_div(other.sext_value()))
    }

    pub fn srem(
        &self,
        other: Self,
    ) -> Self {
        Self::from_signed(self.width, self.sext_value().wrapping_rem(other.sext_value()))
    }

    pub fn shl(
        &self,
        by: u32,
    ) -> Self {
        Self::new(self.width, if by >= 128 { 0 } else { self.bits << by })
    }

    pub fn lshr(
        &self,
        by: u32,
    ) -> Self {
        Self::new(self.width, if by >= 128 { 0 } else { self.bits >> by })
    }

    /// `ashr`.
    pub fn ashr(
        &self,
        by: u32,
    ) -> Self {
        Self::from_signed(
            self.width,
            if by >= 128 { if self.is_negative() { -1 } else { 0 } } else { self.sext_value() >> by },
        )
    }

    /// `trunc`.
    pub fn trunc(
        &self,
        width: u32,
    ) -> Self {
        debug_assert!(width <= self.width);
        Self::new(width, self.bits)
    }

    /// `zext`.
    pub fn zext(
        &self,
        width: u32,
    ) -> Self {
        debug_assert!(width >= self.width);
        Self::new(width, self.bits)
    }

    /// `sext`.
    pub fn sext(
        &self,
        width: u32,
    ) -> Self {
        debug_assert!(width >= self.width);
        Self::from_signed(width, self.sext_value())
    }

    pub fn ult(
        &self,
        other: Self,
    ) -> bool {
        self.bits < other.bits
    }

    pub fn ule(
        &self,
        other: Self,
    ) -> bool {
        self.bits <= other.bits
    }

    pub fn slt(
        &self,
        other: Self,
    ) -> bool {
        self.sext_value() < other.sext_value()
    }

    pub fn sle(
        &self,
        other: Self,
    ) -> bool {
        self.sext_value() <= other.sext_value()
    }

    /// `APIntOps::umax`.
    pub fn umax(
        self,
        other: Self,
    ) -> Self {
        if self.ult(other) { other } else { self }
    }

    /// `APIntOps::umin`.
    pub fn umin(
        self,
        other: Self,
    ) -> Self {
        if self.ult(other) { self } else { other }
    }

    /// `APIntOps::smax`.
    pub fn smax(
        self,
        other: Self,
    ) -> Self {
        if self.slt(other) { other } else { self }
    }

    /// `APIntOps::smin`.
    pub fn smin(
        self,
        other: Self,
    ) -> Self {
        if self.slt(other) { self } else { other }
    }

    /// `umul_ov`: the product and whether it overflowed unsigned.
    pub fn umul_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        let (product, over) = self.bits.overflowing_mul(other.bits);
        (Self::new(self.width, product), over || product > mask(self.width))
    }

    /// `smul_ov`.
    pub fn smul_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        let (product, over) = self.sext_value().overflowing_mul(other.sext_value());
        let result = Self::from_signed(self.width, product);
        (result, over || result.sext_value() != product)
    }

    /// `uadd_ov`.
    pub fn uadd_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        let (sum, over) = self.bits.overflowing_add(other.bits);
        (Self::new(self.width, sum), over || sum > mask(self.width))
    }

    /// `sadd_ov`.
    pub fn sadd_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        let (sum, over) = self.sext_value().overflowing_add(other.sext_value());
        let result = Self::from_signed(self.width, sum);
        (result, over || result.sext_value() != sum)
    }

    /// `ssub_ov`.
    pub fn ssub_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        let (difference, over) = self.sext_value().overflowing_sub(other.sext_value());
        let result = Self::from_signed(self.width, difference);
        (result, over || result.sext_value() != difference)
    }

    /// `usub_ov`.
    pub fn usub_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        (self.sub(other), self.bits < other.bits)
    }

    /// `ConstantInt::print`: signed decimal, `true` and `false` for `i1`.
    pub fn display(&self) -> String {
        if self.width == 1 {
            return if self.bits == 1 { "true" } else { "false" }.to_owned();
        }
        self.sext_value().to_string()
    }
}
