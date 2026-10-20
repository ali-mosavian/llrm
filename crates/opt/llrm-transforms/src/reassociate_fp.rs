//! Floating constants for Reassociate: the few it makes and reads.
//!
//! DEVIATE: a floating constant is held as the nearest `f32` or `f64`
//! (`floatfold`'s representation), so an `x86_fp80` constant is a double and
//! `ConstantFP::get(Ty, NumFound)` is exact only up to 2^53.
//!
//! # Used by
//! `crate::reassociate_optimize`, `crate::reassociate_negate`.
//!
//! # Example
//! `x * 4.0 * 0.25`: the constants fold to `1.0` (`fold_float`), the identity
//! of `fmul`, so the result is `x`.

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::module::Operand;
use llrm_mir::types::{FloatKind, Type, TypeId};

use crate::floatfold;

fn kind_of(
    context: &Context,
    ty: TypeId,
) -> FloatKind {
    match context.types.get(ty) {
        Type::Float(kind) => *kind,
        other => unreachable!("a float type, not {other:?}"),
    }
}

/// `ConstantFP::get(Ty, value)`.
pub fn float_constant(
    context: &Context,
    ty: TypeId,
    value: f64,
) -> Operand {
    floatfold::float_constant(context, ty, kind_of(context, ty), value)
}

/// The value of a floating constant.
pub fn float_value(
    context: &Context,
    v: Operand,
) -> Option<f64> {
    if let Operand::Constant(id) = v
        && matches!(context.get(id).kind, ConstantKind::Zero | ConstantKind::Null)
        && matches!(context.types.get(context.get(id).ty), Type::Float(_))
    {
        return Some(0.0);
    }
    floatfold::float(context, v).map(|(_, value)| value)
}

/// `ConstantFoldBinaryOpOperands` of `fadd` and `fmul` of two floating
/// constants (`llrm_mir::constfold` has no floating arithmetic): the sum or
/// product in double, rounded once to the type (exact for `f32` by the 2p+2
/// rule, and a double's own for `f64`).
pub fn fold_float(
    context: &Context,
    op: llrm_mir::opcode::BinaryOp,
    a: Operand,
    b: Operand,
) -> Option<Operand> {
    use llrm_mir::opcode::BinaryOp;
    let (x, y) = (float_value(context, a)?, float_value(context, b)?);
    let ty = match a {
        Operand::Constant(id) => context.get(id).ty,
        _ => return None,
    };
    let value = match op {
        BinaryOp::FAdd => x + y,
        BinaryOp::FMul => x * y,
        _ => return None,
    };
    Some(float_constant(context, ty, value))
}
