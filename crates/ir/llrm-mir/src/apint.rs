//! The `APInt` operations the ValueTracking translations need, on a machine
//! word and a bit width: `v` holds the low `width` bits and no others.
//!
//! # How it works
//! [`Word`] is the integer the bits sit in: `u128` for MIR's own integers
//! (`i1` to `i128`), [`U256`] for the intermediates that LLVM computes at twice
//! the width (`ConstantRange::multiply`, `KnownBits::mulhs`). Everything here
//! that needs no division is generic over it. The signed division and
//! remainder, the rounding divisions and the square root are `u128` only: no
//! wide intermediate divides.
//!
//! # Example
//! `countl_one(0b1110_u128, 4)` is 3; `signed(0b1110, 4)` is -2.

use std::cmp::min;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, Shr};

/// An unsigned machine integer the `APInt` operations run on.
pub trait Word:
    Copy
    + Debug
    + Eq
    + Ord
    + Hash
    + Not<Output = Self>
    + BitAnd<Output = Self>
    + BitOr<Output = Self>
    + BitXor<Output = Self>
    + BitAndAssign
    + BitOrAssign
    + BitXorAssign
    + Shl<u32, Output = Self>
    + Shr<u32, Output = Self>
{
    const ZERO: Self;
    const ONE: Self;
    const BITS: u32;
    /// The word with `value` in its low bits.
    fn from_u128(value: u128) -> Self;
    /// The low 128 bits.
    fn low_u128(self) -> u128;

    fn from_u32(value: u32) -> Self {
        Self::from_u128(u128::from(value))
    }

    fn wrapping_add(
        self,
        other: Self,
    ) -> Self;
    fn wrapping_sub(
        self,
        other: Self,
    ) -> Self;
    fn overflowing_add(
        self,
        other: Self,
    ) -> (Self, bool);
    fn overflowing_mul(
        self,
        other: Self,
    ) -> (Self, bool);
    fn leading_zeros(self) -> u32;
    fn trailing_zeros(self) -> u32;
    fn count_ones(self) -> u32;

    fn wrapping_mul(
        self,
        other: Self,
    ) -> Self {
        self.overflowing_mul(other).0
    }

    fn is_zero(self) -> bool {
        self == Self::ZERO
    }
}

impl Word for u128 {
    const ZERO: Self = 0;
    const ONE: Self = 1;
    const BITS: u32 = 128;

    fn from_u128(value: u128) -> Self {
        value
    }

    fn low_u128(self) -> u128 {
        self
    }

    fn wrapping_add(
        self,
        other: Self,
    ) -> Self {
        u128::wrapping_add(self, other)
    }

    fn wrapping_sub(
        self,
        other: Self,
    ) -> Self {
        u128::wrapping_sub(self, other)
    }

    fn overflowing_add(
        self,
        other: Self,
    ) -> (Self, bool) {
        u128::overflowing_add(self, other)
    }

    fn overflowing_mul(
        self,
        other: Self,
    ) -> (Self, bool) {
        u128::overflowing_mul(self, other)
    }

    fn leading_zeros(self) -> u32 {
        u128::leading_zeros(self)
    }

    fn trailing_zeros(self) -> u32 {
        u128::trailing_zeros(self)
    }

    fn count_ones(self) -> u32 {
        u128::count_ones(self)
    }
}

/// 256 bits: the room for a product of two 128-bit integers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct U256 {
    hi: u128,
    lo: u128,
}

impl Ord for U256 {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        (self.hi, self.lo).cmp(&(other.hi, other.lo))
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Not for U256 {
    type Output = Self;

    fn not(self) -> Self {
        Self { hi: !self.hi, lo: !self.lo }
    }
}

impl BitAnd for U256 {
    type Output = Self;

    fn bitand(
        self,
        other: Self,
    ) -> Self {
        Self { hi: self.hi & other.hi, lo: self.lo & other.lo }
    }
}

impl BitOr for U256 {
    type Output = Self;

    fn bitor(
        self,
        other: Self,
    ) -> Self {
        Self { hi: self.hi | other.hi, lo: self.lo | other.lo }
    }
}

impl BitXor for U256 {
    type Output = Self;

