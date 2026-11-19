//! `fp128` constants: a binary128 value is `ConstantKind::Float128` with its
//! 128 bits, and the arithmetic on it is `APFloat`'s IEEE quad, which
//! `apfloat_golden.txt` runs against LLVM `d1106deb71cc` on every operation.
//!
//! # Example
//! `fmul fp128 3.5, 10.0` is `Float128(0x4004_1800…)`, 35.0.
//!
//! # See also
//! `constfold.rs` folds with it; `fp80.rs` is the x87 counterpart.

use crate::apfloat::{ApFloat, IEEE_QUAD, OP_INVALID_OP, OP_OVERFLOW, OP_UNDERFLOW};
use crate::context::{Constant, ConstantId, ConstantKind, Context};
use crate::fpclass::RoundingMode;
use crate::opcode::BinaryOp;
use crate::types::TypeId;

/// The value a constant of type `fp128` holds, or none for another kind.
pub fn value(kind: &ConstantKind) -> Option<ApFloat> {
    match kind {
        ConstantKind::Float128(bits) => Some(ApFloat::from_bits(IEEE_QUAD, *bits)),
        ConstantKind::Null | ConstantKind::Zero => Some(ApFloat::get_zero(IEEE_QUAD, false)),
        _ => None,
    }
}

/// The constant of type `ty` (an `fp128`) holding `number`.
pub fn constant(
    context: &Context,
    ty: TypeId,
    number: &ApFloat,
) -> ConstantId {
    context.constant(Constant { ty, kind: ConstantKind::Float128(number.to_bits()) })
}

/// `left op right` in binary128, rounding to nearest even; none for a NaN, an
/// infinity, a division by zero or a result that is not finite (the same
/// narrowing as `fp80::binary`).
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

/// `a * b + c` when the product is exact, so that the fused and the unfused
/// answers are one number. (`APFloat::fusedMultiplyAdd` is not ported.)
pub fn multiply_add_exact(
    a: &ApFloat,
    b: &ApFloat,
    c: &ApFloat,
) -> Option<ApFloat> {
    if !a.is_finite() || !b.is_finite() || !c.is_finite() {
        return None;
    }
    let mut product = *a;
    let status = product.multiply(b, RoundingMode::NearestTiesToEven);
    if status != 0 {
        return None;
    }
    binary(BinaryOp::FAdd, &product, c)
}

/// The integer `number` rounds to in `mode`, when a signed or unsigned
/// `width`-bit integer holds it (`width` is at most 127); none for a NaN, an
/// infinity or a value out of range.
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
    let negative = (bits >> 127) & 1 == 1;
    let exponent = ((bits >> 112) & 0x7fff) as i32;
    let fraction = bits & ((1_u128 << 112) - 1);
    let magnitude: u128 = if exponent == 0 {
        0
    } else {
        let significand = fraction | (1_u128 << 112);
        match exponent - 16383 - 112 {
            ..=-113 => 0,
            shift @ -112..=0 => significand >> -shift,
            shift @ 1..=14 => significand << shift,
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
