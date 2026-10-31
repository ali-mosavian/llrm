//! `foldCastedBitwiseLogic`: `and`, `or` and `xor` of two like casts is the
//! cast of the logic at the source type.
//!
//! Translation of
//! [`foldCastedBitwiseLogic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L1821)
//! and `shouldOptimizeCast` (:1764).
//!
//! MIRROR: the C++ order and its one-use conditions.
//!
//! DEVIATE 1: the `(A >> BW-1)` with `zext(icmp)` fold is left out.
//!
//! DEVIATE 2: ours runs it from `visitAnd` ahead of the folds of a bool
//! extension, where the C++ reaches it after `reassociateBooleanAndOr`.
//!
//! # Used by
//! `combine_and_select::fold_and_select`, `combine_or::visit_or` and
//! `combine_xor::visit_xor`.
//!
//! # Example
//! Three tests of a boolean, `or`ed, narrow to one `or` of `i1`:
//!
//! ```text
//! %a = sext i1 %x to i16
//! %b = sext i1 %y to i16          %o = or i1 %x, %y
//! %r = or i16 %a, %b      ==>     %r = sext i1 %o to i16
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};

use crate::combine::{Combiner, Visit, defining};
use crate::combine_casts::is_eliminable_cast_pair;
use crate::combine_minmax::lossless_trunc;
use crate::combine_shared::{has_one_use, width};

/// A cast: its instruction, kind and source.
fn cast_of(
    c: &Combiner,
    v: Operand,
) -> Option<(InstId, CastOp, Operand)> {
    let inst = defining(c.function, v)?;
    let instruction = c.function.instruction(inst);
    match instruction.opcode {
        Opcode::Cast(kind) => Some((inst, kind, instruction.operands[0])),
        _ => None,
    }
}

/// `InstCombinerImpl::shouldOptimizeCast`.
fn should_optimize_cast(
    c: &Combiner,
    inst: InstId,
    source: Operand,
) -> bool {
    // Noop casts and casts of constants are eliminated trivially.
    if c.type_of(source) == c.function.instruction(inst).ty || !matches!(source, Operand::Value(_)) {
        return false;
    }
    // A cast paired with one that folds away is left to that fold.
    if let Some(preceding) = defining(c.function, source) {
        if matches!(c.function.instruction(preceding).opcode, Opcode::Cast(_))
            && is_eliminable_cast_pair(c, preceding, inst).is_some()
        {
            return false;
        }
    }
    true
}

/// `foldLogicCastConstant` (:1781): `logic (zext X), C` and `logic (sext X), C`
/// with `C` unchanged in the source type is the extension of the logic there.
fn fold_logic_cast_constant(
    c: &mut Combiner,
    inst: InstId,
    cast: Operand,
    constant: Operand,
    kind: CastOp,
    source: Operand,
) -> Option<Visit> {
    let Opcode::Binary(op) = c.function.instruction(inst).opcode else { return None };
    let value = c.constant_int(constant)?;
    if !has_one_use(c, cast) || !matches!(kind, CastOp::ZExt | CastOp::SExt) {
        return None;
    }
    let (bits, small) = (width(c, constant), width(c, source));
    let narrow = lossless_trunc(value, bits, small, kind)?;
    let small_ty = c.type_of(source);
    let narrow_c = c.int(small_ty, narrow);
    let logic = c.create_binop(inst, op, source, narrow_c, Flags::default());
    let dest = c.function.instruction(inst).ty;
    let mut flags = Flags::default();
    if kind == CastOp::ZExt {
        // `nneg` where the constant is also lossless as a signed truncation,
        // and the cast had it.
        let cast_flags = c.function.instruction(defining(c.function, cast)?).flags;
        if lossless_trunc(value, bits, small, CastOp::SExt).is_some() && cast_flags.contains(Flags::NNEG) {
            flags.insert(Flags::NNEG);
        }
    }
    Some(c.replace_with_new(inst, Opcode::Cast(kind), dest, vec![logic], flags))
}

/// `foldCastedBitwiseLogic` for the `and`, `or` or `xor` `inst`.
pub fn fold_casted_bitwise_logic(
    c: &mut Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let Opcode::Binary(op @ (BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)) = c.function.instruction(inst).opcode
    else {
        return None;
    };
    let (cast0, kind, source0) = cast_of(c, op0)?;
    let src_ty = c.type_of(source0);
    // The source must be an integer to narrow the logic to it.
    c.context.types.int_bits(src_ty)?;
    if let Some(found) = fold_logic_cast_constant(c, inst, op0, op1, kind, source0) {
        return Some(found);
    }
    let (cast1, kind1, source1) = cast_of(c, op1)?;
    // The casts must be of one kind to reduce.
    if kind != kind1 {
        return None;
    }
    // Neither with one use: no profit.
    if !has_one_use(c, op0) && !has_one_use(c, op1) {
        return None;
    }
    let dest = c.function.instruction(inst).ty;
    let flags = c.function.instruction(inst).flags;
    if matches!(kind, CastOp::ZExt | CastOp::SExt) {
        let (mut x, mut y) = (source0, source1);
        let (x_bits, y_bits) = (width(c, x), width(c, y));
        if x_bits != y_bits {
            // Widen the narrower source only when both casts are one-use.
            if !has_one_use(c, op0) || !has_one_use(c, op1) {
                return None;
            }
            if x_bits < y_bits {
                let ty = c.type_of(y);
                x = c.create_cast(inst, kind, x, ty, Flags::default());
            } else {
                let ty = c.type_of(x);
                y = c.create_cast(inst, kind, y, ty, Flags::default());
            }
        }
        let mut narrow = Flags::default();
        if flags.contains(Flags::DISJOINT) {
            narrow.insert(Flags::DISJOINT);
        }
        let logic = c.create_binop(inst, op, x, y, narrow);
        return Some(c.replace_with_new(inst, Opcode::Cast(kind), dest, vec![logic], Flags::default()));
    }
    // Other casts: the source types must agree.
    if src_ty != c.type_of(source1) {
        return None;
    }
    if !(should_optimize_cast(c, cast0, source0) && should_optimize_cast(c, cast1, source1)) {
        return None;
    }
    let logic = c.create_binop(inst, op, source0, source1, Flags::default());
    let (f0, f1) = (c.function.instruction(cast0).flags, c.function.instruction(cast1).flags);
    let mut cast_flags = Flags::default();
    if kind == CastOp::Trunc {
        let nuw = if op == BinaryOp::And {
            f0.contains(Flags::NUW) || f1.contains(Flags::NUW)
        } else {
            f0.contains(Flags::NUW) && f1.contains(Flags::NUW)
        };
        if nuw {
            cast_flags.insert(Flags::NUW);
        }
        if f0.contains(Flags::NSW) && f1.contains(Flags::NSW) {
            cast_flags.insert(Flags::NSW);
        }
    }
    Some(c.replace_with_new(inst, Opcode::Cast(kind), dest, vec![logic], cast_flags))
}

#[cfg(test)]
#[path = "combine_casted_logic_tests.rs"]
mod tests;
