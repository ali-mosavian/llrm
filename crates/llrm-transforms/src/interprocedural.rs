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
//! pipeline takes the module, its analyses and the body's id. `propagated`, the calls
//! whose return was carried, has nothing to hold: a carried return leaves
//! the call's result unread. The old `Module` is `Proved`, the name being
//! llrm-mir's. `Interprocedural` is the step as a `ModulePass`.
//!
//! The old module had no tests of its own.

use std::cell::RefCell;
use std::collections::BTreeSet;

use llrm_analysis::alias::{self, Procedure, Summary};
use llrm_analysis::effects;
use llrm_analysis::interprocedural as facts;
use llrm_analysis::globalsaa;
use llrm_analysis::manager::Summaries;
use llrm_analysis::memory::{Identity, MemoryKind, Slice, Unit};
use llrm_mir::callgraph::CallGraph;
use llrm_mir::context::GlobalId;
use llrm_mir::memory::Effects;
use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Module};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{ModuleAnalyses, ModuleAnalysis, ModulePass, PreservedAnalyses};
use llrm_mir::target::Neutral;
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
/// The program's target prices inlining, and the roots are the bodies
/// something outside the module may call.
pub struct Interprocedural {
    pub pipeline: Box<dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str)>,
    /// What the last run proved.
    pub proved: Option<Proved>,
}

