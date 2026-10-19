//! `ComputeNumSignBits`: how many top bits of a value are copies of its sign
//! bit.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`ComputeNumSignBitsImpl` and its helpers), each function linking the line
//! it translates. Depth stops at [`super::MAX_ANALYSIS_RECURSION_DEPTH`].
//! Not translated, as MIR has no such thing:
//! - vectors: `computeNumSignBitsVectorConstant`, the `BitCast` case,
//!   `ExtractElement` and `ShuffleVector` (L4374-4396, L4509-4535, L4693-4728);
//! - `safeCxtI` (L363): the caller sets `Query::cxt`.
//!
//! # Example
//! `sext i16 %x to i32` has at least 17 sign bits.

use super::known::{compute_known_bits, is_signed_min_max_clamp, is_signed_min_max_intrinsic_clamp};
use super::pattern::{IntrinsicId, apint, intrinsic_id};
use super::query::Query;
use crate::apint::{countl_one, countl_zero, signed};
use crate::context::mask;
use crate::module::Operand;
use crate::opcode::{BinaryOp, CastOp, Opcode};

/// `APInt::getNumSignBits`.
fn num_sign_bits(
    value: u128,
    width: u32,
) -> u32 {
    if signed(value, width) < 0 { countl_one(value, width) } else { countl_zero(value, width) }
}

/// `APInt::isStrictlyPositive`.
fn is_strictly_positive(
    value: u128,
    width: u32,
) -> bool {
    signed(value, width) > 0
}

/// `APInt::logBase2`.
fn log_base2(
    value: u128,
    width: u32,
) -> u32 {
    width - 1 - countl_zero(value, width)
}

/// `APInt::ceilLogBase2`.
fn ceil_log_base2(
    value: u128,
    width: u32,
) -> u32 {
    width - countl_zero(value.wrapping_sub(1) & mask(width), width)
}

/// `ComputeMaxSignificantBits(V, DL, AC, CxtI, DT, Depth)`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L371>
pub fn compute_max_significant_bits(
    q: &Query,
    v: Operand,
    depth: u32,
) -> u32 {
    let sign_bits = compute_num_sign_bits(q, v, depth);
    q.scalar_bits(v) - sign_bits + 1
}

