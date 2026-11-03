//! `foldFBinOpOfIntCasts`: a floating-point `fadd`, `fsub` or `fmul` of two
//! integer conversions, or of one and a whole-number constant, is one integer
//! operation and one conversion, if the conversions are exact and the integer
//! operation cannot overflow.
//!
//! Translation of
//! [`foldFBinOpOfIntCasts`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1705)
//! and `foldFBinOpOfIntCastsFromSign` (L1545) at d1106deb71cc.
//!
//! MIRROR: both orders of sign are tried, unsigned first; `uitofp nneg` and
//! `sitofp` of a non-negative are the same. A signed product also needs both
//! operands non-zero (no `-0.0`).
//!
//! DEVIATE 1: an `x86_fp80` constant is held as a double, so it joins only if
//! that double is a whole number the integer type holds exactly.
//!
//! # Used by
//! `combine_fp::visit_fadd_or_fmul` and `visit_fsub`, at the pin's position
//! less the folds before it that are not translated.
//!
//! # Example
//! ```text
//! %x = sitofp i16 %a to x86_fp80       %s = add nsw i16 %a, %b
//! %y = sitofp i16 %b to x86_fp80  ==>  %r = sitofp i16 %s to x86_fp80
//! %r = fadd x86_fp80 %x, %y
//! ```

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::{FloatKind, Type};
use llrm_mir::valuetracking::range::{
    OverflowResult, compute_overflow_for_signed_add, compute_overflow_for_signed_mul, compute_overflow_for_signed_sub,
    compute_overflow_for_unsigned_add, compute_overflow_for_unsigned_mul, compute_overflow_for_unsigned_sub,
};
use llrm_mir::valuetracking::{Query, known, nonzero, pattern, signbits};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

/// `foldFBinOpOfIntCasts(BO)`.
pub fn fold_fbinop_of_int_casts(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let instruction = c.function.instruction(inst).clone();
    let op = match instruction.opcode {
        Opcode::Binary(op @ (BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul)) => op,
        _ => return None,
    };
    let (lhs, rhs) = (instruction.operands[0], instruction.operands[1]);
    let (first, int_ty, first_is_signed) = {
        let q = query(c, Some(inst));
        let (signed, x) = int_cast(&q, lhs)?;
        (x, q.type_of(x)?, signed)
    };
    // `m_Constant(Op1FpC)`, else `m_IToFP(m_Value(IntOps[1]))`.
    let (second, second_is_signed, constant) = match float_value(c, rhs) {
        Some(value) => (None, false, Some(value)),
        None => {
            let q = query(c, Some(inst));
            let (signed, y) = int_cast(&q, rhs)?;
            (Some(y), signed, None)
        }
    };
    let from = Shape {
        op,
        fp_ty: instruction.ty,
        int_ty,
        ops: [first, second.unwrap_or(first)],
        is_signed: [first_is_signed, second_is_signed],
        constant,
    };
    // Unsigned first, then signed.
    fold_from_sign(c, inst, &from, false).or_else(|| fold_from_sign(c, inst, &from, true))
}

/// What `foldFBinOpOfIntCasts` matched.
struct Shape {
    op: BinaryOp,
    fp_ty: llrm_mir::types::TypeId,
    int_ty: llrm_mir::types::TypeId,
    /// The integers converted; the second is the first's when `constant`.
    ops: [Operand; 2],
    /// Whether each conversion is a `sitofp`.
    is_signed: [bool; 2],
    /// The right operand, if it is a floating-point constant.
    constant: Option<f64>,
}

/// `m_IToFP(m_Value(X))`: whether it is signed, and `X`.
fn int_cast(
    q: &Query,
    v: Operand,
) -> Option<(bool, Operand)> {
    if let Some((_, _, x)) = pattern::cast(q, v, CastOp::SIToFP) {
        return Some((true, x));
    }
    pattern::cast(q, v, CastOp::UIToFP).map(|(_, _, x)| (false, x))
}

/// The value of a floating-point constant (an `x86_fp80` one is held as a
/// double).
fn float_value(
    c: &Combiner,
    v: Operand,
) -> Option<f64> {
    let Operand::Constant(id) = v else { return None };
    let constant = c.context.get(id);
    let Type::Float(kind) = *c.context.types.get(constant.ty) else { return None };
    match constant.kind {
        ConstantKind::Float(bits) => {
            Some(if kind == FloatKind::Float { f64::from(f32::from_bits(bits as u32)) } else { f64::from_bits(bits) })
        }
        _ => None,
    }
}

/// The bits a float type represents exactly (`APFloat::semanticsPrecision`).
fn precision(
    c: &Combiner,
    ty: llrm_mir::types::TypeId,
) -> Option<u32> {
    match c.context.types.get(ty) {
        Type::Float(FloatKind::Float) => Some(24),
        Type::Float(FloatKind::Double) => Some(53),
        Type::Float(FloatKind::X86Fp80) => Some(64),
        _ => None,
    }
}

