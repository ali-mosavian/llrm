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
//! The comparison of two floating constants is `compare_float`.
//! DEVIATE: no vectors, no floating-point arithmetic
//! (`FAdd`...: `ConstantFoldBinaryInstruction`'s FP cases), no constant
//! expressions; an operation with `nuw`/`nsw`/`exact` is not folded
//! (LLVM's `ConstantExpr::get(Opc, L, R, Flags)` makes it poison on overflow;
//! the builder makes the instruction and a later pass decides).
//!
//! # Used by
//! `build::Builder::{binary, icmp, fcmp, cast, select}`; `constantfolding` for
//! `FCmp`.

use std::cmp::Ordering;

use crate::apfloat::{CmpResult, IEEE_DOUBLE, IEEE_SINGLE};
use crate::context::{Constant, ConstantExpr, ConstantId, ConstantKind, Context, mask, signed};
use crate::fp80;
use crate::fpclass::RoundingMode;
use crate::module::{GlobalKind, GlobalValue, Linkage, UnnamedAddr};
use crate::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate};
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

/// `Constant::isNullValue`.
pub(crate) fn is_null_value(
    context: &Context,
    id: ConstantId,
) -> bool {
    match &context.get(id).kind {
        ConstantKind::Int(bits) => *bits == 0,
        ConstantKind::Float(bits) => *bits == 0,
        ConstantKind::Null | ConstantKind::Zero => true,
        ConstantKind::Aggregate(members) => members.iter().all(|&one| is_null_value(context, one)),
        ConstantKind::Bytes(bytes) => bytes.iter().all(|&one| one == 0),
        _ => false,
    }
}

/// `Constant::getNullValue`.
pub fn null_value(
    context: &Context,
    ty: TypeId,
) -> ConstantId {
    let kind = match context.types.get(ty) {
        Type::Int(_) => ConstantKind::Int(0),
        Type::Float(_) => ConstantKind::Float(0),
        Type::Pointer(_) => ConstantKind::Null,
        _ => ConstantKind::Zero,
    };
    context.constant(Constant { ty, kind })
}

fn is_poison(
    context: &Context,
    id: ConstantId,
) -> bool {
    context.get(id).kind == ConstantKind::Poison
}

fn poison(
    context: &Context,
    ty: TypeId,
) -> ConstantId {
    context.constant(Constant { ty, kind: ConstantKind::Poison })
}

/// `isa<UndefValue>(C) && !isa<PoisonValue>(C)`: the callers test poison first.
fn is_undef(
    context: &Context,
    id: ConstantId,
) -> bool {
    context.get(id).kind == ConstantKind::Undef
}

fn undef(
    context: &Context,
    ty: TypeId,
) -> ConstantId {
    context.constant(Constant { ty, kind: ConstantKind::Undef })
}

