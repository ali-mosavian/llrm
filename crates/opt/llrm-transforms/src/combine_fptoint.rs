//! InstCombine's `visitFPToUI` and `visitFPToSI`, from
//! [`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp)
//! at d1106deb71cc.
//!
//! [`visit_fptoi`] is
//! [`visitFPToUI`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L2606)
//! and `visitFPToSI` (:2616), the same three steps in the same order:
//! `foldItoFPtoI` (:2469), `foldFPtoI` (:2537), `commonCastTransforms`.
//! `canBeCastedExactlyIntToFP` (:2197) is [`can_be_casted_exactly_int_to_fp`].
//!
//! MIRROR: the order of the steps and each condition.
//!
//! DEVIATE 1: MIR has no vectors, and no saturating variant of
//! `foldItoFPtoI` (`fptosi.sat`) is translated.
//!
//! DEVIATE 2: `foldFPtoI`'s divisor is read through `f32`/`f64`; an `x86_fp80`
//! divisor is left alone.
//!
//! # Example
//! ```text
//! %f = sitofp i16 %x to double           %r = %x
//! %r = fptosi double %f to i16
//! ```
//!
//! # Used by
//! `combine` (its `visit`, the `FPToUI` and `FPToSI` opcodes).

use llrm_mir::apfloat::{semantics_of, semantics_precision};
use llrm_mir::context::ConstantKind;
use llrm_mir::fpclass::{FC_NORMAL, FC_POS_NORMAL};
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::{FloatKind, Type, TypeId};
use llrm_mir::valuetracking::fp::compute_known_fp_class;
use llrm_mir::valuetracking::known::compute_known_bits;
use llrm_mir::valuetracking::signbits::compute_num_sign_bits;

use crate::combine::{Combiner, Visit};
use crate::combine_casts::common_cast_transforms;
use crate::combine_shared::{self as shared, query};

/// `visitFPToUI` and `visitFPToSI`.
pub fn visit_fptoi(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if let Some(found) = fold_itofp_toi(c, inst) {
        return found;
    }
    if let Some(found) = fold_fptoi(c, inst) {
        return found;
    }
    common_cast_transforms(c, inst).unwrap_or(Visit::None)
}

/// The instruction that defines `v`.
fn def_of(
    c: &Combiner,
    v: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = v else { return None };
    match c.function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        _ => None,
    }
}

/// `Type::getFPMantissaWidth`: the precision of a float type.
fn mantissa_width(
    c: &Combiner,
    ty: TypeId,
) -> Option<u32> {
    match c.context.types.get(ty) {
        Type::Float(kind) => Some(semantics_precision(semantics_of(*kind))),
        _ => None,
    }
}

/// `canBeCastedExactlyIntToFP(V, FPTy, IsSigned, CxtI)`.
fn can_be_casted_exactly_int_to_fp(
    c: &Combiner,
    v: Operand,
    fp_ty: TypeId,
    is_signed: bool,
    context: InstId,
) -> bool {
    let src_bits = i64::from(c.context.types.int_bits(c.type_of(v)).unwrap_or(0));
    let src_size = src_bits - i64::from(is_signed);
    let dest_num_sig_bits = i64::from(mantissa_width(c, fp_ty).unwrap_or(0));

    // Easy case - if the source integer type has less bits than the FP
    // mantissa, then the cast must be exact.
    if src_size <= dest_num_sig_bits {
        return true;
    }

    // Cast from FP to integer and back to FP is independent of the
    // intermediate integer width because of poison on overflow.
    if let Some(def) = def_of(c, v)
        && let Opcode::Cast(from_fp @ (CastOp::FPToSI | CastOp::FPToUI)) = c.function.instruction(def).opcode
    {
        let f = c.function.instruction(def).operands[0];
        // If this is uitofp (fptosi F), the source needs an extra bit to avoid
        // potential rounding of negative FP input values.
        let mut src_num_sig_bits = i64::from(mantissa_width(c, c.type_of(f)).unwrap_or(0));
        if !is_signed && from_fp == CastOp::FPToSI {
            src_num_sig_bits += 1;
        }
        // [su]itofp (fpto[su]i F) --> exact if the source type has less or
        // equal significant bits than the destination.
        if src_num_sig_bits > 0 && dest_num_sig_bits > 0 && src_num_sig_bits <= dest_num_sig_bits {
            return true;
        }
    }

    // Try harder to find if the source integer type has less significant bits.
    // Compute number of sign bits or determine trailing zeros.
    let q = query(c, Some(context));
    let known = compute_known_bits(&q, v, 0);
    let sig_bits = src_bits - i64::from(known.count_min_leading_zeros()) - i64::from(known.count_min_trailing_zeros());
    if sig_bits <= dest_num_sig_bits {
        return true;
    }

    // For sitofp, the sign maps to the FP sign bit, so only magnitude bits
    // (BitWidth - NumSignBits) consume mantissa.
    if is_signed {
        let sig_bits = src_bits - i64::from(compute_num_sign_bits(&q, v, 0));
        if sig_bits <= dest_num_sig_bits {
            return true;
        }
    }
    false
}