/// `foldFBinOpOfIntCastsFromSign(BO, OpsFromSigned, ...)`.
fn fold_from_sign(
    c: &mut Combiner,
    inst: InstId,
    shape: &Shape,
    ops_from_signed: bool,
) -> Option<Visit> {
    let max_bits = precision(c, shape.fp_ty)?;
    let int_sz = c.context.types.int_bits(shape.int_ty)?;
    let mut ops = shape.ops;
    let mut used = [int_sz; 2];
    let int_op = match shape.op {
        BinaryOp::FAdd => BinaryOp::Add,
        BinaryOp::FSub => BinaryOp::Sub,
        _ => BinaryOp::Mul,
    };
    let (cast_op, flags) = {
        let q = query(c, Some(inst));
        // A whole-number constant on the right joins as an integer.
        if let Some(value) = shape.constant {
            // Signed + Mul req non-zero
            if ops_from_signed && shape.op == BinaryOp::FMul && value == 0.0 {
                return None;
            }
            let as_int = whole_number(value, int_sz, ops_from_signed, shape.fp_ty, c)?;
            ops[1] = c.int(shape.int_ty, as_int);
        }

        let valid_promotion = |at: usize, used: &mut [u32; 2]| -> bool {
            let known_bits = known::compute_known_bits(&q, ops[at], 0);
            // Can we treat this operand as the desired sign?
            if ops_from_signed != shape.is_signed[at] && !known_bits.is_non_negative() {
                return false;
            }
            if max_bits < int_sz {
                used[at] = if ops_from_signed {
                    int_sz - signbits::compute_num_sign_bits(&q, ops[at], 0)
                } else {
                    int_sz - known_bits.count_min_leading_zeros()
                };
            }
            if max_bits < used[at] {
                return false;
            }
            // Signed + Mul also requires that op is non-zero to avoid -0 cases.
            !ops_from_signed || shape.op != BinaryOp::FMul || nonzero::is_known_non_zero(&q, ops[at], 0)
        };

        if shape.constant.is_none() && !valid_promotion(1, &mut used) {
            return None;
        }
        if !valid_promotion(0, &mut used) {
            return None;
        }

        // Finally, whether the integer operation overflows. The precision
        // checks often rule it out.
        let mut needs_check = true;
        let mut output_bits = if ops_from_signed { 2 } else { 1 };
        let widest = used[0].max(used[1]);
        output_bits += if int_op == BinaryOp::Mul { widest * 2 } else { widest };
        let mut output_signed = ops_from_signed;
        if output_bits < int_sz {
            needs_check = false;
            // An unsigned subtraction is bounded to an in-range signed value.
            if int_op == BinaryOp::Sub {
                output_signed = true;
            }
        }
        if needs_check && !will_not_overflow(&q, int_op, ops[0], ops[1], output_signed) {
            return None;
        }
        let mut flags = Flags::default();
        flags.insert(if output_signed { Flags::NSW } else { Flags::NUW });
        (if output_signed { CastOp::SIToFP } else { CastOp::UIToFP }, flags)
    };
    let sum = c.insert_before(inst, Opcode::Binary(int_op), shape.int_ty, vec![ops[0], ops[1]], flags);
    let sum = c.value_of(sum);
    Some(c.replace_with_new(inst, Opcode::Cast(cast_op), shape.fp_ty, vec![sum], Flags::default()))
}

/// `fpto{s|u}i` of `value` to `bits`, if `{s|u}itofp` of that gives `value`
/// back in `fp_ty`.
fn whole_number(
    value: f64,
    bits: u32,
    signed: bool,
    fp_ty: llrm_mir::types::TypeId,
    c: &Combiner,
) -> Option<u128> {
    if !value.is_finite() || value.fract() != 0.0 || (value == 0.0 && value.is_sign_negative()) {
        return None;
    }
    let (low, high) = if signed {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (0, (1i128 << bits) - 1)
    };
    // An f64 holds the integer exactly only below 2^53 in magnitude here.
    if value.abs() >= 9_007_199_254_740_992.0 {
        return None;
    }
    let int = value as i128;
    if int < low || int > high {
        return None;
    }
    let round_trip = match c.context.types.get(fp_ty) {
        Type::Float(FloatKind::Float) => f64::from(int as f32),
        _ => int as f64,
    };
    (round_trip == value).then_some((int as u128) & llrm_mir::context::mask(bits))
}

/// `willNotOverflow(Opcode, LHS, RHS, CxtI, IsSigned)`.
fn will_not_overflow(
    q: &Query,
    op: BinaryOp,
    lhs: Operand,
    rhs: Operand,
    signed: bool,
) -> bool {
    let result = match (op, signed) {
        (BinaryOp::Add, true) => compute_overflow_for_signed_add(q, lhs, rhs),
        (BinaryOp::Add, false) => compute_overflow_for_unsigned_add(q, lhs, rhs),
        (BinaryOp::Sub, true) => compute_overflow_for_signed_sub(q, lhs, rhs),
        (BinaryOp::Sub, false) => compute_overflow_for_unsigned_sub(q, lhs, rhs),
        (_, true) => compute_overflow_for_signed_mul(q, lhs, rhs),
        (_, false) => compute_overflow_for_unsigned_mul(q, lhs, rhs, false),
    };
    result == OverflowResult::NeverOverflows
}

#[cfg(test)]
#[path = "combine_fp_int_casts_tests.rs"]
mod tests;
