//! Adapted from llrm-core's `optimize/interprocedural.rs`: the whole-program
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
//! tables have no counterpart and a procedure is its module and
//! `GlobalId`; a procedure has every caller in the program when the program
//! does not export it, not by a parameter. A body changes in place, so the
//! pipeline takes the module, its analyses and the body's id. `propagated`,
//! the calls whose return was carried, has nothing to hold: a carried
//! return leaves the call's result unread. The old `Module` is `Proved`,
//! the name being llrm-mir's. `Interprocedural` is the step as a
//! `ProgramPass`.
//!
//! The old module had no tests of its own.

use std::collections::BTreeSet;

use llrm_analysis::alias::{self, Procedure, Summary};
use llrm_analysis::cfg::Shape;
use llrm_analysis::effects;
use llrm_analysis::interprocedural as facts;
use llrm_analysis::manager::{GlobalsAA, ProgramSummaries, Summaries};
use llrm_analysis::memory::{Identity, MemoryKind, Slice, Unit};
use llrm_mir::callgraph::{CallGraph, CallGraphAnalysis, Defined};
use llrm_mir::context::GlobalId;
use llrm_mir::memory::Effects;
use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Module};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::{Declarations, ModuleAnalyses, PreservedAnalyses};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};
use llrm_mir::types::Type;

use crate::inline;
use crate::profit::OperationCosts;

/// What the step proved about the program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proved {
    /// Procedures no outside code calls that cannot return.
    pub noreturn: BTreeSet<Defined>,
    /// Procedures a root still calls, roots included.
    pub reachable: BTreeSet<Defined>,
}

/// The step as a program pass: `pipeline` is each changed body's pipeline.
/// The program's target prices inlining, and the roots are the bodies it
/// exports.
pub struct Interprocedural {
    pub pipeline: Box<dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str)>,
    /// What the last run proved.
    pub proved: Option<Proved>,
    pub inline: inline::Threshold,
    /// Inlining weighs code bytes, not clocks.
    pub size: bool,
}

impl ProgramPass for Interprocedural {
    fn name(&self) -> &'static str {
        "interprocedural"
    }

    fn run(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<(), String> {
        let pipeline = &mut self.pipeline;
        let costs = if self.size { program.target.size_costs() } else { program.target.costs() };
        let roots = roots(program);
        let mut modules = managers(program, analyses);
        let proved = optimized::<String>(
            program,
            &mut modules,
            &roots,
            &costs,
            self.inline,
            !self.size,
            &mut |module, analyses, id, stage| {
                pipeline(module, analyses, id, stage);
                Ok(())
            },
            &mut |_, _, _| Ok(()),
        )?;
        self.proved = Some(proved);
        analyses.invalidate();
        Ok(())
    }
}

/// Each module's analyses under `analyses`' program results, summaries
/// kept for the function passes to read.
pub fn managers(program: &Program, analyses: &mut ProgramAnalyses) -> Vec<ModuleAnalyses> {
    analyses.get::<ProgramSummaries>(program);
    (0..program.modules.len())
        .map(|at| {
            let mut one = ModuleAnalyses::new(analyses.proxy(program, at));
            one.require::<Summaries>();
            one
        })
        .collect()
}

/// The bodies code outside the program may call.
pub fn roots(program: &Program) -> BTreeSet<Defined> {
    defined(program).filter(|&(at, id)| program.exports.exported(program.modules[at].global(id))).collect()
}

/// Every defined procedure, in program order.
fn defined(program: &Program) -> impl Iterator<Item = Defined> + '_ {
    program.modules.iter().enumerate().flat_map(|(at, module)| module.functions().filter(|(_, _, function)| !function.is_declaration()).map(move |(id, _, _)| (at, id)))
}

/// Defined procedures of module `at`, in module order.
fn procedures(program: &Program, at: usize) -> Vec<GlobalId> {
    defined(program).filter(|&(one, _)| one == at).map(|(_, id)| id).collect()
}

