//! `x86_fp80` constants: how MIR holds a value of the x87 extended format and
//! the arithmetic on it, in `APFloat`'s x87 semantics (64-bit significand).
//!
//! A value some double holds exactly is `ConstantKind::Float` with that
//! double's bits; any other is `ConstantKind::Float80` with the 80 bits. One
//! value has one form, so equal values are one interned constant.
//!
//! # Example
//! 0.00005 times 10000 is `Float80(0x3ffe_ffff_ffff_ffff_ffff…)`, just under
//! 0.5, where the double product is exactly 0.5.
//!
//! # See also
//! `constfold.rs` folds with it; `simplify.rs` item 2 owed it.

use crate::apfloat::{ApFloat, IEEE_DOUBLE, OP_INVALID_OP, OP_OVERFLOW, OP_UNDERFLOW, X87_DOUBLE_EXTENDED};
use crate::context::{Constant, ConstantId, ConstantKind, Context};
use crate::fpclass::RoundingMode;
use crate::opcode::BinaryOp;
use crate::types::TypeId;

/// The value a constant of type `x86_fp80` holds, or none for another kind.
pub fn value(kind: &ConstantKind) -> Option<ApFloat> {
    match kind {
        ConstantKind::Float(double) => {
            let mut one = ApFloat::from_bits(IEEE_DOUBLE, u128::from(*double));
            one.convert(X87_DOUBLE_EXTENDED, RoundingMode::NearestTiesToEven);
            Some(one)
        }
        ConstantKind::Float80(bits) => Some(ApFloat::from_bits(X87_DOUBLE_EXTENDED, *bits)),
        ConstantKind::Null | ConstantKind::Zero => Some(ApFloat::get_zero(X87_DOUBLE_EXTENDED, false)),
        _ => None,
    }
}

/// The nearest double's bits, rounding to nearest even.
pub fn to_double_bits(bits: u128) -> u64 {
    let mut one = ApFloat::from_bits(X87_DOUBLE_EXTENDED, bits);
    one.convert(IEEE_DOUBLE, RoundingMode::NearestTiesToEven);
    one.to_bits() as u64
}

/// The kind that holds `number`: the double's bits when a double holds it
/// exactly, else all 80.
pub fn kind(number: &ApFloat) -> ConstantKind {
    let mut narrow = *number;
    let (_, lost) = narrow.convert(IEEE_DOUBLE, RoundingMode::NearestTiesToEven);
    if lost || number.is_nan() {
        ConstantKind::Float80(number.to_bits())
    } else {
        ConstantKind::Float(narrow.to_bits() as u64)
    }
}

/// The constant of type `ty` (an `x86_fp80`) holding `number`.
pub fn constant(
    context: &Context,
    ty: TypeId,
    number: &ApFloat,
) -> ConstantId {
    context.constant(Constant { ty, kind: kind(number) })
}

/// `left op right` in the x87's format, rounding to nearest even as the
/// hardware does at full precision; none for a NaN, an infinity, a division by
/// zero or a result that is not finite.
pub fn binary(
    op: BinaryOp,
    left: &ApFloat,
    right: &ApFloat,
) -> Option<ApFloat> {
    if !left.is_finite() || !right.is_finite() {
        return None;
    }
    let mut result = *left;
    let status = match op {
        BinaryOp::FAdd => result.add(right, RoundingMode::NearestTiesToEven),
        BinaryOp::FSub => result.subtract(right, RoundingMode::NearestTiesToEven),
        BinaryOp::FMul => result.multiply(right, RoundingMode::NearestTiesToEven),
        BinaryOp::FDiv if right.is_zero() => return None,
        BinaryOp::FDiv => result.divide(right, RoundingMode::NearestTiesToEven),
        _ => return None,
    };
    (result.is_finite() && status & (OP_OVERFLOW | OP_UNDERFLOW | OP_INVALID_OP) == 0).then_some(result)
}

/// The integer `number` rounds to in `mode`, when a signed or unsigned
/// `width`-bit integer holds it (`width` is at most 127 for a signed one); none
/// for a NaN, an infinity or a value out of range.
pub fn to_integer(
    number: &ApFloat,
    mode: RoundingMode,
    width: u32,
    signed: bool,
) -> Option<i128> {
    if !number.is_finite() {
        return None;
    }
    let mut rounded = *number;
    rounded.round_to_integral(mode);
    let bits = rounded.to_bits();
    let negative = (bits >> 79) & 1 == 1;
    let exponent = ((bits >> 64) & 0x7fff) as i32;
    let significand = bits as u64;
    let magnitude: u128 = if exponent == 0 {
        0
    } else {
        let shift = exponent - 16383 - 63;
        match shift {
            ..=-64 => 0,
            -63..=0 => u128::from(significand >> -shift),
            1..=63 => u128::from(significand) << shift,
            _ => return None,
        }
    };
    let limit = if signed { 1_u128 << (width - 1) } else { 1_u128 << width };
    match (negative, signed) {
        (false, _) if magnitude < limit => Some(magnitude as i128),
        (true, true) if magnitude <= limit => Some(-(magnitude as i128)),
        (true, false) if magnitude == 0 => Some(0),
        _ => None,
    }
}

/// The exact value of the integer `magnitude` with `negative`'s sign.
pub fn from_integer(
    magnitude: u64,
    negative: bool,
) -> ApFloat {
    if magnitude == 0 {
        return ApFloat::get_zero(X87_DOUBLE_EXTENDED, false);
    }
    let lead = magnitude.leading_zeros();
    let exponent = 16383 + 63 - lead as u128;
    let bits = (u128::from(negative) << 79) | (exponent << 64) | u128::from(magnitude << lead);
    ApFloat::from_bits(X87_DOUBLE_EXTENDED, bits)
}
