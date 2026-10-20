//! A far pointer parameter that every call fills from one near space becomes
//! a near pointer parameter: the caller passes the offset, and neither
//! segment is pushed or loaded.
//!
//! # How it works
//! A parameter is a candidate when its function is reached only by direct
//! calls (`callgraph::direct_only`, none of them refused) and the parameter
//! is a far pointer that does not carry (a pair, not a huge one). For each
//! call the actual's root (`valuetracking::underlying`) says which near space
//! the object is in:
//! - a module global variable in the data space (DGROUP's, `Spaces::data`);
//! - a stack object of the caller (SS, which DS need not be: `Spaces::stack`);
//! - a near parameter an earlier round narrowed, of that parameter's space;
//! - the function's own parameter passed on in a recursive call: no evidence
//!   either way.
//!
//! The parameter is narrowed when at least one call names a space, every one
//! that does names the same, and every other call is a pass-on. An actual that
//! may be elsewhere, or in the other space, keeps the parameter far. Rounds
//! repeat until nothing narrows, so a callee's narrowing feeds the callers that
//! pass its parameter on.
//!
//! ```text
//!  callee(ptr addrspace(1) %a)          callee(ptr %a)
//!    ... load %a ...                      %w = addrspacecast %a to as(1)
//!                                         ... load %w ...
//!  caller: call callee(%far)    =>      caller:
//!    %far = addrspacecast @g to as(1)     %n = addrspacecast %far to ptr
//!                                         call callee(%n)
//! ```
//!
//! The callee's new near parameter replaces the old one; what read the old one
//! reads a cast of it to the far pointer, and the recursive calls that passed
//! it pass the near one. Each other call passes a cast of its actual to the
//! near pointer. [`crate::inferspace`] then folds the accesses in the callee
//! and the casts at each call, so the segment is neither pushed nor loaded.
//!
//! # Example
//! Before:
//! ```mir
//! define ... @"_next@3"(ptr addrspace(1) nocapture readonly %0, i16 %1) ... {
//! b1:
//!   %2 = mul i16 %1, 2
//!   %3 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 %2
//!   %4 = load i16, ptr addrspace(1) %3, !tbaa !9
//!   ret i16 %4
//! }
//!
//! define ... @"_walk@3"(i16 %0) ... {
//! b1:
//!   %1 = addrspacecast ptr @_table to ptr addrspace(1)
//!   %2 = call regparm3cc i16 @"_next@3"(ptr addrspace(1) %1, i16 %0)
//!   %3 = call regparm3cc i16 @"_next@3"(ptr addrspace(1) %1, i16 1)
//!   %4 = add nsw i16 %2, %3
//!   ret i16 %4
//! }
//! ```
//! After:
//! ```mir
//! define ... @"_next@3"(ptr nocapture readonly %0, i16 %1) ... {
//! b1:
//!   %2 = mul i16 %1, 2
//!   %3 = getelementptr inbounds i8, ptr %0, i16 %2
//!   %4 = load i16, ptr %3, !tbaa !9
//!   ret i16 %4
//! }
//!
//! define ... @"_walk@3"(i16 %0) ... {
//! b1:
//!   %1 = call regparm3cc i16 @"_next@3"(ptr @_table, i16 %0)
//!   %2 = call regparm3cc i16 @"_next@3"(ptr @_table, i16 1)
//!   %3 = add nsw i16 %1, %2
//!   ret i16 %3
//! }
//! ```
//! Both calls pass the near `@_table`, so `_next@3` takes a near pointer: the
//! casts and the segment the far parameter carried are gone from the callee and
//! from each call.
//!
//! From `--dump` of a program whose static `next(int far *p, int n)` is called
//! twice with the near global `table`, at -O2 -m16 with inlining off, stages
//! `pipeline` and `interprocedural`.
//!
//! # Runs
//! In [`crate::interprocedural`] as stage `narrow.`, after argument promotion
//! and before the constant-return rounds; the functions it changes go back
//! through the per-body pipeline. On wherever the interprocedural step is
//! (every level above -O0).
//!
//! # Needs
//! The target's `Spaces` (data and stack roles) and its address-space layout.
//!
//! # See also
//! [`crate::argpromotion`], [`crate::deadargs`].

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::callgraph::{direct_calls, direct_only};
use llrm_mir::context::{ConstantKind, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{GlobalKind, InstId, Module, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::spaces::Spaces;
use llrm_mir::types::Type;
use llrm_mir::valuetracking::underlying;

use crate::inferspace::on_stack;

/// The functions of `module` that narrowed a parameter, and the functions that
/// call them.
pub fn narrowed(
    module: &mut Module,
    layout: &DataLayout,
    roles: Spaces,
) -> Vec<GlobalId> {
    let only = direct_only(module);
    let llrm_mir::callgraph::DirectCalls { sites, refused } = direct_calls(module);
    // The parameters narrowed so far: a near pointer of one is DGROUP's, for
    // the callers it feeds.
    let mut proven: BTreeMap<(GlobalId, usize), u32> = BTreeMap::new();
    let mut changed = BTreeSet::new();
    let mut again = true;
    while std::mem::take(&mut again) {
        for &id in only.iter().filter(|id| !refused.contains(id)) {
            let Some(calls) = sites.get(&id) else { continue };
            let count = module.global(id).function().map_or(0, |one| one.parameters().len());
            for parameter in (0..count).rev() {
                let Some(far) = far_space(module, layout, id, parameter) else { continue };
                let spaces: Vec<Option<u32>> = calls
                    .iter()
                    .map(|&(caller, call)| space_of(module, layout, roles, &proven, id, parameter, caller, call))
                    .collect();
                let Some(space) = spaces.iter().flatten().copied().next() else { continue };
                if spaces.iter().flatten().any(|&one| one != space)
                    || spaces.iter().any(Option::is_none)
                        && !spaces.iter().zip(calls).all(|(one, &(caller, call))| {
                            one.is_some() || passes_on(module, id, parameter, caller, call)
                        })
                {
                    continue;
                }
                changed.extend(narrow(module, id, parameter, far, space, calls));
                changed.insert(id);
                llrm_mir::memory::unsplit(
                    &mut crate::interprocedural::function_mut(module, id).1.attrs,
                    llrm_mir::memory::Moved::OutOfArguments,
                );
                // The callers of this function that pass what they received,
                // now a near pointer of it.
                proven.retain(|&(function, at), _| function != id || at < parameter);
                proven.insert((id, parameter), space);
                again = true;
            }
        }
    }
    changed.into_iter().collect()
}

/// The far space parameter `at` of `id` points into, where its selector may be
/// dropped.
fn far_space(
    module: &Module,
    layout: &DataLayout,
    id: GlobalId,
    at: usize,
) -> Option<u32> {
    let function = module.global(id).function()?;
    let &parameter = function.parameters().get(at)?;
    match module.context.types.get(function.value(parameter).ty) {
        Type::Pointer(space) if layout.is_pair(*space) && !layout.carries(*space) => Some(*space),
        _ => None,
    }
}

/// Whether `call` of `caller` passes its own parameter `parameter` on to `id`,
/// itself.
fn passes_on(
    module: &Module,
    id: GlobalId,
    parameter: usize,
    caller: GlobalId,
    call: InstId,
) -> bool {
    let Some(function) = module.global(caller).function() else { return false };
    caller == id && function.instruction(call).operands[parameter] == Operand::Value(function.parameters()[parameter])
}

/// The near space call `call` of `caller` fills parameter `parameter` of `id`
/// from: where the object it addresses is. None for a pass-on and for what may
/// be anywhere.
fn space_of(
    module: &Module,
    layout: &DataLayout,
    roles: Spaces,
    proven: &BTreeMap<(GlobalId, usize), u32>,
    id: GlobalId,
    parameter: usize,
    caller: GlobalId,
    call: InstId,
) -> Option<u32> {
    let function = module.global(caller).function()?;
    let actual = function.instruction(call).operands[parameter];
    if passes_on(module, id, parameter, caller, call) {
        return None;
    }
    let (root, _) = underlying(&module.context, layout, function, actual);
    match root {
        Operand::Constant(constant) => match module.context.get(constant).kind {
            ConstantKind::Global(global) => (matches!(module.global(global).kind, GlobalKind::Variable(_))
                && module.global(global).address_space == roles.data)
                .then_some(roles.data),
            _ => None,
        },
        Operand::Value(value) => match function.value(value).def {
            // A near parameter an earlier narrowing made.
            ValueDef::Argument(at) => proven.get(&(caller, at as usize)).copied(),
            ValueDef::Instruction(_) if on_stack(&module.context, layout, function, actual) => Some(roles.stack),
            ValueDef::Instruction(_) => None,
        },
        _ => None,
    }
}

/// Parameter `parameter` of `id` becomes a near pointer, and each call's
/// argument its offset.
fn narrow(
    module: &mut Module,
    id: GlobalId,
    parameter: usize,
    far: u32,
    space: u32,
    calls: &[(GlobalId, InstId)],
) -> BTreeSet<GlobalId> {
    let mut changed = BTreeSet::new();
    let near = module.context.types.ptr(space);
    let wide = module.context.types.ptr(far);
    let ty = {
        let Module { context, globals, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
        let old = function.parameters()[parameter];
        let made = function.insert_parameters(context, parameter, &[near])[0];
        let own: Vec<InstId> = calls
            .iter()
            .filter(|&&(caller, call)| {
                caller == id && function.instruction(call).operands[parameter] == Operand::Value(old)
            })
            .map(|&(_, call)| call)
            .collect();
        for &call in &own {
            function.replace_argument(call, parameter, &[Operand::Value(made)], function.ty);
        }
        // What else reads it reads the far pointer it was.
        let entry = function.entry().expect("a body");
        let first = function.block(entry).first().expect("a block with an instruction");
        let cast = function.create_instruction(
            Opcode::Cast(CastOp::AddrSpaceCast),
            wide,
            vec![Operand::Value(made)],
            Default::default(),
            None,
        );
        function.insert(cast, Position::Before(first)).expect("placed");
        let widened = function.instruction(cast).result.expect("a cast's value");
        function.replace_all_uses_with(old, Operand::Value(widened));
        function.remove_parameter(context, parameter + 1);
        for &call in &own {
            function.set_call_type(call, function.ty);
        }
        function.ty
    };
    for &(caller, call) in calls {
        let Module { globals, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[caller.0 as usize].kind else { unreachable!("a caller") };
        // A recursive call that passed the parameter on already passes the near
        // one.
        if caller == id
            && function.instruction(call).operands[parameter] == Operand::Value(function.parameters()[parameter])
        {
            continue;
        }
        let actual = function.instruction(call).operands[parameter];
        let cast = function.create_instruction(
            Opcode::Cast(CastOp::AddrSpaceCast),
            near,
            vec![actual],
            Default::default(),
            None,
        );
        function.insert(cast, Position::Before(call)).expect("placed");
        let narrowed = function.instruction(cast).result.expect("a cast's value");
        function.replace_argument(call, parameter, &[Operand::Value(narrowed)], ty);
        changed.insert(caller);
    }
    changed
}
