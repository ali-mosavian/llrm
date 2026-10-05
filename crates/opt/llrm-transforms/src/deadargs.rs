//! LLVM's DeadArgumentElimination for a function only direct calls reach: a parameter nothing reads
//! (but its own recursive calls, which pass it on to itself) goes, and with it the argument of every
//! call. Promotion and the constants propagated into a body leave them: a length every caller passes
//! as 12, folded into the body, is still pushed at every call.

use std::collections::BTreeSet;

use llrm_mir::callgraph::{direct_calls, direct_only};
use llrm_mir::context::GlobalId;
use llrm_mir::memory;
use llrm_mir::module::{GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::Opcode;

/// The functions of `module` that lost a parameter, and the functions that call them.
pub fn removed(module: &mut Module) -> Vec<GlobalId> {
    let only = direct_only(module);
    let llrm_mir::callgraph::DirectCalls { sites, refused } = direct_calls(module);
    let mut changed = BTreeSet::new();
    for &id in only.iter().filter(|id| !refused.contains(id)) {
        let Some(calls) = sites.get(&id) else { continue };
        let count = module.global(id).function().map_or(0, |one| one.parameters().len());
        for parameter in (0..count).rev() {
            if !dead(module, id, parameter) {
                continue;
            }
            let own: Vec<InstId> = calls.iter().filter(|&&(caller, _)| caller == id).map(|&(_, inst)| inst).collect();
            let ty = {
                let Module { context, globals, .. } = &mut *module;
                let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
                for &call in &own {
                    function.replace_argument(call, parameter, &[], function.ty);
                }
                function.remove_parameter(context, parameter);
                for &call in &own {
                    function.set_call_type(call, function.ty);
                }
                function.ty
            };
            for &(caller, call) in calls.iter().filter(|&&(caller, _)| caller != id) {
                let GlobalKind::Function(function) = &mut module.globals[caller.0 as usize].kind else { unreachable!("a caller") };
                function.replace_argument(call, parameter, &[], ty);
                changed.insert(caller);
            }
            changed.insert(id);
        }
    }
    changed.into_iter().collect()
}

/// Whether nothing reads parameter `at` of `id` but the calls it makes to itself with it in place.
fn dead(module: &Module, id: GlobalId, at: usize) -> bool {
    let Some(function) = module.global(id).function() else { return false };
    let Some(&value) = function.parameters().get(at) else { return false };
    function.users(value).iter().all(|one| {
        matches!(function.instruction(one.user).opcode, Opcode::Call(_))
            && one.index as usize == at
            && memory::callee(&module.context, function, one.user) == Some(id)
            && function.instruction(one.user).operands.iter().filter(|&&operand| operand == Operand::Value(value)).count() == 1
    })
}