    fn bitxor(
        self,
        other: Self,
    ) -> Self {
        Self { hi: self.hi ^ other.hi, lo: self.lo ^ other.lo }
    }
}

impl BitAndAssign for U256 {
    fn bitand_assign(
        &mut self,
        other: Self,
    ) {
        *self = *self & other;
    }
}

impl BitOrAssign for U256 {
    fn bitor_assign(
        &mut self,
        other: Self,
    ) {
        *self = *self | other;
    }
}

impl BitXorAssign for U256 {
    fn bitxor_assign(
        &mut self,
        other: Self,
    ) {
        *self = *self ^ other;
    }
}

impl Shl<u32> for U256 {
    type Output = Self;

    fn shl(
        self,
        by: u32,
    ) -> Self {
        match by {
            0 => self,
            1..=127 => Self { hi: self.hi << by | self.lo >> (128 - by), lo: self.lo << by },
            128..=255 => Self { hi: self.lo << (by - 128), lo: 0 },
            _ => Self::ZERO,
        }
    }
}

impl Shr<u32> for U256 {
    type Output = Self;

    fn shr(
        self,
        by: u32,
    ) -> Self {
        match by {
            0 => self,
            1..=127 => Self { hi: self.hi >> by, lo: self.lo >> by | self.hi << (128 - by) },
            128..=255 => Self { hi: 0, lo: self.hi >> (by - 128) },
            _ => Self::ZERO,
        }
    }
}

impl Word for U256 {
    const ZERO: Self = Self { hi: 0, lo: 0 };
    const ONE: Self = Self { hi: 0, lo: 1 };
    const BITS: u32 = 256;

    fn from_u128(value: u128) -> Self {
        Self { hi: 0, lo: value }
    }

    fn low_u128(self) -> u128 {
        self.lo
    }

    fn wrapping_add(
        self,
        other: Self,
    ) -> Self {
        self.overflowing_add(other).0
    }

    fn wrapping_sub(
        self,
        other: Self,
    ) -> Self {
        let (lo, borrow) = self.lo.overflowing_sub(other.lo);
        Self { hi: self.hi.wrapping_sub(other.hi).wrapping_sub(u128::from(borrow)), lo }
    }

    fn overflowing_add(
        self,
        other: Self,
    ) -> (Self, bool) {
        let (lo, carry) = self.lo.overflowing_add(other.lo);
        let (hi, over_a) = self.hi.overflowing_add(other.hi);
        let (hi, over_b) = hi.overflowing_add(u128::from(carry));
        (Self { hi, lo }, over_a || over_b)
    }

    fn overflowing_mul(
        self,
        other: Self,
    ) -> (Self, bool) {
        let limbs = |v: Self| [v.lo as u64, (v.lo >> 64) as u64, v.hi as u64, (v.hi >> 64) as u64];
        let (a, b) = (limbs(self), limbs(other));
        let mut product = [0_u64; 8];
        for (i, x) in a.iter().enumerate() {
            let mut carry = 0_u128;
            for (j, y) in b.iter().enumerate() {
                let t = u128::from(*x) * u128::from(*y) + u128::from(product[i + j]) + carry;
                product[i + j] = t as u64;
                carry = t >> 64;
            }
            product[i + 4] = carry as u64;
        }
        let word = |at: usize| u128::from(product[at]) | u128::from(product[at + 1]) << 64;
        (Self { hi: word(2), lo: word(0) }, product[4..].iter().any(|limb| *limb != 0))
    }

    fn leading_zeros(self) -> u32 {
        if self.hi != 0 { self.hi.leading_zeros() } else { 128 + self.lo.leading_zeros() }
    }

    fn trailing_zeros(self) -> u32 {
        if self.lo != 0 { self.lo.trailing_zeros() } else { 128 + self.hi.trailing_zeros() }
    }