impl ModulePass for Interprocedural {
    fn name(&self) -> &'static str {
        "interprocedural"
    }

    fn run(&mut self, module: &mut Module, analyses: &mut ModuleAnalyses) -> Vec<GlobalId> {
        let changed = RefCell::new(BTreeSet::new());
        let pipeline = &mut self.pipeline;
        let costs = analyses.program().target.costs();
        let roots = roots(module);
        let proved = optimized::<String>(
            module,
            analyses,
            &roots,
            &costs,
            &mut |module, analyses, id, stage| {
                changed.borrow_mut().insert(id);
                pipeline(module, analyses, id, stage);
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

/// The bodies something outside the module may call.
fn roots(module: &Module) -> BTreeSet<GlobalId> {
    module
        .functions()
        .filter(|(_, global, function)| !function.is_declaration() && !matches!(global.linkage, Linkage::Internal | Linkage::Private))
        .map(|(id, _, _)| id)
        .collect()
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

/// Run the whole-module step over `module`, its module analyses those
/// `analyses` holds; each edit drops them.
///
/// `reoptimised(module, analyses, id, stage)` runs procedure `id`'s
/// pipeline again on a body `stage` changed; `spliced` sees a body straight
/// after inlining, before that.
pub fn optimized<E: From<String>>(
    module: &mut Module,
    analyses: &mut ModuleAnalyses,
    roots: &BTreeSet<GlobalId>,
    costs: &OperationCosts,
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
    spliced: &mut dyn FnMut(&Module, GlobalId, &str) -> Result<(), E>,
) -> Result<Proved, E> {
    let procedures = procedures(module);
    let private = private(module);
    let edited = |analyses: &mut ModuleAnalyses| analyses.invalidate(&PreservedAnalyses::none());

    // Inline only after each independent body has reached its local fixed
    // point; the splice's result goes straight back through the pipeline.
    // What each body does, stated on it, is what inlining and the dead-call
    // removal below read.
    if !stamped(module).map_err(E::from)?.is_empty() {
        edited(analyses);
    }
    let pure = facts::stated_pure(module);
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
            edited(analyses);
            let stage = format!("inline{inline_round}");
            spliced(module, id, &stage)?;
            reoptimised(module, analyses, id, &format!("{stage}."))?;
            changed = true;
            inline_round += 1;
        }
        if !changed {
            break;
        }
    }

    let mut return_round = 0;

    // Materialize every newly constant result.
    let propagate_constant_returns = |module: &mut Module,
                                      analyses: &mut ModuleAnalyses,
                                      return_round: &mut i64,
                                      reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>|
     -> Result<(), E> {
            loop {
                let returns = facts::constant_returns(module);
                let mut changed = false;
                for &id in &procedures {
                    let (context, function) = function_mut(module, id);
                    if !facts::propagate_returns(context, function, &returns) {
                        continue;
                    }
                    edited(analyses);
                    reoptimised(module, analyses, id, &format!("ipa{return_round}."))?;
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
    propagate_constant_returns(module, analyses, &mut return_round, reoptimised)?;
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
            edited(analyses);
            reoptimised(module, analyses, id, &format!("ipa-args{argument_round}."))?;
            changed = true;
        }
        if changed {
            argument_round += 1;
            propagate_constant_returns(module, analyses, &mut return_round, reoptimised)?;
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
            edited(analyses);
            reoptimised(module, analyses, id, &format!("ipa-inline{argument_round}."))?;
            inlined = true;
        }
        if inlined {
            propagate_constant_returns(module, analyses, &mut return_round, reoptimised)?;
        }
        if !changed && !inlined {
            break;
        }
    }
    // Propagation may have left a body doing less than it states.
    if !stamped(module).map_err(E::from)?.is_empty() {
        edited(analyses);
    }
    let declarations = effects::declarations(module);
    for &id in &procedures {
        let (context, function) = function_mut(module, id);
        if facts::remove_dead_pure_calls(context, &declarations, function) {
            edited(analyses);
            reoptimised(module, analyses, id, "ipa-pure.")?;
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
            edited(analyses);
            reoptimised(module, analyses, id, "ipa-noreturn.")?;
            changed = true;
        }
        if !changed {
            break noreturn;
        }
    };
    Ok(Proved { noreturn, reachable: reachable(module, &procedures, roots) })
}

/// Each body whose definition is exact stamped with what it is proved to
/// do, as LLVM's FunctionAttrs states it, callees first so a caller sees
/// what they state:
/// - `memory(...)`: alias's summary through the pointer parameters and
///   elsewhere, its own frame aside; volatile accesses and callees reach
///   inaccessible memory. A stated one only narrows.
/// - on each pointer parameter it keeps no copy of, `nocapture`, then
///   `readnone`, `readonly` or `writeonly`, and `initializes`.
/// - `willreturn` where every path returns without looping and every call
///   states it; `nounwind` where every call states it and no access can
///   fault (`interprocedural::cannot_fault`).
///
/// Any other attribute already stated stays. The bodies stamped.
pub fn stamped(module: &mut Module) -> Result<Vec<GlobalId>, String> {
    let layout = match &module.datalayout {
        Some(text) => DataLayout::parse(text)?,
        None => DataLayout::default(),
    };
    let mut neutral = ModuleAnalyses::of(module, std::rc::Rc::new(Neutral));
    let known = Summaries::run(module, &mut neutral)?;
    let globals = globalsaa::analysis(module, neutral.program())?;
    let mut declarations = effects::declarations(module);
    let mut changed = Vec::new();
    for id in CallGraph::new(module).bottom_up() {
        let global = module.global(id);
        let exact = matches!(global.linkage, Linkage::External | Linkage::Internal | Linkage::Private);
        let (Some(name), Some(function), true) = (global.name.as_ref(), global.function(), exact) else { continue };
        let Some(summary) = known.get(name) else { continue };
        let procedure = Procedure::of(Unit::of(module, &layout, function).with_globals_aa(&globals));
        let initialized = alias::initialized(&procedure, &known)?;
        let calls = function.walk().map(|(_, inst)| inst).filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_))).collect::<Vec<_>>();
        let states = |flag: &str| calls.iter().all(|&inst| effects::states(&module.context, &declarations, function, inst, flag));
        let returns = facts::returns_without_looping(function) && states("willreturn");
        let nounwind = states("nounwind") && facts::cannot_fault(module, &layout, function);
        let volatile = function.walk().any(|(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }));
        let mut hidden = if volatile { Effects::ANY } else { Effects::NONE };
        for &inst in &calls {
            let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { continue };
            let declared = llrm_mir::memory::callee(&module.context, function, inst).and_then(|one| declarations.get(one.0 as usize)).and_then(GlobalValue::function).map_or(Effects::ANY, |one| llrm_mir::memory::inaccessible(&one.attrs));
            hidden = _either(hidden, _both(declared, llrm_mir::memory::inaccessible(&info.attrs)));
        }
        let pointers = function.parameters().iter().map(|&one| matches!(module.context.types.get(function.value(one).ty), Type::Pointer(_))).collect::<Vec<_>>();
        let function = function_mut(module, id).1;
        let before = (function.attrs.clone(), function.parameter_attrs.clone());
        _narrowed(&mut function.attrs, summary, hidden);
        let has = llrm_mir::memory::has;
        for (flag, proved) in [("willreturn", returns), ("nounwind", nounwind)] {
            if proved && !has(&function.attrs, flag) {
                function.attrs.push(Attribute::Flag(flag.to_owned()));
            }
        }
        for (index, attrs) in function.parameter_attrs.iter_mut().enumerate().filter(|(index, _)| pointers[*index]) {
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
            declarations[id.0 as usize] = module.global(id).declaration();
            changed.push(id);
        }
    }
    Ok(changed)
}

