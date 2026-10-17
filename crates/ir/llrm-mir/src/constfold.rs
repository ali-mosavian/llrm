//! Integer constant folding at creation: the part of LLVM's `ConstantFolder`
//! (the `IRBuilder` default) that `llrm` has constants for.
//!
//! A builder asks for an operation of constants and gets a constant, no
//! instruction. Each function translates one function of
//! [`ConstantFold.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp)
//! at d1106deb71cc,
//! `ConstantFolder::FoldBinOp`/`FoldCmp`/`FoldCast`/`FoldSelect`
//! being the callers
//! ([`ConstantFolder.h`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/IR/ConstantFolder.h)).
//!
//! MIRROR: the identity and absorber shortcuts, poison propagation, `X / 0` and
//! `X % 0` as poison, `X % 1` as zero, the arithmetic modulo the width, the
//! comparison of two numbers, the cast of a number, a select on a number.
//! The casts between integers and floats of a constant (`fptrunc`, `fpext`,
//! `fptosi`, `fptoui`, `sitofp`, `uitofp`) are folded at the nearest-even
//! rounding `APFloat` uses; an `x86_fp80` constant is held as the nearest
//! double, so its folds are the double's (DEVIATE: the 64-bit significand).
//! DEVIATE: no `undef` (MIR has none), no vectors, no floating-point arithmetic
//! (`FAdd`...: `ConstantFoldBinaryInstruction`'s FP cases), no constant
//! expressions; an operation with `nuw`/`nsw`/`exact` is not folded
//! (LLVM's `ConstantExpr::get(Opc, L, R, Flags)` makes it poison on overflow;
//! the builder makes the instruction and a later pass decides).
//!
//! # Used by
//! `build::Builder::{binary, icmp, cast, select}`.

use crate::context::{Constant, ConstantId, ConstantKind, Context, mask, signed};
use crate::opcode::{BinaryOp, CastOp, IntPredicate};
use crate::types::{FloatKind, Type, TypeId};

/// A number and its width: `ConstantInt`, and the zero a `Null` or `Zero` is.
fn number(
    context: &Context,
    id: ConstantId,
) -> Option<(u128, u32)> {
    let constant = context.get(id);
    let width = context.types.int_bits(constant.ty).filter(|&width| width <= 128)?;
    match constant.kind {
        ConstantKind::Int(bits) => Some((bits, width)),
        ConstantKind::Null | ConstantKind::Zero => Some((0, width)),
        _ => None,
    }
}

fn is_poison(
    context: &Context,
    id: ConstantId,
) -> bool {
    context.get(id).kind == ConstantKind::Poison
}

fn poison(
    context: &mut Context,
    ty: TypeId,
) -> ConstantId {
    context.constant(Constant { ty, kind: ConstantKind::Poison })
}