/// `ConstantFoldBinaryInstruction`
/// ([ConstantFold.cpp:637](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L637)).
pub fn binary(
    context: &Context,
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
    if is_undef(context, c1) || is_undef(context, c2) {
        return Some(binary_with_undef(context, op, c1, c2, ty, width));
    }
    // A number on the right is an absorber (`X * 0`, `X & 0`, `X | -1`) or
    // makes `X / 0`, `X % 0` poison and `X % 1` zero whatever `X` is. A number
    // on the left of a commutative operation is swapped over to the right; on
    // the left of the others it is an absorber for `0 << X`, `0 / X`...
    let absorber = match op {
        BinaryOp::Mul | BinaryOp::And => Some(0),
        BinaryOp::Or => Some(ones(width)),
        _ => None,
    };
    let left_absorber = matches!(
        op,
        BinaryOp::Shl
            | BinaryOp::LShr
            | BinaryOp::AShr
            | BinaryOp::SDiv
            | BinaryOp::UDiv
            | BinaryOp::URem
            | BinaryOp::SRem
    );
    let commutative = matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
    );
    if let Some(y) = y {
        if absorber == Some(y) {
            return Some(c2);
        }
        match op {
            BinaryOp::UDiv | BinaryOp::SDiv if y == 0 => return Some(poison(context, ty)),
            BinaryOp::URem | BinaryOp::SRem if y == 1 => {
                return Some(context.constant(Constant { ty, kind: ConstantKind::Int(0) }));
            }
            BinaryOp::URem | BinaryOp::SRem if y == 0 => return Some(poison(context, ty)),
            _ => {}
        }
    } else if let Some(x) = x {
        if (commutative && absorber == Some(x)) || (left_absorber && x == 0) {
            return Some(c1);
        }
    }
    let (x, y) = (x?, y?);
    let folded = match op {
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

/// The `HasScalarUndefOrScalableVectorUndef` switch of
/// `ConstantFoldBinaryInstruction`
/// ([ConstantFold.cpp:659](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFold.cpp#L659)):
/// each operation by what its undef operand may be chosen as. The
/// `FSub`..`FRem` cases are DEVIATE: `binary` folds integers only.
fn binary_with_undef(
    context: &Context,
    op: BinaryOp,
    c1: ConstantId,
    c2: ConstantId,
    ty: TypeId,
    width: u32,
) -> ConstantId {
    let (u1, u2) = (is_undef(context, c1), is_undef(context, c2));
    let null = |context: &Context| context.constant(Constant { ty, kind: ConstantKind::Int(0) });
    match op {
        BinaryOp::Xor if u1 && u2 => null(context),
        BinaryOp::Xor | BinaryOp::Add | BinaryOp::Sub => undef(context, ty),
        BinaryOp::And if u1 && u2 => c1,
        BinaryOp::And => null(context),
        BinaryOp::Mul if u1 && u2 => c1,
        BinaryOp::Mul => {
            // X * undef is undef for odd X, else 0.
            let odd = number(context, c1).or_else(|| number(context, c2)).is_some_and(|(x, _)| x & 1 == 1);
            if odd { undef(context, ty) } else { null(context) }
        }
        BinaryOp::SDiv | BinaryOp::UDiv | BinaryOp::URem | BinaryOp::SRem => {
            // X / undef and X / 0 are poison.
            if u2 || number(context, c2).is_some_and(|(y, _)| y == 0) { poison(context, ty) } else { null(context) }
        }
        BinaryOp::Or if u1 && u2 => c1,
        BinaryOp::Or => context.constant(Constant { ty, kind: ConstantKind::Int(mask(width)) }),
        // The pin's `Shl` comment says undef but returns poison.
        BinaryOp::LShr | BinaryOp::AShr | BinaryOp::Shl if u2 => poison(context, ty),
        BinaryOp::LShr | BinaryOp::AShr | BinaryOp::Shl => null(context),
        _ => unreachable!("an integer operation"),
    }
}

/// Whether `id` is of type `x86_fp80`.
fn is_x87(
    context: &Context,
    id: ConstantId,
) -> bool {
    matches!(
        context.types.get(context.get(id).ty),
        Type::Float(FloatKind::X86Fp80)
    )
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
    context: &Context,
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
    context: &Context,
    op: CastOp,
    c: ConstantId,
    to: TypeId,
) -> Option<ConstantId> {
    if is_poison(context, c) {
        return Some(poison(context, to));
    }
    if is_undef(context, c) {
        // zext, sext and [us]itofp of undef are 0: the result is bounded.
        let bounded = matches!(
            op,
            CastOp::ZExt | CastOp::SExt | CastOp::UIToFP | CastOp::SIToFP
        );
        return Some(if bounded { null_value(context, to) } else { undef(context, to) });
    }
    if is_null_value(context, c) && op != CastOp::AddrSpaceCast {
        return Some(null_value(context, to));
    }
    let dest = context.types.get(to).clone();
    match (op, dest) {
        // The bits of a number are those of the float of the same width.
        (CastOp::BitCast, Type::Float(kind @ (FloatKind::Float | FloatKind::Double))) => {
            let (x, from) = number(context, c)?;
            if from != if kind == FloatKind::Float { 32 } else { 64 } {
                return None;
            }
            Some(context.constant(Constant { ty: to, kind: ConstantKind::Float(x as u64) }))
        }
        (CastOp::BitCast, Type::Int(width @ (32 | 64))) => {
            let ConstantKind::Float(bits) = context.get(c).kind else { return None };
            let float = match context.types.get(context.get(c).ty) {
                Type::Float(FloatKind::Float) => 32,
                Type::Float(FloatKind::Double) => 64,
                _ => return None,
            };
            (float == width).then(|| context.constant(Constant { ty: to, kind: ConstantKind::Int(u128::from(bits)) }))
        }
        (CastOp::FPTrunc | CastOp::FPRound, Type::Float(kind @ (FloatKind::Float | FloatKind::Double)))
            if is_x87(context, c) =>
        {
            let mut number = fp80::value(&context.get(c).kind)?;
            if !number.is_finite() {
                return None;
            }
            let semantics = if kind == FloatKind::Float { IEEE_SINGLE } else { IEEE_DOUBLE };
            number.convert(semantics, RoundingMode::NearestTiesToEven);
            (number.is_finite())
                .then(|| context.constant(Constant { ty: to, kind: ConstantKind::Float(number.to_bits() as u64) }))
        }
        (CastOp::FPToSI | CastOp::FPToUI, Type::Int(width)) if width <= 127 && is_x87(context, c) => {
            let number = fp80::value(&context.get(c).kind)?;
            // Out of range or not a number is poison, as for the other formats.
            let Some(value) = fp80::to_integer(&number, RoundingMode::TowardZero, width, op == CastOp::FPToSI) else {
                return Some(poison(context, to));
            };
            Some(context.constant(Constant { ty: to, kind: ConstantKind::Int(value as u128 & mask(width)) }))
        }
        (CastOp::SIToFP | CastOp::UIToFP, Type::Float(FloatKind::X86Fp80)) => {
            let (x, from) = number(context, c)?;
            let (magnitude, negative) = if op == CastOp::SIToFP && signed(x, from) < 0 {
                (signed(x, from).unsigned_abs(), true)
            } else if op == CastOp::SIToFP {
                (signed(x, from) as u128, false)
            } else {
                (x, false)
            };
            let magnitude = u64::try_from(magnitude).ok()?;
            Some(fp80::constant(context, to, &fp80::from_integer(magnitude, negative)))
        }
        (CastOp::FPTrunc | CastOp::FPExt | CastOp::FPRound, Type::Float(kind)) => {
            let (from, value) = float_number(context, c)?;
            // An x87 constant is held as a double, and a NaN's payload and
            // quiet bit are `APFloat::convert`'s: neither is folded. Widening a
            // float or a double to x86_fp80 is exact, and held as the double.
            let widened = op == CastOp::FPExt && kind == FloatKind::X86Fp80;
            if from == FloatKind::X86Fp80 || (kind == FloatKind::X86Fp80 && !widened) || value.is_nan() {
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
/// ([ConstantFold.cpp:1135](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L1135))
/// for a caller that has no module: a global is a pointer nothing is known of.
pub fn compare(
    context: &Context,
    predicate: IntPredicate,
    c1: ConstantId,
    c2: ConstantId,
) -> Option<ConstantId> {
    compare_in(context, &[], predicate, c1, c2)
}

/// `ConstantFoldCompareInstruction`'s floating-point cases: `FCMP_FALSE` and
/// `FCMP_TRUE` first, poison, an undef operand (a NaN is chosen for it, which
/// makes an unordered predicate pass and an ordered one fail), then
/// `FCmpInst::compare` of two numbers. DEVIATE: an `x86_fp80` constant is held
/// as the nearest double, so a compare of two is not folded.
pub fn compare_float(
    context: &Context,
    predicate: FloatPredicate,
    c1: ConstantId,
    c2: ConstantId,
) -> Option<ConstantId> {
    let i1 = context.types.int(1);
    let result = |held: bool| context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(held)) });
    if matches!(predicate, FloatPredicate::False | FloatPredicate::True) {
        return Some(result(predicate == FloatPredicate::True));
    }
    if is_poison(context, c1) || is_poison(context, c2) {
        return Some(poison(context, i1));
    }
    if is_undef(context, c1) || is_undef(context, c2) {
        return Some(result(predicate.holds(None)));
    }
    if is_x87(context, c1) {
        let (a, b) = (fp80::value(&context.get(c1).kind)?, fp80::value(&context.get(c2).kind)?);
        let order = match a.compare(&b) {
            CmpResult::LessThan => Some(Ordering::Less),
            CmpResult::Equal => Some(Ordering::Equal),
            CmpResult::GreaterThan => Some(Ordering::Greater),
            CmpResult::Unordered => None,
        };
        return Some(result(predicate.holds(order)));
    }
    let ((kind, x), (_, y)) = (float_number(context, c1)?, float_number(context, c2)?);
    if kind == FloatKind::X86Fp80 {
        return None;
    }
    Some(result(predicate.holds(x.partial_cmp(&y))))
}

/// `ConstantFoldFPInstOperands`' arithmetic
/// ([ConstantFolding.cpp:1574](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/ConstantFolding.cpp#L1574)):
/// `fadd`, `fsub`, `fmul`, `fdiv` and `frem` of two numbers of one format.
///
/// DEVIATE (narrower than the pin, docs/working/fp-constant-fold-plan.md): the
/// pin rounds to the format; QB, Nib and C on an x87 target evaluate wider, so
/// an inexact fold could change a printed value. This folds only when the
/// result is exact in the format, which every evaluation width at least the
/// format's reproduces. It refuses a non-finite operand or result, an
/// `x86_fp80`, an inexact result, and (as the pin does when non-deterministic
/// results are not allowed) `nsz`, `reassoc`, `contract` and `arcp`. Exit: a
/// "strict float width" fact in the profile, true only under the
/// standard-precision flag (every assignment rounded), where the pin's fold in
/// the format is faithful.
pub fn float_binary(
    context: &Context,
    op: BinaryOp,
    flags: Flags,
    c1: ConstantId,
    c2: ConstantId,
) -> Option<ConstantId> {
    if flags.contains(Flags::NSZ)
        || flags.contains(Flags::REASSOC)
        || flags.contains(Flags::CONTRACT)
        || flags.contains(Flags::ARCP)
    {
        return None;
    }
    if is_x87(context, c1) {
        let (a, b) = (fp80::value(&context.get(c1).kind)?, fp80::value(&context.get(c2).kind)?);
        let ty = context.get(c1).ty;
        return Some(fp80::constant(context, ty, &fp80::binary(op, &a, &b)?));
    }
    let ((kind, x), (kind2, y)) = (float_number(context, c1)?, float_number(context, c2)?);
    if kind != kind2 || kind == FloatKind::X86Fp80 || !x.is_finite() || !y.is_finite() {
        return None;
    }
    // Operands of an f32 are exact in f64; an f32 result must also be exact.
    let r = match op {
        BinaryOp::FAdd | BinaryOp::FSub => {
            let y = if op == BinaryOp::FSub { -y } else { y };
            let s = x + y;
            let bb = s - x;
            let err = (x - (s - bb)) + (y - bb);
            if err != 0.0 {
                return None;
            }
            s
        }
        BinaryOp::FMul => {
            let p = x * y;
            if x.mul_add(y, -p) != 0.0 {
                return None;
            }
            p
        }
        BinaryOp::FDiv => {
            if y == 0.0 {
                return None;
            }
            let q = x / y;
            if q.mul_add(y, -x) != 0.0 {
                return None;
            }
            q
        }
        BinaryOp::FRem => {
            if y == 0.0 {
                return None;
            }
            x % y
        }
        _ => return None,
    };
    if !r.is_finite() || (kind == FloatKind::Float && f64::from(r as f32) != r) {
        return None;
    }
    let ty = context.get(c1).ty;
    Some(float_constant(context, ty, kind, r))
}

/// What `evaluateICmpRelation` finds of two pointer constants (an
/// `ICmpInst::Predicate`; the others it names cannot come out of its cases).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Relation {
    Eq,
    Ne,
    Ugt,
    Ult,
}

impl Relation {
    fn swapped(self) -> Self {
        match self {
            Self::Ugt => Self::Ult,
            Self::Ult => Self::Ugt,
            other => other,
        }
    }
}

fn global_of<'a>(
    context: &Context,
    globals: &'a [GlobalValue],
    id: ConstantId,
) -> Option<&'a GlobalValue> {
    match context.get(id).kind {
        ConstantKind::Global(global) => globals.get(global.0 as usize),
        _ => None,
    }
}

