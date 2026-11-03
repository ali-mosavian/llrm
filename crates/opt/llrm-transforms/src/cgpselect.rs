//! A float-to-integer conversion of a select with a constant arm, made on the
//! other arm's path only.
//!
//! # How it works
//! `%r = fptosi (select c, K, x)`, where the select has no other user, is
//! `br c, join, other` with `%f = fptosi x` in `other` and `%r = phi [K', b],
//! [%f, other]` in `join`, `K'` the conversion of `K` made here. The path that
//! takes the constant converts nothing. Applied only where the conversion costs
//! more than twice a branch (`float_to_int` against `branch` in the target's
//! costs): on x87 `fistp` with the control word loaded around it is 43 clocks
//! against 3, so the half of the iterations that take `K` save about 21; where
//! one `cvttsd2si` converts, the select stays.
//!
//! ```text
//!  %s = select c, -3.0, %x            br c, %join, %other
//!  %r = fptosi %s                 other: %f = fptosi %x ; br %join
//!                                 join:  %r = phi [-3, %b], [%f, %other]
//! ```
//!
//! A clamp is a select of a select; the diamond made for the outer one holds
//! `fptosi (select ..)` again and is taken in the next round.
//!
//! DEVIATE CS1 (exit 2026-11-20: the pin's
//! `SelectOptimize`/`optimizeSelectInst` priced by `getCastInstrCost`): the pin
//! keeps one conversion after the select (`FoldOpIntoSelect` is skipped when
//! the compare has the select's type) and its x86 has no price that makes a
//! conversion dear. gcc converts the constant at compile time (x_ldcmp -O2:
//! 2020 `fistp` against 1180).
//!
//! The condition is branched on as it is (no `freeze`): a poison condition
//! made a poison select, and now a branch on poison, which the pin's
//! `optimizeSelectInst` guards with `freeze`.

use llrm_mir::constantfolding::constant_fold_cast_operand;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, Opcode};

use crate::edges::split_block_at;

/// Every conversion of a select that costs more than twice a branch, made on
/// its non-constant arm's path. Whether any changed.
pub fn casts_branched(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    conversion: i64,
    branch: i64,
) -> bool {
    if conversion <= 2 * branch {
        return false;
    }
    let mut changed = false;
    while let Some(site) = find(context, function) {
        branched(context, layout, function, site);
        changed = true;
    }
    changed
}

struct Site {
    cast: InstId,
    select: InstId,
    /// The arm that is a constant: the true one or the false one.
    constant_is_true: bool,
}

fn find(
    context: &Context,
    function: &Function,
) -> Option<Site> {
    for &block in function.layout() {
        for cast in function.iter_block(block) {
            let instruction = function.instruction(cast);
            if !matches!(
                instruction.opcode,
                Opcode::Cast(CastOp::FPToSI | CastOp::FPToUI)
            ) {
                continue;
            }
            let [Operand::Value(value)] = instruction.operands[..] else { continue };
            let ValueDef::Instruction(select) = function.value(value).def else { continue };
            if function.instruction(select).opcode != Opcode::Select
                || function.parent(select) != Some(block)
                || !function.users(value).has_one_use()
            {
                continue;
            }
            let [Operand::Value(condition), yes, no] = function.instruction(select).operands[..] else { continue };
            if context.types.int_bits(function.value(condition).ty) != Some(1) {
                continue;
            }
            let float_constant = |arm: Operand| {
                matches!(
                    arm,
                    Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Float(_) | ConstantKind::Float80(_))
                )
            };
            // Two constant arms are a select of two integers: no branch.
            match (float_constant(yes), float_constant(no)) {
                (true, false) => return Some(Site { cast, select, constant_is_true: true }),
                (false, true) => return Some(Site { cast, select, constant_is_true: false }),
                _ => {}
            }
        }
    }
    None
}

fn branched(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    site: Site,
) {
    let Site { cast, select, constant_is_true } = site;
    let Opcode::Cast(op) = function.instruction(cast).opcode else { unreachable!("a cast") };
    let ty = function.instruction(cast).ty;
    let result = function.instruction(cast).result.expect("a conversion's value");
    let [condition, yes, no] = function.instruction(select).operands[..] else { unreachable!("a select") };
    let (constant, other) = if constant_is_true { (yes, no) } else { (no, yes) };
    let Operand::Constant(constant) = constant else { unreachable!("a constant arm") };
    let Some(converted) = constant_fold_cast_operand(context, layout, op, constant, ty) else {
        return;
    };
    if !matches!(context.get(converted).kind, ConstantKind::Int(_)) {
        return;
    }
    let home = function.parent(cast).expect("a placed conversion");
    let join = split_block_at(function, home, cast);
    let jump = function.terminator(home).expect("the jump the split made");
    let void = function.instruction(jump).ty;
    function.erase(jump).expect("nothing reads a jump");
    let side = function.create_block(None);
    function.insert_block(side, Some(home)).expect("a new block");
    let moved = function.create_instruction(Opcode::Cast(op), ty, vec![other], Flags::default(), None);
    function.insert(moved, Position::End(side)).expect("an unplaced instruction");
    let back = function.create_instruction(Opcode::Br, void, vec![Operand::Block(join)], Flags::default(), None);
    function.insert(back, Position::End(side)).expect("an unplaced instruction");
    let (on_true, on_false) = if constant_is_true { (join, side) } else { (side, join) };
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![condition, Operand::Block(on_true), Operand::Block(on_false)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::End(home)).expect("an unplaced instruction");
    let made = function.instruction(moved).result.expect("a conversion's value");
    let phi = function.create_instruction(
        Opcode::Phi,
        ty,
        vec![Operand::Constant(converted), Operand::Block(home), Operand::Value(made), Operand::Block(side)],
        Flags::default(),
        None,
    );
    function.insert(phi, Position::Before(cast)).expect("an unplaced instruction");
    let value = function.instruction(phi).result.expect("a phi's value");
    function.replace_all_uses_with(result, Operand::Value(value));
    function.erase(cast).expect("nothing reads it now");
    function.erase(select).expect("nothing reads it now");
    // A select only the conversion reads moves with it: the next round finds
    // the pair in one block.
    if let Operand::Value(value) = other
        && let ValueDef::Instruction(inner) = function.value(value).def
        && function.instruction(inner).opcode == Opcode::Select
        && function.parent(inner) == Some(home)
        && function.users(value).has_one_use()
    {
        function.move_to(inner, Position::Before(moved)).expect("a placed instruction");
    }
}

#[cfg(test)]
#[path = "cgpselect_tests.rs"]
mod tests;
