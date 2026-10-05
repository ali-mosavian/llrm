//! A far pointer parameter of a function only direct calls reach, which every call fills with an address
//! in DGROUP (a module global, or the near pointer a narrowed parameter is), is a near pointer: the
//! caller passes the offset, the callee reads through DS, and the segment is neither pushed nor loaded.
//! A recursive call passes the parameter on. One actual that may be elsewhere, or on the stack, keeps it
//! far. `inferspace` then folds the accesses in the callee and the casts at each call.
//!
//! Which near space the parameter narrows to is `inferspace::NEAR_DATA`: where SS is DS, a stack
//! object's address would join it; no frontend states that.

use std::collections::BTreeSet;

use llrm_mir::callgraph::{direct_calls, direct_only};
use llrm_mir::context::{ConstantKind, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{GlobalKind, InstId, Module, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::underlying;

use crate::inferspace::NEAR_DATA;

/// The functions of `module` that narrowed a parameter, and the functions that call them.
pub fn narrowed(module: &mut Module, layout: &DataLayout) -> Vec<GlobalId> {
    let only = direct_only(module);
    let llrm_mir::callgraph::DirectCalls { sites, refused } = direct_calls(module);
    // The parameters narrowed so far: a near pointer of one is DGROUP's, for the callers it feeds.
    let mut proven: BTreeSet<(GlobalId, usize)> = BTreeSet::new();
    let mut changed = BTreeSet::new();
    let mut again = true;
    while std::mem::take(&mut again) {
        for &id in only.iter().filter(|id| !refused.contains(id)) {
            let Some(calls) = sites.get(&id) else { continue };
            let count = module.global(id).function().map_or(0, |one| one.parameters().len());
            for parameter in (0..count).rev() {
                let Some(far) = far_space(module, layout, id, parameter) else { continue };
                if !calls.iter().all(|&(caller, call)| in_dgroup(module, layout, &proven, id, parameter, caller, call)) {
                    continue;
                }
                changed.extend(narrow(module, id, parameter, far, calls));
                changed.insert(id);
                // Callers now pass the near parameter on as they received it.
                proven.retain(|&(function, at)| function != id || at < parameter);
                proven.insert((id, parameter));
                again = true;
            }
        }
    }
    changed.into_iter().collect()
}

/// The far space parameter `at` of `id` points into, where its selector may be dropped.
fn far_space(module: &Module, layout: &DataLayout, id: GlobalId, at: usize) -> Option<u32> {
    let function = module.global(id).function()?;
    let &parameter = function.parameters().get(at)?;
    match module.context.types.get(function.value(parameter).ty) {
        Type::Pointer(space) if layout.is_pair(*space) && !layout.carries(*space) => Some(*space),
        _ => None,
    }
}

/// Whether call `call` of `caller` fills parameter `parameter` of `id` with a DGROUP address.
fn in_dgroup(module: &Module, layout: &DataLayout, proven: &BTreeSet<(GlobalId, usize)>, id: GlobalId, parameter: usize, caller: GlobalId, call: InstId) -> bool {
    let Some(function) = module.global(caller).function() else { return false };
    let actual = function.instruction(call).operands[parameter];
    let (root, _) = underlying(&module.context, layout, function, actual);
    match root {
        Operand::Constant(constant) => match module.context.get(constant).kind {
            ConstantKind::Global(global) => matches!(module.global(global).kind, GlobalKind::Variable(_)) && module.global(global).address_space == NEAR_DATA,
            _ => false,
        },
        Operand::Value(value) => match function.value(value).def {
            // The function's own parameter passed on, or a near one an earlier narrowing made.
            ValueDef::Argument(at) => (caller == id && at as usize == parameter && actual == Operand::Value(value)) || proven.contains(&(caller, at as usize)),
            ValueDef::Instruction(_) => false,
        },
        _ => false,
    }
}

/// Parameter `parameter` of `id` becomes a near pointer, and each call's argument its offset.
fn narrow(module: &mut Module, id: GlobalId, parameter: usize, far: u32, calls: &[(GlobalId, InstId)]) -> BTreeSet<GlobalId> {
    let mut changed = BTreeSet::new();
    let near = module.context.types.ptr(NEAR_DATA);
    let wide = module.context.types.ptr(far);
    let ty = {
        let Module { context, globals, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
        let old = function.parameters()[parameter];
        let made = function.insert_parameters(context, parameter, &[near])[0];
        let own: Vec<InstId> = calls.iter().filter(|&&(caller, call)| caller == id && function.instruction(call).operands[parameter] == Operand::Value(old)).map(|&(_, call)| call).collect();
        for &call in &own {
            function.replace_argument(call, parameter, &[Operand::Value(made)], function.ty);
        }
        // What else reads it reads the far pointer it was.
        let entry = function.entry().expect("a body");
        let first = function.block(entry).instructions()[0];
        let cast = function.create_instruction(Opcode::Cast(CastOp::AddrSpaceCast), wide, vec![Operand::Value(made)], Default::default(), None);
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
        let Module { context, globals, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[caller.0 as usize].kind else { unreachable!("a caller") };
        // A recursive call that passed the parameter on already passes the near one.
        if caller == id && function.instruction(call).operands[parameter] == Operand::Value(function.parameters()[parameter]) {
            continue;
        }
        let actual = function.instruction(call).operands[parameter];
        let cast = function.create_instruction(Opcode::Cast(CastOp::AddrSpaceCast), near, vec![actual], Default::default(), None);
        function.insert(cast, Position::Before(call)).expect("placed");
        let narrowed = function.instruction(cast).result.expect("a cast's value");
        function.replace_argument(call, parameter, &[Operand::Value(narrowed)], ty);
        changed.insert(caller);
    }
    changed
}
