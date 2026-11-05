//! `visitSelectInst`'s min/max canonicalisation: a select that is a min or a
//! max of values behind a cast is rebuilt on the uncast values and the cast is
//! moved outside.
//!
//! Translation of
//! [`visitSelectInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4991)
//! at d1106deb71cc (the `isMinOrMax` block).
//!
//! MIRROR: `matchSelectPattern` with a `CastOp`; the compare is rebuilt with
//! `getMinMaxPred` (a floating-point one carries the old compare's fast-math
//! flags); the select takes the old one's `prof`; the result is cast with the
//! pattern's `CastOp`.
//!
//! DEVIATE 1: `foldSPFofSPF` (the step before this block) is not translated.
//!
//! # Used by
//! `visitSelectInst`.
//!
//! # Example
//! ```text
//! %c = fcmp oge float %p, 6.5535e4                %q = select i1 %c, float 6.5535e4, float %p
//! %t = fptosi float %p to i64              ==>    %r = fptosi float %q to i64
//! %r = select i1 %c, i64 65535, i64 %t
//! ```

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::range::{Predicate, SelectOperand, get_min_max_pred, match_select_pattern_cast};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;
use crate::combine_visit_select::Sel;

fn operand_of(
    c: &mut Combiner,
    one: Option<SelectOperand>,
) -> Option<Operand> {
    match one? {
        SelectOperand::Value(operand) => Some(operand),
        SelectOperand::Int { ty, bits } => Some(c.int(ty, bits)),
        SelectOperand::Float { ty, bits } => {
            Some(Operand::Constant(c.context.constant(Constant { ty, kind: ConstantKind::Float(bits) })))
        }
        SelectOperand::Poison(_) => None,
    }
}

/// The min/max block of `visitSelectInst` (:4991-:5035).
pub(crate) fn select_min_max_cast(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let is_float = matches!(c.context.types.get(sel.ty), Type::Float(_));
    if c.context.types.int_bits(sel.ty).is_none() && !is_float {
        return None;
    }
    let (cond_inst, cond) = {
        let (id, instruction) = query(c, None).instruction_of(sel.cond)?;
        (id, instruction.clone())
    };
    let found = match_select_pattern_cast(&query(c, Some(inst)), c.value_of(inst).into(), true, 0);
    if !found.flavor.is_min_or_max() {
        return None;
    }
    let (lhs, rhs) = (operand_of(c, found.lhs)?, operand_of(c, found.rhs)?);
    let is_cast_needed = c.type_of(lhs) != sel.ty;
    let [cmp_lhs, cmp_rhs] = cond.operands[..] else { return None };
    let float_clamp = matches!(c.context.types.get(c.type_of(lhs)), Type::Float(_))
        && ((cmp_lhs != lhs && cmp_lhs != rhs) || (cmp_rhs != lhs && cmp_rhs != rhs));
    if !is_cast_needed && !float_clamp {
        return None;
    }
    let cmp = match get_min_max_pred(found.flavor, found.ordered) {
        Predicate::Int(pred) => c.create_icmp(inst, pred, lhs, rhs),
        Predicate::Float(pred) => {
            let ty = c.context.types.int(1);
            let fmf = c.function.instruction(cond_inst).flags.intersect(Flags::FAST);
            let new = c.insert_before(inst, Opcode::FCmp(pred), ty, vec![lhs, rhs], fmf);
            c.value_of(new)
        }
    };
    let select = c.select_like(inst, inst, [cmp, lhs, rhs], Flags::default());
    if !is_cast_needed {
        return Some(Visit::Replace(select));
    }
    let cast = c.create_cast(inst, found.cast_op?, select, sel.ty, Flags::default());
    Some(Visit::Replace(cast))
}

#[cfg(test)]
#[path = "combine_select_minmax_cast_tests.rs"]
mod tests;