/// `ConstantFoldBinaryInstruction`
/// ([ConstantFold.cpp:637](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L637)).
pub fn binary(
    context: &mut Context,
    op: BinaryOp,
    c1: ConstantId,
    c2: ConstantId,
) -> Option<ConstantId> {
    let ty = context.get(c1).ty;
    let ones = |width: u32| mask(width);
    // The identity values first: no-ops that return the other value, a poison
    // one included.
    let (x, width) = number(context, c1).map_or((None, context.types.int_bits(ty)?), |(x, width)| (Some(x), width));
    let y = number(context, c2).map(|(y, _)| y);
    if width > 128 {
        return None;
    }
    let identity = match op {
        BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor => Some((0, true)),
        BinaryOp::Mul => Some((1, true)),
        BinaryOp::And => Some((ones(width), true)),
        BinaryOp::Sub | BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr => Some((0, false)),
        BinaryOp::UDiv | BinaryOp::SDiv => Some((1, false)),
        _ => None,
    };
    if let Some((identity, lhs_too)) = identity {
        if lhs_too && x == Some(identity) {
            return Some(c2);
        }
        if y == Some(identity) {
            return Some(c1);
        }
    }
    // Binary operations propagate poison.
    if is_poison(context, c1) || is_poison(context, c2) {
        return Some(poison(context, ty));
    }
    let (x, y) = (x?, y?);
    // The absorber on the right: `X * 0`, `X & 0`, `X | -1` are the constant
    // itself.
    let absorber = match op {
        BinaryOp::Mul | BinaryOp::And => Some(0),
        BinaryOp::Or => Some(ones(width)),
        _ => None,
    };
    if absorber == Some(y) {
        return Some(c2);
    }
    let folded = match op {
        BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem if y == 0 => {
            return Some(poison(context, ty));
        }
        BinaryOp::URem | BinaryOp::SRem if y == 1 => 0,
        BinaryOp::UDiv => x / y,
        BinaryOp::URem => x % y,
        BinaryOp::SDiv | BinaryOp::SRem => {
            let (sx, sy) = (signed(x, width), signed(y, width));
            // The smallest number over -1 does not fit: poison.
            if sy == -1 && sx == signed(1u128 << (width - 1), width) {
                return Some(poison(context, ty));
            }
            (if op == BinaryOp::SDiv { sx / sy } else { sx % sy }) as u128
        }
        BinaryOp::Add => x.wrapping_add(y),
        BinaryOp::Sub => x.wrapping_sub(y),
        BinaryOp::Mul => x.wrapping_mul(y),
        BinaryOp::And => x & y,
        BinaryOp::Or => x | y,
        BinaryOp::Xor => x ^ y,
        // A shift by the width or more is poison.
        BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr if y >= u128::from(width) => return Some(poison(context, ty)),
        BinaryOp::Shl => x << y,
        BinaryOp::LShr => x >> y,
        BinaryOp::AShr => (signed(x, width) >> y) as u128,
        _ => return None,
    };
    Some(context.constant(Constant { ty, kind: ConstantKind::Int(folded & mask(width)) }))
}

/// A floating constant's value as a double and its kind: `Float` bits are IEEE
/// single in the low 32, `Double` IEEE double, `X86Fp80` the double nearest its
/// value (MIR holds an 80-bit constant that way), and a zero is +0.0.
fn float_number(
    context: &Context,
    id: ConstantId,
) -> Option<(FloatKind, f64)> {
    let constant = context.get(id);
    let Type::Float(kind) = *context.types.get(constant.ty) else { return None };
    match constant.kind {
        ConstantKind::Float(bits) => Some((
            kind,
            match kind {
                FloatKind::Float => f64::from(f32::from_bits(bits as u32)),
                FloatKind::Double | FloatKind::X86Fp80 => f64::from_bits(bits),
            },
        )),
        ConstantKind::Null | ConstantKind::Zero => Some((kind, 0.0)),
        _ => None,
    }
}

/// The constant of `kind` nearest `value`, rounding to nearest even as
/// `APFloat::convert` does (an `X86Fp80` constant is never folded: it is held
/// as the nearest double).
fn float_constant(
    context: &mut Context,
    ty: TypeId,
    kind: FloatKind,
    value: f64,
) -> ConstantId {
    let bits = match kind {
        FloatKind::Float => u64::from((value as f32).to_bits()),
        FloatKind::Double | FloatKind::X86Fp80 => value.to_bits(),
    };
    context.constant(Constant { ty, kind: ConstantKind::Float(bits) })
}

