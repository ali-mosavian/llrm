//! An integer that is a machine word until it must not be.
//!
//! # How it works
//! `Num` holds an `i128` and falls back to `BigInt` only where a checked
//! operation overflows, as LLVM's `APInt` keeps a word inline and allocates
//! only past 64 bits. It is normalised: a value that fits `i128` is always
//! `Small`, so equality and hashing are those of the number. Every operation
//! gives what `BigInt` gives, including the floor of `>>` and the two's
//! complement of `&` on negatives.
//!
//! # Provides
//! [`Num`], for [`crate::ranges`] and [`crate::induction`].
//!
//! # See also
//! LLVM `llvm/include/llvm/ADT/APInt.h`, `APInt::U` (inline word or heap
//! words).

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Rem, Shl, Shr, Sub};

use num_bigint::BigInt;
use num_traits::ToPrimitive;

/// An integer of any size, inline while it fits 128 bits.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Num {
    Small(i128),
    Big(BigInt),
}

impl Num {
    fn of(big: BigInt) -> Self {
        match big.to_i128() {
            Some(small) => Self::Small(small),
            None => Self::Big(big),
        }
    }

    pub fn to_big(&self) -> BigInt {
        match self {
            Self::Small(small) => BigInt::from(*small),
            Self::Big(big) => big.clone(),
        }
    }

    fn as_small(&self) -> Option<i128> {
        match self {
            Self::Small(small) => Some(*small),
            Self::Big(_) => None,
        }
    }
}

impl fmt::Display for Num {
    fn fmt(
        &self,
        out: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Small(small) => small.fmt(out),
            Self::Big(big) => big.fmt(out),
        }
    }
}

impl PartialOrd for Num {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Num {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        match (self, other) {
            (Self::Small(one), Self::Small(two)) => one.cmp(two),
            _ => self.to_big().cmp(&other.to_big()),
        }
    }
}

macro_rules! from_primitive {
    ($($ty:ty),*) => {$(
        impl From<$ty> for Num {
            fn from(n: $ty) -> Self {
                Self::of_primitive(i128::from(n))
            }
        }
    )*};
}
from_primitive!(i8, i16, i32, i64, i128, u8, u16, u32, u64);

impl Num {
    fn of_primitive(n: i128) -> Self {
        Self::Small(n)
    }
}

impl From<usize> for Num {
    fn from(n: usize) -> Self {
        Self::Small(n as i128)
    }
}

impl From<u128> for Num {
    fn from(n: u128) -> Self {
        match i128::try_from(n) {
            Ok(small) => Self::Small(small),
            Err(_) => Self::Big(BigInt::from(n)),
        }
    }
}

impl From<BigInt> for Num {
    fn from(big: BigInt) -> Self {
        Self::of(big)
    }
}

impl From<&BigInt> for Num {
    fn from(big: &BigInt) -> Self {
        match big.to_i128() {
            Some(small) => Self::Small(small),
            None => Self::Big(big.clone()),
        }
    }
}

impl From<Num> for BigInt {
    fn from(n: Num) -> Self {
        match n {
            Num::Small(small) => BigInt::from(small),
            Num::Big(big) => big,
        }
    }
}

macro_rules! try_from_num {
    ($($ty:ty),*) => {$(
        impl TryFrom<&Num> for $ty {
            type Error = ();
            fn try_from(n: &Num) -> Result<Self, ()> {
                match n {
                    Num::Small(small) => <$ty>::try_from(*small).map_err(|_| ()),
                    Num::Big(_) => Err(()),
                }
            }
        }
        impl TryFrom<Num> for $ty {
            type Error = ();
            fn try_from(n: Num) -> Result<Self, ()> {
                <$ty>::try_from(&n)
            }
        }
    )*};
}
try_from_num!(i64, i128, u32, u64, usize);

impl Neg for Num {
    type Output = Num;
    fn neg(self) -> Num {
        -&self
    }
}

impl Neg for &Num {
    type Output = Num;
    fn neg(self) -> Num {
        match self {
            Num::Small(small) => match small.checked_neg() {
                Some(negated) => Num::Small(negated),
                None => Num::of(-BigInt::from(*small)),
            },
            Num::Big(big) => Num::of(-big),
        }
    }
}

