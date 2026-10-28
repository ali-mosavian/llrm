//! `visitFDiv`'s `foldFDivConstantDivisor`: a division by a constant is a
//! multiply by its reciprocal.
//!
//! Translation of
//! [`foldFDivConstantDivisor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp)
//! at d1106deb71cc.
//!
//! MIRROR: `X / C --> X * (1/C)` when `C` has an exact inverse (a normal power
//! of two whose reciprocal is normal), or with `arcp` when `C` is a normal
//! number; the product takes the division's fast-math flags.
//!
//! DEVIATE 1: the `-X / C --> X / -C` arm and `foldFDivConstantDividend`,
//! `foldFDivPowDivisor` and the sqrt/exp rules of `visitFDiv`, and everything
//! in `visitFAdd` and `visitFMul` after `SimplifyAssociativeOrCommutative`, are
//! not translated.
//!
//! DEVIATE 3: `visitFSub` stops after `X - (-Y) --> X + Y`: the
//! `foldFNegIntoConstant`, `foldFBinOpOfIntCasts`, cast-of-fneg, `fmul`/`fdiv`
//! of a negated factor and the `reassoc nsz` arms are not translated; a half or
//! `x86_fp80` constant is not negated.
//!
//! DEVIATE 2: an `x86_fp80` constant is held as a double: its reciprocal is
//! formed only where exact.
//!
//! # Used by
//! `combine::visitors` for `fdiv`.
//!
//! # Example
//! ```text
//! %r = fdiv arcp double %x, 3.0     ==>   %r = fmul arcp double %x, 0x3FD5555555555555
//! %r = fdiv double %x, 4.0          ==>   %r = fmul double %x, 2.5e-1
//! ```

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::types::{FloatKind, Type};

use crate::combine::{Combiner, Visit};
use crate::combine_muldivrem::simplify_first;
use crate::combine_shared::simplify_associative_or_commutative;

/// `visitFAdd` and `visitFMul`, as far as translated: `simplify*Inst`, then
/// `SimplifyAssociativeOrCommutative` (constants to the right; `(X op C1) op
/// C2` is `X op (C1 op C2)` with `reassoc nsz`). DEVIATE 1 covers the rest.
pub fn visit_fadd_or_fmul(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if let Some(found) = simplify_first(c, inst) {
        return Visit::Replace(found);
    }
    if simplify_associative_or_commutative(c, inst) { Visit::Changed } else { Visit::None }
}

/// `visitFSub` (InstCombineAddSub.cpp:3322), up to `X - (-Y) --> X + Y`: the
/// `fneg` spelled `fsub -0.0, X`, `X - C --> X + (-C)` for a float or double
/// constant, and a subtrahend that is an `fneg`. DEVIATE 3 covers the rest.
pub fn visit_fsub(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if let Some(found) = simplify_first(c, inst) {
        return Visit::Replace(found);
    }
    let instruction = c.function.instruction(inst).clone();
    let (op0, op1) = (instruction.operands[0], instruction.operands[1]);
    // fsub -0.0, X ==> fneg X; fsub nsz 0.0, X ==> fneg nsz X
    if let Some((kind, value)) = float_constant(c, op0) {
        let negative_zero = value == 0.0 && value.is_sign_negative();
        let nsz_zero = value == 0.0 && instruction.flags.contains(Flags::NSZ);
        if (negative_zero || nsz_zero) && kind != FloatKind::X86Fp80 {
            return c.replace_with_new(inst, Opcode::FNeg, instruction.ty, vec![op1], instruction.flags);
        }
    }
    // X - C --> X + (-C)
    if let Some(negated) = negated_constant(c, op1) {
        return c.replace_with_binop(inst, BinaryOp::FAdd, op0, negated, instruction.flags);
    }
    // X - (-Y) --> X + Y
    if let Some(y) = fneg_operand(c, op1) {
        return c.replace_with_binop(inst, BinaryOp::FAdd, op0, y, instruction.flags);
    }
    Visit::None
}

/// `m_FNeg(Y)`: `fneg Y`, or `fsub -0.0, Y`.
fn fneg_operand(
    c: &Combiner,
    v: Operand,
) -> Option<Operand> {
    let (_, instruction) = crate::combine_shared::query(c, None).instruction_of(v)?;
    match instruction.opcode {
        Opcode::FNeg => Some(instruction.operands[0]),
        Opcode::Binary(BinaryOp::FSub)
            if float_constant(c, instruction.operands[0]).is_some_and(|(_, x)| x == 0.0 && x.is_sign_negative()) =>
        {
            Some(instruction.operands[1])
        }
        _ => None,
    }
}