/// Defined procedures no outside code calls: every caller is in the program.
fn unexported(program: &Program) -> BTreeSet<Defined> {
    defined(program).filter(|&(at, id)| !program.exports.exported(program.modules[at].global(id))).collect()
}

/// Module `at`'s procedures every caller of which is in the module: no
/// outside code, and no other module, calls them.
fn private(program: &Program, at: usize) -> BTreeSet<GlobalId> {
    let others: BTreeSet<Defined> = program
        .modules
        .iter()
        .enumerate()
        .filter(|&(other, _)| other != at)
        .flat_map(|(other, module)| (0..module.globals.len() as u32).filter_map(move |id| program.definition(other, GlobalId(id))))
        .collect();
    unexported(program).into_iter().filter(|&(one, id)| one == at && !others.contains(&(at, id))).map(|(_, id)| id).collect()
}

/// Run the whole-program step over `program`, each module's analyses
/// those `modules` holds; each edit drops its module's.
///
/// `reoptimised(module, analyses, id, stage)` runs procedure `id`'s
/// pipeline again on a body `stage` changed; `spliced` sees a body straight
/// after inlining, before that. A callee is inlined from its own module
/// only.
pub fn optimized<E: From<String>>(
    program: &mut Program,
    modules: &mut [ModuleAnalyses],
    roots: &BTreeSet<Defined>,
    costs: &OperationCosts,
    threshold: inline::Threshold,
    hints: bool,
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
    spliced: &mut dyn FnMut(&Module, GlobalId, &str) -> Result<(), E>,
) -> Result<Proved, E> {
    let count = program.modules.len();
    let procedures: Vec<Vec<GlobalId>> = (0..count).map(|at| procedures(program, at)).collect();
    let private: Vec<BTreeSet<GlobalId>> = (0..count).map(|at| private(program, at)).collect();
    let unexported = unexported(program);
    let edited = |analyses: &mut ModuleAnalyses, bodies: &[GlobalId]| {
        for &id in bodies {
            analyses.changed(id);
        }
        analyses.invalidate(&PreservedAnalyses::none());
    };

    // Inline only after each independent body has reached its local fixed
    // point; the splice's result goes straight back through the pipeline.
    // What each body does, stated on it, is what inlining and the dead-call
    // removal below read.
    stamped_all(program, modules).map_err(E::from)?;
    let mut inline_round = 0;
    loop {
        let mut changed = false;
        for at in 0..count {
            let module = &mut program.modules[at];
            let counts = inline::call_counts(module);
            let available = inline::candidates(module, &program.layout, &counts, &private[at], costs, threshold, hints);
            let recursive = inline::recursive(module);
            for &id in &procedures[at] {
                let caller = module.global(id).function().expect("a procedure");
                let constants = facts::current_call_constants(&module.context, caller);
                let constant = inline::constant_sites(module, &program.layout, &recursive, caller, &constants, costs, threshold);
                let (context, function) = function_mut(module, id);
                let by = inline::Caller { layout: &program.layout, recursive: recursive.contains(&id) };
                if !inline::expanded(context, function, &by, &available, Some(&constant))? {
                    continue;
                }
                edited(&mut modules[at], &[id]);
                let stage = format!("inline{inline_round}");
                spliced(module, id, &stage)?;
                reoptimised(module, &mut modules[at], id, &format!("{stage}."))?;
                changed = true;
                inline_round += 1;
            }
        }
        if !changed {
            break;
        }
    }

    let mut return_round = 0;

    // Materialize every newly constant result.
    let propagate_constant_returns = |program: &mut Program,
                                      modules: &mut [ModuleAnalyses],
                                      return_round: &mut i64,
                                      reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>|
     -> Result<(), E> {
        loop {
            let returns = facts::program_returns(program);
            let mut changed = false;
            for at in 0..count {
                for &id in &procedures[at] {
                    let (context, function) = function_mut(&mut program.modules[at], id);
                    if !facts::propagate_returns(context, function, &returns[at]) {
                        continue;
                    }
                    edited(&mut modules[at], &[id]);
                    reoptimised(&mut program.modules[at], &mut modules[at], id, &format!("ipa{return_round}."))?;
                    changed = true;
                }
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
    propagate_constant_returns(program, modules, &mut return_round, reoptimised)?;
    let mut argument_round = 0;
    loop {
        let constants = facts::program_parameters(program, &unexported);
        let mut changed = false;
        for at in 0..count {
            for &id in &procedures[at] {
                let Some(constants_for_body) = constants[at].get(&id) else {
                    continue;
                };
                let (context, function) = function_mut(&mut program.modules[at], id);
                if !facts::specialize_parameters(context, function, constants_for_body) {
                    continue;
                }
                edited(&mut modules[at], &[id]);
                reoptimised(&mut program.modules[at], &mut modules[at], id, &format!("ipa-args{argument_round}."))?;
                changed = true;
            }
        }
        if changed {
            argument_round += 1;
            propagate_constant_returns(program, modules, &mut return_round, reoptimised)?;
        }

        // A single current-MIR constant may be worth cloning even where
        // another caller keeps the private body dynamic.
        let mut inlined = false;
        for at in 0..count {
            let module = &mut program.modules[at];
            let counts = inline::call_counts(module);
            let available = inline::candidates(module, &program.layout, &counts, &private[at], costs, threshold, hints);
            let recursive = inline::recursive(module);
            for &id in &procedures[at] {
                let caller = module.global(id).function().expect("a procedure");
                let current = facts::current_call_constants(&module.context, caller);
                let constant = inline::constant_sites(module, &program.layout, &recursive, caller, &current, costs, threshold);
                let (context, function) = function_mut(module, id);
                let by = inline::Caller { layout: &program.layout, recursive: recursive.contains(&id) };
                if !inline::expanded(context, function, &by, &available, Some(&constant))? {
                    continue;
                }
                edited(&mut modules[at], &[id]);
                reoptimised(module, &mut modules[at], id, &format!("ipa-inline{argument_round}."))?;
                inlined = true;
            }
        }
        if inlined {
            propagate_constant_returns(program, modules, &mut return_round, reoptimised)?;
        }
        if !changed && !inlined {
            break;
        }
    }
    // Propagation may have left a body doing less than it states.
    stamped_all(program, modules).map_err(E::from)?;
    for at in 0..count {
        let declarations = modules[at].get::<Declarations>(&program.modules[at]);
        for &id in &procedures[at] {
            let (context, function) = function_mut(&mut program.modules[at], id);
            if facts::remove_dead_pure_calls(context, &declarations, function) {
                edited(&mut modules[at], &[id]);
                reoptimised(&mut program.modules[at], &mut modules[at], id, "ipa-pure.")?;
            }
        }
    }
    // A direct private body whose every path stops makes the tail of every
    // call site unreachable: keep the physical call, remove only the code
    // that would require it to return, and repeat.
    let noreturn = loop {
        let declarations: Vec<_> = (0..count).map(|at| modules[at].get::<Declarations>(&program.modules[at])).collect();
        let noreturn = facts::program_noreturn(program, &declarations.iter().map(|one| one.as_slice()).collect::<Vec<_>>(), &unexported);
        let mut changed = false;
        for at in 0..count {
            let local = program.local(at, &noreturn);
            for &id in &procedures[at] {
                let (context, function) = function_mut(&mut program.modules[at], id);
                if !facts::terminal_calls(context, &declarations[at], function, &local) {
                    continue;
                }
                edited(&mut modules[at], &[id]);
                reoptimised(&mut program.modules[at], &mut modules[at], id, "ipa-noreturn.")?;
                changed = true;
            }
        }
        if !changed {
            break noreturn;
        }
    };
    Ok(Proved { noreturn, reachable: reachable(program, roots) })
}

/// `stamped` over every module, callees' modules first, each body's
/// attributes then stated on every declaration of it; the analyses of the
/// modules it changed dropped.
pub fn stamped_all(program: &mut Program, modules: &mut [ModuleAnalyses]) -> Result<(), String> {
    let mut order: Vec<usize> = Vec::new();
    for (at, _) in CallGraph::of(program).bottom_up() {
        if !order.contains(&at) {
            order.push(at);
        }
    }
    order.extend((0..program.modules.len()).filter(|at| !order.contains(at)).collect::<Vec<_>>());
    for at in order {
        let bodies = stamped(&mut program.modules[at], &mut modules[at])?;
        if bodies.is_empty() {
            continue;
        }
        for &id in &bodies {
            modules[at].changed(id);
        }
        modules[at].invalidate(&PreservedAnalyses::none());
        for id in bodies {
            for other in published(program, at, id) {
                modules[other].invalidate(&PreservedAnalyses::none());
            }
        }
    }
    Ok(())
}

/// Body `id` of module `at`'s attributes stated on each other module's
/// declaration of it; the modules changed.
fn published(program: &mut Program, at: usize, id: GlobalId) -> Vec<usize> {
    let mut changed = Vec::new();
    for other in (0..program.modules.len()).filter(|&other| other != at) {
        let declared: Vec<GlobalId> = (0..program.modules[other].globals.len() as u32).map(GlobalId).filter(|&one| program.definition(other, one) == Some((at, id))).collect();
        for one in declared {
            if program.restate(at, id, other, one) && !changed.contains(&other) {
                changed.push(other);
            }
        }
    }
    changed
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
/// Any other attribute already stated stays. The bodies stamped; the
/// caller drops `analyses` when there are any.
pub fn stamped(module: &mut Module, analyses: &mut ModuleAnalyses) -> Result<Vec<GlobalId>, String> {
    let program = std::rc::Rc::clone(analyses.program());
    let layout = &program.layout;
    let known = analyses.get::<Summaries>(module);
    let known = Result::as_ref(&*known).map_err(String::clone)?;
    let globals = analyses.get::<GlobalsAA>(module);
    let globals = Result::as_ref(&*globals).map_err(String::clone)?;
    let mut declarations = (*analyses.get::<Declarations>(module)).clone();
    let mut changed = Vec::new();
    for id in analyses.get::<CallGraphAnalysis>(module).bottom_up() {
        let global = module.global(id);
        let exact = matches!(global.linkage, Linkage::External | Linkage::Internal | Linkage::Private);
        let (Some(name), Some(function), true) = (global.name.as_ref(), global.function(), exact) else { continue };
        let Some(summary) = known.get(name) else { continue };
        let shape = analyses.function::<Shape>(module, id);
        let procedure = Procedure::of(Unit { program: Some(&program), ..Unit::of(module, layout, function) }.with_globals_aa(globals).with_shape(&shape));
        let initialized = alias::initialized(&procedure, known)?;
        let calls = function.walk().map(|(_, inst)| inst).filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_))).collect::<Vec<_>>();
        let states = |flag: &str| calls.iter().all(|&inst| effects::states(&module.context, &declarations, function, inst, flag));
        let returns = facts::returns_without_looping(function) && states("willreturn");
        let nounwind = states("nounwind") && facts::cannot_fault(module, layout, function);
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
fn reachable(program: &Program, roots: &BTreeSet<Defined>) -> BTreeSet<Defined> {
    let every = defined(program).collect::<BTreeSet<_>>();
    if roots.is_empty() {
        return every;
    }
    let mut reached = BTreeSet::new();
    let mut pending = roots.intersection(&every).copied().collect::<Vec<_>>();
    while let Some((at, id)) = pending.pop() {
        if !reached.insert((at, id)) {
            continue;
        }
        let module = &program.modules[at];
        let function = module.global(id).function().expect("a procedure");
        pending.extend(
            function
                .walk()
                .filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_)))
                .filter_map(|(_, inst)| llrm_mir::memory::callee(&module.context, function, inst))
                .filter_map(|target| program.definition(at, target))
                .filter(|target| every.contains(target) && !reached.contains(target)),
        );
    }
    reached
}

#[cfg(test)]
#[path = "interprocedural_tests.rs"]
mod tests;