macro_rules! arithmetic {
    ($trait:ident, $method:ident, $checked:ident, $op:tt) => {
        impl $trait<&Num> for &Num {
            type Output = Num;
            fn $method(self, other: &Num) -> Num {
                if let (Some(one), Some(two)) = (self.as_small(), other.as_small())
                    && let Some(found) = one.$checked(two)
                {
                    return Num::Small(found);
                }
                Num::of(self.to_big() $op other.to_big())
            }
        }
        impl $trait<&Num> for Num {
            type Output = Num;
            fn $method(self, other: &Num) -> Num {
                &self $op other
            }
        }
        impl $trait<Num> for &Num {
            type Output = Num;
            fn $method(self, other: Num) -> Num {
                self $op &other
            }
        }
        impl $trait<Num> for Num {
            type Output = Num;
            fn $method(self, other: Num) -> Num {
                &self $op &other
            }
        }
        impl<T: Primitive> $trait<T> for Num {
            type Output = Num;
            fn $method(self, other: T) -> Num {
                &self $op &Num::from(other.widen())
            }
        }
        impl<T: Primitive> $trait<T> for &Num {
            type Output = Num;
            fn $method(self, other: T) -> Num {
                self $op &Num::from(other.widen())
            }
        }
    };
}

/// A machine integer an operator takes as its other side.
pub trait Primitive {
    fn widen(self) -> i128;
}
macro_rules! primitive {
    ($($ty:ty),*) => {$(impl Primitive for $ty { fn widen(self) -> i128 { self as i128 } })*};
}
primitive!(i8, i16, i32, i64, i128, u8, u16, u32, u64, usize);

arithmetic!(Add, add, checked_add, +);
arithmetic!(Sub, sub, checked_sub, -);
arithmetic!(Mul, mul, checked_mul, *);

macro_rules! bitwise {
    ($trait:ident, $method:ident, $op:tt, $assign:ident, $assign_method:ident) => {
        impl $trait<&Num> for &Num {
            type Output = Num;
            fn $method(self, other: &Num) -> Num {
                match (self.as_small(), other.as_small()) {
                    (Some(one), Some(two)) => Num::Small(one $op two),
                    _ => Num::of(self.to_big() $op other.to_big()),
                }
            }
        }
        impl $trait<&Num> for Num {
            type Output = Num;
            fn $method(self, other: &Num) -> Num {
                &self $op other
            }
        }
        impl $trait<Num> for &Num {
            type Output = Num;
            fn $method(self, other: Num) -> Num {
                self $op &other
            }
        }
        impl $trait<Num> for Num {
            type Output = Num;
            fn $method(self, other: Num) -> Num {
                &self $op &other
            }
        }
        impl<T: Primitive> $trait<T> for Num {
            type Output = Num;
            fn $method(self, other: T) -> Num {
                &self $op &Num::from(other.widen())
            }
        }
        impl<T: Primitive> $trait<T> for &Num {
            type Output = Num;
            fn $method(self, other: T) -> Num {
                self $op &Num::from(other.widen())
            }
        }
        impl std::ops::$assign<Num> for Num {
            fn $assign_method(&mut self, other: Num) {
                *self = &*self $op &other;
            }
        }
        impl std::ops::$assign<&Num> for Num {
            fn $assign_method(&mut self, other: &Num) {
                *self = &*self $op other;
            }
        }
    };
}
bitwise!(BitAnd, bitand, &, BitAndAssign, bitand_assign);
bitwise!(BitOr, bitor, |, BitOrAssign, bitor_assign);
bitwise!(BitXor, bitxor, ^, BitXorAssign, bitxor_assign);

impl Num {
    fn shifted_left(
        &self,
        bits: usize,
    ) -> Num {
        if let Some(small) = self.as_small() {
            if small == 0 {
                return Num::Small(0);
            }
            if bits < 127 {
                let shifted = small << bits;
                if shifted >> bits == small {
                    return Num::Small(shifted);
                }
            }
        }
        Num::of(self.to_big() << bits)
    }

    fn shifted_right(
        &self,
        bits: usize,
    ) -> Num {
        match self {
            Num::Small(small) => Num::Small(if bits >= 128 { if *small < 0 { -1 } else { 0 } } else { small >> bits }),
            Num::Big(big) => Num::of(big >> bits),
        }
    }
}

fn amount(bits: impl Primitive) -> usize {
    usize::try_from(bits.widen()).expect("a shift is not negative")
}

