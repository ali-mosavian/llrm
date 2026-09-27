//! Facts that cross direct procedure boundaries, adapted from llrm-core's
//! `analysis/interprocedural.rs`.
//!
//! A direct call is opaque to ordinary SCCP even when every return in the
//! named body has the same value. This module computes that fact over the
//! whole module and materialises it at the call. The call itself stays
//! unless a separate purity proof says its effects are unobservable.
//!
//! What changed with the representation:
//! - A call names its callee and carries its arguments, so the old
//!   call-site and ARG tables are gone, with `argument_sites` and the C call
//!   contract. Procedures are keyed by `GlobalId`.
//! - A constant is an operand. SCCP states what it proved by rewriting the
//!   value to its constant, so the old lookup in `consts::known` is reading
//!   the operand, and `constant_parameters` (frontend facts) and
//!   `current_parameter_constants` (SCCP facts) are one function.
//! - A body returns one value, so `Returns` holds one constant, not a tuple.
//! - `specialize_parameters` and `propagate_returns` replace uses with the
//!   constant rather than seeding `initial` or defining fresh copies, which
//!   leaves nothing to redo and no `done` set to keep.
//! - A callee's own attributes state what the fixed points would prove of a
//!   body this module lacks: `memory(none)`, `willreturn` and `nounwind`
//!   admit a call to the pure and readonly sets, and `noreturn` ends a path.
//! - Division is C's and floating exceptions the machine's, so the old
//!   trapping and floating kinds refuse nothing; the old `Escape`, `Opaque`
//!   and `Fill` are calls, judged as calls.
//! - A frame access is one whose pointer `frameescape::framed` places in an
//!   alloca; a static one is a constant offset (`pointerfacts`) from a near
//!   global variable this module defines.
//! - `noreturn_procedures`, `terminal_sites` and the `terminal_calls` cut
//!   are noreturn's facts and edit, asked for here.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, GlobalKind, InstId, Linkage, Module, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::types::Type;
use llrm_mir::{ConstantId, ConstantKind, Context, GlobalId};
use llrm_support::hash::IndexMap;

use crate::cfg;
use crate::effects::{self, Declarations};
use crate::frameescape;
use crate::noreturn;
pub use crate::noreturn::terminal_sites;
use crate::pointerfacts;

pub type Returns = IndexMap<GlobalId, ConstantId>;
pub type Parameters = IndexMap<GlobalId, Vec<Option<ConstantId>>>;

/// Parameter constants agreed by every surviving direct call to a private
/// body.
///
/// A call whose actual becomes constant only after another private return
/// is summarized counts once `propagate_returns` has rewritten it.
pub fn constant_parameters(module: &Module, eligible: &BTreeSet<GlobalId>) -> Parameters {
    let mut actuals: IndexMap<GlobalId, Vec<Vec<Option<ConstantId>>>> = eligible.iter().map(|&id| (id, Vec::new())).collect();
    for (_, _, function) in module.functions() {
        for (at, values) in current_call_constants(&module.context, function) {
            if let Some(sites) = effects::callee(&module.context, function, at).and_then(|target| actuals.get_mut(&target)) {
                sites.push(values);
            }
        }
    }

    _agreed_parameters(&actuals)
}

/// The constant actuals of every direct call.
///
/// The result is deliberately per-call instead of per-callee: a costed
/// inlining decision may use one constant call even when a second dynamic
/// call prevents whole-body parameter specialization.
pub fn current_call_constants(context: &Context, function: &Function) -> IndexMap<InstId, Vec<Option<ConstantId>>> {
    let mut out = IndexMap::default();
    for (_, call) in function.walk() {
        let instruction = function.instruction(call);
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &instruction.opcode else { continue };
        if effects::callee(context, function, call).is_none() {
            continue;
        }
        let Type::Function { parameters, .. } = context.types.get(info.function_type) else { continue };
        let values = instruction.operands[..parameters.len()].iter().map(|&operand| _constant_argument(context, operand)).collect();
        out.insert(call, values);
    }
    out
}

fn _constant_argument(context: &Context, argument: Operand) -> Option<ConstantId> {
    match argument {
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_)) => Some(id),
        _ => None,
    }
}

/// Facts shared by every call in an already-normalized actual map.
fn _agreed_parameters(actuals: &IndexMap<GlobalId, Vec<Vec<Option<ConstantId>>>>) -> Parameters {
    let mut out = Parameters::default();
    for (&name, sites) in actuals {
        if sites.is_empty() || sites.iter().map(Vec::len).collect::<BTreeSet<_>>().len() != 1 {
            continue;
        }
        let mut agreed = Vec::new();
        for index in 0..sites[0].len() {
            let values = sites.iter().map(|site| site[index]).collect::<BTreeSet<_>>();
            agreed.push(if values.len() == 1 && !values.contains(&None) { values.into_iter().next().flatten() } else { None });
        }
        if agreed.iter().any(Option::is_some) {
            out.insert(name, agreed);
        }
    }
    out
}

