//! Adapted from llrm-core's `optimize/interprocedural.rs`: the whole-module
//! step a frontend runs once every body has reached its own fixed point:
//! inline, carry constants across direct calls, drop dead pure calls and
//! the tails of terminal ones, and send each changed body back through its
//! pipeline until no body changes.
//!
//! A frontend hands in its pipeline; nothing here names a language or a
//! machine.
//!
//! What changed with the IR: a call names its callee and carries its
//! actuals, so the old `Procedure`'s call, parameter, constant and argument
//! tables have no counterpart and a procedure is its `GlobalId`; a
//! procedure has every caller in the module when its linkage is `internal`
//! or `private`, not by a parameter. A body changes in place, so the
//! pipeline takes the module and the body's id. `propagated`, the calls
//! whose return was carried, has nothing to hold: a carried return leaves
//! the call's result unread. The old `Module` is `Proved`, the name being
//! llrm-mir's. `Interprocedural` is the step as a `ModulePass`.
//!
//! The old module had no tests of its own.

use std::cell::RefCell;
use std::collections::BTreeSet;

use llrm_analysis::alias::{self, Procedure, Summary};
use llrm_analysis::effects;
use llrm_analysis::globalsaa;
use llrm_analysis::interprocedural as facts;
use llrm_analysis::manager::Summaries;
use llrm_analysis::memory::{Identity, MemoryKind, Slice, Unit};
use llrm_mir::callgraph::CallGraph;
use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::Effects;
use llrm_mir::module::{GlobalKind, Linkage, Module};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::{ModuleAnalysis, ModulePass};
use llrm_mir::types::Type;

use crate::inline;
use crate::profit::OperationCosts;

/// What the step proved about the module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proved {
    /// Private procedures that cannot return.
    pub noreturn: BTreeSet<GlobalId>,
    /// Procedures a root still calls, roots included.
    pub reachable: BTreeSet<GlobalId>,
}

/// The step as a module pass: `pipeline` is each changed body's pipeline.
pub struct Interprocedural {
    pub costs: OperationCosts,
    pub roots: BTreeSet<GlobalId>,
    pub pipeline: Box<dyn FnMut(&mut Module, GlobalId, &str)>,
    /// What the last run proved.
    pub proved: Option<Proved>,
}

impl ModulePass for Interprocedural {
    fn name(&self) -> &'static str {
        "interprocedural"
    }

    fn run(&mut self, module: &mut Module) -> Vec<GlobalId> {
        let changed = RefCell::new(BTreeSet::new());
        let pipeline = &mut self.pipeline;
        let proved = optimized::<String>(
            module,
            &self.roots,
            &self.costs,
            &mut |module, id, stage| {
                changed.borrow_mut().insert(id);
                pipeline(module, id, stage);
                Ok(())
            },
            &mut |_, id, _| {
                changed.borrow_mut().insert(id);
                Ok(())
            },
        )
        .unwrap_or_else(|error| panic!("interprocedural: {error}"));
        self.proved = Some(proved);
        changed.into_inner().into_iter().collect()
    }
}

/// Every defined procedure, in module order.
fn procedures(module: &Module) -> Vec<GlobalId> {
    module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(id, _, _)| id).collect()
}

/// Defined procedures whose every caller is in the module.
fn private(module: &Module) -> BTreeSet<GlobalId> {
    module
        .functions()
        .filter(|(_, global, function)| !function.is_declaration() && matches!(global.linkage, Linkage::Internal | Linkage::Private))
        .map(|(id, _, _)| id)
        .collect()
}

