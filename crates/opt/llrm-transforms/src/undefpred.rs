//! A predecessor that gives a block a value that is undefined behavior to use
//! there stops reaching it: SimplifyCFG's `removeUndefIntroducingPredecessor`.
//!
//! # How it works
//! `removed` looks at each input of each phi of `block`. When
//! [`always_undefined`](crate::alwaysundef) says the input is undefined where
//! the phi is used (a null the block loads through, say), the predecessor
//! that gives it ends differently: a jump becomes `unreachable`; a
//! conditional branch becomes a jump to its other target (or `unreachable`
//! where both targets are the block); a `switch` sends the cases that reach
//! the block to a new block that is `unreachable`. One predecessor per call.
//!
//! # Example
//! The way in that gives the null is cut:
//!
//! ```text
//!   b1: br b3                       b1: unreachable
//!   b2: br b3                       b2: br b3
//!   b3: %p = phi ptr [ null, %b1 ], [ %q, %b2 ]      b3: %p = phi ptr [ %q, %b2 ]
//!       %v = load i16, ptr %p                            %v = load i16, ptr %p
//! ```
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`removeUndefIntroducingPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9230) | `removed` | MIRROR; D1 |
//!
//! - D1. A conditional branch keeps no `llvm.assume` of its guarding condition:
//!   a function pass cannot declare `llvm.assume`, a global of the module.
//!   Exit: the pass given the module's declarations (issue to file with the
//!   first user of an intrinsic created by a function pass).

use llrm_mir::context::Context;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};

use crate::twoentryphi::Speculation;

/// `Unreachable` as the last instruction of `block`, replacing `old`.
fn made_unreachable(
    function: &mut Function,
    block: BlockId,
    old: InstId,
) {
    let void = function.instruction(old).ty;
    function.erase(old).expect("a terminator defines nothing");
    let end = function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
    function.insert(end, llrm_mir::edit::Position::End(block)).expect("a placed block");
}

/// The predecessor of `block` that gives it an input that is undefined
/// behavior no longer reaches it. Whether one did.
pub fn removed(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    // A block with no phi has no input to be undefined.
    if function.iter_block(block).next().is_none_or(|first| function.instruction(first).opcode != Opcode::Phi) {
        return false;
    }
    for phi in crate::edges::phis(function, block) {
        // Only a constant is undefined on the edge: the pin's
        // `passingValueIsAlwaysUndefined` returns on anything else first.
        if !function.instruction(phi).operands.chunks(2).any(|arm| matches!(arm[0], Operand::Constant(_))) {
            continue;
        }
        for (value, pred) in llrm_analysis::occurrence::arms(function, phi) {
            if !crate::alwaysundef::always_undefined(context, spec, function, value, phi, false) {
                continue;
            }
            let Some(last) = function.terminator(pred) else { continue };
            let operands = function.instruction(last).operands.clone();
            match (&function.instruction(last).opcode, &operands[..]) {
                (Opcode::Br, [Operand::Block(_)]) => {
                    function.predecessor_removed(context, block, pred, true);
                    made_unreachable(function, pred, last);
                    return true;
                }
                (Opcode::Br, [_, Operand::Block(taken), Operand::Block(other)]) => {
                    function.predecessor_removed(context, block, pred, true);
                    if taken == other {
                        // Two edges.
                        function.predecessor_removed(context, block, pred, true);
                        made_unreachable(function, pred, last);
                    } else {
                        let void = function.instruction(last).ty;
                        let to = if *taken == block { *other } else { *taken };
                        function.erase(last).expect("a terminator defines nothing");
                        let jump = function.create_instruction(
                            Opcode::Br,
                            void,
                            vec![Operand::Block(to)],
                            Flags::default(),
                            None,
                        );
                        function.insert(jump, llrm_mir::edit::Position::End(pred)).expect("a placed block");
                    }
                    return true;
                }
                (Opcode::Switch, _) => {
                    // Every edge to the block goes to a block of its own that
                    // does not return.
                    let void = function.instruction(last).ty;
                    let dead = function.create_block(Some("unreachable"));
                    let before = function.layout().iter().position(|&one| one == block).expect("a placed block");
                    let after = before.checked_sub(1).map(|at| function.layout()[at]);
                    function.insert_block(dead, after).expect("a new block");
                    let end =
                        function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
                    function.insert(end, llrm_mir::edit::Position::End(dead)).expect("a placed block");
                    for (at, operand) in operands.iter().enumerate() {
                        if *operand == Operand::Block(block) {
                            function.predecessor_removed(context, block, pred, true);
                            function.set_operand(last, at, Operand::Block(dead));
                        }
                    }
                    return true;
                }
                _ => {}
            }
        }
    }
    false
}

#[cfg(test)]
#[path = "undefpred_tests.rs"]
mod tests;
