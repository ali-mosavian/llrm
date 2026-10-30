//! LoopInstSimplify: the instructions of a loop that equal an operand or a
//! constant are replaced by it, and what that leaves dead goes.
//!
//! # How it works
//! A translation of LLVM's
//! [`LoopInstSimplify.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopInstSimplify.cpp)
//! `simplifyLoopInst` (:41) at d1106deb71cc. MIRROR: the blocks of a loop are
//! walked in reverse postorder from its header, so a definition is met before
//! its uses but for phis; the first sweep tries every instruction, the next
//! ones only the users of what was replaced, and a phi already visited this
//! sweep that reads a replaced value sends the loop round again. A
//! replacement that would break LCSSA form is skipped. What nothing reads
//! goes when the sweep ends, with what only it read.
//!
//! DEVIATE 1: the slot runs the pass over every loop, innermost first, before
//! the next slot; the pin's loop pass manager runs LoopInstSimplify,
//! LoopSimplifyCFG, LICM and the rest on one loop, then on the next.
//!
//! DEVIATE 2: no MemorySSA to update (llrm builds it where a pass asks).
//!
//! # Parity slot
//! `loop-instsimplify@715`, first of the loop pass manager `LPM1`
//! ([PassBuilderPipelines.cpp#L715](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L715)),
//! after Reassociate and ConstraintElimination, at -O1 too.
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.
//!
//! # Example
//! An `add %x, 0` in a loop body is replaced by `%x`; a phi that merges the
//! same value from both sides is replaced by it.

use llrm_analysis::effects::Declarations;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::memory::Callees;
use llrm_mir::module::{Function, GlobalValue, InstId, MetadataNode, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::simplify::simplify_instruction_in;
use llrm_mir::valuetracking::FunctionAnalyses;

use crate::local::is_instruction_trivially_dead;
use crate::loopdeletion::loop_blocks_rpo;
use crate::looprotate::replacement_preserves_lcssa_form;

/// `RecursivelyDeleteTriviallyDeadInstructions`: `dead` erased, then each
/// operand's defining instruction that nothing reads now.
fn delete_dead(
    context: &Context,
    callees: &Callees,
    declarations: &Declarations,
    function: &mut Function,
    mut dead: Vec<InstId>,
) {
    while let Some(inst) = dead.pop() {
        // One whose reader is still to go is met again through that reader.
        if function.parent(inst).is_none()
            || !is_instruction_trivially_dead(context, callees, declarations, function, inst)
        {
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

/// `simplifyLoopInst` for the loop headed by `header`. Whether anything
/// changed.
#[allow(clippy::too_many_arguments)]
fn simplify_loop_inst(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    loops: &LoopInfo,
    tree: &DomTree,
    analyses: &FunctionAnalyses,
    header: llrm_mir::module::BlockId,
) -> bool {
    let lp = loops.get_loop_for(header).expect("a loop headed by this block");
    // On the first pass over the loop body every instruction is tried; on
    // later ones only those whose inputs were updated.
    let (mut to_simplify, mut next) = (IdSet::<InstId>::default(), IdSet::<InstId>::default());
    // The phis visited during this iteration, to tell when it must go round.
    let mut visited_phis = IdSet::<InstId>::default();
    // Dead code found while simplifying goes at the end of an iteration.
    let mut dead_insts: Vec<InstId> = Vec::new();
    let rpo = loop_blocks_rpo(function, lp);

    let mut changed = false;
    loop {
        for &block in &rpo {
            for inst in function.iter_block(block).collect::<Vec<_>>() {
                if function.instruction(inst).opcode == Opcode::Phi {
                    visited_phis.insert(inst);
                }
                let result = function.instruction(inst).result;
                if result.is_none_or(|result| function.users(result).is_empty()) {
                    if is_instruction_trivially_dead(context, callees, globals, function, inst) {
                        dead_insts.push(inst);
                    }
                    continue;
                }
                let result = result.expect("an instruction with users has a result");

                // The first iteration is told by the empty set.
                let is_first_iteration = to_simplify.is_empty();
                if !is_first_iteration && !to_simplify.contains(&inst) {
                    continue;
                }

                let Some(found) = simplify_instruction_in(context, globals, layout, Some(analyses), function, inst)
                else {
                    continue;
                };
                if found == Operand::Value(result) || !replacement_preserves_lcssa_form(loops, function, inst, found) {
                    continue;
                }

                let users: Vec<InstId> = function.users(result).into_iter().map(|used| used.user).collect();
                function.replace_value(result, found);
                for user in users {
                    // Do not bother dealing with unreachable code.
                    let Some(user_block) = function.parent(user) else { continue };
                    if !tree.is_reachable(user_block) {
                        continue;
                    }
                    // A phi already processed this iteration needs another
                    // one to converge.
                    if function.instruction(user).opcode == Opcode::Phi && visited_phis.contains(&user) {
                        next.insert(user);
                        continue;
                    }
                    // Only targeted instructions are tried after the first
                    // iteration: a user in the loop is one, and its
                    // definitions were met before it. A use outside the loop
                    // is a phi of LCSSA form: not worth trying.
                    if !is_first_iteration && lp.contains(user_block) {
                        to_simplify.insert(user);
                    }
                }

                if is_instruction_trivially_dead(context, callees, globals, function, inst) {
                    dead_insts.push(inst);
                }
                changed = true;
            }
        }

        // Delete what was found dead now that an iteration is over.
        if !dead_insts.is_empty() {
            changed = true;
            delete_dead(context, callees, globals, function, std::mem::take(&mut dead_insts));
        }

        // No phi to simplify again: done.
        if next.is_empty() {
            break;
        }
        std::mem::swap(&mut next, &mut to_simplify);
        next.clear();
        visited_phis.clear();
        dead_insts.clear();
    }
    changed
}

/// The pass over every loop of `function`, innermost first. Whether anything
/// changed.
pub fn loop_instsimplify(
    context: &mut Context,
    globals: &[GlobalValue],
    metadata: &[MetadataNode],
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
) -> bool {
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let analyses = FunctionAnalyses::of(context, globals, metadata, layout, function);
    let headers: Vec<_> =
        loops.get_loops_in_reverse_sibling_preorder().iter().rev().map(|one| one.get_header()).collect();
    let mut changed = false;
    for header in headers {
        changed |= simplify_loop_inst(context, globals, layout, callees, function, &loops, &tree, &analyses, header);
    }
    changed
}

pub struct LoopInstSimplify;

impl FunctionPass for LoopInstSimplify {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "loop-instsimplify"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("loop-instsimplify")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        if loop_instsimplify(unit.context, &outer.globals, unit.metadata, unit.layout, outer.callees(), unit.function) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "loopinstsimplify_tests.rs"]
mod tests;