/// Run the whole-module step over `module`.
///
/// `reoptimised(module, id, stage)` runs procedure `id`'s pipeline again
/// on a body `stage` changed; `spliced` sees a body straight after inlining,
/// before that.
pub fn optimized<E: From<String>>(
    module: &mut Module,
    roots: &BTreeSet<GlobalId>,
    costs: &OperationCosts,
    reoptimised: &mut dyn FnMut(&mut Module, GlobalId, &str) -> Result<(), E>,
    spliced: &mut dyn FnMut(&Module, GlobalId, &str) -> Result<(), E>,
) -> Result<Proved, E> {
    let procedures = procedures(module);
    let private = private(module);

    // Inline only after each independent body has reached its local fixed
    // point; the splice's result goes straight back through the pipeline.
    let pure = facts::pure_procedures(module);
    let mut inline_round = 0;
    loop {
        let counts = inline::call_counts(module);
        let available = inline::candidates(module, &counts, &private, &pure, costs);
        let mut changed = false;
        for &id in &procedures {
            let caller = module.global(id).function().expect("a procedure");
            let constants = facts::current_call_constants(&module.context, caller);
            let constant = inline::constant_sites(module, caller, &constants, &private, &pure, costs);
            let (context, function) = function_mut(module, id);
            if !inline::expanded(context, function, &available, Some(&constant))? {
                continue;
            }
            let stage = format!("inline{inline_round}");
            spliced(module, id, &stage)?;
            reoptimised(module, id, &format!("{stage}."))?;
            changed = true;
            inline_round += 1;
        }
        if !changed {
            break;
        }
    }

    let mut return_round = 0;

    // Materialize every newly constant result.
    let propagate_constant_returns =
        |module: &mut Module, return_round: &mut i64, reoptimised: &mut dyn FnMut(&mut Module, GlobalId, &str) -> Result<(), E>| -> Result<(), E> {
            loop {
                let returns = facts::constant_returns(module);
                let mut changed = false;
                for &id in &procedures {
                    let (context, function) = function_mut(module, id);
                    if !facts::propagate_returns(context, function, &returns) {
                        continue;
                    }
                    reoptimised(module, id, &format!("ipa{return_round}."))?;
                    changed = true;
                }
                if !changed {
                    return Ok(());
                }
                *return_round += 1;
            }
        };

    // A return fact may make the actual of a different direct call
    // constant.  Alternate that current-MIR proof with return propagation
    // until neither side discovers a new fact.
    propagate_constant_returns(module, &mut return_round, reoptimised)?;
    let mut argument_round = 0;
    loop {
        let constants = facts::constant_parameters(module, &private);
        let mut changed = false;
        for &id in &procedures {
            let Some(constants_for_body) = constants.get(&id) else {
                continue;
            };
            let (context, function) = function_mut(module, id);
            if !facts::specialize_parameters(context, function, constants_for_body) {
                continue;
            }
            reoptimised(module, id, &format!("ipa-args{argument_round}."))?;
            changed = true;
        }
        if changed {
            argument_round += 1;
            propagate_constant_returns(module, &mut return_round, reoptimised)?;
        }

        // A single current-MIR constant may be worth cloning even where
        // another caller keeps the private body dynamic.
        let counts = inline::call_counts(module);
        let available = inline::candidates(module, &counts, &private, &pure, costs);
        let mut inlined = false;
        for &id in &procedures {
            let caller = module.global(id).function().expect("a procedure");
            let current = facts::current_call_constants(&module.context, caller);
            let constant = inline::constant_sites(module, caller, &current, &private, &pure, costs);
            let (context, function) = function_mut(module, id);
            if !inline::expanded(context, function, &available, Some(&constant))? {
                continue;
            }
            reoptimised(module, id, &format!("ipa-inline{argument_round}."))?;
            inlined = true;
        }
        if inlined {
            propagate_constant_returns(module, &mut return_round, reoptimised)?;
        }
        if !changed && !inlined {
            break;
        }
    }
    let layout = match &module.datalayout {
        Some(text) => DataLayout::parse(text).map_err(E::from)?,
        None => DataLayout::default(),
    };
    stamped(module).map_err(E::from)?;
    let readonly = facts::readonly_procedures(module, &layout);
    let declarations = effects::declarations(module);
    for &id in &procedures {
        let (context, function) = function_mut(module, id);
        if facts::remove_dead_pure_calls(context, &declarations, function, &readonly) {
            reoptimised(module, id, "ipa-pure.")?;
        }
    }
    // A direct private body whose every path stops makes the tail of every
    // call site unreachable: keep the physical call, remove only the code
    // that would require it to return, and repeat.
    let noreturn = loop {
        let noreturn = facts::noreturn_procedures(module, &private);
        let declarations = effects::declarations(module);
        let mut changed = false;
        for &id in &procedures {
            let (context, function) = function_mut(module, id);
            if !facts::terminal_calls(context, &declarations, function, &noreturn) {
                continue;
            }
            reoptimised(module, id, "ipa-noreturn.")?;
            changed = true;
        }
        if !changed {
            break noreturn;
        }
    };
    Ok(Proved { noreturn, reachable: reachable(module, &procedures, roots) })
}

