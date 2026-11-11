//! `APInt` for ScalarEvolution: an integer of 1 to 256 bits, the low 128 in
//! `bits` and the rest in `hi`.
//!
//! Only what the SCEV translation reads. MIR integers stop at 128 bits, but
//! `willNotOverflow` widens an `i128` operation to `i256`
//! ([ScalarEvolution.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2299)),
//! so a value of up to 256 bits has to be held. A value of 128 bits or fewer
//! keeps `hi` at zero and every operation on it is the `u128` one.

use llrm_mir::apint::{self, U256, Word};
use llrm_mir::context::{mask, signed};

/// An `APInt` of `width` bits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ap {
    pub width: u32,
    /// The low 128 bits.
    pub bits: u128,
    /// Bits 128 to 255; zero when `width` is at most 128.
    pub hi: u128,
}

impl Ap {
    /// `APInt(width, value)`: `value` modulo `2^width`.
    pub fn new(
        width: u32,
        value: u128,
    ) -> Self {
        debug_assert!((1..=256).contains(&width), "width {width}");
        Self { width, bits: value & mask(width), hi: 0 }
    }

    /// The value as a 256-bit word.
    pub fn word(&self) -> U256 {
        U256::from_limbs([self.bits, self.hi, 0, 0])
    }

    /// `word` modulo `2^width`.
    pub fn from_word(
        width: u32,
        word: U256,
    ) -> Self {
        let limbs = (word & apint::mask::<U256>(width)).to_limbs();
        Self { width, bits: limbs[0], hi: limbs[1] }
    }

    /// `APInt(width, value, isSigned)` of a signed `value`.
    pub fn from_signed(
        width: u32,
        value: i128,
    ) -> Self {
        if width > 128 {
            return Self::from_word(width, U256::from_limbs([value as u128, if value < 0 { u128::MAX } else { 0 }, 0, 0]));
        }
        Self::new(width, value as u128)
    }

    pub fn zero(width: u32) -> Self {
        Self::new(width, 0)
    }

    pub fn one(width: u32) -> Self {
        Self::new(width, 1)
    }

    pub fn all_ones(width: u32) -> Self {
        if width > 128 {
            return Self::from_word(width, !U256::ZERO);
        }
        Self::new(width, u128::MAX)
    }

    /// `APInt::getSignedMaxValue`.
    pub fn signed_max(width: u32) -> Self {
        if width > 128 {
            return Self::from_word(width, apint::mask::<U256>(width) >> 1);
        }
        Self::new(width, mask(width) >> 1)
    }

    /// `APInt::getSignedMinValue`.
    pub fn signed_min(width: u32) -> Self {
        if width > 128 {
            return Self::from_word(width, U256::ONE << (width - 1));
        }
        Self::new(width, 1u128 << (width - 1))
    }

    pub fn is_zero(&self) -> bool {
        self.bits == 0 && self.hi == 0
    }

    pub fn is_one(&self) -> bool {
        self.bits == 1 && self.hi == 0
    }

    pub fn is_all_ones(&self) -> bool {
        if self.width > 128 {
            return self.word() == apint::mask::<U256>(self.width);
        }
        self.bits == mask(self.width)
    }

    /// `isNegative`: the sign bit.
    pub fn is_negative(&self) -> bool {
        if self.width > 128 {
            return apint::is_negative(self.word(), self.width);
        }
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
        if self.width > 128 {
            return self.word() == U256::ONE << (self.width - 1);
        }
        self.bits == 1u128 << (self.width - 1)
    }

    /// `getSExtValue`.
    pub fn sext_value(&self) -> i128 {
        debug_assert!(self.width <= 128, "a value of {} bits is not an i128", self.width);
        signed(self.bits, self.width)
    }

    /// `getZExtValue`.
    pub fn zext_value(&self) -> u128 {
        debug_assert!(self.hi == 0, "a value of {} bits is not a u128", self.width);
        self.bits
    }

    /// `isPowerOf2`.
    pub fn is_power_of_2(&self) -> bool {
        self.bits.count_ones() + self.hi.count_ones() == 1
    }

    /// `countr_zero`.
    pub fn countr_zero(&self) -> u32 {
        if self.width > 128 {
            return apint::countr_zero(self.word(), self.width);
        }
        if self.bits == 0 { self.width } else { self.bits.trailing_zeros() }
    }

    /// `countl_zero`.
    pub fn countl_zero(&self) -> u32 {
        if self.width > 128 {
            return apint::countl_zero(self.word(), self.width);
        }
        self.bits.leading_zeros() - (128 - self.width)
    }

    /// `getActiveBits`.
    pub fn active_bits(&self) -> u32 {
        self.width - self.countl_zero()
    }

    /// `getSignificantBits`: the width of the smallest signed type holding it.
    pub fn significant_bits(&self) -> u32 {
        if self.width > 128 {
            return apint::significant_bits(self.word(), self.width);
        }
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
        if self.width > 128 {
            return Self::from_word(self.width, self.word().wrapping_add(other.word()));
        }
        Self::new(self.width, self.bits.wrapping_add(other.bits))
    }

