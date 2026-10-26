//! `foldSelectInstWithICmp`'s first step: a select that is a min, a max or an
//! absolute value of a compare becomes the intrinsic.
//!
//! Translation of
//! [`canonicalizeSPF`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1623)
//! at d1106deb71cc, reached from
//! [`foldSelectInstWithICmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2419).
//!
//! MIRROR: `matchDecomposedSelectPattern` of the compare and the two arms;
//! `smin`, `smax`, `umin` and `umax` for the min/max flavors; `abs` (poison at
//! the minimum when the other arm is the `nsw` negation of the first) for the
//! absolute value, with `0 - abs` for its negation; an absolute value is
//! skipped when neither the compare nor its negation has one use.
//!
//! DEVIATE 1: `foldSelectInstWithICmp`'s other steps
//! (`foldSelectInstWithICmpConst`, `canonicalizeClampLike`, ...) are not
//! translated here; the min/max of `FoldSelectWithAndOrCond` is the next strip.
//!
//! # Used by
//! `visitSelectInst`.
//!
//! # Example
//! ```text
//! %c = icmp slt i16 %x, 0                 %r = call i16 @llvm.smax.i16(i16 %x, i16 0)
//! %r = select i1 %c, i16 0, i16 %x   ==>
//! ```

use llrm_mir::module::InstId;
use llrm_mir::module::Operand;
use llrm_mir::valuetracking::pattern;
use llrm_mir::valuetracking::range::{
    FastMathFlags, SelectOperand, SelectPatternFlavor, match_decomposed_select_pattern,
};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};
use crate::combine_visit_select::Sel;

/// The operand a pattern result names: a value, or a constant it made.
fn operand_of(
    c: &mut Combiner,
    one: Option<SelectOperand>,
) -> Option<Operand> {
    match one? {
        SelectOperand::Value(operand) => Some(operand),
        SelectOperand::Int { ty, bits } => Some(c.int(ty, bits)),
        _ => None,
    }
}

/// `canonicalizeSPF(Cmp, TrueVal, FalseVal)` for a select on an `icmp`.
pub(crate) fn select_with_icmp(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    c.context.types.int_bits(sel.ty)?;
    let q = query(c, Some(inst));
    let (cmp, ..) = pattern::icmp(&q, sel.cond)?;
    let found = match_decomposed_select_pattern(&q, cmp, sel.if_true, sel.if_false, FastMathFlags::default(), false, 0);
    let flavor = found.flavor;
    if flavor == SelectPatternFlavor::None {
        return None;
    }
    let (lhs, rhs) = (operand_of(c, found.lhs)?, operand_of(c, found.rhs)?);
    let base = match flavor {
        SelectPatternFlavor::SMin => "smin",
        SelectPatternFlavor::SMax => "smax",
        SelectPatternFlavor::UMin => "umin",
        SelectPatternFlavor::UMax => "umax",
        SelectPatternFlavor::Abs | SelectPatternFlavor::NAbs => {
            if !has_one_use(c, sel.cond) && !has_one_use(c, rhs) {
                return None;
            }
            let q = query(c, Some(inst));
            let int_min_is_poison = flavor == SelectPatternFlavor::Abs
                && pattern::neg(&q, rhs)
                    .is_some_and(|(_, x, flags)| x == lhs && flags.contains(llrm_mir::opcode::Flags::NSW));
            let flag = c.int(c.context.types.int(1), u128::from(int_min_is_poison));
            let abs = c.create_intrinsic(inst, "abs", sel.ty, &[lhs, flag]);
            if flavor == SelectPatternFlavor::NAbs {
                let zero = c.int(sel.ty, 0);
                return Some(Visit::Replace(c.create_sub(inst, zero, abs)));
            }
            return Some(Visit::Replace(abs));
        }
        _ => return None,
    };
    Some(Visit::Replace(c.create_intrinsic(inst, base, sel.ty, &[lhs, rhs])))
}

#[cfg(test)]
#[path = "combine_select_spf_tests.rs"]
mod tests;
