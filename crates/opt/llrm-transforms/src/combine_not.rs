//! `InstCombinerImpl::foldNot`: a `not` (`xor X, -1`) pushed into its operand.
//!
//! Translation of
//! [`foldNot`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L5237)
//! at d1106deb71cc, called by `visit_xor` where `visitXor` calls it.
//!
//! MIRROR: the order of the arms, and the one-use condition of each.
//!
//! DEVIATE 1: no vectors, and only integer constants are inverted.
//!
//! DEVIATE 2: owed, each where it stood: the `bitcast` arms (a `not` of a
//! `bitcast` of a compare, of a `sext` of a bool), min/max and `is_fpclass`
//! intrinsics, `foldNotXor`, and `fcmp`.
//!
//! DEVIATE 3: `sinkNotIntoLogicalOp` does not copy or swap the select's
//! `!prof`.
//!
//! # Example
//! `~(X + 5)` is `-6 - X`:
//!
//! ```text
//! %a = add i32 %x, 5
//! %r = xor i32 %a, -1   ==>   %r = sub i32 -6, %x
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};

/// `m_c_<Op>(F, m_Value(Other))`: `v` is `op a, b` where `f` matches `a`, else
/// `b`; what it matched, and the other.
fn commutative<T>(
    q: &Query,
    v: Operand,
    op: BinaryOp,
    f: impl Fn(Operand) -> Option<T>,
) -> Option<(T, Operand)> {
    let (_, _, a, b) = pattern::binary(q, v, op)?;
    f(a).map(|found| (found, b)).or_else(|| f(b).map(|found| (found, a)))
}

/// The `bits`-wide value `~constant`, if `v` is an integer constant.
fn inverted_constant(
    c: &Combiner,
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    let (bits, width) = q.int_constant(v)?;
    Some(c.int(c.type_of(v), !bits & mask(width)))
}

fn is_negative_constant(
    q: &Query,
    v: Operand,
) -> Option<bool> {
    let (bits, width) = q.int_constant(v)?;
    Some(bits >> (width - 1) & 1 == 1)
}

/// `foldNot` (:5237) for the `xor` `inst`.
pub(crate) fn fold_not(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let result = Operand::Value(c.function.instruction(inst).result?);
    let not_op = pattern::not(&query(c, Some(inst)), result)?;
    let ty = c.type_of(result);
    let one_use = has_one_use(c, not_op);

    // DeMorgan's Law for 'nand' / 'nor' logic with an inverted operand. We
    // must eliminate the and/or (one-use) for these transforms to not
    // increase the instruction count.
    if one_use {
        let q = query(c, Some(inst));
        let not_of = |x: Operand| pattern::not(&q, x);
        // ~(~X & Y) --> (X | ~Y)
        let and = commutative(&q, not_op, BinaryOp::And, not_of);
        // ~(~X | Y) --> (X & ~Y)
        let or = commutative(&q, not_op, BinaryOp::Or, not_of);
        let logical_and = pattern::logical_and(&q, not_op).and_then(|(a, b)| Some((not_of(a)?, b)));
        let logical_or = pattern::logical_or(&q, not_op).and_then(|(a, b)| Some((not_of(a)?, b)));
        if let Some((x, y)) = and {
            let not_y = c.create_not(inst, y);
            return Some(c.replace_with_binop(inst, BinaryOp::Or, x, not_y, Flags::default()));
        }
        if let Some((x, y)) = logical_and {
            let not_y = c.create_not(inst, y);
            let on_true = c.all_ones(ty);
            return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![x, on_true, not_y], Flags::default()));
        }
        if let Some((x, y)) = or {
            let not_y = c.create_not(inst, y);
            return Some(c.replace_with_binop(inst, BinaryOp::And, x, not_y, Flags::default()));
        }
        if let Some((x, y)) = logical_or {
            let not_y = c.create_not(inst, y);
            let on_false = c.int(ty, 0);
            return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![x, not_y, on_false], Flags::default()));
        }
    }

    // Is this a 'not' (~) fed by a binary operator?
    if let Some(found) = fold_not_of_binary(c, inst, not_op, ty) {
        return Some(found);
    }

    // not (cmp A, B) = !cmp A, B
    if let Some(compare) = c.definition_of(not_op)
        && let Opcode::ICmp(pred) = c.function.instruction(compare).opcode
        && one_use
    {
        c.function.set_icmp_predicate(compare, pred.inverse());
        c.add_to_worklist(compare);
        return Some(Visit::Replace(not_op));
    }

    // not (and/or (free to invert x), (free to invert y)) = (~x) or/and (~y)
    if sink_not_into_logical_op(c, inst, not_op) {
        return Some(Visit::Changed);
    }

    if one_use && let Some(found) = fold_not_of_select(c, not_op) {
        return Some(found);
    }

    // TODO (pin): could handle multi-use better by checking if all uses of
    // NotOp (other than I) can be inverted.
    let mut consumes = false;
    let inverted = c.get_freely_inverted(inst, not_op, one_use, true, &mut consumes)?;
    Some(Visit::Replace(inverted))
}