/// `isKnownExactCastIntToFP(I)`.
fn is_known_exact_cast_int_to_fp(
    c: &Combiner,
    cast: InstId,
) -> bool {
    let instruction = c.function.instruction(cast);
    let Opcode::Cast(op) = instruction.opcode else { return false };
    can_be_casted_exactly_int_to_fp(c, instruction.operands[0], instruction.ty, op == CastOp::SIToFP, cast)
}

/// `foldItoFPtoI`: fpto{s/u}i({u/s}itofp(X)) --> X or zext(X) or sext(X) or
/// trunc(X). Safe if the intermediate type has enough bits in its mantissa to
/// accurately represent all values of X.
fn fold_itofp_toi(
    c: &mut Combiner,
    fi: InstId,
) -> Option<Visit> {
    let source = c.function.instruction(fi).operands[0];
    let op_i = def_of(c, source)?;
    let Opcode::Cast(from_int @ (CastOp::SIToFP | CastOp::UIToFP)) = c.function.instruction(op_i).opcode else {
        return None;
    };
    let x = c.function.instruction(op_i).operands[0];
    let dest_type = c.function.instruction(fi).ty;
    let is_input_signed = from_int == CastOp::SIToFP;
    let is_output_signed = c.function.instruction(fi).opcode == Opcode::Cast(CastOp::FPToSI);

    // Since we can assume the conversion won't overflow, our decision as to
    // whether the input will fit in the float should depend on the minimum of
    // the input range and output range. This is also safe for a signed input
    // and unsigned output, since a negative input would lead to undefined
    // behavior.
    if !is_known_exact_cast_int_to_fp(c, op_i) {
        // The first cast may not round exactly based on the source integer
        // width and FP width, but the overflow UB rules can still allow this
        // to fold. If the destination type is narrow, that means the
        // intermediate FP value must be large enough to hold the source value
        // exactly.
        let output_size = c.context.types.int_bits(dest_type)?;
        if output_size > mantissa_width(c, c.function.instruction(op_i).ty)? {
            return None;
        }
    }

    let src_width = c.context.types.int_bits(c.type_of(x))?;
    let dest_width = c.context.types.int_bits(dest_type)?;
    if dest_width > src_width {
        let cast = if is_input_signed && is_output_signed { CastOp::SExt } else { CastOp::ZExt };
        return Some(c.replace_with_new(fi, Opcode::Cast(cast), dest_type, vec![x], Flags::default()));
    }
    if dest_width < src_width {
        return Some(c.replace_with_new(fi, Opcode::Cast(CastOp::Trunc), dest_type, vec![x], Flags::default()));
    }
    Some(Visit::Replace(x))
}

