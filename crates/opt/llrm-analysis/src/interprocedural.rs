//! Facts that cross direct procedure boundaries, adapted from llrm-core's
//! `analysis/interprocedural.rs`.
//!
//! A direct call is opaque to ordinary SCCP even when every return in the
//! named body has the same value. This module computes that fact over the
//! whole module and materialises it at the call. The call itself stays
//! unless a separate purity proof says its effects are unobservable.
//!
//! What changed with the representation:
//! - A call names its callee and carries its arguments, so the old call-site
//!   and ARG tables are gone, with `argument_sites` and the C call contract.
//!   Procedures are keyed by `GlobalId`.
//! - A constant is an operand. SCCP states what it proved by rewriting the
//!   value to its constant, so the old lookup in `consts::known` is reading the
//!   operand, and `constant_parameters` (frontend facts) and
//!   `current_parameter_constants` (SCCP facts) are one function.
//! - A body returns one value, so `Returns` holds one constant, not a tuple.
//! - `specialize_parameters` and `propagate_returns` replace uses with the
//!   constant rather than seeding `initial` or defining fresh copies, which
//!   leaves nothing to redo and no `done` set to keep.
//! - Purity is stated, not a set: the whole-module step stamps each body's
//!   attributes, and a call is pure or erasable as it and its callee state
//!   (`stated_pure`, `erasable`). What the old fixed points proved,
//!   `returns_without_looping` and `cannot_fault` answer for the stamp.
//!   `noreturn` ends a path.
//! - Division is C's and floating exceptions the machine's, so the old trapping
//!   and floating kinds refuse nothing; the old `Escape`, `Opaque` and `Fill`
//!   are calls, judged as calls.
//! - A frame access is one whose pointer `frameescape::framed` places in an
//!   alloca; a static one is a constant offset (`pointerfacts`) from a near
//!   global variable this module defines.
//! - `noreturn_procedures`, `terminal_sites` and the `terminal_calls` cut are
//!   noreturn's facts and edit, asked for here.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::callgraph::Defined;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::facts::{Fact, Facts};
use llrm_mir::memory;
use llrm_mir::module::{Function, GlobalKind, InstId, Linkage, Module, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::program::Program;
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
pub fn constant_parameters(
    module: &Module,
    eligible: &BTreeSet<GlobalId>,
) -> Parameters {
    let mut actuals: Actuals = eligible.iter().map(|&id| (id, Vec::new())).collect();
    for (own, _, function) in module.functions() {
        for (at, values) in current_call_constants(&module.context, function) {
            let target = effects::callee(&module.context, function, at);
            let same = _passed_on(function, at, target == Some(own));
            if let Some(sites) = target.and_then(|target| actuals.get_mut(&target)) {
                sites.push((values, same));
            }
        }
    }

    _agreed_parameters(&actuals)
}

/// `constant_parameters` over a program: the actuals of every call in any
/// module to an `eligible` body, imported into its module, each module's
/// agreed parameters.
pub fn program_parameters(
    program: &mut Program,
    eligible: &BTreeSet<Defined>,
) -> Vec<Parameters> {
    let mut actuals: BTreeMap<Defined, Vec<(Vec<Option<(usize, ConstantId)>>, Vec<bool>)>> =
        eligible.iter().map(|&one| (one, Vec::new())).collect();
    for (at, module) in program.modules.iter().enumerate() {
        for (own, _, function) in module.functions() {
            for (call, values) in current_call_constants(&module.context, function) {
                let target =
                    effects::callee(&module.context, function, call).and_then(|target| program.definition(at, target));
                let same = _passed_on(function, call, target == Some((at, own)));
                if let Some(sites) = target.and_then(|target| actuals.get_mut(&target)) {
                    sites.push((values.into_iter().map(|one| one.map(|constant| (at, constant))).collect(), same));
                }
            }
        }
    }
    let mut out = vec![IndexMap::default(); program.modules.len()];
    for (at, local) in out.iter_mut().enumerate() {
        let mut mine = IndexMap::default();
        for (&(defined, id), sites) in &actuals {
            if defined != at {
                continue;
            }
            let sites = sites
                .iter()
                .map(|(site, same)| {
                    (
                        site.iter()
                            .map(|one| one.and_then(|(from, constant)| program.imported(from, constant, at)))
                            .collect(),
                        same.clone(),
                    )
                })
                .collect();
            mine.insert(id, sites);
        }
        *local = _agreed_parameters(&mine);
    }
    out
}

/// `constant_returns` of each module, as it names the bodies: its own, and
/// those another module defines, their constants imported.
pub fn program_returns(program: &mut Program) -> Vec<Returns> {
    let own: Vec<Returns> = program.modules.iter().map(constant_returns).collect();
    let mut out = own.clone();
    for at in 0..program.modules.len() {
        let declared: Vec<(GlobalId, Defined)> = (0..program.modules[at].globals.len() as u32)
            .map(GlobalId)
            .filter_map(|id| Some((id, program.definition(at, id).filter(|&(there, _)| there != at)?)))
            .collect();
        for (id, (there, defined)) in declared {
            if let Some(constant) = own[there].get(&defined).and_then(|&constant| program.imported(there, constant, at))
            {
                out[at].insert(id, constant);
            }
        }
    }
    out
}

/// The constant actuals of every direct call.
///
/// The result is deliberately per-call instead of per-callee: a costed
/// inlining decision may use one constant call even when a second dynamic
/// call prevents whole-body parameter specialization.
pub fn current_call_constants(
    context: &Context,
    function: &Function,
) -> IndexMap<InstId, Vec<Option<ConstantId>>> {
    let mut out = IndexMap::default();
    for (_, call) in function.walk() {
        let instruction = function.instruction(call);
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &instruction.opcode else { continue };
        if effects::callee(context, function, call).is_none() {
            continue;
        }
        let Type::Function { parameters, .. } = context.types.get(info.function_type) else { continue };
        let values = instruction.operands[..parameters.len()]
            .iter()
            .map(|&operand| _constant_argument(context, operand))
            .collect();
        out.insert(call, values);
    }
    out
}

fn _constant_argument(
    context: &Context,
    argument: Operand,
) -> Option<ConstantId> {
    match argument {
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_)) => Some(id),
        _ => None,
    }
}