/// `Type::isEmptyTy`.
fn is_empty_type(
    context: &Context,
    ty: TypeId,
) -> bool {
    match context.types.get(ty) {
        Type::Struct { fields, .. } => fields.iter().all(|&field| is_empty_type(context, field)),
        Type::Named(name) => {
            context.types.body(name).is_some_and(|body| body.fields.iter().all(|&field| is_empty_type(context, field)))
        }
        Type::Array { element, count } => *count == 0 || is_empty_type(context, *element),
        _ => false,
    }
}

/// `areGlobalsPotentiallyEqual`
/// ([ConstantFold.cpp:996](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L996)).
/// DEVIATE: MIR has no `GlobalAlias` and no semantic-interposition module flag
/// (`isInterposable` is the linkage test alone).
fn globals_potentially_equal(
    context: &Context,
    a: &GlobalValue,
    b: &GlobalValue,
) -> Option<Relation> {
    let unsafe_for_equality = |global: &GlobalValue| {
        let interposable =
            matches!(
                global.linkage,
                Linkage::Weak | Linkage::LinkOnce | Linkage::Common | Linkage::ExternWeak
            );
        if interposable || global.unnamed_addr == UnnamedAddr::Global {
            return true;
        }
        match &global.kind {
            GlobalKind::Variable(variable) => {
                let sized = !matches!(
                    context.types.get(variable.ty),
                    Type::Void | Type::Label | Type::Metadata | Type::Token | Type::Function { .. }
                );
                !sized || is_empty_type(context, variable.ty)
            }
            GlobalKind::Function(_) => false,
        }
    };
    (!unsafe_for_equality(a) && !unsafe_for_equality(b)).then_some(Relation::Ne)
}

