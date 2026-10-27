//! Loads of an internal variable that its only store dominates read the
//! stored constant: LLVM GlobalOpt's `forwardStoredOnceStore`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/GlobalOpt.cpp#L1448>).
//!
//! # How it works
//! A variable qualifies when it is internal, initialized, not `constant`, not
//! named by another global's initializer, and every use of it is a plain load
//! or store. It must be stored once, anywhere in the program, with a constant
//! other than its initializer. A load of the variable's own address in the
//! storing function, of the stored type, that the store dominates reads that
//! constant, even when the function recurses; the load is replaced by it.
//! Loads elsewhere, or not dominated, still read the initializer or the
//! constant and stay.
//!
//! # Example
//! ```mir
//! @c = internal global float 0.0
//! define float @f() {          define float @f() {
//! b0:                          b0:
//!   store float 18.0, ptr @c     store float 18.0, ptr @c
//!   br label %b1                 br label %b1
//! b1:                    =>    b1:
//!   %a = load float, ptr @c      ret float 18.0
//!   ret float %a               }
//! }
//! ```
//!
//! # Runs
//! After `globalsra`, which localizes a variable one non-recursive body uses
//! before this sees it.
//!
//! # See also
//! `globalsra`, which splits a variable into the pieces this forwards.

use std::collections::BTreeSet;

use llrm_mir::context::GlobalId;
use llrm_mir::domtree::DomTree;
use llrm_mir::module::{GlobalKind, InstId, Linkage, Module, Operand};
use llrm_mir::opcode::Opcode;

/// The functions changed.
pub fn forwarded(module: &mut Module) -> Vec<GlobalId> {
    let held = crate::globalopt::held_by_live(module);
    let reached = crate::globalsra::reaching(module);
    let mut plans = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        let id = GlobalId(at as u32);
        let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
        let Some(initializer) = variable.initializer else { continue };
        if !internal
            || global.address_space != 0
            || variable.constant
            || variable.externally_initialized
            || held.contains(&id)
        {
            continue;
        }
        let mut loads: Vec<(GlobalId, InstId)> = Vec::new();
        let mut stores: Vec<(GlobalId, InstId)> = Vec::new();
        let mut plain = true;
        for (&function_id, table) in &reached {
            let Some(found) = table.get(&id) else { continue };
            let Some(found) = found else {
                plain = false;
                break;
            };
            let GlobalKind::Function(function) = &module.globals[function_id.0 as usize].kind else { continue };
            for &(inst, index) in &found.uses {
                match function.instruction(inst).opcode {
                    Opcode::Load { volatile: false, .. } if index == 0 => loads.push((function_id, inst)),
                    Opcode::Store { volatile: false, .. } if index == 1 => stores.push((function_id, inst)),
                    _ => plain = false,
                }
            }
        }
        let [(owner, store)] = stores[..] else { continue };
        if !plain {
            continue;
        }
        let GlobalKind::Function(function) = &module.globals[owner.0 as usize].kind else { continue };
        let Operand::Constant(stored) = function.instruction(store).operands[0] else { continue };
        let target = |operand: Operand| {
            matches!(
                operand,
                Operand::Constant(one) if module.context.get(one).kind == llrm_mir::context::ConstantKind::Global(id)
            )
        };
        if stored == initializer || !target(function.instruction(store).operands[1]) {
            continue;
        }
        let stored_type = module.context.get(stored).ty;
        let tree = DomTree::of(function);
        let forwards: Vec<InstId> = loads
            .iter()
            .filter(|&&(function_id, load)| {
                // Another function's instruction ids are not this one's.
                function_id == owner && {
                    let instruction = function.instruction(load);
                    instruction.ty == stored_type
                        && target(instruction.operands[0])
                        && tree.instruction_dominates(function, store, load)
                }
            })
            .map(|&(_, load)| load)
            .collect();
        if forwards.is_empty() {
            continue;
        }
        plans.push((owner, stored, forwards));
    }
    let mut changed = BTreeSet::new();
    for (owner, stored, forwards) in plans {
        let GlobalKind::Function(function) = &mut module.globals[owner.0 as usize].kind else { continue };
        for load in forwards {
            if let Some(result) = function.instruction(load).result {
                function.replace_all_uses_with(result, Operand::Constant(stored));
            }
            let _ = function.erase(load);
        }
        changed.insert(owner);
    }
    changed.into_iter().collect()
}

#[cfg(test)]
#[path = "globalforward_tests.rs"]
mod tests;
