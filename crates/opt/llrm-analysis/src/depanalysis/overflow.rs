//! `OverflowSafeSignedAPInt`: a signed integer that is "invalid" once an
//! operation overflowed, so a chain `a + b + c` needs one check at its end.
//!
//! # How it works
//! `Safe(None)` is invalid and stays so through every operation. `Some`
//! carries the value at its width; an addition, subtraction, multiplication
//! or negation that overflows in a signed sense makes the result invalid.
//!
//! # Example
//! `i8` 100 + 100 is invalid; invalid + 1 is invalid; `-(-128)` is invalid.

use crate::scev::ap::Ap;

/// `OverflowSafeSignedAPInt`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Safe(pub Option<Ap>);

impl Safe {
    /// The invalid value.
    pub const INVALID: Self = Self(None);

    pub fn of(value: Ap) -> Self {
        Self(Some(value))
    }

    /// `operator bool`.
    pub fn is_valid(&self) -> bool {
        self.0.is_some()
    }

    /// `operator*`.
    pub fn value(&self) -> Ap {
        self.0.expect("Value is not available.")
    }

    pub fn add(
        self,
        other: Self,
    ) -> Self {
        let (Some(a), Some(b)) = (self.0, other.0) else { return Self::INVALID };
        let (result, overflow) = a.sadd_ov(b);
        if overflow { Self::INVALID } else { Self::of(result) }
    }

    pub fn sub(
        self,
        other: Self,
    ) -> Self {
        let (Some(a), Some(b)) = (self.0, other.0) else { return Self::INVALID };
        let (result, overflow) = a.ssub_ov(b);
        if overflow { Self::INVALID } else { Self::of(result) }
    }

    pub fn mul(
        self,
        other: Self,
    ) -> Self {
        let (Some(a), Some(b)) = (self.0, other.0) else { return Self::INVALID };
        let (result, overflow) = a.smul_ov(b);
        if overflow { Self::INVALID } else { Self::of(result) }
    }

    /// `operator+(int)`.
    pub fn add_int(
        self,
        rhs: i64,
    ) -> Self {
        let Some(a) = self.0 else { return Self::INVALID };
        self.add(Self::of(Ap::from_signed(a.width, i128::from(rhs))))
    }

    /// `operator-(int)`.
    pub fn sub_int(
        self,
        rhs: i64,
    ) -> Self {
        let Some(a) = self.0 else { return Self::INVALID };
        self.sub(Self::of(Ap::from_signed(a.width, i128::from(rhs))))
    }

    /// `operator-()`: invalid for the most negative value.
    pub fn neg(self) -> Self {
        let Some(a) = self.0 else { return Self::INVALID };
        if a.is_signed_min() { Self::INVALID } else { Self::of(a.neg()) }
    }
}
