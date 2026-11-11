//! `visitFNeg`: an `fneg` of a one-use value is pushed into the value.
//!
//! Translation of
//! [`InstCombinerImpl::visitFNeg`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L3191),
//! [`foldFNegIntoConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L3096)
//! and
//! [`hoistFNegAboveFMulFDiv`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L3157).
//!
//! MIRROR: the C++ order: `foldFNegIntoConstant`, `-(X - Y)` under `nsz`, then
//! (the operand having one use) the hoist above `fmul`/`fdiv`, then the select
//! arms. The flag algebra of `foldFNegIntoConstant` (`unionValue`,
//! `intersectRewrite`, `ninf` of both) is the pin's.
//!
//! DEVIATE 1: `simplifyFNegInst` is the generic simplifier `visit` runs first.
//!
//! DEVIATE 2: the `ldexp` arm of the hoist, `fneg (copysign x, y)`, and the
//! vector arms (shuffle, reverse) are not translated: MIR has no vectors, and
//! `copysign` and `ldexp` calls need `CreateCall`/`CreateCopySign`. The
//! `copyMetadata` of the `fdiv` arms is not done: MIR instructions carry no
//! metadata here. A constant other than a `float` or `double` is not negated
//! (`negated_constant`).
//!
//! # Used by
//! `combine::visitors` for `fneg`.
//!
//! # Example
//! ```text
//! %m = fmul float %x, 2.0 ; %n = fneg float %m   ==>   %n = fmul float %x, -2.0
//! %m = fmul float %x, %y  ; %n = fneg float %m   ==>   %0 = fneg float %y ; %n = fmul float %x, %0
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison;

use crate::combine::{Combiner, Visit};
use crate::combine_fp::{fneg_operand, is_constant, negated_constant};
use crate::combine_shared::{binop, has_one_use, query};

/// The flags that describe a value (`unionValue`'s mask).
fn value_flags() -> Flags {
    let mut f = Flags::NNAN;
    f.insert(Flags::NINF);
    f.insert(Flags::NSZ);
    f
}

/// The flags that permit a rewrite (`intersectRewrite`'s mask): `reassoc`,
/// `arcp`, `contract`, `afn`.
fn rewrite_flags() -> Flags {
    Flags::FAST.without(value_flags())
}

/// `flags` with `flag` set exactly when both `a` and `b` have it.
fn with_both(
    mut flags: Flags,
    flag: Flags,
    a: Flags,
    b: Flags,
) -> Flags {
    flags.remove(flag);
    if a.contains(flag) && b.contains(flag) {
        flags.insert(flag);
    }
    flags
}

/// `Builder.CreateFNegFMF(v, flags)`: what `simplifyFNegInst` finds (a negated
/// constant, the operand of an `fneg`), else a new `fneg` before `at`.
fn create_fneg(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    flags: Flags,
) -> Operand {
    if let Some(negated) = negated_constant(c, v) {
        return negated;
    }
    if let Some(inner) = fneg_operand(c, v) {
        return inner;
    }
    let ty = c.type_of(v);
    let new = c.insert_before(at, Opcode::FNeg, ty, vec![v], flags);
    c.value_of(new)
}

/// `visitFNeg`.
pub fn visit_fneg(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let instruction = c.function.instruction(inst).clone();
    let op = instruction.operands[0];
    let flags = instruction.flags;
    if let Some(found) = fold_fneg_into_constant(c, inst, op, flags) {
        return found;
    }
    // If we can ignore the sign of zeros: -(X - Y) --> (Y - X)
    if flags.contains(Flags::NSZ) && has_one_use(c, op) {
        if let Some((x, y, _)) = binop(&query(c, Some(inst)), op, BinaryOp::FSub) {
            return c.replace_with_binop(inst, BinaryOp::FSub, y, x, flags);
        }
    }
    if !has_one_use(c, op) {
        return Visit::None;
    }
    if let Some(found) = hoist_fneg_above_fmul_fdiv(c, inst, op, flags) {
        return found;
    }
    fold_fneg_of_select(c, inst, op, flags).unwrap_or(Visit::None)
}

