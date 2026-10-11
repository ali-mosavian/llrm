//! Which bits of each integer value anything reads: LLVM's `DemandedBits`, the
//! backward dual of `valuetracking::known_zero`. A bit no user reads can be
//! anything, so a mask over it, or an operand whose bits are all of that kind,
//! goes.
//!
//! A value an instruction outside the table reads (a store, a call, a compare,
//! a divide, a pointer computation) is read in every bit; the table is
//! `valuetracking::demanded_operands`. A loop's phis are solved to a fixed
//! point: demand only grows.

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
