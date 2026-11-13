//! `icmp pred min|max(X, Y), Z`.
//!
//! Translation of the pin's (d1106deb71cc, InstCombineCompares.cpp)
//! [`foldICmpWithMinMax`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5832),
//! called from `foldICmpCommutative` (L7810) for the min/max in either operand.
//!
//! MIRROR: the order of the checks and each table of the pin (equality, then
//! the eight relational predicates). `samesign` is dropped, as the pin does,
//! since a compare of `X` or `Y` against `Z` is speculated.
//!
//! # Used by
//! `combine_compares::visit_icmp`.
//!
//! # Example
//! ```text
//! %m = call i32 @llvm.umin.i32(i32 %x, i32 %y)
//! %r = icmp ult i32 %m, 7              ; %x u< 7 is known: true
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::IntPredicate;
use llrm_mir::simplify::simplify_icmp_operands;
use llrm_mir::valuetracking::{known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_icmp_const::new_icmp;
use crate::combine_shared::{min_max, query};

/// `foldICmpWithMinMax(I, MinMax, Z, Pred)`: `minmax` is the min/max call.
pub(crate) fn fold_icmp_with_min_max(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    minmax: Operand,
    z: Operand,
) -> Option<Visit> {
    let (mm_pred, mut x, mut y) = min_max(&query(c, None), minmax)?;
    if pred.is_signed() && !mm_pred.is_signed() {
        return None;
    }
    let mut pred = pred;
    if pred.is_unsigned() && mm_pred.is_signed() {
        // Revert the transform signed pred -> unsigned pred.
        let q = query(c, Some(inst));
        if known::is_known_non_negative(&q, z, 0) && known::is_known_non_negative(&q, minmax, 0) {
            pred = pattern::CmpPredicate::flipped_signedness(pred);
        } else {
            return None;
        }
    }
    // `IsCondKnownTrue(simplifyICmpInst(pred, a, b))`.
    let known_cmp = |c: &Combiner, pred: IntPredicate, a: Operand, b: Operand| -> Option<bool> {
        let found = simplify_icmp_operands(c.context, c.env.layout, c.function, pred, a, b)?;
        let q = query(c, Some(inst));
        if pattern::one(&q, found) {
            Some(true)
        } else if pattern::zero(&q, found) {
            Some(false)
        } else {
            None
        }
    };
    let mut cmp_xz = known_cmp(c, pred, x, z);
    let mut cmp_yz = known_cmp(c, pred, y, z);
    if cmp_xz.is_none() && cmp_yz.is_none() {
        return None;
    }
    if cmp_xz.is_none() {
        std::mem::swap(&mut x, &mut y);
        std::mem::swap(&mut cmp_xz, &mut cmp_yz);
    }
    let i1 = c.context.types.int(1);
    let result = |c: &mut Combiner, value: bool| Some(Visit::Replace(c.int(i1, u128::from(value))));
    match pred {
        IntPredicate::Eq | IntPredicate::Ne => {
            let equal = pred == IntPredicate::Eq;
            if equal == cmp_xz? {
                // min(X, Y) == Z with X == Z is X <= Y; the others by symmetry.
                let mut new_pred = mm_pred.non_strict();
                if !equal {
                    new_pred = new_pred.inverse();
                }
                return Some(new_icmp(c, inst, new_pred, x, y));
            }
            // Otherwise X != Z.
            let mut mm_cmp_xz = known_cmp(c, mm_pred, x, z);
            if mm_cmp_xz.is_none() {
                std::mem::swap(&mut x, &mut y);
                std::mem::swap(&mut cmp_xz, &mut cmp_yz);
                // Re-check pre-condition X != Z.
                if cmp_xz? == equal {
                    return None;
                }
                mm_cmp_xz = known_cmp(c, mm_pred, x, z);
            }
            match mm_cmp_xz? {
                // min(X, Y) == Z with X < Z is false; != is true.
                true => result(c, !equal),
                // min(X, Y) == Z with X > Z is Y == Z.
                false => Some(fold_into_cmp_yz(c, inst, pred, y, z, cmp_yz)),
            }
        }
        IntPredicate::Slt
        | IntPredicate::Ult
        | IntPredicate::Sle
        | IntPredicate::Ule
        | IntPredicate::Sgt
        | IntPredicate::Ugt
        | IntPredicate::Sge
        | IntPredicate::Uge => {
            let is_same = mm_pred == pred.strict();
            match (cmp_xz?, is_same) {
                // min(X, Y) < Z with X < Z: true.
                (true, true) => result(c, true),
                // max(X, Y) < Z with X < Z: Y < Z.
                (true, false) | (false, true) => Some(fold_into_cmp_yz(c, inst, pred, y, z, cmp_yz)),
                // max(X, Y) < Z with X >= Z: false.
                (false, false) => result(c, false),
            }
        }
    }
}

/// The pin's `FoldIntoCmpYZ`: the known answer for `Y pred Z`, else the
/// compare.
fn fold_into_cmp_yz(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    y: Operand,
    z: Operand,
    cmp_yz: Option<bool>,
) -> Visit {
    match cmp_yz {
        Some(value) => {
            let i1 = c.context.types.int(1);
            Visit::Replace(c.int(i1, u128::from(value)))
        }
        None => new_icmp(c, inst, pred, y, z),
    }
}

#[cfg(test)]
#[path = "combine_icmp_minmax_tests.rs"]
mod tests;