fn _both(one: Effects, other: Effects) -> Effects {
    Effects { reads: one.reads && other.reads, writes: one.writes && other.writes }
}

fn _either(one: Effects, other: Effects) -> Effects {
    Effects { reads: one.reads || other.reads, writes: one.writes || other.writes }
}

/// `attrs`' `memory(...)` narrowed to what `summary` reads and writes
/// through the pointer parameters and elsewhere, its own frame aside, and
/// to `hidden` on inaccessible memory.
fn _narrowed(attrs: &mut Vec<Attribute>, summary: &Summary, hidden: Effects) {
    let local = |one: &Slice| matches!(one.object.kind, MemoryKind::Frame | MemoryKind::Stack);
    let found = |slices: &BTreeSet<Slice>, parameter: bool| slices.iter().filter(|one| !local(one)).any(|one| (one.object.kind == MemoryKind::Parameter) == parameter);
    let (stated_arguments, stated_other) = llrm_mir::memory::located(attrs);
    let arguments = _both(stated_arguments, Effects { reads: found(&summary.reads, true), writes: found(&summary.writes, true) });
    let other = _both(stated_other, Effects { reads: found(&summary.reads, false) || summary.unknown_read, writes: found(&summary.writes, false) || summary.unknown_write });
    let hidden = _both(llrm_mir::memory::inaccessible(attrs), hidden);
    if (arguments, hidden, other) == (stated_arguments, llrm_mir::memory::inaccessible(attrs), stated_other) {
        return;
    }
    let access = |one: Effects| {
        match (one.reads, one.writes) {
            (true, true) => "readwrite",
            (true, false) => "read",
            (false, true) => "write",
            (false, false) => "none",
        }
        .to_owned()
    };
    let named = [("argmem", arguments), ("inaccessiblemem", hidden)].into_iter().filter(|(_, one)| *one != other).map(|(location, one)| (Some(location.to_owned()), access(one))).collect::<Vec<_>>();
    let mut locations = Vec::new();
    if other != Effects::NONE || named.is_empty() {
        locations.push((None, access(other)));
    }
    locations.extend(named);
    // What `readnone`, `readonly` or `writeonly` said, it now says.
    attrs.retain(|one| match one {
        Attribute::Memory(_) => false,
        Attribute::Flag(flag) => !["readnone", "readonly", "writeonly"].contains(&flag.as_str()),
        _ => true,
    });
    attrs.push(Attribute::Memory(locations));
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