/// `evaluateICmpRelation`
/// ([ConstantFold.cpp:1026](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L1026)).
/// DEVIATE: MIR has no `BlockAddress`.
fn evaluate_icmp_relation(
    context: &Context,
    globals: &[GlobalValue],
    v1: ConstantId,
    v2: ConstantId,
) -> Option<Relation> {
    if v1 == v2 {
        return Some(Relation::Eq);
    }
    if !matches!(context.types.get(context.get(v1).ty), Type::Pointer(_)) {
        return None;
    }
    let complexity = |id: ConstantId| match context.get(id).kind {
        ConstantKind::Expr(_) => 3,
        ConstantKind::Global(_) => 2,
        _ => 0,
    };
    if complexity(v1) < complexity(v2) {
        return evaluate_icmp_relation(context, globals, v2, v1).map(Relation::swapped);
    }
    let null_is_defined = |global: &GlobalValue| global.address_space != 0;
    if let Some(global) = global_of(context, globals, v1) {
        if let Some(other) = global_of(context, globals, v2) {
            return globals_potentially_equal(context, global, other);
        }
        if is_null_value(context, v2) && global.linkage != Linkage::ExternWeak && !null_is_defined(global) {
            return Some(Relation::Ugt);
        }
        return None;
    }
    let ConstantKind::Expr(ConstantExpr::GetElementPtr { inbounds, operands, .. }) = &context.get(v1).kind else {
        return None;
    };
    let base = *operands.first()?;
    let all_zero = |operands: &[ConstantId]| operands[1..].iter().all(|&index| is_null_value(context, index));
    let gep_base = |id: ConstantId| match &context.get(id).kind {
        ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => operands.first().copied(),
        _ => None,
    };
    if is_null_value(context, v2) {
        let global = global_of(context, globals, base)?;
        return (global.linkage != Linkage::ExternWeak && *inbounds).then_some(Relation::Ugt);
    }
    if let Some(other) = global_of(context, globals, v2) {
        let global = global_of(context, globals, base)?;
        return if base != v2 && all_zero(operands) { globals_potentially_equal(context, global, other) } else { None };
    }
    let other_base = gep_base(v2)?;
    let (global, other) = (global_of(context, globals, base)?, global_of(context, globals, other_base)?);
    let ConstantKind::Expr(ConstantExpr::GetElementPtr { operands: other_operands, .. }) = &context.get(v2).kind else {
        return None;
    };
    if base != other_base && all_zero(operands) && all_zero(other_operands) {
        return globals_potentially_equal(context, global, other);
    }
    None
}