/// `foldFNegIntoConstant`: `-(X * C)`, `-(X / C)`, `-(C / X)`, and under
/// `nsz` `-(X + C)`, with the negation folded into the constant.
fn fold_fneg_into_constant(
    c: &mut Combiner,
    inst: InstId,
    op: Operand,
    flags: Flags,
) -> Option<Visit> {
    if !has_one_use(c, op) {
        return None;
    }
    let q = query(c, Some(inst));
    let (_, def) = q.instruction_of(op)?;
    let (def_flags, operands) = (def.flags, def.operands.clone());
    let Opcode::Binary(kind) = def.opcode else { return None };
    let (a, b) = (*operands.first()?, *operands.get(1)?);
    match kind {
        // -(X * C) --> X * (-C)
        BinaryOp::FMul if is_constant(b) => {
            let negated = negated_constant(c, b)?;
            let mut fmf = flags.intersect(value_flags()).union(def_flags.intersect(value_flags()));
            fmf.insert(flags.intersect(def_flags).intersect(rewrite_flags()));
            let fmf = with_both(fmf, Flags::NINF, flags, def_flags);
            Some(c.replace_with_binop(inst, BinaryOp::FMul, a, negated, fmf))
        }
        // -(X / C) --> X / (-C); -(C / X) --> (-C) / X
        BinaryOp::FDiv if is_constant(b) || is_constant(a) => {
            let (x, constant_second) = if is_constant(b) { (a, true) } else { (b, false) };
            let negated = negated_constant(c, if constant_second { b } else { a })?;
            // The special value exceptions may not apply to the fdiv: nsz and
            // ninf survive only if both have them.
            let fmf = with_both(with_both(flags, Flags::NSZ, flags, def_flags), Flags::NINF, flags, def_flags);
            Some(if constant_second {
                c.replace_with_binop(inst, BinaryOp::FDiv, x, negated, fmf)
            } else {
                c.replace_with_binop(inst, BinaryOp::FDiv, negated, x, fmf)
            })
        }
        // With NSZ: -(X + C) --> -C - X
        BinaryOp::FAdd if flags.contains(Flags::NSZ) && is_constant(b) => {
            let negated = negated_constant(c, b)?;
            Some(c.replace_with_binop(inst, BinaryOp::FSub, negated, a, flags))
        }
        _ => None,
    }
}

/// `hoistFNegAboveFMulFDiv`: `-(X * Y)` is `X * -Y` (the constant or the
/// `fneg` more likely to simplify goes on the right) and `-(X / Y)` is
/// `-X / Y`.
fn hoist_fneg_above_fmul_fdiv(
    c: &mut Combiner,
    inst: InstId,
    op: Operand,
    flags: Flags,
) -> Option<Visit> {
    let q = query(c, Some(inst));
    if let Some((x, y, _)) = binop(&q, op, BinaryOp::FMul) {
        let negated = create_fneg(c, inst, y, flags);
        return Some(c.replace_with_binop(inst, BinaryOp::FMul, x, negated, flags));
    }
    if let Some((x, y, _)) = binop(&q, op, BinaryOp::FDiv) {
        let negated = create_fneg(c, inst, x, flags);
        return Some(c.replace_with_binop(inst, BinaryOp::FDiv, negated, y, flags));
    }
    None
}

/// Eliminate the `fneg` if at least one arm of the select is negated:
/// `-(C ? -P : Y)`, `-(C ? X : -P)` and `-(C ? X : K)` with a constant arm.
fn fold_fneg_of_select(
    c: &mut Combiner,
    inst: InstId,
    op: Operand,
    flags: Flags,
) -> Option<Visit> {
    let q = query(c, Some(inst));
    let (_, def) = q.instruction_of(op)?;
    if def.opcode != Opcode::Select {
        return None;
    }
    let (select_flags, cond, x, y) = (def.flags, def.operands[0], def.operands[1], def.operands[2]);
    // Not safe to propagate nsz unless the original select has it: union the
    // flags and remove nsz if needed.
    let select_fmf = |c: &Combiner, common_operand: bool| -> Flags {
        let mut fmf = flags.union(select_flags);
        if !select_flags.contains(Flags::NSZ)
            && !common_operand
            && !is_guaranteed_not_to_be_undef_or_poison(&query(c, Some(inst)), cond, 0)
        {
            fmf.remove(Flags::NSZ);
        }
        fmf
    };
    let ty = c.type_of(x);
    // -(Cond ? -P : Y) --> Cond ? P : -Y
    if let Some(p) = fneg_operand(c, x) {
        let negated = create_fneg(c, inst, y, flags);
        let fmf = select_fmf(c, p == y);
        return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![cond, p, negated], fmf));
    }
    // -(Cond ? X : -P) --> Cond ? -X : P
    if let Some(p) = fneg_operand(c, y) {
        let negated = create_fneg(c, inst, x, flags);
        let fmf = select_fmf(c, p == x);
        return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![cond, negated, p], fmf));
    }
    // -(Cond ? X : C) --> Cond ? -X : -C; -(Cond ? C : Y) --> Cond ? -C : -Y
    if is_constant(x) || is_constant(y) {
        let negated_x = create_fneg(c, inst, x, flags);
        let negated_y = create_fneg(c, inst, y, flags);
        let fmf = select_fmf(c, true);
        return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![cond, negated_x, negated_y], fmf));
    }
    None
}

#[cfg(test)]
#[path = "combine_fneg_tests.rs"]
mod tests;
