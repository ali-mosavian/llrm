//! A load made where its one reader is: what lies between them only computes, so the load moves down to just before
//! the reader. A value held on a stack machine (x87) is on the stack from the load, and what is computed meanwhile has
//! to work around it, with exchanges, or leave it. LLVM's MachineSink and the scheduler's bottom-up order, within one
//! block, for the loads.

use llrm_analysis::memoryssa::Accesses;
use llrm_mir::edit::Position;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};

pub struct LoadSink;

impl FunctionPass for LoadSink {
    fn name(&self) -> &'static str {
        "loadsink"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        if sunk(unit, analyses) { PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>() } else { PreservedAnalyses::all() }
    }
}

/// Whether `inst` may run later or earlier past a load: it writes no memory, orders nothing, and cannot fault.
fn _only_computes(unit: &passes::Unit, accesses: &Accesses, inst: InstId) -> bool {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Store { .. } | Opcode::Phi => return false,
        Opcode::Load { volatile, .. } if volatile => return false,
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem | BinaryOp::SDiv | BinaryOp::SRem) if !crate::hoist::_cannot_fault(unit, inst) => return false,
        _ => {}
    }
    !instruction.opcode.is_terminator() && accesses.writes(inst).is_some_and(<[_]>::is_empty)
}

/// Each block's loads with one reader in it, moved before that reader where only computation lies between.
pub fn sunk(unit: &mut passes::Unit, analyses: &mut Analyses) -> bool {
    let Ok(accesses) = Accesses::managed(unit.context, unit.layout, unit.function, analyses) else { return false };
    let mut moves: Vec<(InstId, InstId)> = Vec::new();
    for &block in unit.function.layout() {
        let insts = unit.function.block(block).instructions().to_vec();
        for (index, &inst) in insts.iter().enumerate() {
            let instruction = unit.function.instruction(inst);
            let (Opcode::Load { volatile: false, .. }, Some(result)) = (&instruction.opcode, instruction.result) else { continue };
            let users = unit.function.users(result);
            let [only] = users else { continue };
            let user = only.user;
            if unit.function.parent(user) != Some(block) || unit.function.instruction(user).opcode == Opcode::Phi {
                continue;
            }
            let Some(at) = insts.iter().position(|&one| one == user) else { continue };
            if at <= index + 1 || !insts[index + 1..at].iter().all(|&between| _only_computes(unit, &accesses, between)) {
                continue;
            }
            // Its address is still computed before it, wherever it lands: the operands were defined above.
            let _ = Operand::Block;
            moves.push((inst, user));
        }
    }
    for &(load, user) in &moves {
        unit.function.move_to(load, Position::Before(user)).expect("a placed load");
    }
    !moves.is_empty()
}

#[cfg(test)]
#[path = "loadsink_tests.rs"]
mod tests;
