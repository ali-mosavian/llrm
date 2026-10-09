//! LLVM's DeadArgumentElimination for a function only direct calls reach: a
//! parameter nothing reads (but its own recursive calls, which pass it on to
//! itself) goes, and with it the argument of every call. Promotion and the
//! constants propagated into a body leave them: a length every caller passes as
//! 12, folded into the body, is still pushed at every call.

use std::collections::BTreeSet;

use llrm_mir::callgraph::{direct_calls, direct_only};
use llrm_mir::context::GlobalId;
use llrm_mir::memory;
use llrm_mir::module::{GlobalKind, InstId, Module};
use llrm_mir::opcode::Opcode;

/// The functions of `module` that lost a parameter, and the functions that call
/// them.
pub fn removed(module: &mut Module) -> Vec<GlobalId> {
    let only = direct_only(module);
    let llrm_mir::callgraph::DirectCalls { sites, refused } = direct_calls(module);
    let mut changed = BTreeSet::new();
    for &id in only.iter().filter(|id| !refused.contains(id)) {
        let Some(calls) = sites.get(&id) else { continue };
        let gone = dead(module, id);
        if gone.is_empty() {
            continue;
        }
        let own: Vec<InstId> = calls.iter().filter(|&&(caller, _)| caller == id).map(|&(_, inst)| inst).collect();
        let ty = {
            let Module { context, globals, .. } = &mut *module;
            let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
            // Every argument at a removed position goes first: each use of a
            // removed parameter is one of them.
            for &call in &own {
                for &parameter in gone.iter().rev() {
                    function.replace_argument(call, parameter, &[], function.ty);
                }
            }
            for &parameter in gone.iter().rev() {
                function.remove_parameter(context, parameter);
            }
            for &call in &own {
                function.set_call_type(call, function.ty);
            }
            function.ty
        };
        for &(caller, call) in calls.iter().filter(|&&(caller, _)| caller != id) {
            let GlobalKind::Function(function) = &mut module.globals[caller.0 as usize].kind else {
                unreachable!("a caller")
            };
            for &parameter in gone.iter().rev() {
                function.replace_argument(call, parameter, &[], ty);
            }
            changed.insert(caller);
        }
        changed.insert(id);
    }
    changed.into_iter().collect()
}

/// The parameters of `id` nothing reads but its own calls to itself, each as
/// the argument of a parameter that nothing reads either: LLVM's
/// DeadArgumentElimination (`MarkValue`/`SurveyUse`: a use as an argument of
/// the function's own call is live only if that parameter is), taken to its
/// fixed point. `hanoi(n - 1, a, c, b)` permutes three parameters among
/// themselves and reads none.
fn dead(
    module: &Module,
    id: GlobalId,
) -> BTreeSet<usize> {
    let Some(function) = module.global(id).function() else { return BTreeSet::new() };
    let count = function.parameters().len();
    // Where each parameter is passed; none when something else reads it.
    let passed: Vec<Option<Vec<usize>>> = function
        .parameters()
        .iter()
        .map(|&value| {
            function
                .users(value)
                .iter()
                .map(|one| {
                    let call = function.instruction(one.user);
                    let reads = matches!(call.opcode, Opcode::Call(_))
                        && memory::callee(&module.context, function, one.user) == Some(id)
                        && (one.index as usize) < count;
                    reads.then_some(one.index as usize)
                })
                .collect()
        })
        .collect();
    let mut live: BTreeSet<usize> = (0..count).filter(|&at| passed[at].is_none()).collect();
    while let Some(at) =
        (0..count).find(|at| !live.contains(at) && passed[*at].iter().flatten().any(|to| live.contains(to)))
    {
        live.insert(at);
    }
    (0..count).filter(|at| !live.contains(at)).collect()
}