    fn count_ones(self) -> u32 {
        self.hi.count_ones() + self.lo.count_ones()
    }
}

/// A power of two: one bit set.
pub(crate) fn is_power_of_two<W: Word>(v: W) -> bool {
    !v.is_zero() && (v & v.wrapping_sub(W::ONE)).is_zero()
}

/// The low `bits` bits set.
pub fn mask<W: Word>(bits: u32) -> W {
    if bits >= W::BITS { !W::ZERO } else { (W::ONE << bits).wrapping_sub(W::ONE) }
}

/// `v` with its top `count` of `width` bits set.
pub(crate) fn set_high<W: Word>(
    v: W,
    width: u32,
    count: u32,
) -> W {
    v | (mask::<W>(width) & !mask::<W>(width - min(count, width)))
}

/// `v` with bits `[low, high)` set.
pub(crate) fn set_range<W: Word>(
    v: W,
    low: u32,
    high: u32,
) -> W {
    if high <= low { v } else { v | (mask::<W>(high) & !mask::<W>(low)) }
}

pub(crate) fn clear_range<W: Word>(
    v: W,
    low: u32,
    high: u32,
) -> W {
    if high <= low { v } else { v & !(mask::<W>(high) & !mask::<W>(low)) }
}

pub(crate) fn bit<W: Word>(
    v: W,
    at: u32,
) -> bool {
    at < W::BITS && (v >> at) & W::ONE == W::ONE
}

pub(crate) fn with_bit<W: Word>(
    v: W,
    at: u32,
    on: bool,
) -> W {
    if on { v | (W::ONE << at) } else { v & !(W::ONE << at) }
}

pub(crate) fn countl_zero<W: Word>(
    v: W,
    width: u32,
) -> u32 {
    if v.is_zero() { width } else { v.leading_zeros() - (W::BITS - width) }
}

pub(crate) fn countl_one<W: Word>(
    v: W,
    width: u32,
) -> u32 {
    countl_zero(!v & mask::<W>(width), width)
}

pub(crate) fn countr_zero<W: Word>(
    v: W,
    width: u32,
) -> u32 {
    if v.is_zero() { width } else { v.trailing_zeros() }
}

pub(crate) fn countr_one<W: Word>(
    v: W,
    width: u32,
) -> u32 {
    countr_zero(!v & mask::<W>(width), width)
}

pub(crate) fn popcount<W: Word>(v: W) -> u32 {
    v.count_ones()
}

pub(crate) fn sign_bit<W: Word>(width: u32) -> W {
    W::ONE << (width - 1)
}

/// `a` as a signed number is below `b`.
pub(crate) fn slt<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> bool {
    (a ^ sign_bit::<W>(width)) < (b ^ sign_bit::<W>(width))
}

pub(crate) fn sgt<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> bool {
    slt(b, a, width)
}

/// Whether `v` is negative at `width`.
pub(crate) fn is_negative<W: Word>(
    v: W,
    width: u32,
) -> bool {
    bit(v, width - 1)
}

/// `v` extended from `from` bits to `to`, copying the sign.
pub(crate) fn sext<W: Word>(
    v: W,
    from: u32,
    to: u32,
) -> W {
    if is_negative(v, from) { (v | !mask::<W>(from)) & mask::<W>(to) } else { v }
}

pub(crate) fn ashr<W: Word>(
    v: W,
    width: u32,
    by: u32,
) -> W {
    let by = min(by, width - 1);
    let shifted = v >> by;
    if is_negative(v, width) { shifted | (mask::<W>(width) & !mask::<W>(width - by)) } else { shifted }
}

pub(crate) fn lshr<W: Word>(
    v: W,
    by: u32,
) -> W {
    if by >= W::BITS { W::ZERO } else { v >> by }
}

/// `APInt::shl(unsigned)`.
pub(crate) fn shl<W: Word>(
    v: W,
    width: u32,
    by: u32,
) -> W {
    if by >= width { W::ZERO } else { (v << by) & mask::<W>(width) }
}

/// `APInt::ushl_ov`: the shift, and whether a set bit left the width.
pub(crate) fn ushl_ov<W: Word>(
    v: W,
    width: u32,
    by: u32,
) -> (W, bool) {
    if by >= width {
        return (W::ZERO, true);
    }
    ((v << by) & mask::<W>(width), by > countl_zero(v, width))
}

pub(crate) fn uadd_ov<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> (W, bool) {
    let (sum, over) = a.overflowing_add(b);
    if width >= W::BITS { (sum, over) } else { (sum & mask::<W>(width), over || sum > mask::<W>(width)) }
}

pub(crate) fn usub_ov<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> (W, bool) {
    (a.wrapping_sub(b) & mask::<W>(width), a < b)
}

pub(crate) fn umul_ov<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> (W, bool) {
    let (product, over) = a.overflowing_mul(b);
    if over || product > mask::<W>(width) { (product & mask::<W>(width), true) } else { (product, false) }
}

pub(crate) fn uadd_sat<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    let (sum, over) = uadd_ov(a, b, width);
    if over { mask::<W>(width) } else { sum }
}