/// `ConstantFoldUnaryOpOperand(FNeg, C)` of a float or double constant.
fn negated_constant(
    c: &mut Combiner,
    v: Operand,
) -> Option<Operand> {
    let Operand::Constant(id) = v else { return None };
    let constant = c.context.get(id).clone();
    let Type::Float(kind) = *c.context.types.get(constant.ty) else { return None };
    let ConstantKind::Float(bits) = constant.kind else { return None };
    let flipped = match kind {
        FloatKind::Float => bits ^ (1 << 31),
        FloatKind::Double => bits ^ (1 << 63),
        _ => return None,
    };
    Some(Operand::Constant(c.context.constant(Constant { ty: constant.ty, kind: ConstantKind::Float(flipped) })))
}

/// `visitFDiv` (InstCombineMulDivRem.cpp).
pub fn visit_fdiv(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifyFDivInst
    if let Some(found) = simplify_first(c, inst) {
        return Visit::Replace(found);
    }
    fold_fdiv_constant_divisor(c, inst)
}

/// The value of a float constant operand, with its format and type.
fn float_constant(
    c: &Combiner,
    v: Operand,
) -> Option<(FloatKind, f64)> {
    let Operand::Constant(id) = v else { return None };
    let constant = c.context.get(id);
    let Type::Float(kind) = *c.context.types.get(constant.ty) else { return None };
    match constant.kind {
        ConstantKind::Float(bits) => Some((
            kind,
            if kind == FloatKind::Float { f64::from(f32::from_bits(bits as u32)) } else { f64::from_bits(bits) },
        )),
        _ => None,
    }
}

/// `APFloat::getExactInverse` succeeds: a normal power of two whose inverse is
/// normal.
fn has_exact_inverse(
    kind: FloatKind,
    divisor: f64,
) -> bool {
    match kind {
        FloatKind::Float => {
            let divisor = divisor as f32;
            divisor.is_normal() && divisor.to_bits() & 0x7f_ffff == 0 && (1.0 / divisor).is_normal()
        }
        FloatKind::Double | FloatKind::X86Fp80 => {
            divisor.is_normal() && divisor.to_bits() & 0xf_ffff_ffff_ffff == 0 && (1.0 / divisor).is_normal()
        }
    }
}

/// `isNormalFP`: not zero, infinity, NaN or denormal.
fn is_normal(
    kind: FloatKind,
    value: f64,
) -> bool {
    if kind == FloatKind::Float { (value as f32).is_normal() } else { value.is_normal() }
}

fn fold_fdiv_constant_divisor(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let instruction = c.function.instruction(inst).clone();
    let (dividend, divisor) = (instruction.operands[0], instruction.operands[1]);
    let Some((kind, value)) = float_constant(c, divisor) else { return Visit::None };
    let exact = has_exact_inverse(kind, value);
    let arcp = instruction.flags.contains(Flags::ARCP);
    if !(exact || (arcp && is_normal(kind, value))) {
        return Visit::None;
    }
    // DEVIATE 2: an inexact x86_fp80 reciprocal is not formed.
    if kind == FloatKind::X86Fp80 && !exact {
        return Visit::None;
    }
    let reciprocal = if kind == FloatKind::Float { f64::from(1.0f32 / value as f32) } else { 1.0 / value };
    let bits = if kind == FloatKind::Float { u64::from((reciprocal as f32).to_bits()) } else { reciprocal.to_bits() };
    // ConstantFoldBinaryOpOperands(FDiv, 1.0, C): a denormal or infinite
    // reciprocal of an arcp division is left (`isNormalFP` guards the pin's).
    if !is_normal(kind, reciprocal) {
        return Visit::None;
    }
    let ty = instruction.ty;
    let recip = Operand::Constant(c.context.constant(Constant { ty, kind: ConstantKind::Float(bits) }));
    c.replace_with_binop(inst, BinaryOp::FMul, dividend, recip, instruction.flags)
}

#[cfg(test)]
#[path = "combine_fp_tests.rs"]
mod tests;