/// Each body's calls: the constant actuals of each, and which actuals are the
/// body's own parameter, at the same position, passed on by a call to itself.
type Actuals = IndexMap<GlobalId, Vec<(Vec<Option<ConstantId>>, Vec<bool>)>>;

/// Which actuals of `call`, a call `recursive`ly of the function it is in,
/// are that function's own parameter at the same position: unchanged by the
/// call, so no new value for it.
fn _passed_on(
    function: &Function,
    call: InstId,
    recursive: bool,
) -> Vec<bool> {
    let operands = &function.instruction(call).operands;
    function
        .parameters()
        .iter()
        .enumerate()
        .map(|(at, &parameter)| recursive && operands.get(at) == Some(&Operand::Value(parameter)))
        .collect()
}

/// Facts shared by every call in an already-normalized actual map: those a
/// call that passes the parameter on does not change are not counted.
fn _agreed_parameters(actuals: &Actuals) -> Parameters {
    let mut out = Parameters::default();
    for (&name, sites) in actuals {
        if sites.is_empty() || sites.iter().map(|(site, _)| site.len()).collect::<BTreeSet<_>>().len() != 1 {
            continue;
        }
        let mut agreed = Vec::new();
        for index in 0..sites[0].0.len() {
            let values = sites
                .iter()
                .filter(|(_, same)| !same.get(index).copied().unwrap_or(false))
                .map(|(site, _)| site[index])
                .collect::<BTreeSet<_>>();
            agreed.push(if values.len() == 1 && !values.contains(&None) {
                values.into_iter().next().flatten()
            } else {
                None
            });
        }
        if agreed.iter().any(Option::is_some) {
            out.insert(name, agreed);
        }
    }
    out
}

