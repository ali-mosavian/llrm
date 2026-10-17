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
//! DEVIATE: no `undef` (MIR has none), no vectors, no floating point, no
//! constant expressions; an operation with `nuw`/`nsw`/`exact` is not folded
//! (LLVM's `ConstantExpr::get(Opc, L, R, Flags)` makes it poison on overflow;
//! the builder makes the instruction and a later pass decides).
//!
//! # Used by
//! `build::Builder::{binary, icmp, cast, select}`.

use crate::context::{Constant, ConstantId, ConstantKind, Context, mask, signed};
use crate::opcode::{BinaryOp, CastOp, IntPredicate};
use crate::types::TypeId;

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

/// `ConstantFoldCastInstruction`
/// ([ConstantFold.cpp:163](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L163)).
pub fn cast(
    context: &mut Context,
    op: CastOp,
    c: ConstantId,
    to: TypeId,
) -> Option<ConstantId> {
    let width = context.types.int_bits(to).filter(|&width| width <= 128)?;
    if is_poison(context, c) {
        return Some(poison(context, to));
    }
    let (x, from) = number(context, c)?;
    let bits = match op {
        CastOp::Trunc if width < from => x,
        CastOp::ZExt if width > from => x,
        CastOp::SExt if width > from => signed(x, from) as u128,
        _ => return None,
    };
    Some(context.constant(Constant { ty: to, kind: ConstantKind::Int(bits & mask(width)) }))
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
