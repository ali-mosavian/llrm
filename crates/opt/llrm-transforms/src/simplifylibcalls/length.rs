//! The `strlen` arm of `LibCallSimplifier`: `optimizeStrLen` (:696) through
//! `optimizeStringLength` (:631) at d1106deb71cc, for an 8-bit character.
//!
//! `strlen(x) == 0` is `*x == 0`, `strlen("xyz")` is 3 and
//! `strlen(c ? "foo" : "bars")` is `c ? 3 : 4`.
//! The arm that folds `strlen(s + x)` of a string literal to `strlen(s) - x`
//! is not translated (D2 of the pass).

use llrm_mir::module::Operand;
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::passes::Unit;
use llrm_mir::valuetracking::{Query, known};

use super::{Rewrite, Site, build, string_length};

/// `optimizeStrLen`.
pub(super) fn str_len(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let source = unit.function.instruction(site.call).operands[0];
    let result = unit.function.instruction(site.call).ty;
    // strlen(x) != 0 --> *x != 0, and likewise ==.
    if known::is_only_used_in_zero_equality_comparison(
        &Query::bare(unit.context, unit.function, unit.layout),
        site.call,
    ) {
        let i8 = unit.context.types.int(8);
        let first = build(unit, site, Opcode::Load { align: None, volatile: false }, i8, vec![source]);
        return Some(Rewrite::Value(build(unit, site, Opcode::Cast(CastOp::ZExt), result, vec![first])));
    }
    // strlen("xyz") --> 3
    let length = string_length(unit, site, source);
    if length != 0 {
        return Some(Rewrite::Value(Operand::Constant(unit.context.int(result, i128::from(length - 1)))));
    }
    // strlen(x ? "foo" : "bars") --> x ? 3 : 4
    let Operand::Value(value) = source else { return None };
    let SelectOperands { condition, on_true, on_false } = select_of(unit, value)?;
    let (true_length, false_length) = (string_length(unit, site, on_true), string_length(unit, site, on_false));
    if true_length == 0 || false_length == 0 {
        return None;
    }
    let constant = |unit: &Unit, length: u64| Operand::Constant(unit.context.int(result, i128::from(length - 1)));
    let (a, b) = (constant(unit, true_length), constant(unit, false_length));
    Some(Rewrite::Value(build(unit, site, Opcode::Select, result, vec![condition, a, b])))
}

struct SelectOperands {
    condition: Operand,
    on_true: Operand,
    on_false: Operand,
}

/// The operands of the `select` that defines `value`.
fn select_of(
    unit: &Unit,
    value: llrm_mir::module::ValueId,
) -> Option<SelectOperands> {
    let llrm_mir::module::ValueDef::Instruction(def) = unit.function.value(value).def else { return None };
    let one = unit.function.instruction(def);
    if one.opcode != Opcode::Select {
        return None;
    }
    Some(SelectOperands { condition: one.operands[0], on_true: one.operands[1], on_false: one.operands[2] })
}
