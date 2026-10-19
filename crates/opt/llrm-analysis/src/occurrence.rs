//! Walks a function's instructions as occurrences, operations and phis apart,
//! so an analysis can key by identity and keep two equal instructions distinct.
//!
//! # How it works
//! An [`InstId`] names an instruction and outlives rewrites, so it is the key
//! of an occurrence. Two equal instructions have different ids. `operations`
//! yields every non-phi instruction in layout order with its block;
//! `operations_in` yields those of a set of blocks (numbered as `cfg::id`) in
//! block-id order, so walking a loop costs the loop, not the function; `phis`
//! yields the phis in layout order. Operations and phis are separate walks.
//!
//! ```text
//!  block b2:  %p = phi ...   <- phis()
//!             %y = add ...   <- operations()
//!             ret %y         <- operations()
//! ```
//!
//! # Example
//! ```mir
//! define i16 @f(i1 %c, i16 %x) {
//! b0:
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   br label %b2
//!
//! b2:
//!   %p = phi i16 [ %x, %b0 ], [ 1, %b1 ]
//!   %y = add i16 %p, 1
//!   ret i16 %y
//! }
//! ```
//!
//! | walk           | yields, in layout order                              |
//! |----------------|------------------------------------------------------|
//! | `phis`         | `%p`, with block `b2`                                |
//! | `operations`   | the two `br`s, `%y`, the `ret`                       |
//!
//! From unit test
//! `operations_and_phis_partition_every_instruction_in_layout_order`
//! in `occurrence.rs`.
//!
//! # Provides
//! The three walks. Consumers: [`crate::induction`], [`crate::ssa`], and the
//! `unswitch` and `loopexit` passes in llrm-transforms.

use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand};
use llrm_mir::opcode::Opcode;

fn instructions(
    function: &Function,
    phi: bool,
) -> impl Iterator<Item = (InstId, BlockId, &Instruction)> {
    function
        .walk()
        .map(move |(block, inst)| (inst, block, function.instruction(inst)))
        .filter(move |(_, _, instruction)| (instruction.opcode == Opcode::Phi) == phi)
}

/// Enumerate every operation, phis aside, in layout/block order.
pub fn operations(function: &Function) -> impl Iterator<Item = (InstId, BlockId, &Instruction)> {
    instructions(function, false)
}

/// Enumerate the operations of the blocks `inside` (by `cfg::id`), phis aside,
/// in block-id order: the walk of a loop costs the loop, not the function.
pub fn operations_in<'a>(
    function: &'a Function,
    inside: impl IntoIterator<Item = &'a i64> + 'a,
) -> impl Iterator<Item = (InstId, BlockId, &'a Instruction)> {
    inside
        .into_iter()
        .map(|&at| crate::cfg::block(at))
        .flat_map(move |block| function.iter_block(block).map(move |inst| (inst, block, function.instruction(inst))))
        .filter(|(_, _, instruction)| instruction.opcode != Opcode::Phi)
}

/// Enumerate every phi in layout/block order.
pub fn phis(function: &Function) -> impl Iterator<Item = (InstId, BlockId, &Instruction)> {
    instructions(function, true)
}

/// A phi's inputs, each value with the block it comes from.
pub fn arms(
    function: &Function,
    phi: InstId,
) -> Vec<(Operand, BlockId)> {
    function
        .instruction(phi)
        .operands
        .chunks(2)
        .map(|pair| match pair[1] {
            Operand::Block(block) => (pair[0], block),
            other => unreachable!("a phi's block operand, not {other:?}"),
        })
        .collect()
}

/// The operands of a phi with these inputs: `arms`' inverse.
pub fn from_arms(arms: &[(Operand, BlockId)]) -> Vec<Operand> {
    arms.iter().flat_map(|&(value, block)| [value, Operand::Block(block)]).collect()
}

#[cfg(test)]
mod tests {
    use super::{operations, phis};
    use crate::testing::{block, function, parsed};

    #[test]
    fn direct_induction_transparent_aliases_distinguishes_equal_operation_occurrences() {
        let module = parsed(
            "define void @f(ptr %p) {
b0:
  store i16 1, ptr %p
  store i16 1, ptr %p
  ret void
}
",
        );
        let function = function(&module, "f");
        let found = operations(function).map(|(occurrence, _, _)| occurrence).collect::<Vec<_>>();

        assert_eq!(found.len(), 3);
        assert_ne!(found[0], found[1]);
        assert_eq!(function.instruction(found[0]), function.instruction(found[1]));
        assert_eq!(&found[..2], function.iter_block(block(function, "b0")).take(2).collect::<Vec<_>>());
    }

    #[test]
    fn direct_induction_transparent_aliases_distinguishes_equal_phi_occurrences() {
        let module = parsed(
            "define void @f(i16 %x) {
b0:
  br label %b1

b1:
  %a = phi i16 [ %x, %b0 ]
  %b = phi i16 [ %x, %b0 ]
  ret void
}
",
        );
        let function = function(&module, "f");
        let found = phis(function).map(|(occurrence, _, _)| occurrence).collect::<Vec<_>>();

        assert_eq!(found.len(), 2);
        assert_ne!(found[0], found[1]);
        assert_eq!(found, function.iter_block(block(function, "b1")).take(2).collect::<Vec<_>>());
    }

    #[test]
    fn operations_and_phis_partition_every_instruction_in_layout_order() {
        let module = parsed(
            "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %p = phi i16 [ %x, %b0 ], [ 1, %b1 ]
  %y = add i16 %p, 1
  ret i16 %y
}
",
        );
        let function = function(&module, "f");
        let everything = function.walk().map(|(_, inst)| inst).collect::<Vec<_>>();
        let ops = operations(function).map(|(inst, _, _)| inst).collect::<Vec<_>>();
        let found = phis(function).map(|(inst, at, _)| (inst, at)).collect::<Vec<_>>();
        assert_eq!(found, [(everything[2], block(function, "b2"))]);
        assert_eq!(ops, [everything[0], everything[1], everything[3], everything[4]]);
    }
}
