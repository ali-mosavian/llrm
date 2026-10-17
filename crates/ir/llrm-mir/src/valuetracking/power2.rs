//! `isKnownToBeAPowerOfTwo`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueTracking.cpp`
//! (`isKnownToBeAPowerOfTwo`, `isPowerOfTwoRecurrence`,
//! `isImpliedToBeAPowerOfTwoFromCond`), each function linking the line it
//! translates. Scalars only. Where MIR lacks what the C++ reads, the case is
//! skipped:
//! - `Q.DC`, the dominating-condition cache (2794-2810): the query has none, as
//!   the `SimplifyQuery` of the oracle.
//! - `read_register` and `read_volatile_register` (2958-2965): MIR keeps no
//!   target triple.
//!
//! # Example
//! `shl i32 1, %n` is a power of two.

use super::pattern::{self, IntrinsicId};
use super::query::Query;
use super::{MAX_ANALYSIS_RECURSION_DEPTH, context, known, nonzero};
use crate::context::mask;
use crate::knownbits::KnownBits;
use crate::module::{InstId, Operand};
use crate::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L2692>
fn is_power_of_two_recurrence(
    q: &mut Query,
    phi: InstId,
    or_zero: bool,
    depth: u32,
) -> bool {
    let Some((bo, start, step)) = pattern::match_simple_recurrence(q, phi) else { return false };
    let bo_instruction = q.function.instruction(bo);

    let incoming: Vec<_> = q.function.instruction(phi).operands.chunks(2).map(|one| (one[0], one[1])).collect();
    for (value, block) in incoming {
        if value == start {
            let Operand::Block(block) = block else { return false };
            q.cxt = q.function.terminator(block);
            if !is_known_to_be_a_power_of_two(q, start, or_zero, depth) {
                return false;
            }
        }
    }

    let Opcode::Binary(op) = bo_instruction.opcode else { return false };
    if op != BinaryOp::Mul && bo_instruction.operands[1] != step {
        return false;
    }

    q.cxt = q.block_of(bo).and_then(|block| q.function.terminator(block));
    let no_wrap = bo_instruction.flags.contains(Flags::NUW) || bo_instruction.flags.contains(Flags::NSW);
    let exact = bo_instruction.flags.contains(Flags::EXACT);
    match op {
        BinaryOp::Mul => (or_zero || no_wrap) && is_known_to_be_a_power_of_two(q, step, or_zero, depth),
        BinaryOp::SDiv | BinaryOp::UDiv => {
            if op == BinaryOp::SDiv && (!pattern::power2_constant(q, start).is_some() || pattern::sign_mask(q, start)) {
                return false;
            }
            (or_zero || exact) && is_known_to_be_a_power_of_two(q, step, false, depth)
        }
        BinaryOp::Shl => or_zero || no_wrap,
        BinaryOp::AShr | BinaryOp::LShr => {
            if op == BinaryOp::AShr && (!pattern::power2_constant(q, start).is_some() || pattern::sign_mask(q, start)) {
                return false;
            }
            or_zero || exact
        }
        _ => false,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L2749>
fn is_implied_to_be_a_power_of_two_from_cond(
    q: &Query,
    v: Operand,
    or_zero: bool,
    cond: Operand,
    cond_is_true: bool,
) -> bool {
    let Some((_, _, pred, lhs, rhs)) = pattern::icmp(q, cond) else { return false };
    let mut pred = pred.pred;
    let Some((ctpop, call)) = pattern::intrinsic(q, lhs, IntrinsicId::Ctpop) else { return false };
    let _ = ctpop;
    if call.operands[0] != v {
        return false;
    }
    let Some((rhsc, _)) = pattern::apint(q, rhs) else { return false };
    if !cond_is_true {
        pred = pred.inverse();
    }
    if or_zero && pred == IntPredicate::Ult && rhsc == 2 {
        return true;
    }
    pred == IntPredicate::Eq && rhsc == 1
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L2769>
pub fn is_known_to_be_a_power_of_two(
    q: &Query,
    v: Operand,
    or_zero: bool,
    mut depth: u32,
) -> bool {
    if matches!(v, Operand::Constant(_)) {
        return if or_zero { pattern::power2_or_zero(q, v) } else { pattern::power2_constant(q, v).is_some() };
    }

    if or_zero && q.type_of(v).and_then(|ty| q.context.types.int_bits(ty)) == Some(1) {
        return true;
    }

    if q.assumptions.is_some() && q.cxt.is_some() {
        for &assume in q.assumptions.map_or(&[][..], |cache| cache.assumptions_for(v)) {
            let condition = q.function.instruction(assume).operands[0];
            if is_implied_to_be_a_power_of_two_from_cond(q, v, or_zero, condition, true)
                && q.cxt.is_some_and(|cxt| context::is_valid_assume_for_context(q, assume, cxt, false))
            {
                return true;
            }
        }
    }

    let Some((inst, i)) = q.instruction_of(v) else { return false };

    if q.cxt.is_some() && pattern::intrinsic(q, v, IntrinsicId::Vscale).is_some() {
        return q.function.attrs.iter().any(|one| {
            matches!(
                one,
                crate::opcode::Attribute::Flag(name) | crate::opcode::Attribute::Int(name, _) if name == "vscale_range"
            )
        });
    }

    if let Some((_, _, lhs, _)) = pattern::binary(q, v, BinaryOp::Shl)
        && pattern::one(q, lhs)
    {
        return true;
    }

    if let Some((_, _, lhs, _)) = pattern::binary(q, v, BinaryOp::LShr)
        && pattern::sign_mask(q, lhs)
    {
        return true;
    }

    let before = depth;
    depth += 1;
    if before == MAX_ANALYSIS_RECURSION_DEPTH {
        return false;
    }

    let no_wrap = i.flags.contains(Flags::NUW) || i.flags.contains(Flags::NSW);
    let exact = i.flags.contains(Flags::EXACT);
    match &i.opcode {
        Opcode::Cast(CastOp::ZExt) => is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth),
        Opcode::Cast(CastOp::Trunc) => or_zero && is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth),
        Opcode::Binary(BinaryOp::Shl) => {
            if or_zero || no_wrap {
                return is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth);
            }
            false
        }
        Opcode::Binary(BinaryOp::LShr) => {
            if or_zero || exact {
                return is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth);
            }
            false
        }
        Opcode::Binary(BinaryOp::UDiv) => {
            if exact {
                return is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth);
            }
            false
        }
        Opcode::Binary(BinaryOp::Mul) => {
            is_known_to_be_a_power_of_two(q, i.operands[1], or_zero, depth)
                && is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth)
                && (or_zero || nonzero::is_known_non_zero(q, v, depth))
        }
        Opcode::Binary(BinaryOp::And) => {
            if or_zero
                && (is_known_to_be_a_power_of_two(q, i.operands[1], true, depth)
                    || is_known_to_be_a_power_of_two(q, i.operands[0], true, depth))
            {
                return true;
            }
            let is_neg_of = |negated: Operand, x: Operand| {
                matches!(
                    pattern::binary(q, negated, BinaryOp::Sub),
                    Some((_, _, zero, rhs)) if pattern::zero_int(q, zero) && rhs == x
                )
            };
            if is_neg_of(i.operands[0], i.operands[1]) || is_neg_of(i.operands[1], i.operands[0]) {
                return or_zero || nonzero::is_known_non_zero(q, i.operands[0], depth);
            }
            false
        }
        Opcode::Binary(BinaryOp::Add) => {
            let nuw = i.flags.contains(Flags::NUW);
            if or_zero || nuw || i.flags.contains(Flags::NSW) {
                let and_with = |and: Operand, x: Operand| {
                    matches!(
                        pattern::binary(q, and, BinaryOp::And),
                        Some((_, _, a, b)) if a == x || b == x
                    )
                };
                if and_with(i.operands[0], i.operands[1])
                    && is_known_to_be_a_power_of_two(q, i.operands[1], or_zero, depth)
                {
                    return true;
                }
                if and_with(i.operands[1], i.operands[0])
                    && is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth)
                {
                    return true;
                }

                let lhs_bits: KnownBits = known::compute_known_bits(q, i.operands[0], depth);
                let rhs_bits: KnownBits = known::compute_known_bits(q, i.operands[1], depth);
                let width = lhs_bits.width;
                let candidates = !(lhs_bits.zero & rhs_bits.zero) & mask(width);
                if candidates.is_power_of_two() && (or_zero || rhs_bits.one != 0 || lhs_bits.one != 0) {
                    return true;
                }
            }

            if (or_zero || nuw)
                && let Some((_, _, lshr, one)) = pattern::binary(q, v, BinaryOp::Add)
                && let Some((_, _, all_ones, _)) = pattern::binary(q, lshr, BinaryOp::LShr)
                && pattern::all_ones(q, all_ones)
                && pattern::one(q, one)
            {
                return true;
            }
            false
        }
        Opcode::Select => {
            is_known_to_be_a_power_of_two(q, i.operands[1], or_zero, depth)
                && is_known_to_be_a_power_of_two(q, i.operands[2], or_zero, depth)
        }
        Opcode::Phi => {
            let mut rec_q = q.without_cond_context();
            if is_power_of_two_recurrence(&mut rec_q, inst, or_zero, depth) {
                return true;
            }

            let result = Operand::Value(i.result.expect("a phi has a result"));
            let new_depth = depth.max(MAX_ANALYSIS_RECURSION_DEPTH - 1);
            let mut rec_q = q.without_cond_context();
            i.operands
                .chunks(2)
                .all(
                    |incoming| {
                        if incoming[0] == result {
                            return true;
                        }
                        let Operand::Block(block) = incoming[1] else { return false };
                        rec_q.cxt = q.function.terminator(block);
                        is_known_to_be_a_power_of_two(&rec_q, incoming[0], or_zero, new_depth)
                    },
                )
        }
        Opcode::Call(_) => match pattern::call_intrinsic(q, inst) {
            Some(IntrinsicId::Umax | IntrinsicId::Smax | IntrinsicId::Umin | IntrinsicId::Smin) => {
                is_known_to_be_a_power_of_two(q, i.operands[1], or_zero, depth)
                    && is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth)
            }
            Some(IntrinsicId::Bitreverse | IntrinsicId::Bswap) => {
                is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth)
            }
            Some(IntrinsicId::Fshr | IntrinsicId::Fshl) => {
                i.operands[0] == i.operands[1] && is_known_to_be_a_power_of_two(q, i.operands[0], or_zero, depth)
            }
            Some(IntrinsicId::RiscvVsetvlimax) => true,
            _ => false,
        },
        _ => false,
    }
}

/// The `p2` and `p2z` columns of `examples/vtdump.rs`.
pub fn column(
    q: &Query,
    v: Operand,
    or_zero: bool,
) -> String {
    format!(" {}={}", if or_zero { "p2z" } else { "p2" }, u8::from(is_known_to_be_a_power_of_two(q, v, or_zero, 0)))
}
