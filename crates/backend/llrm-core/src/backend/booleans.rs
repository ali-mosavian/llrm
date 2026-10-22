//! Setcc chains the target's cost table prices above a branch, made selects
//! for `selects::lowered` to branch.
//!
//! # How it works
//! Without a conditional move a comparison used as a value is `cmp; setcc` and
//! its combiners are more instructions: `x + zext(c)` is `setcc; movzx; add`,
//! `a & b` of two compares is two `setcc` and an `and`. A branch does the same
//! in a jump and the work only one arm needs. Each such chain is priced both
//! ways from the target's cost table (`setcc`, `movzx`, `alu_rr`, `jcc`,
//! `jcc_not_taken`, the description's `timings.times`) and rewritten as a
//! `select` where the branch is cheaper; the arm's instruction (the `add 1`,
//! the second compare) moves into the taken block with the diamond.
//!
//! ```text
//!  %z = zext i1 %c to i32           %y = add %x, 1        ; moved into taken
//!  %r = add i32 %x, %z       ==>    %r = select %c, %y, %x    -> br %c, taken, join
//! ```
//!
//! DEVIATE B2 (no number yet): the pin lowers these as `setcc` chains too
//! (`pin.py llc -m 32` on x_mergesort's merge loop: 21 instructions with the
//! flags spilled; `is_tmp(p){return p>=temps && p<temps+240;}`: `cmp; setae;
//! cmp; setb; and; movzx`), where main branches. The price is the one cost
//! table, not the shape of a loop. Exit: the pin's lowering equals or beats
//! main on the rows (x_mergesort, stft bq32, nhstutil). Plan: docs/working/
//! setcc-chain-plan.md.
//!
//! # Runs
//! Inside `selects::priced`, after the selects of the module were folded and
//! before they are branched, on a target that expands `select` of the
//! chain's type.

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};

/// The cost of one way of making the value, from the table's names.
struct Prices {
    chain_add: i64,
    branch_add: i64,
    chain_and: i64,
    branch_and: i64,
}

fn prices(price: &dyn Fn(&str) -> Option<i64>) -> Option<Prices> {
    let (set, ext, alu) = (price("setcc")?, price("movzx")?, price("alu_rr")?);
    let branch = (price("jcc")? + price("jcc_not_taken")? + 1) / 2;
    Some(Prices {
        chain_add: set + ext + alu,
        branch_add: branch + alu,
        chain_and: 2 * set + alu + ext,
        branch_and: branch + set + ext,
    })
}

/// The instruction defining `operand`, when it has `only` this user.
fn defined_for(
    function: &Function,
    operand: Operand,
    user: InstId,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    let ValueDef::Instruction(def) = function.value(value).def else { return None };
    (function.users(value).only().is_some_and(|only| only.user == user)).then_some(def)
}