impl<T: Primitive> Shl<T> for &Num {
    type Output = Num;
    fn shl(
        self,
        bits: T,
    ) -> Num {
        self.shifted_left(amount(bits))
    }
}

impl<T: Primitive> Shl<T> for Num {
    type Output = Num;
    fn shl(
        self,
        bits: T,
    ) -> Num {
        self.shifted_left(amount(bits))
    }
}

impl<T: Primitive> Shr<T> for &Num {
    type Output = Num;
    fn shr(
        self,
        bits: T,
    ) -> Num {
        self.shifted_right(amount(bits))
    }
}

impl<T: Primitive> Shr<T> for Num {
    type Output = Num;
    fn shr(
        self,
        bits: T,
    ) -> Num {
        self.shifted_right(amount(bits))
    }
}

impl<T: Primitive> std::ops::AddAssign<T> for Num {
    fn add_assign(
        &mut self,
        other: T,
    ) {
        *self = &*self + other;
    }
}

impl<T: Primitive> std::ops::SubAssign<T> for Num {
    fn sub_assign(
        &mut self,
        other: T,
    ) {
        *self = &*self - other;
    }
}

impl Num {
    /// The floor of the square root of a non-negative number.
    pub fn sqrt(&self) -> Num {
        Num::of(self.to_big().sqrt())
    }
}

macro_rules! division {
    ($trait:ident, $method:ident, $checked:ident, $op:tt) => {
        impl $trait<&Num> for &Num {
            type Output = Num;
            fn $method(self, other: &Num) -> Num {
                if let (Some(one), Some(two)) = (self.as_small(), other.as_small())
                    && let Some(found) = one.$checked(two)
                {
                    return Num::Small(found);
                }
                Num::of(self.to_big() $op other.to_big())
            }
        }
        impl $trait<&Num> for Num {
            type Output = Num;
            fn $method(self, other: &Num) -> Num {
                &self $op other
            }
        }
        impl $trait<Num> for &Num {
            type Output = Num;
            fn $method(self, other: Num) -> Num {
                self $op &other
            }
        }
        impl $trait<Num> for Num {
            type Output = Num;
            fn $method(self, other: Num) -> Num {
                &self $op &other
            }
        }
        impl<T: Primitive> $trait<T> for Num {
            type Output = Num;
            fn $method(self, other: T) -> Num {
                &self $op &Num::from(other.widen())
            }
        }
        impl<T: Primitive> $trait<T> for &Num {
            type Output = Num;
            fn $method(self, other: T) -> Num {
                self $op &Num::from(other.widen())
            }
        }
    };
}
division!(Div, div, checked_div, /);
division!(Rem, rem, checked_rem, %);

impl std::ops::AddAssign<Num> for Num {
    fn add_assign(
        &mut self,
        other: Num,
    ) {
        *self = &*self + &other;
    }
}

impl std::ops::AddAssign<&Num> for Num {
    fn add_assign(
        &mut self,
        other: &Num,
    ) {
        *self = &*self + other;
    }
}

impl std::ops::SubAssign<Num> for Num {
    fn sub_assign(
        &mut self,
        other: Num,
    ) {
        *self = &*self - &other;
    }
}

impl std::ops::SubAssign<&Num> for Num {
    fn sub_assign(
        &mut self,
        other: &Num,
    ) {
        *self = &*self - other;
    }
}

impl Num {
    pub fn trailing_zeros(&self) -> Option<u64> {
        match self {
            Num::Small(0) => None,
            Num::Small(small) => Some(u64::from(small.trailing_zeros())),
            Num::Big(big) => big.trailing_zeros(),
        }
    }
}

impl fmt::LowerHex for Num {
    fn fmt(
        &self,
        out: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Num::Small(small) if *small >= 0 => fmt::LowerHex::fmt(small, out),
            _ => fmt::LowerHex::fmt(&self.to_big(), out),
        }
    }
}

impl Num {
    /// The absolute value.
    pub fn magnitude(&self) -> Num {
        if *self < Num::Small(0) { -self } else { self.clone() }
    }

    /// The bits of the absolute value.
    pub fn bits(&self) -> u64 {
        match self {
            Num::Small(small) => u64::from(128 - small.unsigned_abs().leading_zeros()),
            Num::Big(big) => big.bits(),
        }
    }
}

