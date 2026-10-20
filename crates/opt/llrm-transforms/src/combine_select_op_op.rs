//! `foldSelectOpOp`: `select c, (op x, a), (op x, b)` as `op x, (select c, a,
//! b)`.
//!
//! Translation of
//! [`InstCombinerImpl::foldSelectOpOp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L274)
//! at d1106deb71cc, for arms of one opcode: two casts of one source type, two
//! `fneg`s, an `icmp` pair with a common operand, a binary operator or
//! two-operand GEP pair with one.
//!
//! MIRROR: the one-use conditions, which operand is common (a commutative
//! operation may match across), the `freeze` of a condition that could be
//! poison before a division moves under it, the flags both arms had.
//!
//! DEVIATE 1: no vectors. Min/max intrinsics with a common operand, and
//! `ldexp`, are not made: the Builder cannot declare an intrinsic
//! (`combine_build` DEVIATE 2), and a call is cloned only with its own callee.
//!
//! DEVIATE 2: the two `icmp`s match when their predicates are equal (or equal
//! once one is swapped): `CmpPredicate::getMatching`'s `samesign` cases are not
//! found (MIR's `icmp` has none).
//!
//! # Example
//! ```text
//! %x1 = add i16 %x, %a                  %v = select i1 %c, i16 %a, i16 %b
//! %x2 = add i16 %x, %b            ==>   %r = add i16 %x, %v
//! %r = select i1 %c, i16 %x1, i16 %x2
//! ```

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_poison;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};
use crate::combine_visit_select::Sel;

/// The operand two arms share, and what each has besides it.
struct Common {
    shared: Operand,
    other_true: Operand,
    other_false: Operand,
    /// The shared operand is the first of the true arm's.
    match_is_zero: bool,
}

/// `getCommonOp`: `commute` lets the operands match crosswise (either
/// order), `swapped` makes only the crosswise match.
fn common_operand(
    on_true: [Operand; 2],
    on_false: [Operand; 2],
    commute: bool,
    swapped: bool,
) -> Option<Common> {
    if !swapped {
        if on_true[0] == on_false[0] {
            return Some(Common {
                shared: on_true[0],
                other_true: on_true[1],
                other_false: on_false[1],
                match_is_zero: true,
            });
        }
        if on_true[1] == on_false[1] {
            return Some(Common {
                shared: on_true[1],
                other_true: on_true[0],
                other_false: on_false[0],
                match_is_zero: false,
            });
        }
    }
    if !commute && !swapped {
        return None;
    }
    // The crosswise match: the true arm's operand 0 is the false arm's 1.
    if on_true[0] == on_false[1] {
        return Some(Common {
            shared: on_true[0],
            other_true: on_true[1],
            other_false: on_false[0],
            match_is_zero: true,
        });
    }
    if on_true[1] == on_false[0] {
        return Some(Common {
            shared: on_true[1],
            other_true: on_true[0],
            other_false: on_false[1],
            match_is_zero: false,
        });
    }
    None
}

fn is_commutative(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::FAdd | BinaryOp::FMul
    )
}

fn is_equality(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
}

fn swapped(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Eq | IntPredicate::Ne => pred,
        IntPredicate::Ugt => IntPredicate::Ult,
        IntPredicate::Uge => IntPredicate::Ule,
        IntPredicate::Ult => IntPredicate::Ugt,
        IntPredicate::Ule => IntPredicate::Uge,
        IntPredicate::Sgt => IntPredicate::Slt,
        IntPredicate::Sge => IntPredicate::Sle,
        IntPredicate::Slt => IntPredicate::Sgt,
        IntPredicate::Sle => IntPredicate::Sge,
    }
}

