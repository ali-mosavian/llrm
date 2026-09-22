//! Exact finite numeric facts; strict effects remain a separate obligation.
//!
//! Port of `qbopt/analysis/floatfacts.py`.

// ---- early port (agent E) ----
#![allow(dead_code)]

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use crate::model::floating::{Format, Precision, Semantics};
use crate::model::mir::Kind;

/// Python's `Fraction`.
pub(crate) type Fraction = BigRational;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Finite {
    pub value: Fraction,
    pub negative_zero: bool,
}

impl Finite {
    pub(crate) fn new(value: Fraction) -> Self {
        Self { value, negative_zero: false }
    }

    pub(crate) fn with_zero(value: Fraction, negative_zero: bool) -> Self {
        Self { value, negative_zero }
    }

    pub(crate) fn negative(&self) -> bool {
        self.value.is_negative() || self.negative_zero
    }
}

#[allow(non_snake_case)]
fn _BINARY(format: Format) -> Option<(u64, u64, i64)> {
    match format {
        Format::Binary32 => Some((24, 8, 127)),
        Format::Binary64 => Some((53, 11, 1023)),
        _ => None,
    }
}

#[allow(non_snake_case)]
fn _INTEGER(format: Format) -> Option<u64> {
    match format {
        Format::Signed16 => Some(16),
        Format::Signed32 => Some(32),
        Format::Signed64 => Some(64),
        _ => None,
    }
}

#[allow(non_snake_case)]
fn _UNSIGNED(format: Format) -> Option<u64> {
    match format {
        Format::Unsigned64 => Some(64),
        _ => None,
    }
}

fn shl(bits: u64) -> BigInt {
    BigInt::one() << bits
}

fn integer(n: BigInt) -> Fraction {
    Fraction::from_integer(n)
}

pub(crate) fn decoded(bits: &BigInt, format: Format) -> Option<Finite> {
    if let Some(width) = _INTEGER(format) {
        if !(&BigInt::zero() <= bits && bits < &shl(width)) {
            return None;
        }
        let sign = shl(width - 1);
        return Some(Finite::new(integer((bits ^ &sign) - &sign)));
    }
    if let Some(width) = _UNSIGNED(format) {
        return (&BigInt::zero() <= bits && bits < &shl(width)).then(|| Finite::new(integer(bits.clone())));
    }
    let (precision, exponent_bits, bias) = _BINARY(format)?;
    if !(&BigInt::zero() <= bits && bits < &shl(precision + exponent_bits)) {
        return None;
    }
    let fraction: BigInt = bits & (shl(precision - 1) - 1);
    let exponent: BigInt = (bits >> (precision - 1)) & (shl(exponent_bits) - 1);
    let negative = !(bits >> (precision + exponent_bits - 1)).is_zero();
    if exponent == shl(exponent_bits) - 1 || (exponent.is_zero() && !fraction.is_zero()) {
        return None;
    }
    if exponent.is_zero() {
        return Some(Finite::with_zero(Fraction::zero(), negative));
    }
    let significand = shl(precision - 1) | fraction;
    let shift = i64::try_from(&exponent).expect("exponent fits") - bias - precision as i64 + 1;
    let value = Fraction::new(
        significand << shift.max(0) as u64,
        shl((-shift).max(0) as u64),
    );
    Some(Finite::new(if negative { -value } else { value }))
}

fn _fits(value: &Fraction, precision: u64, minimum: i64, maximum: i64) -> bool {
    if value.is_zero() {
        return true;
    }
    let (numerator, denominator) = (value.numer().abs(), value.denom().clone());
    if !(&denominator & (&denominator - BigInt::one())).is_zero() {
        return false;
    }
    let trailing = numerator.trailing_zeros().expect("nonzero numerator");
    let exponent = numerator.bits() as i64 - denominator.bits() as i64;
    numerator.bits() - trailing <= precision && minimum <= exponent && exponent <= maximum
}

pub(crate) fn evaluated(kind: Kind, rule: &Semantics, inputs: &[Finite]) -> Option<Finite> {
    if inputs.len() != rule.inputs.len() {
        return None;
    }
    let result = match (kind, inputs) {
        (Kind::Fload | Kind::Fstore, [value]) => value.clone(),
        (Kind::Fneg, [value]) => Finite::with_zero(
            -value.value.clone(),
            if value.value.is_zero() { !value.negative_zero } else { false },
        ),
        (Kind::Fabs, [value]) => Finite::new(value.value.abs()),
        (Kind::Fsqrt, [value]) => {
            if value.value.is_negative() {
                return None;
            }
            let numerator = value.value.numer().sqrt();
            let denominator = value.value.denom().sqrt();
            if &(&numerator * &numerator) != value.value.numer()
                || &(&denominator * &denominator) != value.value.denom()
            {
                return None;
            }
            Finite::with_zero(Fraction::new(numerator, denominator), value.negative_zero)
        }
        (Kind::Fadd | Kind::Fsub, [left, right]) => {
            let right_value = if kind == Kind::Fadd {
                right.value.clone()
            } else {
                -right.value.clone()
            };
            let value = &left.value + right_value;
            if value.is_zero() {
                let right_negative = right.negative() ^ (kind == Kind::Fsub);
                if !left.value.is_zero() || !right.value.is_zero() || left.negative() != right_negative {
                    return None; // cancellation's zero sign depends on rounding
                }
                Finite::with_zero(value, left.negative())
            } else {
                Finite::new(value)
            }
        }
        (Kind::Fmul | Kind::Fdiv, [left, right]) => {
            if kind == Kind::Fdiv && right.value.is_zero() {
                return None;
            }
            let value = if kind == Kind::Fmul {
                &left.value * &right.value
            } else {
                &left.value / &right.value
            };
            let negative_zero = value.is_zero() && left.negative() != right.negative();
            Finite::with_zero(value, negative_zero)
        }
        _ => return None,
    };
    if let Some(width) = _INTEGER(rule.result) {
        let low = integer(-shl(width - 1));
        let high = integer(shl(width - 1));
        return (result.value.is_integer() && low <= result.value && result.value < high).then_some(result);
    }
    if let Some(width) = _UNSIGNED(rule.result) {
        return (result.value.is_integer() && !result.value.is_negative() && result.value < integer(shl(width)))
            .then_some(result);
    }
    if let Some((precision, _, bias)) = _BINARY(rule.result) {
        return _fits(&result.value, precision, 1 - bias, bias).then_some(result);
    }
    if rule.result == Format::Extended80 {
        let precision = if rule.precision == Precision::Dynamic { 24 } else { 64 };
        return _fits(&result.value, precision, -16382, 16383).then_some(result);
    }
    None
}

pub(crate) fn encoded(value: &Finite, format: Format) -> Option<BigInt> {
    let (precision, exponent_bits, bias) = _BINARY(format)?;
    if !_fits(&value.value, precision, 1 - bias, bias) {
        return None;
    }
    let sign = BigInt::from(u8::from(value.negative())) << (precision + exponent_bits - 1);
    if value.value.is_zero() {
        return Some(sign);
    }
    let magnitude = value.value.abs();
    let exponent = magnitude.numer().bits() as i64 - magnitude.denom().bits() as i64;
    let shift = precision as i64 - 1 - exponent;
    let significand = magnitude * Fraction::new(shl(shift.max(0) as u64), shl((-shift).max(0) as u64));
    Some(
        sign | (BigInt::from(exponent + bias) << (precision - 1))
            | (significand.to_integer() - shl(precision - 1)),
    )
}