/// Replace each agreed parameter by its constant, for ordinary SCCP.
pub fn specialize_parameters(
    context: &Context,
    function: &mut Function,
    constants: &[Option<ConstantId>],
) -> bool {
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
pub fn propagate_returns(
    context: &Context,
    function: &mut Function,
    returns: &Returns,
) -> bool {
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
pub fn returns_without_looping(function: &Function) -> bool {
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

/// Whether the body here is the one that runs: LLVM's `hasExactDefinition`.
/// The linker may swap any other for a different one.
fn _exact(linkage: Linkage) -> bool {
    matches!(
        linkage,
        Linkage::External | Linkage::Internal | Linkage::Private
    )
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
fn _access(
    function: &Function,
    inst: InstId,
) -> Option<(Operand, bool, bool)> {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { volatile, .. } => Some((instruction.operands[0], volatile, false)),
        Opcode::Store { volatile, .. } => Some((instruction.operands[1], volatile, true)),
        _ => None,
    }
}

/// Whether no access of `function` can fault: each is non-volatile and in
/// its frame (`frameescape::framed`) or a constant offset
/// (`pointerfacts`) from a near global variable this module defines. A
/// pointer, an external or far selector may name memory that is not there.
pub fn cannot_fault(
    module: &Module,
    layout: &DataLayout,
    spaces: llrm_mir::spaces::Spaces,
    function: &Function,
) -> bool {
    let context = &module.context;
    let framed = frameescape::framed(function);
    let offsets = pointerfacts::offsets(context, layout, function);
    let is_local = |pointer: Operand| matches!(pointer, Operand::Value(value) if framed.contains_key(&value));
    let is_static = |pointer: Operand| {
        let Some((Operand::Constant(base), _)) = offsets.relative(pointer) else { return false };
        let ConstantKind::Global(global) = context.get(base).kind else { return false };
        let global = module.global(global);
        global.address_space == spaces.near
            && matches!(
                &global.kind,
                GlobalKind::Variable(variable) if variable.initializer.is_some()
            )
    };
    function
        .walk()
        .all(
            |(_, inst)| _access(function, inst)
                .is_none_or(|(pointer, volatile, _)| !volatile && (is_local(pointer) || is_static(pointer))),
        )
}

/// Exactly defined bodies whose attributes state they touch no memory and
/// always come back normally.
pub fn stated_pure(module: &Module) -> BTreeSet<GlobalId> {
    _bodies(module)
        .into_iter()
        .filter(|(_, function)| {
            let attrs = &function.attrs;
            memory::stated(attrs) == memory::Effects::NONE
                && Facts::of(attrs).will_return()
                && Facts::of(attrs).no_unwind()
        })
        .map(|(id, _)| id)
        .collect()
}

/// Whether the call `inst` could go unnoticed: it states it writes no
/// memory and always comes back normally, as LLVM's
/// `wouldInstructionBeTriviallyDead` asks.
pub fn erasable(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    !effects::writes_memory(context, declarations, function, inst)
        && effects::states(context, declarations, function, inst, Fact::WillReturn)
        && effects::states(context, declarations, function, inst, Fact::NoUnwind)
}

/// Direct private procedures that cannot reach a normal return: noreturn's
/// fixed point over the `eligible` bodies.  An unknown, external or public
/// callee stays a returning edge.
/// `noreturn_procedures` over a program.
pub fn program_noreturn(
    program: &Program,
    declarations: &[&Declarations],
    eligible: &BTreeSet<Defined>,
) -> BTreeSet<Defined> {
    noreturn::inferred_in(program, declarations, eligible)
}

pub fn noreturn_procedures(
    module: &Module,
    declarations: &Declarations,
    eligible: &BTreeSet<GlobalId>,
) -> BTreeSet<GlobalId> {
    noreturn::inferred(module, declarations, eligible)
}

/// Noreturn's terminal-call cut, at the direct calls to `noreturn` bodies.
pub fn terminal_calls(
    context: &mut Context,
    declarations: &Declarations,
    function: &mut Function,
    noreturn: &BTreeSet<GlobalId>,
) -> bool {
    let sites = terminal_sites(context, declarations, function, noreturn);
    noreturn::after_terminal_calls(context, function, &sites)
}

/// Remove the calls whose result nothing still reads and that could go
/// unnoticed (`erasable`).
pub fn remove_dead_pure_calls(
    context: &Context,
    declarations: &Declarations,
    function: &mut Function,
) -> bool {
    let removed: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            let instruction = function.instruction(inst);
            matches!(instruction.opcode, Opcode::Call(_))
                && erasable(context, declarations, function, inst)
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
