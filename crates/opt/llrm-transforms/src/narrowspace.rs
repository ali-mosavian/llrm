//! A far pointer parameter of a function only direct calls reach, which every call fills with an address
//! in one near space, is a near pointer of it: the caller passes the offset, the callee reads through that
//! space's segment, and the segment is neither pushed nor loaded. The space is where the object is:
//! DGROUP's for a module global, the stack's for a stack object of the caller (SS, which DS need not be).
//! A recursive call passes the parameter on, a parameter an earlier round narrowed is its own space's.
//! One actual that may be elsewhere, or in the other space, keeps it far. `inferspace` then folds the
//! accesses in the callee and the casts at each call.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::callgraph::{direct_calls, direct_only};
use llrm_mir::context::{ConstantKind, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{GlobalKind, InstId, Module, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::underlying;

use crate::inferspace::on_stack;
use llrm_mir::types::{NEAR_DATA, NEAR_STACK};

/// The functions of `module` that narrowed a parameter, and the functions that call them.
pub fn narrowed(module: &mut Module, layout: &DataLayout) -> Vec<GlobalId> {
    let only = direct_only(module);
    let llrm_mir::callgraph::DirectCalls { sites, refused } = direct_calls(module);
    // The parameters narrowed so far: a near pointer of one is DGROUP's, for the callers it feeds.
    let mut proven: BTreeMap<(GlobalId, usize), u32> = BTreeMap::new();
    let mut changed = BTreeSet::new();
    let mut again = true;
    while std::mem::take(&mut again) {
        for &id in only.iter().filter(|id| !refused.contains(id)) {
            let Some(calls) = sites.get(&id) else { continue };
            let count = module.global(id).function().map_or(0, |one| one.parameters().len());
            for parameter in (0..count).rev() {
                let Some(far) = far_space(module, layout, id, parameter) else { continue };
                let spaces: Vec<Option<u32>> = calls.iter().map(|&(caller, call)| space_of(module, layout, &proven, id, parameter, caller, call)).collect();
                let Some(space) = spaces.iter().flatten().copied().next() else { continue };
                if spaces.iter().flatten().any(|&one| one != space) || spaces.iter().any(Option::is_none) && !spaces.iter().zip(calls).all(|(one, &(caller, call))| one.is_some() || passes_on(module, id, parameter, caller, call)) {
                    continue;
                }
                changed.extend(narrow(module, id, parameter, far, space, calls));
                changed.insert(id);
                // The callers of this function that pass what they received, now a near pointer of it.
                proven.retain(|&(function, at), _| function != id || at < parameter);
                proven.insert((id, parameter), space);
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

/// Whether `call` of `caller` passes its own parameter `parameter` on to `id`, itself.
fn passes_on(module: &Module, id: GlobalId, parameter: usize, caller: GlobalId, call: InstId) -> bool {
    let Some(function) = module.global(caller).function() else { return false };
    caller == id && function.instruction(call).operands[parameter] == Operand::Value(function.parameters()[parameter])
}

/// The near space call `call` of `caller` fills parameter `parameter` of `id` from: where the object it
/// addresses is. None for a pass-on and for what may be anywhere.
fn space_of(module: &Module, layout: &DataLayout, proven: &BTreeMap<(GlobalId, usize), u32>, id: GlobalId, parameter: usize, caller: GlobalId, call: InstId) -> Option<u32> {
    let function = module.global(caller).function()?;
    let actual = function.instruction(call).operands[parameter];
    if passes_on(module, id, parameter, caller, call) {
        return None;
    }
    let (root, _) = underlying(&module.context, layout, function, actual);
    match root {
        Operand::Constant(constant) => match module.context.get(constant).kind {
            ConstantKind::Global(global) => (matches!(module.global(global).kind, GlobalKind::Variable(_)) && module.global(global).address_space == NEAR_DATA).then_some(NEAR_DATA),
            _ => None,
        },
        Operand::Value(value) => match function.value(value).def {
            // A near parameter an earlier narrowing made.
            ValueDef::Argument(at) => proven.get(&(caller, at as usize)).copied(),
            ValueDef::Instruction(_) if on_stack(&module.context, layout, function, actual) => Some(NEAR_STACK),
            ValueDef::Instruction(_) => None,
        },
        _ => None,
    }
}

/// Parameter `parameter` of `id` becomes a near pointer, and each call's argument its offset.
fn narrow(module: &mut Module, id: GlobalId, parameter: usize, far: u32, space: u32, calls: &[(GlobalId, InstId)]) -> BTreeSet<GlobalId> {
    let mut changed = BTreeSet::new();
    let near = module.context.types.ptr(space);
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
