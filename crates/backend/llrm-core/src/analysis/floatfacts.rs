//! Exact finite numeric facts; strict effects remain a separate obligation.
//!
//! Port of `qbopt/analysis/floatfacts.py`.

use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Neg, Sub};

use num_bigint::BigInt;

use super::induction;

/// Python's `fractions.Fraction`: always in lowest terms, denominator positive.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Fraction {
    pub numerator: BigInt,
    pub denominator: BigInt,
}

impl Fraction {
    pub fn new(numerator: impl Into<BigInt>, denominator: impl Into<BigInt>) -> Self {
        let (mut numerator, mut denominator) = (numerator.into(), denominator.into());
        assert!(denominator != BigInt::from(0), "Fraction(_, 0)");
        if denominator < BigInt::from(0) {
            numerator = -numerator;
            denominator = -denominator;
        }
        let divisor = induction::gcd(numerator.magnitude().clone().into(), denominator.clone());
        Self {
            numerator: numerator / &divisor,
            denominator: denominator / divisor,
        }
    }
}

/// Python's `str(fraction)`: `n` over one, else `n/d`.
impl std::fmt::Display for Fraction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.denominator == BigInt::from(1) {
            write!(formatter, "{}", self.numerator)
        } else {
            write!(formatter, "{}/{}", self.numerator, self.denominator)
        }
    }
}

impl Ord for Fraction {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.numerator * &other.denominator).cmp(&(&other.numerator * &self.denominator))
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for &Fraction {
    type Output = Fraction;
    fn add(self, other: Self) -> Fraction {
        Fraction::new(
            &self.numerator * &other.denominator + &other.numerator * &self.denominator,
            &self.denominator * &other.denominator,
        )
    }
}

impl Sub for &Fraction {
    type Output = Fraction;
    fn sub(self, other: Self) -> Fraction {
        self + &-other
    }
}

impl Mul for &Fraction {
    type Output = Fraction;
    fn mul(self, other: Self) -> Fraction {
        Fraction::new(&self.numerator * &other.numerator, &self.denominator * &other.denominator)
    }
}

impl Div for &Fraction {
    type Output = Fraction;
    fn div(self, other: Self) -> Fraction {
        Fraction::new(&self.numerator * &other.denominator, &self.denominator * &other.numerator)
    }
}

impl Neg for &Fraction {
    type Output = Fraction;
    fn neg(self) -> Fraction {
        Fraction {
            numerator: -&self.numerator,
            denominator: self.denominator.clone(),
        }
    }
}