/// `foldSelectOpOp(SI, TI, FI)` for arms that are instructions of one opcode.
pub(crate) fn select_op_op(
    c: &mut Combiner,
    inst: InstId,
    sel: &Sel,
) -> Option<Visit> {
    let on_true = c.definition_of(sel.if_true)?;
    let on_false = c.definition_of(sel.if_false)?;
    let (ti, fi) = (c.function.instruction(on_true).clone(), c.function.instruction(on_false).clone());
    if ti.opcode != fi.opcode {
        return None;
    }
    let (true_one_use, false_one_use) = (has_one_use(c, sel.if_true), has_one_use(c, sel.if_false));

    // The same cast of one source type.
    if let Opcode::Cast(op) = ti.opcode {
        if c.type_of(ti.operands[0]) != c.type_of(fi.operands[0]) || !true_one_use || !false_one_use {
            return None;
        }
        let select = c.select_like(inst, inst, [sel.cond, ti.operands[0], fi.operands[0]], Flags::default());
        return Some(c.replace_with_new(inst, Opcode::Cast(op), ti.ty, vec![select], Flags::default()));
    }

    if true_one_use || false_one_use {
        // Cond ? -X : -Y --> -(Cond ? X : Y)
        if ti.opcode == Opcode::FNeg {
            let flags = ti.flags.intersect(fi.flags);
            let select = c.select_like(inst, inst, [sel.cond, ti.operands[0], fi.operands[0]], flags);
            return Some(c.replace_with_new(inst, Opcode::FNeg, ti.ty, vec![select], flags));
        }
        // icmp with a common operand has it pulled after the select.
        if let (Opcode::ICmp(true_pred), Opcode::ICmp(false_pred)) = (&ti.opcode, &fi.opcode) {
            let (t_ops, f_ops) = ([ti.operands[0], ti.operands[1]], [fi.operands[0], fi.operands[1]]);
            for (pred, crosswise) in [
                (*true_pred == *false_pred).then_some(*true_pred),
                (*true_pred == swapped(*false_pred)).then_some(*true_pred),
            ]
            .into_iter()
            .zip([false, true])
            .filter_map(|(pred, crosswise)| pred.map(|pred| (pred, crosswise)))
            {
                let relational = !is_equality(pred);
                let Some(common) = common_operand(t_ops, f_ops, is_equality(pred), relational && crosswise) else {
                    continue;
                };
                let select =
                    c.select_like(inst, inst, [sel.cond, common.other_true, common.other_false], Flags::default());
                let pred = if common.match_is_zero { pred } else { swapped(pred) };
                return Some(Visit::Replace(c.create_icmp(inst, pred, common.shared, select)));
            }
        }
    }

    // Binary operators and two-operand GEPs, one use each.
    let two_operands = ti.operands.len() == 2 && fi.operands.len() == 2;
    let same_operation = match (&ti.opcode, &fi.opcode) {
        (Opcode::Binary(_), Opcode::Binary(_)) => true,
        (Opcode::GetElementPtr { source: a }, Opcode::GetElementPtr { source: b }) => a == b,
        _ => false,
    };
    if !two_operands || !same_operation || !true_one_use || !false_one_use {
        return None;
    }
    let commutes = matches!(ti.opcode, Opcode::Binary(op) if is_commutative(op));
    let common = common_operand([ti.operands[0], ti.operands[1]], [fi.operands[0], fi.operands[1]], commutes, false)?;

    // A division may be moved under a select only with a condition that
    // cannot be poison: a poison one chosen as zero would divide by zero.
    let mut cond = sel.cond;
    if let Opcode::Binary(op @ (BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem)) = ti.opcode
        && !is_guaranteed_not_to_be_poison(&query(c, Some(inst)), cond, 0)
        // A udiv/urem with a common divisor is safe: the zero is in the
        // original.
        && (matches!(op, BinaryOp::SDiv | BinaryOp::SRem) || common.match_is_zero)
    {
        let freeze = c.insert_before(inst, Opcode::Freeze, c.type_of(cond), vec![cond], Flags::default());
        cond = Operand::Value(c.function.instruction(freeze).result.expect("a value"));
    }

    let select = c.select_like(inst, inst, [cond, common.other_true, common.other_false], Flags::default());
    let (first, second) = if common.match_is_zero { (common.shared, select) } else { (select, common.shared) };
    let flags = ti.flags.intersect(fi.flags);
    Some(c.replace_with_new(inst, ti.opcode.clone(), ti.ty, vec![first, second], flags))
}

#[cfg(test)]
#[path = "combine_select_op_op_tests.rs"]
mod tests;