/// `ConstantFoldCompareInstruction`
/// ([ConstantFold.cpp:1135](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L1135))
/// of the module whose `globals` the constants name. DEVIATE: no
/// vectors, and `i1` equality, which the C++ makes an `xor` expression
/// (#2252); the floating-point cases are [`compare_float`].
pub fn compare_in(
    context: &Context,
    globals: &[GlobalValue],
    predicate: IntPredicate,
    c1: ConstantId,
    c2: ConstantId,
) -> Option<ConstantId> {
    let i1 = context.types.int(1);
    if is_poison(context, c1) || is_poison(context, c2) {
        return Some(poison(context, i1));
    }
    if is_undef(context, c1) || is_undef(context, c2) {
        // Equality, or both undef: undef; else the answer for the undef equal
        // to X.
        if matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) || c1 == c2 {
            return Some(undef(context, i1));
        }
        let true_when_equal =
            matches!(
                predicate,
                IntPredicate::Uge | IntPredicate::Ule | IntPredicate::Sge | IntPredicate::Sle
            );
        return Some(context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(true_when_equal)) }));
    }
    // `X uge 0` is true and `X ult 0` false, whatever X is.
    if is_null_value(context, c2) && matches!(predicate, IntPredicate::Uge | IntPredicate::Ult) {
        return Some(
            context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(predicate == IntPredicate::Uge)) }),
        );
    }
    if let (Some((x, width)), Some((y, _))) = (number(context, c1), number(context, c2)) {
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
        return Some(context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(held)) }));
    }
    if let Some(relation) = evaluate_icmp_relation(context, globals, c1, c2) {
        use IntPredicate::*;
        let result = match (relation, predicate) {
            (Relation::Eq, p) => Some(matches!(p, Eq | Uge | Ule | Sge | Sle)),
            (Relation::Ne, Eq) => Some(false),
            (Relation::Ne, Ne) => Some(true),
            (Relation::Ult, Ult | Ne | Ule) | (Relation::Ugt, Ugt | Ne | Uge) => Some(true),
            (Relation::Ult, Ugt | Eq | Uge) | (Relation::Ugt, Ult | Eq | Ule) => Some(false),
            _ => None,
        };
        if let Some(held) = result {
            return Some(context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(held)) }));
        }
    }
    let is_expr = |id: ConstantId| matches!(context.get(id).kind, ConstantKind::Expr(_));
    if (!is_expr(c1) && is_expr(c2)) || (is_null_value(context, c1) && !is_null_value(context, c2)) {
        return compare_in(context, globals, predicate.swapped(), c2, c1);
    }
    None
}

