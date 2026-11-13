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

    /// The low `bits` bits set, `bits` below `BITS`: [`mask`] without a
    /// shift of the whole word.
    fn low_mask(bits: u32) -> Self {
        (Self::ONE << bits).wrapping_sub(Self::ONE)
    }

    /// Bit `at` alone, `at` below `BITS`.
    fn bit(at: u32) -> Self {
        Self::ONE << at
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
    /// The value in four 128-bit limbs, the lowest first.
    fn to_limbs(self) -> [u128; 4];
    /// The word with those limbs, the ones it has no room for dropped.
    fn from_limbs(limbs: [u128; 4]) -> Self;
    /// The quotient and the remainder; panics on a zero divisor.
    fn divrem(
        self,
        other: Self,
    ) -> (Self, Self);

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

    fn low_mask(bits: u32) -> Self {
        if bits == 0 { 0 } else { u128::MAX >> (128 - bits) }
    }

    fn bit(at: u32) -> Self {
        1 << at
    }

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

    fn divrem(
        self,
        other: Self,
    ) -> (Self, Self) {
        (self / other, self % other)
    }

    fn to_limbs(self) -> [u128; 4] {
        [self, 0, 0, 0]
    }

    fn from_limbs(limbs: [u128; 4]) -> Self {
        limbs[0]
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

#[cfg(test)]
thread_local! {
    /// The `U256` shifts this thread made, for tests that pin the cost of a
    /// construction.
    pub(crate) static SHIFTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Shl<u32> for U256 {
    type Output = Self;

    fn shl(
        self,
        by: u32,
    ) -> Self {
        #[cfg(test)]
        SHIFTS.with(|count| count.set(count.get() + 1));
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

/// `low_mask` and `bit` of every width, looked up: a width is asked for more
/// than once per range operation, and a shift of the whole word is 50 Ir.
const U256_LOW_MASKS: [U256; 256] = {
    let mut table = [U256 { hi: 0, lo: 0 }; 256];
    let mut bits = 1;
    while bits < 256 {
        table[bits] = if bits <= 128 {
            U256 { hi: 0, lo: u128::MAX >> (128 - bits) }
        } else {
            U256 { hi: u128::MAX >> (256 - bits), lo: u128::MAX }
        };
        bits += 1;
    }
    table
};

const U256_BITS: [U256; 256] = {
    let mut table = [U256 { hi: 0, lo: 0 }; 256];
    let mut at = 0;
    while at < 256 {
        table[at] = if at < 128 { U256 { hi: 0, lo: 1 << at } } else { U256 { hi: 1 << (at - 128), lo: 0 } };
        at += 1;
    }
    table
};

impl Word for U256 {
    const ZERO: Self = Self { hi: 0, lo: 0 };
    const ONE: Self = Self { hi: 0, lo: 1 };
    const BITS: u32 = 256;

    fn low_mask(bits: u32) -> Self {
        U256_LOW_MASKS[bits as usize]
    }

    fn bit(at: u32) -> Self {
        U256_BITS[at as usize]
    }

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

    /// Shift and subtract: 256 steps at most, and only for a value or divisor
    /// above 128 bits.
    fn divrem(
        self,
        other: Self,
    ) -> (Self, Self) {
        assert!(!other.is_zero(), "attempt to divide by zero");
        if self.hi == 0 && other.hi == 0 {
            return (Self { hi: 0, lo: self.lo / other.lo }, Self { hi: 0, lo: self.lo % other.lo });
        }
        if self < other {
            return (Self::ZERO, self);
        }
        let (mut quotient, mut remainder) = (Self::ZERO, Self::ZERO);
        for at in (0..256 - self.leading_zeros()).rev() {
            let carried = remainder.hi >> 127 == 1;
            remainder = remainder << 1 | (self >> at) & Self::ONE;
            if carried || remainder >= other {
                remainder = remainder.wrapping_sub(other);
                quotient |= Self::ONE << at;
            }
        }
        (quotient, remainder)
    }

    fn to_limbs(self) -> [u128; 4] {
        [self.lo, self.hi, 0, 0]
    }

    fn from_limbs(limbs: [u128; 4]) -> Self {
        Self { hi: limbs[1], lo: limbs[0] }
    }
}

/// 512 bits: the room for a product of two 256-bit integers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct U512 {
    limbs: [u128; 4],
}

impl U512 {
    fn halves(self) -> [u64; 8] {
        let mut out = [0_u64; 8];
        for (at, limb) in self.limbs.iter().enumerate() {
            out[at * 2] = *limb as u64;
            out[at * 2 + 1] = (*limb >> 64) as u64;
        }
        out
    }

    fn of_halves(halves: [u64; 8]) -> Self {
        let mut limbs = [0_u128; 4];
        for (at, limb) in limbs.iter_mut().enumerate() {
            *limb = u128::from(halves[at * 2]) | u128::from(halves[at * 2 + 1]) << 64;
        }
        Self { limbs }
    }
}

impl Ord for U512 {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        self.limbs.iter().rev().cmp(other.limbs.iter().rev())
    }
}

impl PartialOrd for U512 {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Not for U512 {
    type Output = Self;

    fn not(self) -> Self {
        Self { limbs: self.limbs.map(|limb| !limb) }
    }
}

macro_rules! limbwise {
    ($trait:ident, $method:ident, $assign_trait:ident, $assign:ident, $op:tt) => {
        impl $trait for U512 {
            type Output = Self;

            fn $method(
                self,
                other: Self,
            ) -> Self {
                let mut limbs = self.limbs;
                for (at, limb) in limbs.iter_mut().enumerate() {
                    *limb = *limb $op other.limbs[at];
                }
                Self { limbs }
            }
        }

        impl $assign_trait for U512 {
            fn $assign(
                &mut self,
                other: Self,
            ) {
                *self = *self $op other;
            }
        }
    };
}

limbwise!(BitAnd, bitand, BitAndAssign, bitand_assign, &);
limbwise!(BitOr, bitor, BitOrAssign, bitor_assign, |);
limbwise!(BitXor, bitxor, BitXorAssign, bitxor_assign, ^);

impl Shl<u32> for U512 {
    type Output = Self;

    fn shl(
        self,
        by: u32,
    ) -> Self {
        if by >= 512 {
            return Self::ZERO;
        }
        let (whole, part) = ((by / 128) as usize, by % 128);
        let mut limbs = [0_u128; 4];
        for at in whole..4 {
            let source = self.limbs[at - whole];
            limbs[at] |= source << part;
            if part != 0 && at + 1 < 4 {
                limbs[at + 1] |= source >> (128 - part);
            }
        }
        Self { limbs }
    }
}

impl Shr<u32> for U512 {
    type Output = Self;

    fn shr(
        self,
        by: u32,
    ) -> Self {
        if by >= 512 {
            return Self::ZERO;
        }
        let (whole, part) = ((by / 128) as usize, by % 128);
        let mut limbs = [0_u128; 4];
        for at in 0..4 - whole {
            let source = self.limbs[at + whole];
            limbs[at] |= source >> part;
            if part != 0 && at >= 1 {
                limbs[at - 1] |= source << (128 - part);
            }
        }
        Self { limbs }
    }
}

impl Word for U512 {
    const ZERO: Self = Self { limbs: [0; 4] };
    const ONE: Self = Self { limbs: [1, 0, 0, 0] };
    const BITS: u32 = 512;

    fn from_u128(value: u128) -> Self {
        Self { limbs: [value, 0, 0, 0] }
    }

    fn low_u128(self) -> u128 {
        self.limbs[0]
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
        let mut limbs = [0_u128; 4];
        let mut borrow = false;
        for (at, limb) in limbs.iter_mut().enumerate() {
            let (difference, first) = self.limbs[at].overflowing_sub(other.limbs[at]);
            let (difference, second) = difference.overflowing_sub(u128::from(borrow));
            *limb = difference;
            borrow = first || second;
        }
        Self { limbs }
    }

    fn overflowing_add(
        self,
        other: Self,
    ) -> (Self, bool) {
        let mut limbs = [0_u128; 4];
        let mut carry = false;
        for (at, limb) in limbs.iter_mut().enumerate() {
            let (sum, first) = self.limbs[at].overflowing_add(other.limbs[at]);
            let (sum, second) = sum.overflowing_add(u128::from(carry));
            *limb = sum;
            carry = first || second;
        }
        (Self { limbs }, carry)
    }

    fn overflowing_mul(
        self,
        other: Self,
    ) -> (Self, bool) {
        let (a, b) = (self.halves(), other.halves());
        let mut product = [0_u64; 16];
        for (i, x) in a.iter().enumerate() {
            let mut carry = 0_u128;
            for (j, y) in b.iter().enumerate() {
                let t = u128::from(*x) * u128::from(*y) + u128::from(product[i + j]) + carry;
                product[i + j] = t as u64;
                carry = t >> 64;
            }
            product[i + 8] = carry as u64;
        }
        let mut low = [0_u64; 8];
        low.copy_from_slice(&product[..8]);
        (Self::of_halves(low), product[8..].iter().any(|limb| *limb != 0))
    }

    fn leading_zeros(self) -> u32 {
        let mut zeros = 0;
        for limb in self.limbs.iter().rev() {
            if *limb != 0 {
                return zeros + limb.leading_zeros();
            }
            zeros += 128;
        }
        zeros
    }

    fn trailing_zeros(self) -> u32 {
        let mut zeros = 0;
        for limb in self.limbs {
            if limb != 0 {
                return zeros + limb.trailing_zeros();
            }
            zeros += 128;
        }
        zeros
    }

    fn count_ones(self) -> u32 {
        self.limbs.iter().map(|limb| limb.count_ones()).sum()
    }

    fn divrem(
        self,
        other: Self,
    ) -> (Self, Self) {
        assert!(!other.is_zero(), "attempt to divide by zero");
        if self < other {
            return (Self::ZERO, self);
        }
        let (mut quotient, mut remainder) = (Self::ZERO, Self::ZERO);
        for at in (0..512 - self.leading_zeros()).rev() {
            let carried = remainder.limbs[3] >> 127 == 1;
            remainder = remainder << 1 | (self >> at) & Self::ONE;
            if carried || remainder >= other {
                remainder = remainder.wrapping_sub(other);
                quotient |= Self::ONE << at;
            }
        }
        (quotient, remainder)
    }

    fn to_limbs(self) -> [u128; 4] {
        self.limbs
    }

    fn from_limbs(limbs: [u128; 4]) -> Self {
        Self { limbs }
    }
}

/// A power of two: one bit set.
pub fn is_power_of_two<W: Word>(v: W) -> bool {
    !v.is_zero() && (v & v.wrapping_sub(W::ONE)).is_zero()
}

/// The low `bits` bits set.
pub fn mask<W: Word>(bits: u32) -> W {
    if bits >= W::BITS { !W::ZERO } else { W::low_mask(bits) }
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

pub fn bit<W: Word>(
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

pub fn countl_zero<W: Word>(
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

pub fn countr_zero<W: Word>(
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

pub fn sign_bit<W: Word>(width: u32) -> W {
    W::bit(width - 1)
}

/// `a` as a signed number is below `b`.
pub fn slt<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> bool {
    (a ^ sign_bit::<W>(width)) < (b ^ sign_bit::<W>(width))
}

pub fn sgt<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> bool {
    slt(b, a, width)
}

/// Whether `v` is negative at `width`.
pub fn is_negative<W: Word>(
    v: W,
    width: u32,
) -> bool {
    bit(v, width - 1)
}

/// `v` extended from `from` bits to `to`, copying the sign.
pub fn sext<W: Word>(
    v: W,
    from: u32,
    to: u32,
) -> W {
    if is_negative(v, from) { (v | !mask::<W>(from)) & mask::<W>(to) } else { v }
}

pub fn ashr<W: Word>(
    v: W,
    width: u32,
    by: u32,
) -> W {
    let by = min(by, width - 1);
    let shifted = v >> by;
    if is_negative(v, width) { shifted | (mask::<W>(width) & !mask::<W>(width - by)) } else { shifted }
}

pub fn lshr<W: Word>(
    v: W,
    by: u32,
) -> W {
    if by >= W::BITS { W::ZERO } else { v >> by }
}

/// `APInt::shl(unsigned)`.
pub fn shl<W: Word>(
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
pub fn active_bits<W: Word>(v: W) -> u32 {
    W::BITS - v.leading_zeros()
}

/// `APInt::getSignificantBits`: the bits of the narrowest signed integer
/// holding `v`.
pub fn significant_bits<W: Word>(
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

/// `APInt::udiv`.
pub fn udiv<W: Word>(
    a: W,
    b: W,
) -> W {
    a.divrem(b).0
}

/// `APInt::urem`.
pub fn urem<W: Word>(
    a: W,
    b: W,
) -> W {
    a.divrem(b).1
}

/// The magnitude of `v` as a signed number of `width` bits (`INT_MIN` is its
/// own magnitude, as an unsigned number).
fn magnitude<W: Word>(
    v: W,
    width: u32,
) -> W {
    if is_negative(v, width) { W::ZERO.wrapping_sub(v) & mask::<W>(width) } else { v }
}

/// `APInt::sdiv` at any width: the quotient rounded towards zero
/// (`INT_MIN / -1` wraps).
pub fn sdiv<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    let quotient = udiv(magnitude(a, width), magnitude(b, width));
    let negative = is_negative(a, width) != is_negative(b, width);
    (if negative { W::ZERO.wrapping_sub(quotient) } else { quotient }) & mask::<W>(width)
}

/// `APInt::srem` at any width: the remainder with the sign of the dividend.
pub fn srem<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    let remainder = urem(magnitude(a, width), magnitude(b, width));
    (if is_negative(a, width) { W::ZERO.wrapping_sub(remainder) } else { remainder }) & mask::<W>(width)
}

/// `APIntOps::RoundingUDiv` at any width.
pub fn rounding_udiv<W: Word>(
    a: W,
    b: W,
    up: bool,
) -> W {
    let (quotient, remainder) = a.divrem(b);
    if up && !remainder.is_zero() { quotient.wrapping_add(W::ONE) } else { quotient }
}

/// `APIntOps::RoundingSDiv` at any width, rounding up or down.
pub fn rounding_sdiv<W: Word>(
    a: W,
    b: W,
    width: u32,
    up: bool,
) -> W {
    let quotient = sdiv(a, b, width);
    let remainder = srem(a, b, width);
    let result = if remainder.is_zero() {
        quotient
    } else if (is_negative(remainder, width)) != is_negative(b, width) {
        if up { quotient } else { quotient.wrapping_sub(W::ONE) }
    } else if up {
        quotient.wrapping_add(W::ONE)
    } else {
        quotient
    };
    result & mask::<W>(width)
}

/// `APInt::smul_sat` at any width.
pub fn smul_sat<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> W {
    let (product, over) = smul_ov(a, b, width);
    if !over {
        return product;
    }
    if is_negative(a, width) != is_negative(b, width) { sign_bit::<W>(width) } else { sign_bit::<W>(width).wrapping_sub(W::ONE) }
}

/// `APInt::sadd_ov` at any width.
pub fn sadd_ov<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> (W, bool) {
    let sum = a.wrapping_add(b) & mask::<W>(width);
    (sum, is_negative(a, width) == is_negative(b, width) && is_negative(sum, width) != is_negative(a, width))
}

/// `APInt::ssub_ov` at any width.
pub fn ssub_ov<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> (W, bool) {
    let difference = a.wrapping_sub(b) & mask::<W>(width);
    (difference, is_negative(a, width) != is_negative(b, width) && is_negative(difference, width) != is_negative(a, width))
}

/// `APInt::smul_ov` at any width.
pub fn smul_ov<W: Word>(
    a: W,
    b: W,
    width: u32,
) -> (W, bool) {
    let (product, over) = magnitude(a, width).overflowing_mul(magnitude(b, width));
    let negative = is_negative(a, width) != is_negative(b, width);
    let limit = if negative { sign_bit::<W>(width) } else { sign_bit::<W>(width).wrapping_sub(W::ONE) };
    let result = (if negative { W::ZERO.wrapping_sub(product) } else { product }) & mask::<W>(width);
    (result, over || product > limit)
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

#[cfg(test)]
#[path = "apint_tests.rs"]
mod tests;
