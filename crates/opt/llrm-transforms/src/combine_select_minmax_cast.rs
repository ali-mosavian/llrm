//! A min or max select whose arms are casts of the compared values: the cast
//! moves outside the select.
//!
//! Translation of the integer part of the `isMinOrMax` block of
//! [`visitSelectInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L5010)
//! at d1106deb71cc, behind `matchSelectPattern` with a `CastOp`.
//!
//! MIRROR: when the pattern's operands are not of the select's type, the select
//! is made again over them (`icmp` of the min/max predicate, `select`) and the
//! cast is applied to its result.
//!
//! DEVIATE 1: no float clamp (`CmpLHS != LHS`) rewrite and no `foldSPFofSPF`
//! before it; integers only. Exit 2026-11-12.
//!
//! # Used by
//! `visitSelectInst`.
//!
//! # Example
//! ```text
//! %c = icmp sgt i32 %x, 255                  %c2 = icmp slt i32 %x, 255
//! %t = trunc i32 %x to i8              ==>   %m  = select i1 %c2, i32 %x, i32 255
//! %r = select i1 %c, i8 -1, i8 %t            %r  = trunc i32 %m to i8
//! ```

use llrm_mir::opcode::IntPredicate;
use llrm_mir::valuetracking::pattern;
use llrm_mir::valuetracking::range::{FastMathFlags, SelectOperand, SelectPatternFlavor, match_decomposed_select_pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;
use crate::combine_visit_select::Sel;

pub(crate) fn min_max_cast(
    c: &mut Combiner,
    inst: llrm_mir::module::InstId,
    sel: &Sel,
) -> Option<Visit> {
    c.context.types.int_bits(sel.ty)?;
    let q = query(c, Some(inst));
    let (cmp, ..) = pattern::icmp(&q, sel.cond)?;
    let found = match_decomposed_select_pattern(&q, cmp, sel.if_true, sel.if_false, FastMathFlags::default(), true, 0);
    let pred = match found.flavor {
        SelectPatternFlavor::SMin => IntPredicate::Slt,
        SelectPatternFlavor::SMax => IntPredicate::Sgt,
        SelectPatternFlavor::UMin => IntPredicate::Ult,
        SelectPatternFlavor::UMax => IntPredicate::Ugt,
        _ => return None,
    };
    let value = |one: Option<SelectOperand>, c: &mut Combiner| match one? {
        SelectOperand::Value(operand) => Some(operand),
        SelectOperand::Int { ty, bits } => Some(c.int(ty, bits)),
        _ => None,
    };
    let (lhs, rhs) = (value(found.lhs, c)?, value(found.rhs, c)?);
    if c.type_of(lhs) == sel.ty {
        return None;
    }
    let cast = found.cast_op?;
    let cond = c.create_icmp(inst, pred, lhs, rhs);
    let min_max = c.create_select(inst, cond, lhs, rhs);
    Some(Visit::Replace(c.create_cast(inst, cast, min_max, sel.ty, llrm_mir::opcode::Flags::default())))
}

#[cfg(test)]
#[path = "combine_select_minmax_cast_tests.rs"]
mod tests;