/// The chains of `function` priced above a branch, made branches. How many.
pub fn branched(
    context: &Context,
    function: &mut Function,
    price: &dyn Fn(&str) -> Option<i64>,
    wanted: &dyn Fn(&str) -> bool,
) -> Result<usize, String> {
    let Some(costs) = prices(price) else { return Ok(0) };
    let (add_wins, and_wins) = (costs.chain_add > costs.branch_add, costs.chain_and > costs.branch_and);
    if !add_wins && !and_wins {
        return Ok(0);
    }
    let candidates: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    // Each select made, and the instruction only its taken arm needs.
    let mut chains: Vec<(InstId, InstId, bool)> = Vec::new();
    for inst in candidates {
        if function.is_erased(inst) || function.parent(inst).is_none() {
            continue;
        }
        let one = function.instruction(inst).clone();
        let ty = one.ty;
        let Some(result) = one.result else { continue };
        let bits = context.types.int_bits(ty);
        // add x, zext(c): x + 1 when c.
        if add_wins && one.opcode == Opcode::Binary(BinaryOp::Add) && bits.is_some_and(|bits| bits > 1) {
            if !wanted(&context.types.display(ty).to_string()) {
                continue;
            }
            let [left, right] = one.operands[..] else { continue };
            for (extension, plain) in [(left, right), (right, left)] {
                let Some(zext) = defined_for(function, extension, inst) else { continue };
                let widened = function.instruction(zext).clone();
                if widened.opcode != Opcode::Cast(CastOp::ZExt) {
                    continue;
                }
                let flag = widened.operands[0];
                if function.operand_type(context, flag).and_then(|ty| context.types.int_bits(ty)) != Some(1) {
                    continue;
                }
                // c = !c0 reads the other arm.
                let mut negation = None;
                let (condition, yes_plain) = match defined_for(function, flag, zext) {
                    Some(not) if function.instruction(not).opcode == Opcode::Binary(BinaryOp::Xor) => {
                        let [inner, flip] = function.instruction(not).operands[..] else { continue };
                        if crate::backend::selectconstants::constant(context, flip) == Some(-1) {
                            negation = Some(not);
                            (inner, true)
                        } else {
                            (flag, false)
                        }
                    }
                    _ => (flag, false),
                };
                let one_const = Operand::Constant(context.int(ty, 1));
                let plus = crate::backend::selectconstants::before(
                    function,
                    inst,
                    Opcode::Binary(BinaryOp::Add),
                    ty,
                    vec![plain, one_const],
                )?;
                let arms = if yes_plain { vec![condition, plain, plus] } else { vec![condition, plus, plain] };
                let select = crate::backend::selectconstants::before(function, inst, Opcode::Select, ty, arms)?;
                function.replace_all_uses_with(result, select);
                function.erase(inst)?;
                if widened.result.is_some_and(|value| function.users(value).only().is_none()) {
                    function.erase(zext)?;
                    // The negation read the condition: a second use of it
                    // keeps the compare from fusing with the branch.
                    if let Some(not) = negation
                        && function.instruction(not).result.is_some_and(|value| function.users(value).only().is_none())
                    {
                        function.erase(not)?;
                    }
                }
                let Operand::Value(select_value) = select else { unreachable!("a select's value") };
                let ValueDef::Instruction(select_inst) = function.value(select_value).def else { unreachable!() };
                let Operand::Value(plus_value) = plus else { unreachable!("an add's value") };
                let ValueDef::Instruction(plus_inst) = function.value(plus_value).def else { unreachable!() };
                chains.push((select_inst, plus_inst, yes_plain));
                break;
            }
        }
        // a & b of two compares used as a value: b only when a.
        if and_wins && one.opcode == Opcode::Binary(BinaryOp::And) && bits == Some(1) {
            let only_branches = function
                .users(result)
                .snapshot()
                .iter()
                .all(|used| function.instruction(used.user).opcode == Opcode::Br && used.index == 0);
            if only_branches {
                continue;
            }
            let [left, right] = one.operands[..] else { continue };
            for (first, second) in [(left, right), (right, left)] {
                let Some(compare) = defined_for(function, second, inst) else { continue };
                if !matches!(function.instruction(compare).opcode, Opcode::ICmp(_)) {
                    continue;
                }
                let no = Operand::Constant(context.int(ty, 0));
                let select = crate::backend::selectconstants::before(
                    function,
                    inst,
                    Opcode::Select,
                    ty,
                    vec![first, second, no],
                )?;
                function.replace_all_uses_with(result, select);
                function.erase(inst)?;
                let Operand::Value(select_value) = select else { unreachable!("a select's value") };
                let ValueDef::Instruction(select_inst) = function.value(select_value).def else { unreachable!() };
                chains.push((select_inst, compare, false));
                break;
            }
        }
    }
    // The selects on one condition share one branch (`EmitLoweredSelect` case
    // 1), so they are all made before any is branched.
    let mut made = 0;
    for &(select, _, _) in &chains {
        if function.is_erased(select) || function.parent(select).is_none() {
            continue;
        }
        let block = function.parent(select).expect("a placed select");
        let (run, join_side) = crate::backend::selects::_run(context, function, select, wanted);
        let taken = crate::backend::selects::_branched(function, &run, &join_side)?;
        // The arm of a select that reads `plus` when the condition is false
        // goes in a block of the false edge.
        let (late, early): (Vec<_>, Vec<_>) = chains
            .iter()
            .filter(|(one, ..)| run.contains(one))
            .map(|&(_, arm, late)| (arm, late))
            .partition(|&(_, late)| late);
        let jump = function.terminator(taken).ok_or("a jump out of the taken block")?;
        let early: Vec<InstId> = early.into_iter().map(|(arm, _)| arm).collect();
        function.splice(&early, Position::Before(jump))?;
        if !late.is_empty() {
            let join = *function.successors(taken).iter().next().ok_or("a join after the taken block")?;
            let other = function.create_block(None);
            function.insert_block(other, Some(block))?;
            let void = function.instruction(jump).ty;
            let to_join =
                function.create_instruction(Opcode::Br, void, vec![Operand::Block(join)], Flags::default(), None);
            function.insert(to_join, Position::End(other))?;
            let branch = function.terminator(block).ok_or("the block's branch")?;
            let mut operands = function.instruction(branch).operands.clone();
            operands[2] = Operand::Block(other);
            function.set_operands(branch, operands);
            crate::backend::selects::_renamed_edges(function, join, block, other);
            let late: Vec<InstId> = late.into_iter().map(|(arm, _)| arm).collect();
            function.splice(&late, Position::Before(to_join))?;
        }
        made += 1;
    }
    Ok(made)
}
