//! A float-to-integer conversion, or a float compare, that only one arm of a
//! select with a constant arm needs, made on that arm's path only.
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
//! The same for a compare: `select c1, K, (zext (fcmp ..))` is `br c1, join,
//! other` with the compare and its extension in `other` (`a < b ? -1 : a > b`:
//! the second `fcom; fnstsw; sahf` runs on the path that needs it, 9 clocks at
//! 486, against the branch). Applied where `float_compare` is more than twice
//! a branch.
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
    compare: i64,
    branch: i64,
) -> bool {
    let (convert, compared) = (conversion > 2 * branch, compare > 2 * branch);
    if !convert && !compared {
        return false;
    }
    let mut changed = false;
    while let Some(site) = find(context, function, convert, compared) {
        match site {
            Site::Convert { .. } => branched(context, layout, function, site),
            Site::Compare { .. } => compare_branched(function, site),
        }
        changed = true;
    }
    changed
}

enum Site {
    Convert {
        cast: InstId,
        select: InstId,
        /// The arm that is a constant: the true one or the false one.
        constant_is_true: bool,
    },
    Compare {
        select: InstId,
        constant_is_true: bool,
        /// The compare and the extension of it only the select reads, in order.
        chain: Vec<InstId>,
    },
}

fn find(
    context: &Context,
    function: &Function,
    convert: bool,
    compared: bool,
) -> Option<Site> {
    for &block in function.layout() {
        for cast in function.iter_block(block) {
            let instruction = function.instruction(cast);
            if compared
                && instruction.opcode == Opcode::Select
                && let Some(site) = compare_site(context, function, block, cast)
            {
                return Some(site);
            }
            if !convert || !matches!(
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
                (true, false) => return Some(Site::Convert { cast, select, constant_is_true: true }),
                (false, true) => return Some(Site::Convert { cast, select, constant_is_true: false }),
                _ => {}
            }
        }
    }
    None
}

/// `select c, K, V` (either way round) where `V` is a float compare, or the
/// extension of one, that nothing else reads and that sits in the select's
/// block.
fn compare_site(
    context: &Context,
    function: &Function,
    block: llrm_mir::module::BlockId,
    select: InstId,
) -> Option<Site> {
    let [Operand::Value(condition), yes, no] = function.instruction(select).operands[..] else { return None };
    if context.types.int_bits(function.value(condition).ty) != Some(1) {
        return None;
    }
    let constant = |arm: Operand| {
        matches!(
            arm,
            Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_))
        )
    };
    let (constant_is_true, other) = match (constant(yes), constant(no)) {
        (true, false) => (true, no),
        (false, true) => (false, yes),
        _ => return None,
    };
    let Operand::Value(mut value) = other else { return None };
    let mut chain = Vec::new();
    loop {
        let ValueDef::Instruction(inst) = function.value(value).def else { return None };
        if function.parent(inst) != Some(block) || !function.users(value).has_one_use() {
            return None;
        }
        chain.push(inst);
        match function.instruction(inst).opcode {
            Opcode::FCmp(_) => break,
            Opcode::Cast(CastOp::ZExt | CastOp::SExt) => {
                let [Operand::Value(inner)] = function.instruction(inst).operands[..] else { return None };
                value = inner;
            }
            _ => return None,
        }
    }
    chain.reverse();
    Some(Site::Compare { select, constant_is_true, chain })
}

/// The compare chain moves to a block of its own, taken when the constant is
/// not; the select becomes a phi of the constant and the chain's value.
fn compare_branched(
    function: &mut Function,
    site: Site,
) {
    let Site::Compare { select, constant_is_true, chain } = site else { unreachable!("a compare site") };
    let [condition, yes, no] = function.instruction(select).operands[..] else { unreachable!("a select") };
    let (constant, other) = if constant_is_true { (yes, no) } else { (no, yes) };
    let ty = function.instruction(select).ty;
    let result = function.instruction(select).result.expect("a select's value");
    let home = function.parent(select).expect("a placed select");
    let join = split_block_at(function, home, select);
    let jump = function.terminator(home).expect("the jump the split made");
    let void = function.instruction(jump).ty;
    function.erase(jump).expect("nothing reads a jump");
    let side = function.create_block(None);
    function.insert_block(side, Some(home)).expect("a new block");
    for &inst in &chain {
        function.move_to(inst, Position::End(side)).expect("a placed instruction");
    }
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
    let phi = function.create_instruction(
        Opcode::Phi,
        ty,
        vec![constant, Operand::Block(home), other, Operand::Block(side)],
        Flags::default(),
        None,
    );
    function.insert(phi, Position::Before(select)).expect("an unplaced instruction");
    let value = function.instruction(phi).result.expect("a phi's value");
    function.replace_all_uses_with(result, Operand::Value(value));
    function.erase(select).expect("nothing reads it now");
}

fn branched(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    site: Site,
) {
    let Site::Convert { cast, select, constant_is_true } = site else { unreachable!("a conversion site") };
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
