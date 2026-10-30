//! Which bits of each integer value anything reads: LLVM's `DemandedBits`.
//!
//! Exit: deleted, with `algebraic::_unread_bits` (its only reader), by the
//! InstCombine `SimplifyDemandedBits` translation (instcombine-port); BDCE
//! (`demandedbits`) does not replace it.
//!
//! # How it works
//! A backward fixed point over the use-def graph, the dual of
//! `valuetracking::known_zero`. A bit no user reads can be anything, so a mask
//! that clears only such bits, or an operand all of whose bits are such,
//! can go.
//!
//! Every instruction that is not a pure integer operation reads its integer
//! operands in full: stores, calls, compares, divides and pointer
//! computations. Those seed the map. A pure integer instruction (a phi, or one
//! `valuetracking::demanded_operands` has a rule for) passes on instead: given
//! the bits read of its result, the rule gives the bits read of each operand,
//! and those join into the operand's entry.
//!
//! ```text
//!   %a = load i32          read: 0x000000ff   (only the low byte)
//!   %m = and i32 %a, 255   read: 0xffffffff   <- stored in full
//!   store i32 %m, ptr %p
//!
//!   and passes on 0xff & 0xffffffff = 0xff to %a
//! ```
//!
//! Work goes onto a stack whenever an entry grows, so a loop's phis reach a
//! fixed point: demand only grows and is bounded by the width.
//!
//! Only integers of at most 128 bits are tracked (`u128` masks). A value that
//! is absent from the map is not read at all.
//!
//! # Example
//! No honest example: no test asks `demanded` for its map. The listing above
//! shows the rule by hand; its only consumer is `algebraic` in
//! `llrm-transforms`.
//!
//! # Provides
//! `demanded(context, function)`: a `ValueId -> u128` map. Cached as
//! [`crate::manager::DemandedBits`]. Consumer: `algebraic`'s removal of masks
//! and operands over unread bits (`algebraic` in `llrm-transforms`).
//!
//! # Needs
//! `llrm_mir::valuetracking::demanded_operands`, the per-opcode table.
//!
//! # See also
//! LLVM `DemandedBits` and `SimplifyDemandedBits`.

use llrm_mir::context::Context;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::demanded_operands;

/// Each integer value of up to 128 bits that is read, and the bits of it that
/// are. A value absent is not read; one wider than 128 bits is not asked.
pub fn demanded(
    context: &Context,
    function: &Function,
) -> IdMap<ValueId, u128> {
    let width = |operand: Operand| {
        function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)).filter(|&w| w <= 128)
    };
    let all = |width: u32| if width >= 128 { u128::MAX } else { (1_u128 << width) - 1 };
    let mut found: IdMap<ValueId, u128> = IdMap::new();
    let mut work: Vec<InstId> = Vec::new();
    let add = |found: &mut IdMap<ValueId, u128>, work: &mut Vec<InstId>, operand: Operand, bits: u128| {
        let Operand::Value(value) = operand else { return };
        let Some(w) = width(operand) else { return };
        let bits = bits & all(w);
        let held = found.get(&value).copied().unwrap_or(0);
        if held | bits == held {
            return;
        }
        found.insert(value, held | bits);
        if let ValueDef::Instruction(def) = function.value(value).def {
            work.push(def);
        }
    };
    // Whatever reads in full: all but the pure integer operations, which pass
    // on what their result is read in.
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        let integer = instruction.result.is_some_and(|result| width(Operand::Value(result)).is_some());
        let transfers = integer
            && (matches!(instruction.opcode, Opcode::Phi)
                || demanded_operands(context, function, inst, u128::MAX).is_some());
        if !transfers {
            for &operand in &instruction.operands {
                if let Some(w) = width(operand) {
                    add(&mut found, &mut work, operand, all(w));
                }
            }
        }
    }
    while let Some(inst) = work.pop() {
        let instruction = function.instruction(inst);
        let Some(result) = instruction.result else { continue };
        let Some(&read) = found.get(&result) else { continue };
        if matches!(instruction.opcode, Opcode::Phi) {
            for &operand in &instruction.operands {
                add(&mut found, &mut work, operand, read);
            }
        } else if let Some(per_operand) = demanded_operands(context, function, inst, read) {
            for (&operand, bits) in instruction.operands.iter().zip(per_operand) {
                add(&mut found, &mut work, operand, bits);
            }
        }
    }
    found
}