/// `ConstantFoldCastInstruction`
/// ([ConstantFold.cpp:163](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L163)):
/// integer casts, and the casts between integers and floats of a constant.
pub fn cast(
    context: &mut Context,
    op: CastOp,
    c: ConstantId,
    to: TypeId,
) -> Option<ConstantId> {
    if is_poison(context, c) {
        return Some(poison(context, to));
    }
    let dest = context.types.get(to).clone();
    match (op, dest) {
        (CastOp::FPTrunc | CastOp::FPExt | CastOp::FPRound, Type::Float(kind)) => {
            let (from, value) = float_number(context, c)?;
            // An x87 constant is held as a double, and a NaN's payload and
            // quiet bit are `APFloat::convert`'s: neither is
            // folded.
            if from == FloatKind::X86Fp80 || kind == FloatKind::X86Fp80 || value.is_nan() {
                return None;
            }
            Some(float_constant(context, to, kind, value))
        }
        (CastOp::FPToSI | CastOp::FPToUI, Type::Int(width)) if width <= 128 => {
            let (from, value) = float_number(context, c)?;
            if from == FloatKind::X86Fp80 {
                return None;
            }
            // `convertToInteger` reports an invalid operation for NaN and an
            // overflow: poison.
            let value = value.trunc();
            let (low, high) = if op == CastOp::FPToSI {
                (-(2f64.powi(width as i32 - 1)), 2f64.powi(width as i32 - 1))
            } else {
                (0.0, 2f64.powi(width as i32))
            };
            if value.is_nan() || value < low || value >= high {
                return Some(poison(context, to));
            }
            Some(context.constant(Constant { ty: to, kind: ConstantKind::Int((value as i128) as u128 & mask(width)) }))
        }
        (CastOp::SIToFP | CastOp::UIToFP, Type::Float(kind)) if kind != FloatKind::X86Fp80 => {
            let (x, from) = number(context, c)?;
            // Straight from the integer to the target precision: no rounding
            // through a double.
            let bits = match (kind, op) {
                (FloatKind::Float, CastOp::SIToFP) => u64::from((signed(x, from) as f32).to_bits()),
                (FloatKind::Float, _) => u64::from((x as f32).to_bits()),
                (_, CastOp::SIToFP) => (signed(x, from) as f64).to_bits(),
                (_, _) => (x as f64).to_bits(),
            };
            Some(context.constant(Constant { ty: to, kind: ConstantKind::Float(bits) }))
        }
        (_, Type::Int(width)) if width <= 128 => {
            let (x, from) = number(context, c)?;
            let bits = match op {
                CastOp::Trunc if width < from => x,
                CastOp::ZExt if width > from => x,
                CastOp::SExt if width > from => signed(x, from) as u128,
                _ => return None,
            };
            Some(context.constant(Constant { ty: to, kind: ConstantKind::Int(bits & mask(width)) }))
        }
        _ => None,
    }
}

/// `ConstantFoldCompareInstruction`
/// ([ConstantFold.cpp:1135](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L1135)).
pub fn compare(
    context: &mut Context,
    predicate: IntPredicate,
    c1: ConstantId,
    c2: ConstantId,
) -> Option<ConstantId> {
    let i1 = context.types.int(1);
    if is_poison(context, c1) || is_poison(context, c2) {
        return Some(poison(context, i1));
    }
    let ((x, width), (y, _)) = (number(context, c1)?, number(context, c2)?);
    let (sx, sy) = (signed(x, width), signed(y, width));
    let held = match predicate {
        IntPredicate::Eq => x == y,
        IntPredicate::Ne => x != y,
        IntPredicate::Ugt => x > y,
        IntPredicate::Uge => x >= y,
        IntPredicate::Ult => x < y,
        IntPredicate::Ule => x <= y,
        IntPredicate::Sgt => sx > sy,
        IntPredicate::Sge => sx >= sy,
        IntPredicate::Slt => sx < sy,
        IntPredicate::Sle => sx <= sy,
    };
    Some(context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(held)) }))
}

/// `ConstantFoldSelectInstruction`
/// ([ConstantFold.cpp:295](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L295)):
/// a condition that is a number picks an arm, a poison one is poison.
pub fn select(
    context: &mut Context,
    condition: ConstantId,
    if_true: ConstantId,
    if_false: ConstantId,
) -> Option<ConstantId> {
    if is_poison(context, condition) {
        let ty = context.get(if_true).ty;
        return Some(poison(context, ty));
    }
    let (bits, _) = number(context, condition)?;
    Some(if bits & 1 == 1 { if_true } else { if_false })
}
