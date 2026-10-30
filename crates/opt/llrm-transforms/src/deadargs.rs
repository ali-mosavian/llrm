//! Dead argument elimination (LLVM's DeadArgumentElimination): a parameter
//! nothing reads goes, and with it the argument at every call.
//!
//! # How it works
//! Only functions `direct_only` accepts are candidates (internal, never named
//! but as a callee, every call a plain call with matching arity).
//!
//! A parameter is live when anything but a recursive call reads it. A use as an
//! argument of the function's own call is live only if the parameter it lands
//! on is live, taken to a fixed point. So `hanoi(n - 1, a, c, b)`, which
//! permutes three parameters among themselves and reads none, loses them all.
//!
//! ```text
//!  f(i16 %len, i16 %p)            f(i16 %p)
//!    ... %len unread       ==>      ...
//!  call f(12, %q)                 call f(%q)
//! ```
//!
//! Each dead parameter is removed from the function and the argument from every
//! call, in the function itself and its callers. Constant propagation and
//! promotion are what leave these behind: a length every caller passes as 12,
//! folded into the body, is still pushed at every call.
//!
//! `removed_with` also names the functions whose edit is more than that: the
//! ones that lost a parameter, and a caller whose dropped argument was read by
//! nothing else. A caller not named is vouched for ([`crate::settled`]): its
//! passes need not look at the call again.
//!
//! # Example
//! Before:
//! ```mir
//! define internal i16 @f(i16 %a, i16 %unused) {
//! b0:
//!   %r = add i16 %a, 1
//!   ret i16 %r
//! }
//! define i16 @top(i16 %x) {
//! b0:
//!   %r = call i16 @f(i16 %x, i16 12)
//!   ret i16 %r
//! }
//! ```
//! After, as the test asserts: `@f` takes only `%a`, and `@top` holds
//! `call i16 @f(i16 %x)`; the `12` is no longer pushed.
//!
//! From unit test `a_parameter_nothing_reads_is_not_passed` in
//! `deadargs_tests.rs`.
//!
//! # Runs
//! In the interprocedural step, after the callers' arguments have been
//! propagated as constants and ranges, and before recursive inlining. Each
//! changed body goes through the function pipeline again. Not at -O0.
//!
//! # See also
//! [`crate::argpromotion`]; LLVM's DeadArgumentElimination (`MarkValue`,
//! `SurveyUse`).

use std::collections::BTreeSet;

use llrm_mir::callgraph::{direct_calls, direct_only};
use llrm_mir::context::GlobalId;
use llrm_mir::memory;
use llrm_mir::module::{GlobalKind, InstId, Module};
use llrm_mir::opcode::Opcode;

/// Whether `removed` would change `module`, without changing it.
pub fn any_dead(module: &Module) -> bool {
    let only = direct_only(module);
    let llrm_mir::callgraph::DirectCalls { sites, refused } = direct_calls(module);
    only.iter().filter(|id| !refused.contains(id)).any(|id| sites.contains_key(id) && !dead(module, *id).is_empty())
}

/// The functions of `module` that lost a parameter, and the functions that call
/// them.
pub fn removed(module: &mut Module) -> Vec<GlobalId> {
    removed_with(module).0
}

/// `removed`, and the functions whose edit is more than a call that takes
/// fewer arguments: the ones that lost a parameter, and the callers an
/// argument went from whose value then has no other reader (what computed it
/// is dead now).
pub fn removed_with(module: &mut Module) -> (Vec<GlobalId>, BTreeSet<GlobalId>) {
    let mut unsound = BTreeSet::new();
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
            for &parameter in &gone {
                if let Some(&llrm_mir::module::Operand::Value(value)) =
                    function.instruction(call).operands.get(parameter)
                    && matches!(
                        function.value(value).def,
                        llrm_mir::module::ValueDef::Instruction(_)
                    )
                    && function.users(value).has_one_use()
                {
                    unsound.insert(caller);
                }
            }
            for &parameter in gone.iter().rev() {
                function.replace_argument(call, parameter, &[], ty);
            }
            changed.insert(caller);
        }
        // The function itself has other parameters now.
        unsound.insert(id);
        changed.insert(id);
    }
    (changed.into_iter().collect(), unsound)
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