    pub fn sub(
        &self,
        other: Self,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, self.word().wrapping_sub(other.word()));
        }
        Self::new(self.width, self.bits.wrapping_sub(other.bits))
    }

    pub fn mul(
        &self,
        other: Self,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, self.word().wrapping_mul(other.word()));
        }
        Self::new(self.width, self.bits.wrapping_mul(other.bits))
    }

    pub fn neg(&self) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, U256::ZERO.wrapping_sub(self.word()));
        }
        Self::new(self.width, self.bits.wrapping_neg())
    }

    /// `operator~`.
    pub fn not(&self) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, !self.word());
        }
        Self::new(self.width, !self.bits)
    }

    pub fn and(
        &self,
        other: Self,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, self.word() & other.word());
        }
        Self::new(self.width, self.bits & other.bits)
    }

    pub fn udiv(
        &self,
        other: Self,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, apint::udiv(self.word(), other.word()));
        }
        Self::new(self.width, self.bits / other.bits)
    }

    pub fn urem(
        &self,
        other: Self,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, apint::urem(self.word(), other.word()));
        }
        Self::new(self.width, self.bits % other.bits)
    }

    pub fn sdiv(
        &self,
        other: Self,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, apint::sdiv(self.word(), other.word(), self.width));
        }
        Self::from_signed(self.width, self.sext_value().wrapping_div(other.sext_value()))
    }

    pub fn srem(
        &self,
        other: Self,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, apint::srem(self.word(), other.word(), self.width));
        }
        Self::from_signed(self.width, self.sext_value().wrapping_rem(other.sext_value()))
    }

    pub fn shl(
        &self,
        by: u32,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, apint::shl(self.word(), self.width, by));
        }
        Self::new(self.width, if by >= 128 { 0 } else { self.bits << by })
    }

    pub fn lshr(
        &self,
        by: u32,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, apint::lshr(self.word(), by));
        }
        Self::new(self.width, if by >= 128 { 0 } else { self.bits >> by })
    }

    /// `ashr`.
    pub fn ashr(
        &self,
        by: u32,
    ) -> Self {
        if self.width > 128 {
            return Self::from_word(self.width, apint::ashr(self.word(), self.width, by));
        }
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
        if width > 128 {
            return Self::from_word(width, self.word());
        }
        Self::new(width, self.bits)
    }

    /// `zext`.
    pub fn zext(
        &self,
        width: u32,
    ) -> Self {
        debug_assert!(width >= self.width);
        if width > 128 {
            return Self::from_word(width, self.word());
        }
        Self::new(width, self.bits)
    }

    /// `sext`.
    pub fn sext(
        &self,
        width: u32,
    ) -> Self {
        debug_assert!(width >= self.width);
        if width > 128 {
            return Self::from_word(width, apint::sext(self.word(), self.width, width));
        }
        Self::from_signed(width, self.sext_value())
    }

    pub fn ult(
        &self,
        other: Self,
    ) -> bool {
        if self.width > 128 {
            return self.word() < other.word();
        }
        self.bits < other.bits
    }

    pub fn ule(
        &self,
        other: Self,
    ) -> bool {
        if self.width > 128 {
            return self.word() <= other.word();
        }
        self.bits <= other.bits
    }

    pub fn slt(
        &self,
        other: Self,
    ) -> bool {
        if self.width > 128 {
            return apint::slt(self.word(), other.word(), self.width);
        }
        self.sext_value() < other.sext_value()
    }

    pub fn sle(
        &self,
        other: Self,
    ) -> bool {
        if self.width > 128 {
            return !apint::slt(other.word(), self.word(), self.width);
        }
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
        if self.width > 128 {
            let (value, over) = { let (product, carry) = self.word().overflowing_mul(other.word()); (product, carry || product > apint::mask::<U256>(self.width)) };
            return (Self::from_word(self.width, value), over);
        }
        let (product, over) = self.bits.overflowing_mul(other.bits);
        (Self::new(self.width, product), over || product > mask(self.width))
    }

    /// `smul_ov`.
    pub fn smul_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        if self.width > 128 {
            let (value, over) = apint::smul_ov(self.word(), other.word(), self.width);
            return (Self::from_word(self.width, value), over);
        }
        let (product, over) = self.sext_value().overflowing_mul(other.sext_value());
        let result = Self::from_signed(self.width, product);
        (result, over || result.sext_value() != product)
    }

    /// `uadd_ov`.
    pub fn uadd_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        if self.width > 128 {
            let (value, over) = { let (sum, carry) = self.word().overflowing_add(other.word()); (sum, carry || sum > apint::mask::<U256>(self.width)) };
            return (Self::from_word(self.width, value), over);
        }
        let (sum, over) = self.bits.overflowing_add(other.bits);
        (Self::new(self.width, sum), over || sum > mask(self.width))
    }

    /// `sadd_ov`.
    pub fn sadd_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        if self.width > 128 {
            let (value, over) = apint::sadd_ov(self.word(), other.word(), self.width);
            return (Self::from_word(self.width, value), over);
        }
        let (sum, over) = self.sext_value().overflowing_add(other.sext_value());
        let result = Self::from_signed(self.width, sum);
        (result, over || result.sext_value() != sum)
    }

    /// `ssub_ov`.
    pub fn ssub_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        if self.width > 128 {
            let (value, over) = apint::ssub_ov(self.word(), other.word(), self.width);
            return (Self::from_word(self.width, value), over);
        }
        let (difference, over) = self.sext_value().overflowing_sub(other.sext_value());
        let result = Self::from_signed(self.width, difference);
        (result, over || result.sext_value() != difference)
    }

    /// `usub_ov`.
    pub fn usub_ov(
        &self,
        other: Self,
    ) -> (Self, bool) {
        if self.width > 128 {
            let (value, over) = (self.word().wrapping_sub(other.word()), self.word() < other.word());
            return (Self::from_word(self.width, value), over);
        }
        (self.sub(other), self.bits < other.bits)
    }

    /// `ConstantInt::print`: signed decimal, `true` and `false` for `i1`.
    pub fn display(&self) -> String {
        if self.width > 128 {
            return format!("{:?}", self.word());
        }
        if self.width == 1 {
            return if self.bits == 1 { "true" } else { "false" }.to_owned();
        }
        self.sext_value().to_string()
    }
}

#[cfg(test)]
#[path = "ap_tests.rs"]
mod tests;