pub(crate) fn usub_sat<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    let (diff, over) = usub_ov(a, b, width);
    if over { W::ZERO } else { diff }
}

/// `APInt::umul_sat`.
pub(crate) fn umul_sat<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    let (product, over) = umul_ov(a, b, width);
    if over { mask::<W>(width) } else { product }
}

/// `APInt::sadd_sat`.
pub(crate) fn sadd_sat<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    let sum = a.wrapping_add(b) & mask::<W>(width);
    if is_negative(a, width) == is_negative(b, width) && is_negative(sum, width) != is_negative(a, width) {
        return if is_negative(a, width) { sign_bit::<W>(width) } else { mask::<W>(width) >> 1 };
    }
    sum
}

/// `APInt::ssub_sat`.
pub(crate) fn ssub_sat<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    let diff = a.wrapping_sub(b) & mask::<W>(width);
    if is_negative(a, width) != is_negative(b, width) && is_negative(diff, width) != is_negative(a, width) {
        return if is_negative(a, width) { sign_bit::<W>(width) } else { mask::<W>(width) >> 1 };
    }
    diff
}

pub(crate) fn clmul<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    (0..width).filter(|i| bit(b, *i)).fold(W::ZERO, |acc, i| acc ^ (a << i)) & mask::<W>(width)
}

/// `APInt::getActiveBits`: the bits up to the highest set one.
pub(crate) fn active_bits<W: Word>(v: W) -> u32 {
    W::BITS - v.leading_zeros()
}

/// `APInt::getSignificantBits`: the bits of the narrowest signed integer
/// holding `v`.
pub(crate) fn significant_bits<W: Word>(
    v: W,
    width: u32,
) -> u32 {
    let sign_bits = if bit(v, width - 1) { countl_one(v, width) } else { countl_zero(v, width) };
    width - sign_bits + 1
}

/// `APInt::sshl_ov`: the shift, and whether it changed the sign or lost a bit.
pub(crate) fn sshl_ov<W: Word>(
    v: W,
    width: u32,
    by: u32,
) -> (W, bool) {
    if by >= width {
        return (W::ZERO, true);
    }
    let over = if bit(v, width - 1) { by >= countl_one(v, width) } else { by >= countl_zero(v, width) };
    (shl(v, width, by), over)
}

/// `APInt::ushl_sat`.
pub(crate) fn ushl_sat<W: Word>(
    v: W,
    width: u32,
    by: u32,
) -> W {
    let (shifted, over) = ushl_ov(v, width, by);
    if over { mask::<W>(width) } else { shifted }
}

/// `APInt::sshl_sat`.
pub(crate) fn sshl_sat<W: Word>(
    v: W,
    width: u32,
    by: u32,
) -> W {
    let (shifted, over) = sshl_ov(v, width, by);
    if !over {
        return shifted;
    }
    if bit(v, width - 1) { sign_bit::<W>(width) } else { mask::<W>(width) >> 1 }
}

/// `APInt::getLimitedValue`.
pub(crate) fn limited<W: Word>(
    v: W,
    limit: u32,
) -> u32 {
    if v > W::from_u128(u128::from(limit)) { limit } else { v.low_u128() as u32 }
}

// What follows is `u128` only: it divides, or needs a signed number wider than
// the word.

