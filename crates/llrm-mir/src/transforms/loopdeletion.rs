//! Loops that do nothing removed, as LLVM's LoopDeletion removes them: a
//! loop that counts to its bound, has no effect, and whose values nothing
//! after it reads is skipped, its preheader branching to its one exit.
//! simplifycfg then drops the blocks nothing reaches.

use crate::memory;
use crate::module::{BlockId, Function, Operand, ValueDef};
use crate::opcode::Opcode;
use crate::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, ScalarEvolution, Unit};
use crate::scalarevolution::counted;

pub struct LoopDeletion;

impl FunctionPass for LoopDeletion {
    fn name(&self) -> &'static str {
        "loop-deletion"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let loops = analyses.get::<Loops>(unit.context, unit.layout, unit.function);
        let evolution = analyses.get::<ScalarEvolution>(unit.context, unit.layout, unit.function);
        let mut deleted: Vec<BlockId> = Vec::new();
        // Outer loops first: one deleted takes the loops inside it along.
        for one in &loops.loops {
            if deleted.contains(&one.header) {
                continue;
            }
            let inner = loops.loops.iter().filter(|other| other.blocks.is_subset(&one.blocks));
            if !inner.clone().all(|other| counted(unit.context, unit.function, &tree, &evolution, other).is_some()) {
                continue;
            }
            let function = &*unit.function;
            let outside: Vec<BlockId> = function.predecessors(one.header).into_iter().filter(|block| !one.blocks.contains(block)).collect();
            let [preheader] = outside[..] else { continue };
            if function.successors(preheader) != [one.header] {
                continue;
            }
            let mut exits: Vec<BlockId> = one.blocks.iter().flat_map(|&block| function.successors(block)).filter(|block| !one.blocks.contains(block)).collect();
            exits.dedup();
            let [exit] = exits[..] else { continue };
            let quiet = one.blocks.iter().flat_map(|&block| function.block(block).instructions().to_vec()).all(|inst| {
                let instruction = function.instruction(inst);
                let harmless = match instruction.opcode {
                    Opcode::Load { volatile, .. } => !volatile,
                    Opcode::Call(_) => memory::call_returns(unit.context, unit.callees, function, inst),
                    Opcode::Alloca { .. } | Opcode::Invoke(_) => false,
                    _ => true,
                };
                harmless
                    && !memory::of(unit.context, unit.callees, function, inst).writes
                    && instruction.result.is_none_or(|result| function.users(result).iter().all(|one_use| inside(function, &one.blocks, one_use.user)))
            });
            if !quiet {
                continue;
            }
            // Each exit phi takes one value from the loop, made outside it.
            let mut inputs = Vec::new();
            let phis: Vec<_> = function.block(exit).instructions().iter().copied().take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect();
            let mut fine = true;
            for &phi in &phis {
                let pairs: Vec<(Operand, BlockId)> = function.instruction(phi).operands.chunks(2).map(|pair| match pair[1] {
                    Operand::Block(block) => (pair[0], block),
                    _ => unreachable!("a phi pairs values with blocks"),
                }).collect();
                let from_loop: Vec<Operand> = pairs.iter().filter(|(_, block)| one.blocks.contains(block)).map(|(value, _)| *value).collect();
                let Some(&first) = from_loop.first() else { continue };
                if !from_loop.iter().all(|&one_value| one_value == first) || defined_in(function, &one.blocks, first) {
                    fine = false;
                    break;
                }
                let kept: Vec<Operand> = pairs.iter().filter(|(_, block)| !one.blocks.contains(block)).flat_map(|&(value, block)| [value, Operand::Block(block)]).chain([first, Operand::Block(preheader)]).collect();
                inputs.push((phi, kept));
            }
            if !fine {
                continue;
            }
            for (phi, operands) in inputs {
                unit.function.set_operands(phi, operands);
            }
            // The header, no longer entered from the preheader, forgets it.
            let heads: Vec<_> = unit.function.block(one.header).instructions().iter().copied().take_while(|&inst| unit.function.instruction(inst).opcode == Opcode::Phi).collect();
            for phi in heads {
                let kept = unit.function.instruction(phi).operands.chunks(2).filter(|pair| pair[1] != Operand::Block(preheader)).flatten().copied().collect();
                unit.function.set_operands(phi, kept);
            }
            let branch = unit.function.terminator(preheader).expect("a terminator");
            unit.function.set_operand(branch, 0, Operand::Block(exit));
            deleted.extend(one.blocks.iter().copied());
        }
        if deleted.is_empty() { PreservedAnalyses::all() } else { PreservedAnalyses::none() }
    }
}

fn inside(function: &Function, blocks: &std::collections::BTreeSet<BlockId>, inst: crate::module::InstId) -> bool {
    function.parent(inst).is_some_and(|block| blocks.contains(&block))
}

fn defined_in(function: &Function, blocks: &std::collections::BTreeSet<BlockId>, operand: Operand) -> bool {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => inside(function, blocks, inst),
            ValueDef::Argument(_) => false,
        },
        _ => false,
    }
}