impl TryFrom<Num> for u128 {
    type Error = ();
    fn try_from(n: Num) -> Result<u128, ()> {
        match n {
            Num::Small(small) => u128::try_from(small).map_err(|_| ()),
            Num::Big(big) => u128::try_from(big).map_err(|_| ()),
        }
    }
}

impl num_traits::ToPrimitive for Num {
    fn to_i64(&self) -> Option<i64> {
        i64::try_from(self).ok()
    }

    fn to_u64(&self) -> Option<u64> {
        u64::try_from(self).ok()
    }

    fn to_i128(&self) -> Option<i128> {
        i128::try_from(self).ok()
    }

    fn to_u128(&self) -> Option<u128> {
        u128::try_from(self.clone()).ok()
    }
}

impl num_traits::Zero for Num {
    fn zero() -> Self {
        Num::Small(0)
    }

    fn is_zero(&self) -> bool {
        *self == Num::Small(0)
    }
}

impl num_traits::One for Num {
    fn one() -> Self {
        Num::Small(1)
    }
}

impl TryFrom<&Num> for u128 {
    type Error = ();
    fn try_from(n: &Num) -> Result<u128, ()> {
        u128::try_from(n.clone())
    }
}

impl Num {
    /// Which side of zero.
    pub fn sign(&self) -> num_bigint::Sign {
        match self.cmp(&Num::Small(0)) {
            Ordering::Less => num_bigint::Sign::Minus,
            Ordering::Equal => num_bigint::Sign::NoSign,
            Ordering::Greater => num_bigint::Sign::Plus,
        }
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;

    use super::Num;

    fn samples() -> Vec<BigInt> {
        let mut found = vec![BigInt::from(0), BigInt::from(1), BigInt::from(-1), BigInt::from(7), BigInt::from(-300)];
        for bits in [31_u32, 32, 63, 64, 100, 126, 127, 128, 129, 200] {
            let power = BigInt::from(1) << bits;
            let below: BigInt = &power - 1;
            let above: BigInt = &power + 1;
            found.extend([power.clone(), -power.clone(), below.clone(), -below, above]);
        }
        found
    }

    fn num(n: &BigInt) -> Num {
        Num::from(n.clone())
    }

    /// Every operation gives what `BigInt` gives, where the operands and the
    /// results are on either side of 128 bits: an overflow that wrapped, or a
    /// result kept `Big` that fits, would be a bound that is not the number.
    #[test]
    fn every_operation_gives_what_bigint_gives_across_the_word() {
        let values = samples();
        for one in &values {
            for other in &values {
                let (a, b) = (num(one), num(other));
                assert_eq!(&a + &b, num(&(one + other)), "{one} + {other}");
                assert_eq!(&a - &b, num(&(one - other)), "{one} - {other}");
                assert_eq!(&a * &b, num(&(one * other)), "{one} * {other}");
                assert_eq!(&a & &b, num(&(one & other)), "{one} & {other}");
                assert_eq!(&a | &b, num(&(one | other)), "{one} | {other}");
                assert_eq!(&a ^ &b, num(&(one ^ other)), "{one} ^ {other}");
                assert_eq!(a.cmp(&b), one.cmp(other), "{one} <=> {other}");
                if *other != BigInt::from(0) {
                    assert_eq!(&a / &b, num(&(one / other)), "{one} / {other}");
                    assert_eq!(&a % &b, num(&(one % other)), "{one} % {other}");
                }
            }
            let a = num(one);
            assert_eq!(-&a, num(&-one), "-{one}");
            for bits in [0_usize, 1, 5, 63, 64, 126, 127, 128, 129, 300] {
                assert_eq!(&a << bits, num(&(one << bits)), "{one} << {bits}");
                assert_eq!(&a >> bits, num(&(one >> bits)), "{one} >> {bits}");
            }
        }
    }

    /// A value that fits a word is a word however it was made, so equality and
    /// hashing are the number's.
    #[test]
    fn a_number_is_the_same_number_by_either_path() {
        let big = Num::from(BigInt::from(1) << 200_u32);
        assert_eq!((&big - &big) + 5, Num::from(5));
        assert_eq!((&big >> 190_usize) >> 10_usize, Num::from(1));
        assert_eq!(Num::from(i128::MAX) + 1 - 1, Num::from(i128::MAX));
        assert_eq!(-Num::from(i128::MIN) + Num::from(i128::MIN), Num::from(0));
    }
}
