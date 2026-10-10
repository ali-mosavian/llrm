//! MIR occurrence keys, adapted from llrm-core's `analysis/occurrence.rs`.
//!
//! The old MIR's operations were values, so an immutable body view supplied
//! their identity as a block and operation ordinal.  A rich MIR instruction
//! has its own identity, its [`InstId`], which outlives rewrites; it is the
//! key here, and `OpOccurrence` and `PhiOccurrence` have no counterpart.
//! Operations and phis stay separate walks, as the old MIR kept them.

use llrm_mir::module::{BlockId, Function, InstId, Instruction};
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
    inside: &'a std::collections::BTreeSet<i64>,
) -> impl Iterator<Item = (InstId, BlockId, &'a Instruction)> {
    inside
        .iter()
        .map(|&at| crate::cfg::block(at))
        .flat_map(move |block| {
            function.block(block).instructions().iter().map(move |&inst| (inst, block, function.instruction(inst)))
        })
        .filter(|(_, _, instruction)| instruction.opcode != Opcode::Phi)
}

/// Enumerate every phi in layout/block order.
pub fn phis(function: &Function) -> impl Iterator<Item = (InstId, BlockId, &Instruction)> {
    instructions(function, true)
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
        assert_eq!(&found[..2], &function.block(block(function, "b0")).instructions()[..2]);
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
        assert_eq!(found, function.block(block(function, "b1")).instructions()[..2]);
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