/// The arms of `foldNot` for a `not` of a binary operator.
fn fold_not_of_binary(
    c: &mut Combiner,
    inst: InstId,
    not_op: Operand,
    ty: llrm_mir::TypeId,
) -> Option<Visit> {
    let (not_val, op, flags, a, b) = {
        let q = query(c, Some(inst));
        let (id, instruction) = q.instruction_of(not_op)?;
        let Opcode::Binary(op) = instruction.opcode else { return None };
        (id, op, instruction.flags, instruction.operands[0], instruction.operands[1])
    };
    let one_use = has_one_use(c, not_op);
    let width = c.context.types.int_bits(ty)?;
    let q = query(c, Some(inst));
    let not_of = |x: Operand| pattern::not(&q, x);
    let default = Flags::default();
    match op {
        // ~((-X) | Y) --> (X - 1) & (~Y)
        BinaryOp::Or if one_use => {
            let neg = |x: Operand| (has_one_use(c, x)).then(|| pattern::neg(&q, x).map(|(_, x, _)| x)).flatten();
            let (x, y) = commutative(&q, not_op, BinaryOp::Or, neg)?;
            let ones = c.all_ones(ty);
            let dec_x = c.create_binop(inst, BinaryOp::Add, x, ones, default);
            let not_y = c.create_not(inst, y);
            Some(c.replace_with_binop(inst, BinaryOp::And, dec_x, not_y, default))
        }
        BinaryOp::AShr => {
            // ~(~X >>s Y) --> (X >>s Y)
            if let Some(x) = not_of(a) {
                return Some(c.replace_with_binop(inst, BinaryOp::AShr, x, b, default));
            }
            // Bit-hack form of a signbit test for iN type:
            // ~(X >>s (N - 1)) --> sext i1 (X > -1) to iN
            if one_use && q.int_constant(b).is_some_and(|(shift, _)| shift == u128::from(width - 1)) {
                let ones = c.all_ones(ty);
                let is_not_neg = c.create_icmp(inst, IntPredicate::Sgt, a, ones);
                return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::SExt), ty, vec![is_not_neg], default));
            }
            // ~(C >>s Y) --> ~C >>u Y (when inverting the replicated sign bits)
            if is_negative_constant(&q, a) == Some(true) {
                let inverted = inverted_constant(c, &q, a)?;
                return Some(c.replace_with_binop(inst, BinaryOp::LShr, inverted, b, default));
            }
            None
        }
        BinaryOp::LShr => {
            // Treat lshr with non-negative operand as ashr.
            // ~(~X >>u Y) --> (X >>s Y) iff X is known negative
            if let Some(x) = not_of(a)
                && known::is_known_negative(&query(c, Some(not_val)), x, 0)
            {
                return Some(c.replace_with_binop(inst, BinaryOp::AShr, x, b, default));
            }
            // ~(C >>u Y) --> ~C >>s Y (when inverting the replicated sign bits)
            if is_negative_constant(&q, a) == Some(false) {
                let inverted = inverted_constant(c, &q, a)?;
                return Some(c.replace_with_binop(inst, BinaryOp::AShr, inverted, b, default));
            }
            None
        }
        BinaryOp::Add => {
            // ~(X + C) --> ~C - X
            if pattern::imm_constant(&q, b) {
                let inverted = inverted_constant(c, &q, b)?;
                return Some(c.replace_with_binop(inst, BinaryOp::Sub, inverted, a, default));
            }
            // ~(~X + Y) --> X - Y
            let (x, y) = commutative(&q, not_op, BinaryOp::Add, not_of)?;
            Some(c.replace_with_binop(inst, BinaryOp::Sub, x, y, flags))
        }
        // ~(X - Y) --> ~X + Y
        // FIXME: is it really beneficial to sink the `not` here?
        BinaryOp::Sub if matches!(a, Operand::Constant(_)) || one_use => {
            let not_x = c.create_not(inst, a);
            Some(c.replace_with_binop(inst, BinaryOp::Add, not_x, b, default))
        }
        _ => None,
    }
}

/// Pull 'not' into operands of select if both operands are one-use compares
/// or one is one-use compare and the other one is a constant. Inverting the
/// predicates eliminates the 'not' operation.
fn fold_not_of_select(
    c: &mut Combiner,
    not_op: Operand,
) -> Option<Visit> {
    let select = c.definition_of(not_op).filter(|&def| c.function.instruction(def).opcode == Opcode::Select)?;
    let operands = c.function.instruction(select).operands.clone();
    let q = query(c, None);
    // The compare an arm is, if it has one use; else its inverted constant.
    let invert = |arm: Operand| -> Option<Result<InstId, Operand>> {
        if let Some(def) = c.definition_of(arm)
            && matches!(c.function.instruction(def).opcode, Opcode::ICmp(_))
            && has_one_use(c, arm)
        {
            return Some(Ok(def));
        }
        inverted_constant(c, &q, arm).map(Err)
    };
    let (on_true, on_false) = (invert(operands[1])?, invert(operands[2])?);
    let mut new_operands = operands;
    for (arm, index) in [(on_true, 1), (on_false, 2)] {
        match arm {
            Ok(compare) => {
                let Opcode::ICmp(pred) = c.function.instruction(compare).opcode else { return None };
                c.function.set_icmp_predicate(compare, pred.inverse());
                c.add_to_worklist(compare);
            }
            Err(constant) => new_operands[index] = constant,
        }
    }
    c.function.set_operands(select, new_operands);
    Some(Visit::Replace(not_op))
}

