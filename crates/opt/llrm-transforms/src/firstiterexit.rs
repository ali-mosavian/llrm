//! A loop exit whose condition is loop-invariant is taken on the first trip if
//! at all, so a header phi an exit phi reads has there the value from the
//! preheader: `IndVarSimplify::rewriteFirstIterationLoopExitValues`.
//!
//! # How it works
//! For each exit block of a loop with one latch, and each phi input from a
//! block that dominates the latch and ends in a conditional branch or switch on
//! a loop-invariant value, a header phi read there is replaced by its
//! preheader input.
//!
//! ```text
//!  header: %x = phi [ %init, %pre ], [ %x.next, %latch ]
//!  body:   br i1 %inv, label %exit, label %latch      (%inv defined outside)
//!  exit:   %r = phi [ %x, %body ]   ==>   %r = phi [ %init, %body ]
//! ```

use llrm_analysis::cfg;
use llrm_analysis::cfg::Around;
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::occurrence::arms;
use llrm_mir::domtree::DomTree;
use llrm_mir::module::{Function, Operand, ValueDef};
use llrm_mir::opcode::Opcode;

use crate::edges;

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L544
/// `rewriteFirstIterationLoopExitValues` for each loop; whether any exit phi
/// input changed.
pub fn rewritten(
    function: &mut Function,
    loops: &[Loop],
    tree: &DomTree,
) -> bool {
    let mut changed = false;
    for loop_ in loops {
        changed |= rewritten_in(function, loop_, tree);
    }
    changed
}

fn rewritten_in(
    function: &mut Function,
    loop_: &Loop,
    tree: &DomTree,
) -> bool {
    // `getLoopLatch`: one latch.
    let latches: Vec<i64> = loop_.latches.iter().copied().collect();
    let [latch] = latches[..] else { return false };
    let latch = cfg::block(latch);
    let Some(preheader) = loop_.preheader(function).map(cfg::block) else { return false };
    let header = cfg::block(loop_.header);
    let exit_blocks: std::collections::BTreeSet<i64> = loop_.exits(function).into_iter().map(|(_, to)| to).collect();

    let mut edits = Vec::new();
    for exit in exit_blocks {
        for phi in edges::phis(function, cfg::block(exit)) {
            for (at, (value, from)) in arms(function, phi).into_iter().enumerate() {
                // The exit runs on the first trip if at all.
                if !tree.dominates(from, latch) {
                    continue;
                }
                let Some(terminator) = function.terminator(from) else { continue };
                let branch = function.instruction(terminator);
                let condition = match branch.opcode {
                    Opcode::Br if branch.operands.len() == 3 => branch.operands[0],
                    Opcode::Switch => branch.operands[0],
                    _ => continue,
                };
                // `Loop::isLoopInvariant`: not an instruction of the loop.
                if let Operand::Value(v) = condition
                    && let ValueDef::Instruction(def) = function.value(v).def
                    && function.parent(def).is_some_and(|block| loop_.body.contains(&cfg::id(block)))
                {
                    continue;
                }
                // Only phis of the loop header.
                let Operand::Value(v) = value else { continue };
                let ValueDef::Instruction(exit_val) = function.value(v).def else { continue };
                if function.instruction(exit_val).opcode != Opcode::Phi || function.parent(exit_val) != Some(header) {
                    continue;
                }
                if let Some((initial, _)) = arms(function, exit_val).into_iter().find(|&(_, block)| block == preheader)
                {
                    edits.push((phi, 2 * at, initial));
                }
            }
        }
    }
    let changed = !edits.is_empty();
    for (phi, slot, value) in edits {
        function.set_operand(phi, slot, value);
    }
    changed
}

#[cfg(test)]
#[path = "firstiterexit_tests.rs"]
mod tests;
