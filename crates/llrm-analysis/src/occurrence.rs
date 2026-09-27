//! MIR occurrence keys, adapted from llrm-core's `analysis/occurrence.rs`.
//!
//! The old MIR's operations were values, so an immutable body view supplied
//! their identity as a block and operation ordinal.  A rich MIR instruction
//! has its own identity, its [`InstId`], which outlives rewrites; it is the
//! key here, and `OpOccurrence` and `PhiOccurrence` have no counterpart.
//! Operations and phis stay separate walks, as the old MIR kept them.

use llrm_mir::module::{BlockId, Function, InstId, Instruction};
use llrm_mir::opcode::Opcode;

fn instructions(function: &Function, phi: bool) -> impl Iterator<Item = (InstId, BlockId, &Instruction)> {
    function
        .walk()
        .map(move |(block, inst)| (inst, block, function.instruction(inst)))
        .filter(move |(_, _, instruction)| (instruction.opcode == Opcode::Phi) == phi)
}

/// Enumerate every operation, phis aside, in layout/block order.
pub fn operations(function: &Function) -> impl Iterator<Item = (InstId, BlockId, &Instruction)> {
    instructions(function, false)
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
        let module = parsed("define void @f(ptr %p) {
b0:
  store i16 1, ptr %p
  store i16 1, ptr %p
  ret void
}
");
        let function = function(&module, "f");
        let found = operations(function).map(|(occurrence, _, _)| occurrence).collect::<Vec<_>>();

        assert_eq!(found.len(), 3);
        assert_ne!(found[0], found[1]);
        assert_eq!(function.instruction(found[0]), function.instruction(found[1]));
        assert_eq!(&found[..2], &function.block(block(function, "b0")).instructions()[..2]);
    }

    #[test]
    fn direct_induction_transparent_aliases_distinguishes_equal_phi_occurrences() {
        let module = parsed("define void @f(i16 %x) {
b0:
  br label %b1

b1:
  %a = phi i16 [ %x, %b0 ]
  %b = phi i16 [ %x, %b0 ]
  ret void
}
");
        let function = function(&module, "f");
        let found = phis(function).map(|(occurrence, _, _)| occurrence).collect::<Vec<_>>();

        assert_eq!(found.len(), 2);
        assert_ne!(found[0], found[1]);
        assert_eq!(found, function.block(block(function, "b1")).instructions()[..2]);
    }
}