/// `canFreelyInvert` (InstCombineAndOrXor.cpp:5111): `op` is an instruction
/// that is free to invert, and every user of it but `ignored` takes the
/// inversion.
fn can_freely_invert(
    c: &mut Combiner,
    inst: InstId,
    op: Operand,
    ignored: InstId,
) -> bool {
    let Some(def) = c.definition_of(op) else { return false };
    // `getInsertionPointAfterDef`: none after a terminator.
    if c.function.instruction(def).result.is_none() {
        return false;
    }
    let mut consumes = false;
    c.get_freely_inverted(inst, op, true, false, &mut consumes).is_some()
        && c.can_freely_invert_all_users_of(op, Some(ignored))
}

/// `freelyInvert` (InstCombineAndOrXor.cpp:5120): `~op` made after the
/// definition of `op`, read by every user of `op`, whose own users are then
/// adapted (but `ignored`).
fn freely_invert(
    c: &mut Combiner,
    op: Operand,
    ignored: InstId,
) -> Operand {
    let def = c.definition_of(op).expect("an instruction");
    let ty = c.type_of(op);
    let ones = c.all_ones(ty);
    let not = c.insert_after(def, Opcode::Binary(BinaryOp::Xor), ty, vec![op, ones], Flags::default());
    let not = c.value_of(not);
    let Operand::Value(op_value) = op else { unreachable!("an instruction") };
    let users: Vec<InstId> = c.function.users(op_value).iter().map(|one| one.user).collect();
    for user in users {
        if Some(user) == c.definition_of(not) {
            continue;
        }
        let operands = c.function.instruction(user).operands.iter().map(|&o| if o == op { not } else { o }).collect();
        c.function.set_operands(user, operands);
    }
    c.freely_invert_all_users_of(not, Some(ignored));
    not
}

/// `sinkNotIntoLogicalOp` (InstCombineAndOrXor.cpp:5138): `~(x &/| y)` is
/// `(~x) |/& (~y)` when both are free to invert and every user of the logic op
/// can be adapted. `inst` is the `xor` asking.
fn sink_not_into_logical_op(
    c: &mut Combiner,
    inst: InstId,
    logic: Operand,
) -> bool {
    let Some(logic_inst) = c.definition_of(logic) else { return false };
    let (is_and, op0, op1, binary) = {
        let q = query(c, Some(inst));
        let binary = matches!(c.function.instruction(logic_inst).opcode, Opcode::Binary(_));
        if let Some((a, b)) = pattern::logical_and(&q, logic) {
            (true, a, b, binary)
        } else if let Some((a, b)) = pattern::logical_or(&q, logic) {
            (false, a, b, binary)
        } else {
            return false;
        }
    };
    // If this logic op has not been simplified yet, just bail out and let that
    // happen first. Otherwise, the code below may wrongly invert.
    if op0 == op1 {
        return false;
    }
    // If one of the operands is a user of the other, freelyInvert would change
    // the operands of the logic op, which may cause miscompilation.
    {
        let q = query(c, Some(inst));
        if pattern::not(&q, op0) == Some(op1) || pattern::not(&q, op1) == Some(op0) {
            return false;
        }
    }
    // Can our users be adapted?
    if !c.can_freely_invert_all_users_of(logic, None) {
        return false;
    }
    // And can the operands be adapted?
    if !can_freely_invert(c, inst, op0, logic_inst) || !can_freely_invert(c, inst, op1, logic_inst) {
        return false;
    }
    let op0 = freely_invert(c, op0, logic_inst);
    let op1 = freely_invert(c, op1, logic_inst);

    let ty = c.type_of(logic);
    let new_logic = if binary {
        let op = if is_and { BinaryOp::Or } else { BinaryOp::And };
        let new = c.insert_after(logic_inst, Opcode::Binary(op), ty, vec![op0, op1], Flags::default());
        c.value_of(new)
    } else if is_and {
        // select op0, true, op1
        let on_true = c.all_ones(ty);
        let new = c.insert_after(logic_inst, Opcode::Select, ty, vec![op0, on_true, op1], Flags::default());
        c.value_of(new)
    } else {
        // select op0, op1, false
        let on_false = c.int(ty, 0);
        let new = c.insert_after(logic_inst, Opcode::Select, ty, vec![op0, op1, on_false], Flags::default());
        c.value_of(new)
    };
    c.replace_inst_with(logic_inst, new_logic);
    // We can not just create an outer `not`, it will most likely be immediately
    // folded back, so fold it away at once.
    c.freely_invert_all_users_of(new_logic, None);
    true
}

#[cfg(test)]
#[path = "combine_not_tests.rs"]
mod tests;
