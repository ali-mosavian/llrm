//! `isBytewiseValue`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`: the `i8` that,
//! stored at every byte, writes a value (a splat), or none. An `i8` value is
//! its own splat; a constant is one when all its bytes are equal, an
//! aggregate when its members' splats agree. Where MIR lacks what the C++
//! reads, the case is skipped:
//! - `undef`: MIR has only `poison`, which stands in the `isa<UndefValue>`
//!   branches (LLVM's poison is an `UndefValue`).
//! - `half` (6762): MIR has no `half` type. `x86_fp80` is the "long double" the
//!   C++ declines.
//! - The constant fold of an `inttoptr` operand (`ConstantFoldIntegerCast`) is
//!   done for an integer constant only.
//!
//! # Example
//! `i32 -1` is the byte `i8 -1`; `i32 1` has none.

use super::query::Query;
use crate::context::{ConstantExpr, ConstantKind, mask};
use crate::module::Operand;
use crate::opcode::CastOp;
use crate::types::{FloatKind, Type, TypeId};

/// What `isBytewiseValue` returns (a `Value *`). Constants are uniqued in
/// LLVM, so one byte constant is one value here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Byte {
    /// The `i8` value that was asked, which is its own splat.
    Value(Operand),
    /// `UndefInt8`: any byte will do.
    Undef,
    /// `PoisonValue` of `i8`: the splat of a value that has no bytes, or an
    /// `i8` poison.
    Poison,
    /// A constant `i8`.
    Constant(u8),
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L6734>
pub fn is_bytewise_value(
    q: &Query,
    v: Operand,
) -> Option<Byte> {
    let ty = q.type_of(v)?;
    let types = &q.context.types;
    let kind = match v {
        Operand::Constant(id) => Some(&q.context.get(id).kind),
        _ => None,
    };
    // All byte-wide stores are splatable, even of arbitrary variables.
    if matches!(types.get(ty), Type::Int(8)) {
        return Some(match kind {
            Some(ConstantKind::Int(bits)) => Byte::Constant(*bits as u8),
            Some(ConstantKind::Zero | ConstantKind::Null) => Byte::Constant(0),
            Some(ConstantKind::Poison) => Byte::Poison,
            Some(ConstantKind::Undef) => Byte::Undef,
            _ => Byte::Value(v),
        });
    }

    // Undef don't care.
    if matches!(kind, Some(ConstantKind::Poison | ConstantKind::Undef)) {
        return Some(Byte::Undef);
    }

    // Return poison for zero-sized type.
    if q.layout.store_size(types, ty) == 0 {
        return Some(Byte::Poison);
    }

    let Operand::Constant(_) = v else { return None };
    constant(q, ty, kind?)
}

fn constant(
    q: &Query,
    ty: TypeId,
    kind: &ConstantKind,
) -> Option<Byte> {
    let types = &q.context.types;
    // Handle 'null' ConstantArrayZero etc.
    let null = match kind {
        ConstantKind::Null | ConstantKind::Zero => true,
        ConstantKind::Int(bits) => *bits & mask(width(q, ty)) == 0,
        ConstantKind::Float(bits) => *bits == 0,
        _ => false,
    };
    if null {
        return Some(Byte::Constant(0));
    }

    match kind {
        // Constant floating-point values can be handled as integer values if
        // the corresponding integer value is "byteable". An important case is
        // 0.0. Don't handle long double formats, which have strange
        // constraints.
        ConstantKind::Float(bits) => match types.get(ty) {
            Type::Float(FloatKind::Float) => splat(u128::from(*bits as u32), 32),
            Type::Float(FloatKind::Double) => splat(u128::from(*bits), 64),
            _ => None,
        },
        // We can handle constant integers that are multiple of 8 bits.
        ConstantKind::Int(bits) => splat(*bits, width(q, ty)),
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) => {
            let Type::Pointer(space) = types.get(ty) else { return None };
            let pointer_bits = q.layout.pointer(*space).bits;
            let operand = q.context.get(*value);
            let ConstantKind::Int(bits) = operand.kind else { return None };
            // Zero extended or truncated, as `ConstantFoldIntegerCast(false)`.
            splat(bits & mask(width(q, operand.ty)) & mask(pointer_bits), pointer_bits)
        }
        // ConstantDataSequential.
        ConstantKind::Bytes(bytes) => {
            let mut acc = Byte::Undef;
            for &byte in bytes {
                acc = merge(acc, Byte::Constant(byte))?;
            }
            Some(acc)
        }
        // ConstantAggregate.
        ConstantKind::Aggregate(members) => {
            let mut acc = Byte::Undef;
            for &member in members {
                acc = merge(acc, is_bytewise_value(q, Operand::Constant(member))?)?;
            }
            Some(acc)
        }
        // Don't try to handle the handful of other constants.
        _ => None,
    }
}

fn width(
    q: &Query,
    ty: TypeId,
) -> u32 {
    match q.context.types.get(ty) {
        Type::Int(bits) => *bits,
        _ => 0,
    }
}

/// `CI->getBitWidth() % 8 == 0` and `isSplat(8)`: the byte.
fn splat(
    bits: u128,
    width: u32,
) -> Option<Byte> {
    if width == 0 || width % 8 != 0 {
        return None;
    }
    let byte = bits as u8;
    (0..width / 8).all(|at| (bits >> (8 * at)) as u8 == byte).then_some(Byte::Constant(byte))
}

/// The `Merge` lambda.
fn merge(
    left: Byte,
    right: Byte,
) -> Option<Byte> {
    if left == right {
        return Some(left);
    }
    match (left, right) {
        (Byte::Undef, right) => Some(right),
        (left, Byte::Undef) => Some(left),
        _ => None,
    }
}