/// Each body whose definition is exact stamped with what alias's summary
/// says of it, as LLVM's FunctionAttrs states it: `memory(...)`, and on
/// each pointer parameter `nocapture`, `readnone`, `readonly` or
/// `writeonly`, and `initializes`; callees first, so a caller sees what
/// its callees initialize. An attribute already stated stays. The bodies
/// stamped.
pub fn stamped(module: &mut Module) -> Result<Vec<GlobalId>, String> {
    let layout = match &module.datalayout {
        Some(text) => DataLayout::parse(text)?,
        None => DataLayout::default(),
    };
    let known = Summaries::run(module, &layout, None)?;
    let globals = globalsaa::analysis(module, &layout, None)?;
    let mut changed = Vec::new();
    for id in CallGraph::new(module).bottom_up() {
        let global = module.global(id);
        let exact = matches!(global.linkage, Linkage::External | Linkage::Internal | Linkage::Private);
        let (Some(name), Some(function), true) = (global.name.as_ref(), global.function(), exact) else { continue };
        let Some(summary) = known.get(name) else { continue };
        let procedure = Procedure::of(Unit::of(module, &layout, function).with_globals_aa(&globals));
        let initialized = alias::initialized(&procedure, &known)?;
        let volatile = function.walk().any(|(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }));
        let pointers = function.parameters().iter().map(|&one| matches!(module.context.types.get(function.value(one).ty), Type::Pointer(_))).collect::<Vec<_>>();
        let function = function_mut(module, id).1;
        let before = (function.attrs.clone(), function.parameter_attrs.clone());
        if !function.attrs.iter().any(|one| matches!(one, Attribute::Memory(_))) {
            function.attrs.extend(_memory(summary, volatile));
        }
        for (index, attrs) in function.parameter_attrs.iter_mut().enumerate().filter(|(index, _)| pointers[*index]) {
            let has = |attrs: &[Attribute], flag: &str| attrs.iter().any(|one| matches!(one, Attribute::Flag(stated) if stated == flag));
            let identity = Some(Identity::Int(index as i64));
            if summary.captures.contains(&identity) {
                continue;
            }
            if !has(attrs, "nocapture") {
                attrs.push(Attribute::Flag("nocapture".to_owned()));
            }
            let through = |slices: &BTreeSet<Slice>| slices.iter().any(|one| one.object.kind == MemoryKind::Parameter && one.object.identity == identity);
            let (reads, writes) = (through(&summary.reads) || summary.unknown_read, through(&summary.writes) || summary.unknown_write);
            let access = match (reads, writes) {
                (false, false) => Some("readnone"),
                (true, false) => Some("readonly"),
                (false, true) => Some("writeonly"),
                (true, true) => None,
            };
            if let Some(access) = access
                && !["readnone", "readonly", "writeonly"].iter().any(|one| has(attrs, one))
            {
                attrs.push(Attribute::Flag(access.to_owned()));
            }
            if !initialized[index].is_empty() && !attrs.iter().any(|one| matches!(one, Attribute::Initializes(_))) {
                attrs.push(Attribute::Initializes(initialized[index].clone()));
            }
        }
        if before != (function.attrs.clone(), function.parameter_attrs.clone()) {
            changed.push(id);
        }
    }
    Ok(changed)
}

/// `memory(...)` for what `summary` reads and writes through its pointer
/// parameters and elsewhere, its own frame aside; volatile accesses touch
/// inaccessible memory, as LLVM models them. None where it allows all.
fn _memory(summary: &Summary, volatile: bool) -> Option<Attribute> {
    let local = |one: &Slice| matches!(one.object.kind, MemoryKind::Frame | MemoryKind::Stack);
    let found = |slices: &BTreeSet<Slice>, parameter: bool| slices.iter().filter(|one| !local(one)).any(|one| (one.object.kind == MemoryKind::Parameter) == parameter);
    let arguments = Effects { reads: found(&summary.reads, true), writes: found(&summary.writes, true) };
    let other = Effects { reads: found(&summary.reads, false) || summary.unknown_read, writes: found(&summary.writes, false) || summary.unknown_write };
    let access = |one: Effects| {
        match (one.reads, one.writes) {
            (true, true) => "readwrite",
            (true, false) => "read",
            (false, true) => "write",
            (false, false) => "none",
        }
        .to_owned()
    };
    if other == Effects::ANY && arguments == Effects::ANY {
        return None;
    }
    let mut locations = Vec::new();
    if other != Effects::NONE || (arguments == Effects::NONE && !volatile) {
        locations.push((None, access(other)));
    }
    if arguments != other {
        locations.push((Some("argmem".to_owned()), access(arguments)));
    }
    if volatile && other != Effects::ANY {
        locations.push((Some("inaccessiblemem".to_owned()), "readwrite".to_owned()));
    }
    Some(Attribute::Memory(locations))
}

/// The procedure `id`, with the context its types and constants live in.
pub(crate) fn function_mut(module: &mut Module, id: GlobalId) -> (&mut llrm_mir::context::Context, &mut llrm_mir::module::Function) {
    let Module { context, globals, .. } = module;
    match &mut globals[id.0 as usize].kind {
        GlobalKind::Function(function) => (context, function),
        GlobalKind::Variable(_) => unreachable!("a procedure is a function"),
    }
}

/// Procedures a surviving direct call reaches from `roots`; all of them
/// when there are no roots.
fn reachable(module: &Module, procedures: &[GlobalId], roots: &BTreeSet<GlobalId>) -> BTreeSet<GlobalId> {
    let defined = procedures.iter().copied().collect::<BTreeSet<_>>();
    if roots.is_empty() {
        return defined;
    }
    let mut reached = BTreeSet::new();
    let mut pending = roots.intersection(&defined).copied().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if !reached.insert(id) {
            continue;
        }
        let function = module.global(id).function().expect("a procedure");
        pending.extend(
            function
                .walk()
                .filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_)))
                .filter_map(|(_, inst)| llrm_mir::memory::callee(&module.context, function, inst))
                .filter(|target| defined.contains(target) && !reached.contains(target)),
        );
    }
    reached
}

#[cfg(test)]
#[path = "interprocedural_tests.rs"]
mod tests;
