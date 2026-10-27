//! The entry block's zero writes to a local nothing has touched yet, for a
//! frame the runtime zero-fills (QB's `B$ENRA`): the frame already holds them.
//! Not the pin's (it has no such runtime); both selectors drop the same set.
//!
//! DEVIATE SD222: the DAG path drops these writes as the old selector does;
//! the pin has no runtime that zero-fills a frame. Owner w12. Exit: none
//! (a QB runtime fact). Review 2026-12-10.
//!
//! # Example
//! No honest example: no dump prints the function before and after. In an
//! entry block `store 0 -> local; call f(local)`, the store is dropped, the
//! call stays.
//!
//! # Used by
//! The DAG path (`selectiondag/builder.rs`) and the old selector (`isel.rs`),
//! for every function they select.

use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::{ConstantKind, Module, Opcode};

/// The zero writes in the entry block to a local no earlier instruction
/// reaches. A local's address is its alloca's, so nothing reaches it before the
/// alloca's first user.
pub fn prezeroed(
    module: &Module,
    function: &Function,
) -> IdSet<InstId> {
    let mut out = IdSet::new();
    let Some(&entry) = function.layout().first() else { return out };
    let mut order: IdMap<InstId, usize> = IdMap::new();
    for (index, inst) in function.iter_block(entry).enumerate() {
        order.insert(inst, index);
    }
    for (index, inst) in function.iter_block(entry).enumerate() {
        let Some(local) = zero_write(module, function, inst) else { continue };
        if function
            .users(local)
            .iter()
            .all(|one| one.user == inst || out.contains(&one.user) || order.get(&one.user).is_none_or(|&at| at > index))
        {
            out.insert(inst);
        }
    }
    out
}

/// The local `inst` fills with zeros, where it is a store or memset of zero
/// straight to an alloca.
fn zero_write(
    module: &Module,
    function: &Function,
    inst: InstId,
) -> Option<ValueId> {
    let instruction = function.instruction(inst);
    let zero = |operand: Operand| match operand {
        Operand::Constant(one) => matches!(
            module.context.get(one).kind,
            ConstantKind::Int(0) | ConstantKind::Float(0) | ConstantKind::Null | ConstantKind::Zero
        ),
        _ => false,
    };
    let (value, destination) = match instruction.opcode {
        Opcode::Store { volatile: false, .. } => (instruction.operands[0], instruction.operands[1]),
        Opcode::Call(_) => {
            let [destination, value, _, volatile, Operand::Constant(callee)] = instruction.operands[..] else {
                return None;
            };
            let ConstantKind::Global(global) = module.context.get(callee).kind else { return None };
            if module.global(global).intrinsic() != Some(Intrinsic::MemSet) || !zero(volatile) {
                return None;
            }
            (value, destination)
        }
        _ => return None,
    };
    let Operand::Value(local) = destination else { return None };
    let ValueDef::Instruction(alloca) = function.value(local).def else { return None };
    (zero(value) && matches!(function.instruction(alloca).opcode, Opcode::Alloca { .. })).then_some(local)
}
