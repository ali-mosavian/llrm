//! An add, subtract or extension in a loop that only exit phis read, moved to
//! those exits: LLVM's LICM sinking an instruction only used outside the
//! loop, a copy to each exit that reads it. The last trip's update of a
//! pointer strength reduction carried is computed once, on the way out, and
//! so is the `sext` of a narrow counter whose value `work += i` reads after
//! the loop (mandel: `inc cx; movsx eax,cx; cmp cx,20h` every trip).
//!
//! Adapted from llrm-core's `optimize/exitsink.rs`. Each operand the loop
//! defines is read through an exit phi, as LCSSA has it; an invariant one
//! directly. The old one moved one a call; this one repeats to a fixed
//! point.
//!
//! Dropped, with no rich-MIR counterpart: the refusal of a body whose
//! opaque or barrier operations read registers their operands do not name,
//! flags live into the exit, other results of the moved operation read
//! elsewhere, and its stack and x87 effects.
//!
//! The old test is skipped there: its fixture spied on `strength.reduced`.
//! These are new.

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};

use crate::edges;
use crate::lcssa::{arms, exit_phi, from_arms, place_phi};

/// Every add and subtract only an exit reads, moved there; whether any was.
pub fn sunk(function: &mut Function) -> bool {
    let mut changed = false;
    while let Some((loop_, phi, op)) = _sinkable(function) {
        _sink(function, &loop_, phi, op);
        changed = true;
    }
    changed
}

/// An exit's phi of one input from `loop_`, and the add, subtract or
/// extension in the loop that input is, which only exit phis read.
fn _sinkable(function: &Function) -> Option<(Loop, InstId, InstId)> {
    for loop_ in cfg::Shape::of(function).loops {
        for &block in function.layout().iter().filter(|&&block| !loop_.body.contains(&cfg::id(block))) {
            for phi in edges::phis(function, block) {
                let [(Operand::Value(value), from)] = arms(function, phi)[..] else { continue };
                let ValueDef::Instruction(op) = function.value(value).def else { continue };
                let inside = |inst: InstId| function.parent(inst).is_some_and(|at| loop_.body.contains(&cfg::id(at)));
                if loop_.body.contains(&cfg::id(from))
                    && inside(op)
                    && matches!(function.instruction(op).opcode, Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) | Opcode::Cast(CastOp::SExt | CastOp::ZExt | CastOp::Trunc))
                    && function.users(value).iter().all(|one| function.instruction(one.user).opcode == Opcode::Phi && function.parent(one.user).is_some_and(|at| !loop_.body.contains(&cfg::id(at))))
                {
                    return Some((loop_, phi, op));
                }
            }
        }
    }
    None
}

/// `op` computed in place of `phi`, its operands the loop defines read
/// through the exit's phis.
fn _sink(function: &mut Function, loop_: &Loop, phi: InstId, op: InstId) {
    let exit = function.parent(phi).expect("a placed phi");
    let [(_, from)] = arms(function, phi)[..] else { unreachable!("one input") };
    let mut operands = Vec::new();
    for operand in function.instruction(op).operands.clone() {
        let defined = match operand {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Instruction(inst) => function.parent(inst).filter(|at| loop_.body.contains(&cfg::id(*at))).map(|_| value),
                ValueDef::Argument(_) => None,
            },
            _ => None,
        };
        let Some(value) = defined else {
            operands.push(operand);
            continue;
        };
        let incoming = from_arms(&[(Operand::Value(value), from)]);
        let exported = match edges::phis(function, exit).into_iter().find(|&one| function.instruction(one).operands == incoming) {
            Some(existing) => existing,
            None => {
                let export = exit_phi(function, value);
                function.set_operands(export, incoming);
                place_phi(function, exit, export).expect("a placed exit");
                export
            }
        };
        operands.push(Operand::Value(function.instruction(exported).result.expect("a phi's value")));
    }
    let instruction = function.instruction(op);
    let moved = function.create_instruction(instruction.opcode.clone(), instruction.ty, operands, instruction.flags, None);
    let first = *function.block(exit).instructions().iter().find(|&&one| function.instruction(one).opcode != Opcode::Phi).expect("a terminator");
    function.insert(moved, Position::Before(first)).expect("a placed block");
    let result = function.instruction(phi).result.expect("a phi's value");
    function.replace_all_uses_with(result, Operand::Value(function.instruction(moved).result.expect("a value")));
    function.set_operands(phi, Vec::new());
    function.erase(phi).expect("its uses were replaced");
    // Each other exit that reads it takes its own copy, in its turn.
    if function.users(function.instruction(op).result.expect("a value")).is_empty() {
        function.erase(op).expect("only exit phis read it");
    }
}

#[cfg(test)]
#[path = "exitsink_tests.rs"]
mod tests;