/// `ComputeNumSignBits(V, Q, Depth)`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4410>
pub fn compute_num_sign_bits(
    q: &Query,
    v: Operand,
    depth: u32,
) -> u32 {
    compute_num_sign_bits_impl(q, v, depth)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L4424>
fn compute_num_sign_bits_impl(
    q: &Query,
    v: Operand,
    depth: u32,
) -> u32 {
    let ty_bits = q.scalar_bits(v);
    let mut first_answer = 1;

    if depth == super::MAX_ANALYSIS_RECURSION_DEPTH {
        return 1;
    }

    if let Some((inst, u)) = q.instruction_of(v) {
        let op = |at: usize| u.operands[at];
        let signbits = |operand: Operand| compute_num_sign_bits(q, operand, depth + 1);
        match u.opcode {
            Opcode::Cast(CastOp::SExt) => {
                let tmp = ty_bits - q.scalar_bits(op(0));
                return signbits(op(0)) + tmp;
            }
            Opcode::Binary(BinaryOp::SDiv) => {
                if let Some((denominator, _)) = apint(q, op(1)) {
                    if !is_strictly_positive(denominator, ty_bits) {
                        return fall_through(q, v, first_answer, depth);
                    }
                    let num_bits = signbits(op(0));
                    return ty_bits.min(num_bits + log_base2(denominator, ty_bits));
                }
            }
            Opcode::Binary(BinaryOp::SRem) => {
                let mut tmp = signbits(op(0));
                if let Some((denominator, _)) = apint(q, op(1))
                    && is_strictly_positive(denominator, ty_bits)
                {
                    let res_bits = ty_bits - ceil_log_base2(denominator, ty_bits);
                    tmp = tmp.max(res_bits);
                }
                return tmp;
            }
            Opcode::Binary(BinaryOp::AShr) => {
                let mut tmp = signbits(op(0));
                if let Some((sh_amt, _)) = apint(q, op(1)) {
                    if sh_amt >= u128::from(ty_bits) {
                        return fall_through(q, v, first_answer, depth);
                    }
                    tmp += sh_amt as u32;
                    if tmp > ty_bits {
                        tmp = ty_bits;
                    }
                }
                return tmp;
            }
            Opcode::Binary(BinaryOp::Shl) => {
                if let Some((sh_amt, _)) = apint(q, op(1)) {
                    if sh_amt >= u128::from(ty_bits) {
                        return fall_through(q, v, first_answer, depth);
                    }
                    let zext = q
                        .instruction_of(op(0))
                        .filter(|(_, inner)| matches!(inner.opcode, Opcode::Cast(CastOp::ZExt)))
                        .map(|(_, inner)| inner.operands[0]);
                    let tmp = match zext {
                        Some(x) if sh_amt >= u128::from(ty_bits - q.scalar_bits(x)) => {
                            signbits(x) + (ty_bits - q.scalar_bits(x))
                        }
                        _ => signbits(op(0)),
                    };
                    if sh_amt >= u128::from(tmp) {
                        return fall_through(q, v, first_answer, depth);
                    }
                    return tmp - sh_amt as u32;
                }
            }
            Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) => {
                let tmp = signbits(op(0));
                if tmp != 1 {
                    let tmp2 = signbits(op(1));
                    first_answer = tmp.min(tmp2);
                }
            }
            Opcode::Select => {
                if let Some((_, clow, chigh)) = is_signed_min_max_clamp(q, v) {
                    let width = q.scalar_bits(v);
                    return num_sign_bits(clow, width).min(num_sign_bits(chigh, width));
                }
                let tmp = signbits(op(1));
                if tmp != 1 {
                    let tmp2 = signbits(op(2));
                    return tmp.min(tmp2);
                }
            }
            Opcode::Binary(BinaryOp::Add) => {
                let tmp = signbits(op(0));
                if tmp != 1 {
                    if let Some((rhs, rhs_width)) = q.int_constant(op(1))
                        && rhs == mask(rhs_width)
                    {
                        let known = compute_known_bits(q, op(0), depth + 1);
                        if (known.zero | 1) & mask(ty_bits) == mask(ty_bits) {
                            return ty_bits;
                        }
                        if known.is_non_negative() {
                            return tmp;
                        }
                    }
                    let tmp2 = signbits(op(1));
                    if tmp2 != 1 {
                        return tmp.min(tmp2) - 1;
                    }
                }
            }
            Opcode::Binary(BinaryOp::Sub) => {
                let tmp2 = signbits(op(1));
                if tmp2 != 1 {
                    if let Some((lhs, _)) = q.int_constant(op(0))
                        && lhs == 0
                    {
                        let known = compute_known_bits(q, op(1), depth + 1);
                        if (known.zero | 1) & mask(ty_bits) == mask(ty_bits) {
                            return ty_bits;
                        }
                        if known.is_non_negative() {
                            return tmp2;
                        }
                    }
                    let tmp = signbits(op(0));
                    if tmp != 1 {
                        return tmp.min(tmp2) - 1;
                    }
                }
            }
            Opcode::Binary(BinaryOp::Mul) => {
                let sign_bits_op0 = signbits(op(0));
                if sign_bits_op0 != 1 {
                    let sign_bits_op1 = signbits(op(1));
                    if sign_bits_op1 != 1 {
                        let out_valid_bits = (ty_bits - sign_bits_op0 + 1) + (ty_bits - sign_bits_op1 + 1);
                        return if out_valid_bits > ty_bits { 1 } else { ty_bits - out_valid_bits + 1 };
                    }
                }
            }
            Opcode::Phi => {
                let incoming = u.operands.len() / 2;
                if incoming <= 4 && incoming != 0 {
                    let mut tmp = ty_bits;
                    for pair in u.operands.chunks(2) {
                        if tmp == 1 {
                            return tmp;
                        }
                        let Operand::Block(block) = pair[1] else { unreachable!("a phi names its blocks") };
                        let rec_q = q.without_cond_context().with_cxt(q.function.terminator(block));
                        tmp = tmp.min(compute_num_sign_bits(&rec_q, pair[0], depth + 1));
                    }
                    return tmp;
                }
            }
            Opcode::Cast(CastOp::Trunc) => {
                let tmp = signbits(op(0));
                let operand_ty_bits = q.scalar_bits(op(0));
                if tmp > operand_ty_bits - ty_bits {
                    return tmp - (operand_ty_bits - ty_bits);
                }
                return 1;
            }
            Opcode::Call(_) => match intrinsic_id(q, inst) {
                Some(IntrinsicId::Abs) => {
                    let tmp = signbits(op(0));
                    if tmp != 1 {
                        return tmp - 1;
                    }
                }
                Some(IntrinsicId::Smin | IntrinsicId::Smax) => {
                    if let Some((clow, chigh)) = is_signed_min_max_intrinsic_clamp(q, inst) {
                        let width = q.scalar_bits(v);
                        return num_sign_bits(clow, width).min(num_sign_bits(chigh, width));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fall_through(q, v, first_answer, depth)
}

/// The end of `ComputeNumSignBitsImpl`: what `computeKnownBits` proves of the
/// top bits.
fn fall_through(
    q: &Query,
    v: Operand,
    first_answer: u32,
    depth: u32,
) -> u32 {
    let known = compute_known_bits(q, v, depth);
    first_answer.max(known.count_min_sign_bits())
}

/// The `sb` column of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
) -> String {
    format!(" sb={}", compute_num_sign_bits(q, v, 0))
}