/// The value of a finite `float` or `double` constant.
fn float_constant(
    c: &Combiner,
    v: Operand,
) -> Option<f64> {
    let Operand::Constant(id) = v else { return None };
    let constant = c.context.get(id);
    let Type::Float(kind) = *c.context.types.get(constant.ty) else { return None };
    let bits = match constant.kind {
        ConstantKind::Float(bits) => bits,
        ConstantKind::Null | ConstantKind::Zero => 0,
        _ => return None,
    };
    match kind {
        FloatKind::Float => Some(f64::from(f32::from_bits(bits as u32))),
        FloatKind::Double => Some(f64::from_bits(bits as u64)),
        FloatKind::X86Fp80 => None,
    }
}

/// `foldFPtoI`.
fn fold_fptoi(
    c: &mut Combiner,
    fi: InstId,
) -> Option<Visit> {
    let is_signed = c.function.instruction(fi).opcode == Opcode::Cast(CastOp::FPToSI);
    let ty = c.function.instruction(fi).ty;
    let source = c.function.instruction(fi).operands[0];
    // fpto{u/s}i non-norm --> 0
    let mask = if is_signed { FC_NORMAL } else { FC_POS_NORMAL };
    let class = compute_known_fp_class(&query(c, Some(fi)), source, mask, 0);
    if class.is_known_never(mask) {
        return Some(Visit::Replace(Operand::Constant(c.context.int(ty, 0))));
    }

    // fpto{u/s}i (fdiv ({u/s}itofp X to F), C_fp) --> {u/s}div X, C
    //
    // F has precision p (significand bits incl. hidden bit); C_fp is the exact
    // FP value of the integer constant C. Given N = integer width, this is
    // safe if:
    //   Unsigned: C > 0 and N <= p.
    //   Signed:   C != 0 and N - 1 <= p, excluding (X == INT_MIN, C == -1)
    //             since sdiv INT_MIN, -1 is UB while the FP path only yields
    //             poison.
    if !shared::has_one_use(c, source) {
        return None;
    }
    let divide = def_of(c, source)?;
    if c.function.instruction(divide).opcode != Opcode::Binary(BinaryOp::FDiv) {
        return None;
    }
    let [numerator, divisor] = c.function.instruction(divide).operands[..] else { return None };
    let int_to_fp = def_of(c, numerator)?;
    let wanted = if is_signed { CastOp::SIToFP } else { CastOp::UIToFP };
    if c.function.instruction(int_to_fp).opcode != Opcode::Cast(wanted) {
        return None;
    }
    let x = c.function.instruction(int_to_fp).operands[0];
    let divisor = float_constant(c, divisor)?;
    let int_ty = c.type_of(x);
    if ty != int_ty {
        return None;
    }
    let int_width = c.context.types.int_bits(int_ty)?;
    let precision = mantissa_width(c, c.function.instruction(divide).ty)?;
    if precision + u32::from(is_signed) < int_width {
        return None;
    }
    // APF->isInteger(), and convertToInteger is exact in IntWidth bits.
    if !divisor.is_finite() || divisor != divisor.trunc() {
        return None;
    }
    let (low, high) = if is_signed {
        (-(2_f64.powi(int_width as i32 - 1)), 2_f64.powi(int_width as i32 - 1))
    } else {
        (0.0, 2_f64.powi(int_width as i32))
    };
    if divisor < low || divisor >= high {
        return None;
    }
    let value = divisor as i128;
    if value == 0 {
        return None;
    }
    // sdiv INT_MIN, -1 is UB, not poison, so this isn't valid if X == INT_MIN.
    // fdiv X, -1 gets transformed to fneg anyways, so we do not handle C == -1.
    if is_signed && value == -1 {
        return None;
    }
    let constant = Operand::Constant(c.context.int(int_ty, value));
    let op = if is_signed { BinaryOp::SDiv } else { BinaryOp::UDiv };
    Some(c.replace_with_new(fi, Opcode::Binary(op), ty, vec![x, constant], Flags::default()))
}
