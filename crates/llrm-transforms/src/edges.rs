//! Adapted from llrm-core's `optimize/edges.rs`, the port of
//! `qbopt/optimize/edges.py`: place semantic operations on conditional
//! edges, not on either arm.
//!
//! `fresh` is not ported: block ids are never reused, so `create_block`
//! is the fresh label. Its old callers were unswitch, loadjoins, inline,
//! loopclone, loopsimplify, lower_switches and the BC raise's dispatch.

use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};

/// `target` is the edge `block`'s conditional branch takes when its
/// condition holds: the old branch's own target, not its fall-through.
pub fn explicit(function: &Function, block: BlockId, target: BlockId) -> bool {
    conditional(function, block, target) && branch(function, block).is_some_and(|one| function.instruction(one).operands[1] == Operand::Block(target))
}

/// `block` ends in a two-way conditional branch, one way to `target`.
pub fn conditional(function: &Function, block: BlockId, target: BlockId) -> bool {
    let successors = function.successors(block);
    successors.len() == 2 && successors.contains(&target) && branch(function, block).is_some()
}

/// `block`'s terminator, if it is `br i1`.
fn branch(function: &Function, block: BlockId) -> Option<InstId> {
    function.terminator(block).filter(|&one| {
        let instruction = function.instruction(one);
        instruction.opcode == Opcode::Br && instruction.operands.len() == 3
    })
}

/// A new block on the conditional edge `source` to `target`, holding
/// `instructions` (created, placed nowhere) and a jump on.
pub fn split(function: &mut Function, source: BlockId, target: BlockId, instructions: Vec<InstId>) -> Result<BlockId, String> {
    if !function.layout().contains(&source) || !conditional(function, source, target) {
        return Err("edge split does not identify a conditional edge".into());
    }
    let parent = branch(function, source).expect("a conditional edge ends in a branch");
    let void = function.instruction(parent).ty;
    let bridge = function.create_block(None);
    function.insert_block(bridge, None)?;
    for inst in instructions {
        function.insert(inst, Position::End(bridge))?;
    }
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(target)], Flags::default(), None);
    function.insert(jump, Position::End(bridge))?;
    retarget(function, parent, target, bridge);
    for phi in phis(function, target) {
        let operands = function.instruction(phi).operands.iter().map(|&one| if one == Operand::Block(source) { Operand::Block(bridge) } else { one }).collect();
        function.set_operands(phi, operands);
    }
    Ok(bridge)
}

/// `terminator`'s edges to `target` now go to `to`.
pub(crate) fn retarget(function: &mut Function, terminator: InstId, target: BlockId, to: BlockId) {
    for (at, operand) in function.instruction(terminator).operands.clone().into_iter().enumerate() {
        if operand == Operand::Block(target) {
            function.set_operand(terminator, at, Operand::Block(to));
        }
    }
}

/// `block`'s phis, in order.
pub(crate) fn phis(function: &Function, block: BlockId) -> Vec<InstId> {
    function.block(block).instructions().iter().copied().take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bridge takes over the edge, and the target's phi names it.
    #[test]
    fn test_a_split_edge_carries_the_phi_input() {
        let text = "define i16 @f(i1 %c) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  br label %b3

b3:
  %r = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  ret i16 %r
}
";
        let mut module = llrm_mir::parse::module(text).expect("parses");
        let (_, function) = module.function_mut("f").expect("@f");
        let (b1, b2, b3) = (function.layout()[0], function.layout()[1], function.layout()[2]);
        assert!(explicit(function, b1, b2) && !explicit(function, b1, b3));
        assert!(split(function, b2, b3, vec![]).is_err(), "not a conditional edge");
        split(function, b1, b3, vec![]).expect("splits");
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
        assert_eq!(
            llrm_mir::print::module(&module),
            "define i16 @f(i1 %c) {
b1:
  br i1 %c, label %b2, label %0

b2:
  br label %b3

b3:
  %r = phi i16 [ 1, %0 ], [ 2, %b2 ]
  ret i16 %r

0:
  br label %b3
}
"
        );
    }
}