/// `v` as a signed number.
pub(crate) fn signed(
    v: u128,
    width: u32,
) -> i128 {
    if width >= 128 { v as i128 } else { ((v << (128 - width)) as i128) >> (128 - width) }
}

pub(crate) fn smin_of(width: u32) -> i128 {
    if width >= 128 { i128::MIN } else { -(1_i128 << (width - 1)) }
}

pub(crate) fn smax_of(width: u32) -> i128 {
    if width >= 128 { i128::MAX } else { (1_i128 << (width - 1)) - 1 }
}

/// `APInt::smul_ov`.
pub(crate) fn smul_ov(
    a: u128,
    b: u128,
    width: u32,
) -> (u128, bool) {
    let (x, y) = (signed(a, width), signed(b, width));
    match x.checked_mul(y) {
        Some(p) if (smin_of(width)..=smax_of(width)).contains(&p) => (p as u128 & mask::<u128>(width), false),
        _ => (x.wrapping_mul(y) as u128 & mask::<u128>(width), true),
    }
}

/// `APInt::smul_sat`.
pub(crate) fn smul_sat(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    let (product, over) = smul_ov(a, b, width);
    if !over {
        return product;
    }
    let negative = bit(a, width - 1) != bit(b, width - 1);
    (if negative { smin_of(width) } else { smax_of(width) }) as u128 & mask::<u128>(width)
}

/// `APInt::sdiv`: the quotient rounded towards zero (`INT_MIN / -1` wraps).
pub(crate) fn sdiv(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    signed(a, width).wrapping_div(signed(b, width)) as u128 & mask::<u128>(width)
}

/// `APInt::srem`: the remainder with the sign of the dividend.
pub(crate) fn srem(
    a: u128,
    b: u128,
    width: u32,
) -> u128 {
    signed(a, width).wrapping_rem(signed(b, width)) as u128 & mask::<u128>(width)
}

/// `APIntOps::RoundingUDiv`.
pub(crate) fn rounding_udiv(
    a: u128,
    b: u128,
    up: bool,
) -> u128 {
    let (quotient, rem) = (a / b, a % b);
    if up && rem != 0 { quotient + 1 } else { quotient }
}

/// `APIntOps::RoundingSDiv`, rounding up or down.
pub(crate) fn rounding_sdiv(
    a: u128,
    b: u128,
    width: u32,
    up: bool,
) -> u128 {
    let (x, y) = (signed(a, width), signed(b, width));
    let quotient = x.wrapping_div(y);
    let rem = x.wrapping_rem(y);
    let result = if rem == 0 {
        quotient
    } else if up {
        if (rem < 0) != (y < 0) { quotient } else { quotient + 1 }
    } else if (rem < 0) != (y < 0) {
        quotient - 1
    } else {
        quotient
    };
    result as u128 & mask::<u128>(width)
}

/// `APInt::sqrtFloor`.
pub(crate) fn sqrt_floor(v: u128) -> u128 {
    let (mut low, mut high) = (0_u128, 1_u128 << 64);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if mid.checked_mul(mid).is_some_and(|square| square <= v) { low = mid } else { high = mid }
    }
    low
}

/// `APIntOps::fshl`: the top `width` bits of `(a:b) << (amt mod width)`.
pub(crate) fn fshl(
    a: u128,
    b: u128,
    amt: u128,
    width: u32,
) -> u128 {
    let by = (amt % u128::from(width)) as u32;
    if by == 0 { a } else { ((a << by) | lshr(b, width - by)) & mask::<u128>(width) }
}

/// `APIntOps::fshr`: the low `width` bits of `(a:b) >> (amt mod width)`.
pub(crate) fn fshr(
    a: u128,
    b: u128,
    amt: u128,
    width: u32,
) -> u128 {
    let by = (amt % u128::from(width)) as u32;
    if by == 0 { b } else { ((a << (width - by)) | lshr(b, by)) & mask::<u128>(width) }
}

/// `amt urem width`.
pub(crate) fn rotate_amount(
    amt: u128,
    width: u32,
) -> u32 {
    (amt % u128::from(width)) as u32
}