/// Replace each agreed parameter by its constant, for ordinary SCCP.
pub fn specialize_parameters(context: &Context, function: &mut Function, constants: &[Option<ConstantId>]) -> bool {
    let mut changed = false;
    for (parameter, constant) in function.parameters().to_vec().into_iter().zip(constants) {
        if let Some(constant) = *constant
            && context.get(constant).ty == function.value(parameter).ty
            && !function.users(parameter).is_empty()
        {
            function.replace_all_uses_with(parameter, Operand::Constant(constant));
            changed = true;
        }
    }
    changed
}

/// The common integer produced by every return of each body.
///
/// Absence is the conservative answer for void, floating or disagreeing
/// returns. SCCP's fixed point is in the operands, so promoted locals, phis
/// and folded expressions need no special cases here.
pub fn constant_returns(module: &Module) -> Returns {
    let mut out = Returns::default();
    for (name, _, function) in module.functions().filter(|(_, global, _)| _exact(global.linkage)) {
        let mut returned = Vec::new();
        let mut complete = true;
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            if instruction.opcode != Opcode::Ret {
                continue;
            }
            let Some(&operand) = instruction.operands.first() else { continue };
            match _constant_argument(&module.context, operand) {
                Some(constant) => returned.push(constant),
                None => {
                    complete = false;
                    break;
                }
            }
        }
        if complete && !returned.is_empty() && returned.iter().collect::<BTreeSet<_>>().len() == 1 {
            out.insert(name, returned[0]);
        }
    }
    out
}

/// Define a direct call's known result from its module-level summary.
///
/// Every use of the call's result reads the constant instead, allowing the
/// ordinary body pipeline to fold consumers; the call stays.
pub fn propagate_returns(context: &Context, function: &mut Function, returns: &Returns) -> bool {
    let mut known: Vec<(ValueId, ConstantId)> = Vec::new();
    for (_, inst) in function.walk() {
        let Some(result) = function.instruction(inst).result else { continue };
        if let Some(&constant) = effects::callee(context, function, inst).and_then(|target| returns.get(&target))
            && context.get(constant).ty == function.value(result).ty
            && !function.users(result).is_empty()
        {
            known.push((result, constant));
        }
    }
    for &(result, constant) in &known {
        function.replace_all_uses_with(result, Operand::Constant(constant));
    }
    !known.is_empty()
}

/// Whether every CFG path ends in RETURN without revisiting a block.
fn _acyclic_returning(function: &Function) -> bool {
    let graph = cfg::graph(function);
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();

    fn visit(
        at: i64,
        function: &Function,
        blocks: &BTreeMap<i64, &cfg::Block>,
        visiting: &mut BTreeSet<i64>,
        visited: &mut BTreeSet<i64>,
    ) -> bool {
        if visiting.contains(&at) || !blocks.contains_key(&at) {
            return false;
        }
        if visited.contains(&at) {
            return true;
        }
        visiting.insert(at);
        let block = blocks[&at];
        let okay = if !block.succ.is_empty() {
            block.succ.iter().all(|&one| visit(one, function, blocks, visiting, visited))
        } else {
            function.terminator(cfg::block(at)).is_some_and(|last| function.instruction(last).opcode == Opcode::Ret)
        };
        visiting.remove(&at);
        if okay {
            visited.insert(at);
        }
        okay
    }

    match graph.first() {
        Some(entry) => visit(entry.at, function, &blocks, &mut visiting, &mut visited),
        None => false,
    }
}

/// Whether the call `inst` states it touches no memory and always comes
/// back normally.
fn _stated_pure(context: &Context, declarations: &Declarations, function: &Function, inst: InstId) -> bool {
    !effects::touches_memory(context, declarations, function, inst)
        && effects::states(context, declarations, function, inst, "willreturn")
        && effects::states(context, declarations, function, inst, "nounwind")
}

/// Whether the call `inst` goes to a member of `admitted`, or says it could.
fn _admitted(context: &Context, declarations: &Declarations, function: &Function, inst: InstId, admitted: &BTreeSet<GlobalId>) -> bool {
    effects::callee(context, function, inst).is_some_and(|target| admitted.contains(&target)) || _stated_pure(context, declarations, function, inst)
}

/// Whether the body here is the one that runs: LLVM's `hasExactDefinition`.
/// The linker may swap any other for a different one.
pub fn _exact(linkage: Linkage) -> bool {
    matches!(linkage, Linkage::External | Linkage::Internal | Linkage::Private)
}

/// Each exactly defined function, for the fixed points.
fn _bodies(module: &Module) -> Vec<(GlobalId, &Function)> {
    module
        .functions()
        .filter(|(_, global, function)| _exact(global.linkage) && !function.is_declaration())
        .map(|(id, _, function)| (id, function))
        .collect()
}

/// The pointer operand of a load or store, and whether it is volatile.
fn _access(function: &Function, inst: InstId) -> Option<(Operand, bool, bool)> {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { volatile, .. } => Some((instruction.operands[0], volatile, false)),
        Opcode::Store { volatile, .. } => Some((instruction.operands[1], volatile, true)),
        _ => None,
    }
}

