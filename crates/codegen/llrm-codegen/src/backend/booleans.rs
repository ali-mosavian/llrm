//! Setcc chains the target's cost table prices above a branch, made selects
//! for `selects::lowered` to branch.
//!
//! # How it works
//! Without a conditional move a comparison used as a value is `cmp; setcc` and
//! its combiners are more instructions: `x + zext(c)` is `setcc; movzx; add`.
//! A branch does the same in a jump and the work only one arm needs. That
//! chain is priced both ways from the target's cost table (`setcc`, `movzx`,
//! `alu_rr`, `jcc`, `jcc_not_taken`, the description's `timings.times`) and
//! rewritten as a `select` where the branch is cheaper; the arm's instruction
//! (the `add 1`) moves into the taken block with the diamond.
//!
//! An `and` of two compares used as a value is not branched: the pin keeps two
//! `setcc` and an `and` (`FindMergedConditions` short-circuits only a branch's
//! condition). Main branched it by price; that was STRISTMP-1.
//!
//! ```text
//!  %z = zext i1 %c to i32           %y = add %x, 1        ; moved into taken
//!  %r = add i32 %x, %z       ==>    %r = select %c, %y, %x    -> br %c, taken, join
//! ```
//!
//! DEVIATE B2 (no number yet), the `add` chain: the pin lowers it as a `setcc`
//! chain too (`pin.py llc -m 32` on x_mergesort's merge loop: 21 instructions
//! with the flags spilled), where main branches. The price is the one cost
//! table, not the shape of a loop. Exit: the pin's lowering equals or beats
//! main on the rows (x_mergesort, stft bq32, nhstutil). Plan: docs/working/
//! setcc-chain-plan.md.
//!
//! Rows (own-worktree builds 4d72d84065 / 49a8d96bfb, tools/pairs, m32 i486):
//! x_mergesort clocks -25.6% at -O1..-O3, -25.0% -Os (code -9.8% -Os), x_rle
//! -7.9%, x_primes -1.4%; no row worse. QCport (tools/rows/qcport.py, m16)
//! -Os 260284 -> 260240 (-44 bytes); above base: host +7 (0.028%), `lights` +3
//! (0.708%: `sete; movzx` of a 6-byte pair became `mov cx, 1; jne` and a
//! second `mov`, a branch the cycle table prices cheaper that is not shorter).
//! Row LIGHTS-OS-1, owner llrm-w-2, 2026-11-06: the gate prices bytes too
//! (encoded size of the forms, from the description) so the chain stays where
//! it is shorter at -Os. Compare of operands wider than a register, a chain a
//! branch reads, and an `and` with a non-compare side are left alone (each
//! grew a QCport module at -Os: snd, menu, console).
//!
//! # Example
//! `p >= lo && p < hi` as a value, on a target with no conditional move.
//!
//! Schematic, from the fixture of test
//! `test_an_and_of_two_compares_as_a_value_is_a_branch_around_the_second`
//! (`selects.rs`; not printer output):
//! ```mir
//! %a = icmp uge i16 %p, %lo            ; before: two setcc and an and
//! %b = icmp ult i16 %p, %hi
//! %r = and i1 %a, %b
//!
//! %a = icmp uge i16 %p, %lo            ; after: the second compare only when the first holds
//! br i1 %a, label %taken, label %join
//! taken:  %b = icmp ult i16 %p, %hi ; br label %join
//! join:   %r = phi i1 [ %b, %taken ], [ false, %entry ]
//! ```
//!
//! From unit test
//! `test_an_and_of_two_compares_as_a_value_is_a_branch_around_the_second`
//! in `selects.rs`.
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
}

fn prices(price: &dyn Fn(&str) -> Option<i64>) -> Option<Prices> {
    let (set, ext, alu) = (price("setcc")?, price("movzx")?, price("alu_rr")?);
    let branch = (price("jcc")? + price("jcc_not_taken")? + 1) / 2;
    Some(Prices { chain_add: set + ext + alu, branch_add: branch + alu })
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

/// Whether `value` is a compare of operands the target holds in one register:
/// a wider one is several instructions and live ranges, and moving it into an
/// arm spills (qcport snd -Os: a long compare in 16-bit code).
fn word_compare(
    context: &Context,
    function: &Function,
    value: Operand,
    word_bits: u32,
) -> bool {
    let Operand::Value(value) = value else { return false };
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let compare = function.instruction(def);
    matches!(compare.opcode, Opcode::ICmp(_))
        && compare.operands.iter().all(|&operand| {
            function
                .operand_type(context, operand)
                .and_then(|ty| context.types.int_bits(ty))
                .is_none_or(|bits| bits <= word_bits)
        })
}

/// The chains of `function` priced above a branch, made branches. How many.
pub fn branched(
    context: &Context,
    function: &mut Function,
    price: &dyn Fn(&str) -> Option<i64>,
    wanted: &dyn Fn(&str) -> bool,
    word_bits: u32,
) -> Result<usize, String> {
    let Some(costs) = prices(price) else { return Ok(0) };
    let add_wins = costs.chain_add > costs.branch_add;
    if !add_wins {
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
                if !word_compare(context, function, flag, word_bits)
                    && !{
                        // c = !compare
                        defined_for(function, flag, zext)
                            .filter(|&not| function.instruction(not).opcode == Opcode::Binary(BinaryOp::Xor))
                            .is_some_and(|not| {
                                word_compare(context, function, function.instruction(not).operands[0], word_bits)
                            })
                    }
                {
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
