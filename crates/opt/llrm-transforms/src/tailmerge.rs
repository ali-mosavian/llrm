//! The blocks that end the function with the same terminator become one block
//! each: SimplifyCFG's `tailMergeBlocksWithSimilarFunctionTerminators`.
//!
//! # How it works
//! `merged` takes every block that ends in a `ret` (or `resume`) and has no
//! successor, and, when there are two or more of one kind, makes a block
//! `common.ret` before the first of them. It holds one phi for each operand of
//! the terminator, and the terminator on the phis. Each of the blocks then
//! jumps to it, passing its own operand. The pass that follows empties and
//! removes the blocks that held nothing else.
//!
//! # Example
//! Two returns become one block that returns a phi:
//!
//! ```text
//!   a: ...; ret i16 %x           a: ...; br %common.ret
//!   b: ...; ret i16 %y     ==>   b: ...; br %common.ret
//!                                common.ret: %p = phi [%x, a], [%y, b]
//!                                            ret i16 %p
//! ```
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`tailMergeBlocksWithSimilarFunctionTerminators`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L142) | `merged` | MIRROR; D1 |
//! | [`performBlockTailMerging`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L86) | `block_merged` | MIRROR; D1 |
//!
//! - D1. No `llvm.experimental.deoptimize` and no token types in the MIR; debug
//!   locations are not kept.

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode, Tail};

/// Whether `ret` follows a `musttail` call.
fn after_musttail_call(
    function: &Function,
    terminator: InstId,
) -> bool {
    let block = function.parent(terminator).expect("a placed terminator");
    let mut previous = None;
    for inst in function.iter_block(block) {
        if inst == terminator {
            break;
        }
        previous = Some(inst);
    }
    previous.is_some_and(|inst| {
        matches!(
            &function.instruction(inst).opcode,
            Opcode::Call(info) if info.tail == Tail::MustTail
        )
    })
}

/// The blocks of `function` that end it with a `ret` made into jumps to one
/// block that returns. Whether it did.
pub fn merged(
    context: &Context,
    function: &mut Function,
) -> bool {
    let mut groups: Vec<(Opcode, Vec<BlockId>)> = Vec::new();
    for &block in function.layout() {
        let Some(last) = function.terminator(block) else { continue };
        let opcode = &function.instruction(last).opcode;
        if !matches!(opcode, Opcode::Ret | Opcode::Resume) || !function.successors(block).is_empty() {
            continue;
        }
        if after_musttail_call(function, last) {
            continue;
        }
        match groups.iter_mut().find(|(one, _)| one == opcode) {
            Some((_, blocks)) => blocks.push(block),
            None => groups.push((opcode.clone(), vec![block])),
        }
    }
    let mut changed = false;
    for (opcode, blocks) in groups {
        changed |= block_merged(context, function, &opcode, &blocks);
    }
    changed
}

fn block_merged(
    context: &Context,
    function: &mut Function,
    opcode: &Opcode,
    blocks: &[BlockId],
) -> bool {
    // Only where there are two blocks to merge.
    if blocks.len() < 2 || function.layout().first() == Some(&blocks[0]) {
        return false;
    }
    let first = function.terminator(blocks[0]).expect("a terminated block");
    let void = function.instruction(first).ty;
    let canonical = function.create_block(Some(&format!("common.{}", opcode.mnemonic())));
    let before = function.layout().iter().position(|&one| one == blocks[0]).expect("a placed block");
    let after = function.layout()[before - 1];
    function.insert_block(canonical, Some(after)).expect("a new block");
    let operands = function.instruction(first).operands.clone();
    // Each block's operand in each arm, gathered first so a phi is made once
    // with all its arms.
    let mut arms: Vec<Vec<Operand>> = vec![Vec::with_capacity(2 * blocks.len()); operands.len()];
    for &block in blocks {
        let old = function.terminator(block).expect("a terminated block");
        for (list, &operand) in arms.iter_mut().zip(&function.instruction(old).operands) {
            list.push(operand);
            list.push(Operand::Block(block));
        }
    }
    let mut phis = Vec::new();
    for (operand, list) in operands.iter().zip(arms) {
        let ty = function.operand_type(context, *operand).expect("a value operand");
        let phi = function.create_instruction(Opcode::Phi, ty, list, Flags::default(), Some("common.op"));
        function.insert(phi, Position::End(canonical)).expect("a new phi");
        phis.push(phi);
    }
    let terminator = function.clone_instruction(first);
    function.insert(terminator, Position::End(canonical)).expect("a new terminator");
    for (at, &phi) in phis.iter().enumerate() {
        let result = function.instruction(phi).result.expect("a phi's value");
        function.set_operand(terminator, at, Operand::Value(result));
    }
    for &block in blocks {
        let old = function.terminator(block).expect("a terminated block");
        function.erase(old).expect("a terminator defines nothing");
        let jump =
            function.create_instruction(Opcode::Br, void, vec![Operand::Block(canonical)], Flags::default(), None);
        function.insert(jump, Position::End(block)).expect("a placed block");
    }
    true
}

#[cfg(test)]
#[path = "tailmerge_tests.rs"]
mod tests;