/// `ConstantFoldSelectInstruction`
/// ([ConstantFold.cpp:295](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/ConstantFold.cpp#L295)):
/// a condition that is a number picks an arm, a poison one is poison, an undef
/// one is the false arm unless the true arm is undef.
pub fn select(
    context: &Context,
    condition: ConstantId,
    if_true: ConstantId,
    if_false: ConstantId,
) -> Option<ConstantId> {
    if is_poison(context, condition) {
        let ty = context.get(if_true).ty;
        return Some(poison(context, ty));
    }
    if is_undef(context, condition) {
        return Some(if is_undef(context, if_true) { if_true } else { if_false });
    }
    if let Some((bits, _)) = number(context, condition) {
        return Some(if bits & 1 == 1 { if_true } else { if_false });
    }
    if if_true == if_false {
        return Some(if_true);
    }
    if is_poison(context, if_true) {
        return Some(if_false);
    }
    if is_poison(context, if_false) {
        return Some(if_true);
    }
    // An undef arm gives the other arm if that cannot be poison.
    if is_undef(context, if_true) && not_poison(context, if_false) {
        return Some(if_false);
    }
    if is_undef(context, if_false) && not_poison(context, if_true) {
        return Some(if_true);
    }
    None
}

/// The `NotPoison` lambda of `ConstantFoldSelectInstruction`: what is known no
/// poison, as a number, a global, a null or a float.
fn not_poison(
    context: &Context,
    id: ConstantId,
) -> bool {
    let constant = context.get(id);
    match &constant.kind {
        ConstantKind::Poison | ConstantKind::Expr(_) => false,
        ConstantKind::Int(_) | ConstantKind::Float(_) | ConstantKind::Null | ConstantKind::Global(_) => true,
        ConstantKind::Aggregate(members) if matches!(context.types.get(constant.ty), Type::Vector { .. }) => members
            .iter()
            .all(|&one| !is_poison(context, one) && !matches!(context.get(one).kind, ConstantKind::Expr(_))),
        ConstantKind::Zero => matches!(context.types.get(constant.ty), Type::Vector { .. }),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ConstantExpr, GlobalId};

    fn unknown(
        context: &Context,
        width: u32,
    ) -> ConstantId {
        let ptr = context.types.ptr(0);
        let global = context.constant(Constant { ty: ptr, kind: ConstantKind::Global(GlobalId(0)) });
        let ty = context.types.int(width);
        context.constant(Constant {
            ty,
            kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::PtrToInt, value: global }),
        })
    }

    fn number(
        context: &Context,
        width: u32,
        value: u128,
    ) -> ConstantId {
        let ty = context.types.int(width);
        context.constant(Constant { ty, kind: ConstantKind::Int(value) })
    }

    /// `X * 0`, `X | -1`, `0 << X`, `0 / X`, `X % 1` and `X / 0` fold with `X`
    /// not a number; they stayed unfolded (the builder made the
    /// instruction) because the number test came first.
    #[test]
    fn a_number_absorbs_or_poisons_an_operand_that_is_not_one() {
        let mut context = Context::new();
        let (x, zero, one, ones) = (
            unknown(&mut context, 32),
            number(&mut context, 32, 0),
            number(&mut context, 32, 1),
            number(&mut context, 32, u32::MAX.into()),
        );
        assert_eq!(binary(&mut context, BinaryOp::Mul, x, zero), Some(zero));
        assert_eq!(binary(&mut context, BinaryOp::Mul, zero, x), Some(zero));
        assert_eq!(binary(&mut context, BinaryOp::Or, x, ones), Some(ones));
        assert_eq!(binary(&mut context, BinaryOp::Shl, zero, x), Some(zero));
        assert_eq!(binary(&mut context, BinaryOp::LShr, zero, x), Some(zero));
        assert_eq!(binary(&mut context, BinaryOp::UDiv, zero, x), Some(zero));
        assert_eq!(binary(&mut context, BinaryOp::URem, x, one), Some(zero));
        let poisoned = binary(&mut context, BinaryOp::UDiv, x, zero).expect("x / 0");
        assert!(is_poison(&context, poisoned));
        assert_eq!(binary(&mut context, BinaryOp::Shl, x, zero), Some(x), "X << 0 is X");
        assert_eq!(binary(&mut context, BinaryOp::Sub, zero, x), None, "0 - X is not a number");
    }

    /// A null value of any type casts to the null value of the destination, a
    /// pointer null to an integer included; and the bits of a number are
    /// those of the float of its width.
    #[test]
    fn a_null_casts_to_a_null_and_the_bits_of_an_integer_are_those_of_the_float() {
        let mut context = Context::new();
        let (ptr, i32t, f32t) =
            (context.types.ptr(0), context.types.int(32), context.types.intern(Type::Float(FloatKind::Float)));
        let null = context.constant(Constant { ty: ptr, kind: ConstantKind::Null });
        let made = cast(&mut context, CastOp::PtrToInt, null, i32t).expect("ptrtoint null");
        assert_eq!(context.get(made).kind, ConstantKind::Int(0));
        let one = number(&mut context, 32, 0x3f80_0000);
        let float = cast(&mut context, CastOp::BitCast, one, f32t).expect("bitcast to float");
        assert_eq!(context.get(float).kind, ConstantKind::Float(0x3f80_0000));
        let back = cast(&mut context, CastOp::BitCast, float, i32t).expect("bitcast to int");
        assert_eq!(back, one);
    }
}

