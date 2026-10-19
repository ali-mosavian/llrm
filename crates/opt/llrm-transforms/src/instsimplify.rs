//! InstSimplify as a pass: every instruction that equals one of its operands,
//! or a constant, is replaced by it, and what that leaves dead goes.
//!
//! # How it works
//! A translation of LLVM's
//! [`InstSimplifyPass.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/InstSimplifyPass.cpp)
//! `runImpl` (:27) at d1106deb71cc. MIRROR: the first sweep visits every
//! instruction of every reachable block in layout order, later sweeps only
//! the users of what was replaced; an instruction nothing reads and running
//! does nothing else is deleted, the instruction found by
//! `simplify_instruction_in` replaces the rest and its users are visited
//! again; the dead of a block go when the block is done, and with them what
//! only they read (`RecursivelyDeleteTriviallyDeadInstructions`).
//!
//! The query has the function's dominator tree and assumptions
//! (`FunctionAnalyses`), built once: replacing a value and deleting dead
//! arithmetic changes neither.
//!
//! # Parity slot
//! Slot 38 of the slot table (docs/architecture plan 2.5): `instsimplify`,
//! once per function pipeline after the loop group at -O1/-O2/-O3. Joined by
//! naming it in `llvm_order`.
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::module::{Function, GlobalValue, InstId, MetadataNode, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::simplify::simplify_instruction_in;
use llrm_mir::valuetracking::FunctionAnalyses;

/// `isInstructionTriviallyDead`: nothing reads it and running it does nothing
/// else.
fn is_trivially_dead(
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let Some(result) = instruction.result else { return false };
    function.users(result).is_empty()
        && matches!(
            instruction.opcode,
            Opcode::Binary(_)
                | Opcode::Cast(_)
                | Opcode::ICmp(_)
                | Opcode::FCmp(_)
                | Opcode::Select
                | Opcode::Phi
                | Opcode::GetElementPtr { .. }
                | Opcode::Alloca { .. }
                | Opcode::FNeg
                | Opcode::Freeze
                | Opcode::ExtractValue(_)
                | Opcode::InsertValue(_)
                | Opcode::Load { volatile: false, .. }
        )
}

/// `RecursivelyDeleteTriviallyDeadInstructions`: `dead` erased, then each
/// operand's defining instruction that nothing reads now.
fn delete_dead(
    function: &mut Function,
    mut dead: Vec<InstId>,
) {
    while let Some(inst) = dead.pop() {
        // One whose reader is still to go is met again through that reader.
        if function.parent(inst).is_none() || !is_trivially_dead(function, inst) {
            continue;
        }
        let operands = function.instruction(inst).operands.to_vec();
        function.erase(inst).expect("nothing reads it");
        for operand in operands {
            if let Operand::Value(value) = operand
                && let ValueDef::Instruction(def) = function.value(value).def
            {
                dead.push(def);
            }
        }
    }
}

/// `runImpl`. Whether anything changed.
pub fn instsimplify(
    context: &mut Context,
    globals: &[GlobalValue],
    metadata: &[MetadataNode],
    layout: &DataLayout,
    function: &mut Function,
) -> bool {
    let analyses = FunctionAnalyses::of(context, globals, metadata, layout, function);
    let mut changed = false;
    let (mut to_simplify, mut next) = (IdSet::<InstId>::default(), IdSet::<InstId>::default());
    loop {
        for block in function.layout().to_vec() {
            // Unreachable code can take on strange forms: an instruction may
            // have itself as an operand.
            if !analyses.dominator_tree().is_reachable(block) {
                continue;
            }
            let mut dead = Vec::new();
            for inst in function.iter_block(block).collect::<Vec<_>>() {
                // The first sweep has nothing in `to_simplify` and tries every
                // instruction; later ones only those in it.
                if !to_simplify.is_empty() && !to_simplify.contains(&inst) {
                    continue;
                }
                if is_trivially_dead(function, inst) {
                    dead.push(inst);
                    changed = true;
                    continue;
                }
                let Some(result) = function.instruction(inst).result else { continue };
                if function.users(result).is_empty() {
                    continue;
                }
                let Some(found) = simplify_instruction_in(context, globals, layout, Some(&analyses), function, inst)
                else {
                    continue;
                };
                if found == Operand::Value(result) {
                    continue;
                }
                // Mark all uses for resimplification next time round.
                for used in function.users(result) {
                    next.insert(used.user);
                }
                function.replace_value(result, found);
                changed = true;
                if is_trivially_dead(function, inst) {
                    dead.push(inst);
                }
            }
            delete_dead(function, dead);
        }
        std::mem::swap(&mut to_simplify, &mut next);
        next.clear();
        if to_simplify.is_empty() {
            return changed;
        }
    }
}

pub struct InstSimplify;

impl FunctionPass for InstSimplify {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "instsimplify"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        if instsimplify(unit.context, &outer.globals, unit.metadata, unit.layout, unit.function) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "instsimplify_tests.rs"]
mod tests;
