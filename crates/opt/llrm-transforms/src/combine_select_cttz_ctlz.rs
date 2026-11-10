//! `foldSelectInstWithICmp`'s count-zeros step: a select that guards `cttz` or
//! `ctlz` against a zero input is the intrinsic with its zero case defined.
//!
//! Translation of
//! [`foldSelectCttzCtlz`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L1561)
//! at d1106deb71cc, called from
//! [`foldSelectInstWithICmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L2480).
//!
//! MIRROR: an equality compare; the count, optionally under a `zext` or
//! `trunc`, is of `X` with `X == 0`, `X == -1` for `~X`, or `X == Y` for
//! `X ^ Y`; the arm taken at zero being the count's bit width gives the count
//! (its poison-generating annotations dropped); any other arm, with the count
//! and its extension each used once, relaxes the count to "zero is poison"
//! and drops its UB-implying attributes.
//!
//! DEVIATE 1 (2026-11-09): the relax case ends the visit with `Changed`; the
//! pin keeps trying the later folds of `foldSelectInstWithICmp` on the same
//! visit. The select is revisited, so they run then. Exit: the fold list can
//! tell "changed, go on".
//!
//! # Used by
//! `visitSelectInst`, after `canonicalizeSPF`.
//!
//! # Example
//! ```text
//! %c = icmp eq i32 %x, 0                       %r = call i32 @llvm.ctlz.i32(i32 %x, i1 false)
//! %z = call i32 @llvm.ctlz.i32(i32 %x, i1 true)
//! %r = select i1 %c, i32 32, i32 %z      ==>
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{drop_poison_generating_annotations, has_one_use, query, replace_operand};
use crate::combine_visit_select::Sel;

/// `m_Cttz(m_Value(X), m_Value())` or `m_Ctlz(..)`: the call and `X`.
fn count_zeros(
    c: &Combiner,
    v: Operand,
) -> Option<(InstId, Operand)> {
    let q = query(c, None);
    ["llvm.cttz", "llvm.ctlz"]
        .into_iter()
        .find_map(
            |base| {
                let (call, args) = pattern::intrinsic_call(&q, v, base)?;
                let [x, _] = *args else { return None };
                Some((call, x))
            },
        )
}

/// `foldSelectCttzCtlz(ICI, TrueVal, FalseVal)`.
pub(crate) fn select_cttz_ctlz(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let q = query(c, Some(inst));
    let (_, _, pred, cmp_lhs, cmp_rhs) = pattern::icmp(&q, sel.cond)?;
    // The select condition compares a value for equality.
    if !matches!(pred.pred, IntPredicate::Eq | IntPredicate::Ne) {
        return None;
    }
    let (mut select_arg, mut value_on_zero) = (sel.if_false, sel.if_true);
    if pred.pred == IntPredicate::Ne {
        std::mem::swap(&mut select_arg, &mut value_on_zero);
    }

    // Skip a zero extend or truncate.
    let count = pattern::cast(&q, select_arg, CastOp::ZExt)
        .or_else(|| pattern::cast(&q, select_arg, CastOp::Trunc))
        .map_or(select_arg, |(_, _, count)| count);

    // `count` is a call of cttz or ctlz, and its input is the left side of the
    // compare (or relates to the sides as below).
    let (call, x) = count_zeros(c, count)?;
    let q = query(c, Some(inst));
    let is_int = |v: Operand, value: u128| q.int_constant(v).is_some_and(|(bits, _)| bits == value);
    // (X == 0) ? BitWidth : ctz(X)
    // (X == -1) ? BitWidth : ctz(~X)
    // (X == Y) ? BitWidth : ctz(X ^ Y)
    let xor_of_sides = pattern::binary(&q, x, BinaryOp::Xor)
        .is_some_and(|(_, _, a, b)| (a == cmp_lhs && b == cmp_rhs) || (a == cmp_rhs && b == cmp_lhs));
    if !((x == cmp_lhs && is_int(cmp_rhs, 0))
        || (pattern::not(&q, x) == Some(cmp_lhs) && pattern::all_ones(&q, cmp_rhs))
        || xor_of_sides)
    {
        return None;
    }

    // The value propagated on zero is the bit width of `count`.
    let bits = u128::from(q.scalar_bits(count));
    if is_int(value_on_zero, bits) {
        // A range annotation on the intrinsic may no longer be valid.
        drop_poison_generating_annotations(c, call);
        c.add_to_worklist(call);
        return Some(Visit::Replace(select_arg));
    }

    // The value on zero is not the bit width. If the count (and its extension)
    // end at the select, the count is unused for a zero input: zero is poison.
    let flag_is_true = {
        let flag = c.function.instruction(call).operands[1];
        is_int(flag, 1)
    };
    if has_one_use(c, count) && has_one_use(c, select_arg) && !flag_is_true {
        let one = c.int(c.context.types.int(1), 1);
        replace_operand(c, call, 1, one);
        // `noundef` on the intrinsic may no longer be valid.
        c.function.drop_ub_implying_attrs(call);
        c.add_to_worklist(call);
        return Some(Visit::Changed);
    }
    None
}

#[cfg(test)]
#[path = "combine_select_cttz_ctlz_tests.rs"]
mod tests;