#[cfg(test)]
mod undef_tests {
    use super::*;

    fn int(
        context: &mut Context,
        value: u128,
    ) -> ConstantId {
        let ty = context.types.int(16);
        context.constant(Constant { ty, kind: ConstantKind::Int(value) })
    }

    fn undef(context: &mut Context) -> ConstantId {
        let ty = context.types.int(16);
        context.constant(Constant { ty, kind: ConstantKind::Undef })
    }

    fn kind(
        context: &Context,
        id: Option<ConstantId>,
    ) -> ConstantKind {
        context.get(id.expect("folded")).kind.clone()
    }

    /// Old: an operation on an `undef` constant was not folded at all
    /// (`undef & 5` stayed an instruction). The pin folds each by what the
    /// undef may be: `undef & X` is 0, `undef | X` is -1, `undef + X` undef,
    /// `X / undef` poison (ConstantFold.cpp#L670).
    #[test]
    fn a_binary_operation_on_undef_folds_as_the_pin_does() {
        let mut context = Context::new();
        let (u, five, two, three, zero) = (
            undef(&mut context),
            int(&mut context, 5),
            int(&mut context, 2),
            int(&mut context, 3),
            int(&mut context, 0),
        );
        let mut fold = |op, a, b| {
            let folded = binary(&mut context, op, a, b);
            kind(&context, folded)
        };
        assert_eq!(fold(BinaryOp::Add, u, five), ConstantKind::Undef);
        assert_eq!(fold(BinaryOp::Sub, five, u), ConstantKind::Undef);
        assert_eq!(fold(BinaryOp::Xor, u, five), ConstantKind::Undef);
        assert_eq!(fold(BinaryOp::Xor, u, u), ConstantKind::Int(0), "undef ^ undef");
        assert_eq!(fold(BinaryOp::And, u, five), ConstantKind::Int(0));
        assert_eq!(fold(BinaryOp::And, u, u), ConstantKind::Undef);
        assert_eq!(fold(BinaryOp::Or, five, u), ConstantKind::Int(0xffff));
        assert_eq!(fold(BinaryOp::Or, u, u), ConstantKind::Undef);
        assert_eq!(fold(BinaryOp::Mul, u, three), ConstantKind::Undef, "X * undef, X odd");
        assert_eq!(fold(BinaryOp::Mul, u, two), ConstantKind::Int(0), "X * undef, X even");
        assert_eq!(fold(BinaryOp::UDiv, five, u), ConstantKind::Poison);
        assert_eq!(fold(BinaryOp::SRem, five, u), ConstantKind::Poison);
        assert_eq!(fold(BinaryOp::UDiv, u, five), ConstantKind::Int(0));
        assert_eq!(fold(BinaryOp::URem, u, zero), ConstantKind::Poison, "undef % 0");
        assert_eq!(fold(BinaryOp::Shl, five, u), ConstantKind::Poison);
        assert_eq!(fold(BinaryOp::LShr, five, u), ConstantKind::Poison);
        assert_eq!(fold(BinaryOp::Shl, u, five), ConstantKind::Int(0));
        assert_eq!(fold(BinaryOp::AShr, u, five), ConstantKind::Int(0));
    }

    /// `zext undef` and `sext undef` are 0 (the top bits are known), `trunc
    /// undef` undef (ConstantFold.cpp#L163).
    #[test]
    fn a_cast_of_undef_is_zero_when_the_top_bits_are_known() {
        let mut context = Context::new();
        let (u8t, i32t) = (context.types.int(8), context.types.int(32));
        let u = context.constant(Constant { ty: u8t, kind: ConstantKind::Undef });
        let zext = cast(&mut context, CastOp::ZExt, u, i32t);
        let sext = cast(&mut context, CastOp::SExt, u, i32t);
        let u16 = undef(&mut context);
        let trunc = cast(&mut context, CastOp::Trunc, u16, u8t);
        assert_eq!(kind(&context, zext), ConstantKind::Int(0));
        assert_eq!(kind(&context, sext), ConstantKind::Int(0));
        assert_eq!(kind(&context, trunc), ConstantKind::Undef);
        assert_eq!(context.get(trunc.unwrap()).ty, u8t);
    }

    /// `icmp eq undef, X` is undef, any other predicate the answer for the
    /// undef equal to X: `ult` false, `ule` true (ConstantFold.cpp#L1135).
    #[test]
    fn a_comparison_with_undef_is_undef_for_equality_else_as_if_equal() {
        let mut context = Context::new();
        let (u, five) = (undef(&mut context), int(&mut context, 5));
        let mut run = |predicate, a, b| {
            let folded = compare_in(&mut context, &[], predicate, a, b);
            kind(&context, folded)
        };
        assert_eq!(run(IntPredicate::Eq, u, five), ConstantKind::Undef);
        assert_eq!(run(IntPredicate::Ne, five, u), ConstantKind::Undef);
        assert_eq!(run(IntPredicate::Ult, u, five), ConstantKind::Int(0));
        assert_eq!(run(IntPredicate::Ule, u, five), ConstantKind::Int(1));
        assert_eq!(run(IntPredicate::Slt, u, u), ConstantKind::Undef, "both undef");
    }