fn _local_effects(context: &Context, declarations: &Declarations, function: &Function, pure: &BTreeSet<GlobalId>) -> bool {
    if !_acyclic_returning(function) {
        return false;
    }
    let framed = frameescape::framed(function);
    let is_local = |pointer: Operand| matches!(pointer, Operand::Value(value) if framed.contains_key(&value));
    for (_, inst) in function.walk() {
        if matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
            if !_admitted(context, declarations, function, inst, pure) {
                return false;
            }
            continue;
        }
        if let Some((pointer, volatile, _)) = _access(function, inst)
            && (volatile || !is_local(pointer))
        {
            return false;
        }
    }
    true
}

/// Direct procedures with no observable effects and guaranteed return.
///
/// The least fixed point admits an acyclic call chain once all its callees
/// are admitted.  Recursive SCCs remain conservative because removing one
/// would otherwise remove possible nontermination.
pub fn pure_procedures(module: &Module) -> BTreeSet<GlobalId> {
    let declarations = effects::declarations(module);
    let bodies = _bodies(module);
    let mut pure = BTreeSet::new();
    loop {
        let mut made = pure.clone();
        made.extend(bodies.iter().filter(|(_, function)| _local_effects(&module.context, &declarations, function, &pure)).map(|(id, _)| *id));
        if made == pure {
            return pure;
        }
        pure = made;
    }
}

/// Acyclic user bodies whose unused calls have no observable effect.
///
/// This is intentionally broader than `pure_procedures`: an ordinary,
/// direct read of this module's static data is not observable in C when its
/// result is unused.  It remains narrower than a general no-fault proof:
/// pointer-based, far/externally selected, volatile and floating reads stay
/// out, as do all non-frame writes.  Callers may use this fact only to erase
/// a dead result; it is not an inlining or alias-preservation permission.
pub fn readonly_procedures(module: &Module, layout: &DataLayout) -> BTreeSet<GlobalId> {
    let declarations = effects::declarations(module);
    let bodies = _bodies(module);
    let mut readonly = BTreeSet::new();
    loop {
        let mut made = readonly.clone();
        made.extend(
            bodies.iter().filter(|(_, function)| _readonly_effects(module, layout, &declarations, function, &readonly)).map(|(id, _)| *id),
        );
        if made == readonly {
            return readonly;
        }
        readonly = made;
    }
}

fn _readonly_effects(module: &Module, layout: &DataLayout, declarations: &Declarations, function: &Function, readonly: &BTreeSet<GlobalId>) -> bool {
    if !_acyclic_returning(function) {
        return false;
    }
    let context = &module.context;
    let framed = frameescape::framed(function);
    let offsets = pointerfacts::offsets(context, layout, function);
    let is_local = |pointer: Operand| matches!(pointer, Operand::Value(value) if framed.contains_key(&value));
    // A direct near static data reference is guaranteed to name this
    // module's mapped data.  Do not infer the same from an arbitrary
    // pointer, external selector or far access.
    let is_static = |pointer: Operand| {
        let Some((Operand::Constant(base), _)) = offsets.relative(pointer) else { return false };
        let ConstantKind::Global(global) = context.get(base).kind else { return false };
        let global = module.global(global);
        global.address_space == 0 && matches!(&global.kind, GlobalKind::Variable(variable) if variable.initializer.is_some())
    };
    for (_, inst) in function.walk() {
        if matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
            if !_admitted(context, declarations, function, inst, readonly) {
                return false;
            }
            continue;
        }
        let Some((pointer, volatile, store)) = _access(function, inst) else { continue };
        if volatile {
            return false;
        }
        // Internal frame writes disappear with the call.  Any write to a
        // nonlocal object remains observable, even if it is otherwise an
        // exact direct reference.
        if !is_local(pointer) && (store || !is_static(pointer)) {
            return false;
        }
    }
    true
}

/// Direct private procedures that cannot reach a normal return: noreturn's
/// fixed point over the `eligible` bodies.  An unknown, external or public
/// callee stays a returning edge.
pub fn noreturn_procedures(module: &Module, eligible: &BTreeSet<GlobalId>) -> BTreeSet<GlobalId> {
    noreturn::inferred(module, eligible)
}

/// Noreturn's terminal-call cut, at the direct calls to `noreturn` bodies.
pub fn terminal_calls(context: &mut Context, declarations: &Declarations, function: &mut Function, noreturn: &BTreeSet<GlobalId>) -> bool {
    let sites = terminal_sites(context, declarations, function, noreturn);
    noreturn::after_terminal_calls(context, function, &sites)
}

/// Remove effect-free calls whose result nothing still reads.
pub fn remove_dead_pure_calls(context: &Context, declarations: &Declarations, function: &mut Function, pure: &BTreeSet<GlobalId>) -> bool {
    let removed: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            let instruction = function.instruction(inst);
            matches!(instruction.opcode, Opcode::Call(_))
                && _admitted(context, declarations, function, inst, pure)
                && instruction.result.is_none_or(|result| function.users(result).is_empty())
        })
        .collect();
    for &inst in &removed {
        function.erase(inst).expect("an unused call");
    }
    !removed.is_empty()
}

#[cfg(test)]
#[path = "interprocedural_tests.rs"]
mod tests;
