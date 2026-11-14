//! CodeGenPrepare's `tryToSinkFreeOperands`: operands the target folds into
//! an instruction are made again beside it
//! ([CodeGenPrepare.cpp#L7994](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7994)).
//!
//! # How it works
//! Isel works a block at a time. An operand made in another block (an
//! extension, a splat shuffle, a lane-0 insert) crosses as a register and the
//! instruction cannot fold it (a `v2i64` multiply of two `sext` is one
//! `smull` only with the `sext`s in view). The target's
//! `isProfitableToSinkOperands` names the uses to remake; each is cloned
//! before the instruction (before the dominating one, in a chain), the
//! original goes once nothing reads it.
//!
//! ```text
//! b1: %e = sext <8 x i16> %a to <8 x i64>     b2: %e = sext <8 x i16> %a to <8 x i64>
//! b2: %m = mul <8 x i64> %e, %s            =>     %m = mul <8 x i64> %e, %s
//! ```
//!
//! # Runs
//! In `cgpaddr`'s run, before `SinkCast`.

use llrm_mir::context::Context;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, Use, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::tti::Tti;

/// Whether `inst` stays where it is: a phi, or one that touches memory or
/// calls (`isa<PHINode>`, `mayHaveSideEffects`, `mayReadFromMemory`).
fn pinned(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        function.instruction(inst).opcode,
        Opcode::Phi
            | Opcode::Load { .. }
            | Opcode::Store { .. }
            | Opcode::Call(_)
            | Opcode::Invoke(_)
            | Opcode::VaArg
            | Opcode::Atomic { .. }
    )
}

fn def_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// `tryToSinkFreeOperands` for `inst`: whether anything was remade.
fn operands_sunk(
    context: &Context,
    tti: &dyn Tti,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let mut ops: Vec<Use> = Vec::new();
    if !tti.is_profitable_to_sink_operands(context, function, inst, &mut ops) {
        return false;
    }
    let Some(target) = function.parent(inst) else { return false };
    // "The dominating uses must come first, so we process the ops in reverse."
    let mut to_replace: Vec<Use> = Vec::new();
    let mut insert_point = inst;
    for &at in ops.iter().rev() {
        let Some(def) = def_of(function, function.instruction(at.user).operands[at.index as usize]) else { continue };
        if pinned(function, def) {
            continue;
        }
        if function.parent(def) == Some(target) {
            if function.comes_before(def, insert_point) {
                insert_point = def;
            }
            continue;
        }
        to_replace.push(at);
    }
    let mut changed = false;
    let mut made: IdMap<InstId, InstId> = IdMap::new();
    let mut maybe_dead: Vec<InstId> = Vec::new();
    for at in to_replace {
        let original = function.instruction(at.user).operands[at.index as usize];
        let def = def_of(function, original).expect("a placed definition");
        let copy = function.clone_instruction(def);
        function.insert(copy, Position::Before(insert_point)).expect("a position before the user");
        insert_point = copy;
        made.insert(def, copy);
        maybe_dead.push(def);
        let value = Operand::Value(function.instruction(copy).result.expect("a value"));
        // "Making sure that we update the sunk instruction uses, if it is part
        // of a chain that has already been sunk."
        let user = made.get(&at.user).copied().unwrap_or(at.user);
        function.set_operand(user, at.index as usize, value);
        changed = true;
    }
    for def in maybe_dead {
        let dead = function.instruction(def).result.is_some_and(|result| function.users(result).is_empty());
        if dead && !function.is_erased(def) {
            function.erase(def).expect("an instruction nothing reads goes");
        }
    }
    changed
}

/// `tryToSinkFreeOperands` for each instruction; whether any was remade.
pub fn free_operands_sunk(
    context: &Context,
    tti: &dyn Tti,
    function: &mut Function,
) -> bool {
    let all: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    let mut changed = false;
    for inst in all {
        if !function.is_erased(inst) && operands_sunk(context, tti, function, inst) {
            changed = true;
        }
    }
    changed
}