    /// `select undef, a, b` is b (a if a is undef); with a condition that is
    /// no number, an undef arm gives the other arm unless that may be poison
    /// (ConstantFold.cpp#L295).
    #[test]
    fn a_select_with_undef_picks_the_arm_the_pin_does() {
        let mut context = Context::new();
        let (i1, i16t, ptr) = (context.types.int(1), context.types.int(16), context.types.ptr(0));
        let (u, five, six) = (undef(&mut context), int(&mut context, 5), int(&mut context, 6));
        let poisoned = poison(&mut context, i16t);
        let undef_condition = context.constant(Constant { ty: i1, kind: ConstantKind::Undef });
        assert_eq!(select(&mut context, undef_condition, five, six), Some(six));
        assert_eq!(select(&mut context, undef_condition, u, six), Some(u));
        let global = context.constant(Constant { ty: ptr, kind: ConstantKind::Global(crate::context::GlobalId(0)) });
        let unknown = context.constant(Constant {
            ty: i1,
            kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::PtrToInt, value: global }),
        });
        assert_eq!(select(&mut context, unknown, five, five), Some(five));
        assert_eq!(select(&mut context, unknown, u, five), Some(five));
        assert_eq!(select(&mut context, unknown, five, u), Some(five));
        assert_eq!(select(&mut context, unknown, poisoned, six), Some(six));
        assert_eq!(select(&mut context, unknown, u, poisoned), Some(u), "a poison arm gives the other");
        assert_eq!(select(&mut context, unknown, u, unknown), None, "an expression may be poison");
    }
}

#[cfg(test)]
mod relation_tests {
    use super::*;
    use crate::context::GlobalId;
    use crate::module::GlobalVariable;

    fn variable(
        linkage: Linkage,
        ty: TypeId,
    ) -> GlobalValue {
        GlobalValue {
            name: Default::default(),
            linkage,
            unnamed_addr: UnnamedAddr::None,
            address_space: 0,
            kind: GlobalKind::Variable(GlobalVariable {
                externally_initialized: false,
                ty,
                constant: false,
                initializer: None,
                align: None,
            }),
        }
    }

    fn global(
        context: &Context,
        at: u32,
    ) -> ConstantId {
        let ptr = context.types.ptr(0);
        context.constant(Constant { ty: ptr, kind: ConstantKind::Global(GlobalId(at)) })
    }

    fn held(
        context: &Context,
        folded: Option<ConstantId>,
    ) -> Option<bool> {
        folded.map(|id| context.get(id).kind == ConstantKind::Int(1))
    }

    /// `@a == @b` for two distinct plain globals, `@a ugt null`, and `X uge 0`
    /// folded in ConstantFold.cpp and stayed instructions here, as the
    /// compare knew only numbers.
    #[test]
    fn globals_compare_by_their_linkage_and_a_null_by_its_order() {
        let mut context = Context::new();
        let i32t = context.types.int(32);
        let globals =
            [variable(Linkage::Internal, i32t), variable(Linkage::Internal, i32t), variable(Linkage::Weak, i32t)];
        let ptr = context.types.ptr(0);
        let null = context.constant(Constant { ty: ptr, kind: ConstantKind::Null });
        let (a, b, weak) = (global(&mut context, 0), global(&mut context, 1), global(&mut context, 2));
        let eq = compare_in(&mut context, &globals, IntPredicate::Eq, a, b);
        assert_eq!(held(&context, eq), Some(false));
        let same = compare_in(&mut context, &globals, IntPredicate::Eq, a, a);
        assert_eq!(held(&context, same), Some(true));
        let weak_eq = compare_in(&mut context, &globals, IntPredicate::Eq, a, weak);
        assert_eq!(weak_eq, None, "a weak global may be replaced by the other");
        let ne_null = compare_in(&mut context, &globals, IntPredicate::Ne, null, a);
        assert_eq!(held(&context, ne_null), Some(true), "the null on the left swaps over");
        let weak_null = compare_in(&mut context, &globals, IntPredicate::Ne, weak, null);
        assert_eq!(held(&context, weak_null), Some(true), "weak, not extern_weak: still not null");
        let none = compare_in(&mut context, &[], IntPredicate::Eq, a, b);
        assert_eq!(none, None, "with no module a global is unknown");
        let x = global(&mut context, 0);
        let uge = compare_in(&mut context, &globals, IntPredicate::Uge, x, null);
        assert_eq!(held(&context, uge), Some(true), "X uge 0");
    }
}
