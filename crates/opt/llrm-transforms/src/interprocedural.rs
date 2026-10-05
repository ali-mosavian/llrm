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
use llrm_mir::facts::{Fact, Facts};
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
    /// Inlining weighs code bytes, and what only the clocks admit stays where it comes to no more bytes,
    /// less what the clocks it saves buy at this many thousandths of a clock a byte; None weighs the clocks alone.
    pub rate: Option<i64>,
}

impl ProgramPass for Interprocedural {
    fn name(&self) -> &'static str {
        "interprocedural"
    }

    fn run(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<(), String> {
        let pipeline = &mut self.pipeline;
        let costs = if self.rate.is_some() { program.target.size_costs() } else { program.target.costs() };
        let clocks = program.target.costs();
        let loose = self.rate.map(|_| &clocks);
        let rate = self.rate.unwrap_or(0);
        // The op budget stays the clocks': what it bounds is the body, not its price.
        let reach = program.target.costs().call;
        let roots = roots(program);
        let mut modules = managers(program, analyses);
        let proved = optimized::<String>(
            program,
            &mut modules,
            &roots,
            &costs,
            loose,
            rate,
            reach,
            self.inline,
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

/// What each body does to memory, stated on it before any body's pipeline runs, as LLVM's
/// PostOrderFunctionAttrs runs before the loop passes: a loop that calls a function is judged on what it
/// does, not on a declaration nothing yet describes.
pub struct Stamp;

impl ProgramPass for Stamp {
    fn name(&self) -> &'static str {
        "stamp"
    }

    fn run(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<(), String> {
        let mut modules = managers(program, analyses);
        stamped_all(program, &mut modules)?;
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
            // As the first run's: a body sent back through reads what no code outside reaches.
            one.require::<GlobalsAA>();
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

/// Defined procedures no outside code calls: every caller is in the program,
/// and a call through a pointer is not one, so none has its address taken.
fn unexported(program: &Program) -> BTreeSet<Defined> {
    let addressed: BTreeSet<Defined> = program.modules.iter().enumerate().flat_map(|(at, module)| llrm_mir::callgraph::addressed(module).into_iter().filter_map(move |id| program.definition(at, id))).collect();
    defined(program).filter(|&one @ (at, id)| !program.exports.exported(program.modules[at].global(id)) && !addressed.contains(&one)).collect()
}

/// The calls to inline: those `costs` admits and, tuned for size, those `loose` (the clocks) does too,
/// which `grew` then checks against what they leave: a body that folds on known addresses is
/// nothing the byte price can see.
fn candidates(module: &Module, layout: &llrm_mir::datalayout::DataLayout, counts: &inline::Counter, private: &BTreeSet<GlobalId>, costs: &OperationCosts, loose: Option<&OperationCosts>, reach: i64, threshold: inline::Threshold) -> (llrm_support::hash::IndexMap<GlobalId, inline::Candidate>, llrm_support::hash::IndexMap<GlobalId, inline::Candidate>) {
    let mut found = inline::candidates(module, layout, counts, private, costs, reach, threshold);
    // Held only to inline from, a body's price is what the trial finds, never its size.
    found.retain(|id, _| module.global(*id).linkage != Linkage::AvailableExternally);
    let more = loose.map_or_else(Default::default, |loose| inline::candidates(module, layout, counts, private, loose, reach, threshold).into_iter().filter(|(id, _)| !found.contains_key(id)).collect());
    (found, more)
}

fn constant_sites(module: &Module, layout: &llrm_mir::datalayout::DataLayout, recursive: &BTreeSet<GlobalId>, caller: &llrm_mir::module::Function, constants: &llrm_support::hash::IndexMap<llrm_mir::module::InstId, Vec<Option<llrm_mir::context::ConstantId>>>, costs: &OperationCosts, loose: Option<&OperationCosts>, reach: i64, threshold: inline::Threshold) -> (llrm_support::hash::IndexMap<llrm_mir::module::InstId, inline::Candidate>, llrm_support::hash::IndexMap<llrm_mir::module::InstId, inline::Candidate>) {
    let found = inline::constant_sites(module, layout, recursive, caller, constants, costs, reach, threshold);
    let more = loose.map_or_else(Default::default, |loose| inline::constant_sites(module, layout, recursive, caller, constants, loose, reach, threshold).into_iter().filter(|(at, _)| !found.contains_key(at)).collect());
    (found, more)
}

/// What of a body's size the byte estimate may be off by: 28% in the median over QCport's 532
/// functions (calibration of `bytes_in_code`), taken as a quarter. A change in bytes within it of the
/// body copied is the estimate's noise, and what the clocks admitted stays.
/// The most sites of a callee that are tried.
const TRIED_SITES: i64 = 4;

const ESTIMATE_ERROR: (i64, i64) = (1, 4);

/// Whether callers coming to `after` bytes, less `gone` for the callee that goes, come to no more than
/// `before`, within the estimate's error of the body (`moved` bytes) that was copied.
fn stays(after: i64, gone: i64, before: i64, moved: i64, allowance: i64) -> bool {
    after - gone <= before + moved * ESTIMATE_ERROR.0 / ESTIMATE_ERROR.1 + allowance
}

/// The bytes `sites` calls of `callee` removed may add: what their overhead saves in clocks, at `rate`
/// clocks a byte (`clocks` prices the calls), where 0 allows none.
fn allowance(module: &Module, callee: GlobalId, sites: i64, clocks: &OperationCosts, rate: i64) -> i64 {
    let arguments = module.global(callee).function().map_or(0, |body| module.signature(body.ty).1.len());
    if rate <= 0 { 0 } else { 1000 * sites * inline::call_overhead(clocks, arguments) / rate }
}

/// The constant `sites` of `caller` the clocks admit and the bytes do not, inlined, and put back
/// where `caller` then comes to more bytes (`grew`). Whether any stayed.
fn tried_sites<E: From<String>>(
    module: &mut Module,
    modules: &mut ModuleAnalyses,
    layout: &llrm_mir::datalayout::DataLayout,
    private: &BTreeSet<GlobalId>,
    recursive: &BTreeSet<GlobalId>,
    caller: GlobalId,
    sites: &llrm_support::hash::IndexMap<llrm_mir::module::InstId, inline::Candidate>,
    refused: &mut BTreeSet<(GlobalId, llrm_mir::module::InstId)>,
    costs: &OperationCosts,
    credit: (&OperationCosts, i64),
    stage: &str,
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
) -> Result<bool, E> {
    let mut stayed = false;
    // One site at a time, and one that was put back is not tried again: it is the same call in the
    // same body every round (mdl_ai.c re-ran its pipeline some 600 times for 20 sites).
    let untried: Vec<_> = sites.iter().filter(|(site, _)| !refused.contains(&(caller, **site))).collect();
    for (&site, candidate) in untried {
        let counts = inline::call_counts(module);
        let kept = module.global(caller).function().expect("a procedure").clone();
        let before = inline::size(module, caller, costs);
        let bought = llrm_mir::memory::callee(&module.context, &kept, site).map_or(0, |callee| allowance(module, callee, 1, credit.0, credit.1));
        let (context, function) = function_mut(module, caller);
        let by = inline::Caller { layout, recursive: recursive.contains(&caller) };
        let one = llrm_support::hash::IndexMap::from_iter([(site, candidate.clone())]);
        if !inline::expanded(context, function, &by, &Default::default(), Some(&one)).map_err(E::from)? {
            refused.insert((caller, site));
            continue;
        }
        modules.changed(caller);
        modules.invalidate(&PreservedAnalyses::none());
        reoptimised(module, modules, caller, stage)?;
        if before.is_some_and(|before| grew(module, caller, &counts, private, costs, before, bought)) {
            *function_mut(module, caller).1 = kept;
            modules.changed(caller);
            modules.invalidate(&PreservedAnalyses::none());
            refused.insert((caller, site));
        } else {
            stayed = true;
        }
    }
    Ok(stayed)
}

/// Each of `more`, the callees the clocks admit and the bytes do not, tried (`trial`). Whether any stayed.
fn tried_callees<E: From<String>>(
    module: &mut Module,
    modules: &mut ModuleAnalyses,
    layout: &llrm_mir::datalayout::DataLayout,
    private: &BTreeSet<GlobalId>,
    recursive: &BTreeSet<GlobalId>,
    more: &llrm_support::hash::IndexMap<GlobalId, inline::Candidate>,
    costs: &OperationCosts,
    credit: (&OperationCosts, i64),
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
) -> Result<bool, E> {
    let mut stayed = false;
    let mut refused = llrm_support::hash::IndexMap::default();
    for (callee, candidate) in more {
        if trial(module, modules, layout, private, recursive, &[(*callee, candidate)], None, costs, credit, reoptimised)? {
            stayed = true;
        } else {
            refused.insert(*callee, candidate);
        }
    }
    // Callees that feed one another (a constructor and what reads its result) pay together, not
    // alone: each alone leaves the other's call holding what the pair would fold.
    if refused.len() > 1 {
        let together: Vec<_> = refused.iter().map(|(callee, candidate)| (*callee, *candidate)).collect();
        stayed |= trial(module, modules, layout, private, recursive, &together, None, costs, credit, reoptimised)?;
    }
    Ok(stayed)
}

/// `callees` inlined at every direct call of them in `module`, where the callers and what goes
/// with them come to fewer bytes than before (priced by `costs`): the callers re-run through the
/// pipeline, and put back as they were where they do not.
fn trial<E: From<String>>(
    module: &mut Module,
    modules: &mut ModuleAnalyses,
    layout: &llrm_mir::datalayout::DataLayout,
    private: &BTreeSet<GlobalId>,
    recursive: &BTreeSet<GlobalId>,
    callees: &[(GlobalId, &inline::Candidate)],
    only: Option<GlobalId>,
    costs: &OperationCosts,
    credit: (&OperationCosts, i64),
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
) -> Result<bool, E> {
    let reaches = |function: &llrm_mir::module::Function, module: &Module| function.walk().any(|(_, inst)| llrm_mir::memory::callee(&module.context, function, inst).is_some_and(|one| callees.iter().any(|(callee, _)| *callee == one)));
    let callers: Vec<GlobalId> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter(|(_, _, function)| reaches(function, module))
        .map(|(id, _, _)| id)
        .filter(|id| callees.iter().all(|(callee, _)| id != callee) && only.is_none_or(|one| one == *id))
        .collect();
    // Every call re-runs a caller's pipeline: a callee at many sites is not tried (savegame.c's took
    // 4x the compile time), and an estimate's noise over so many copies is no tolerance.
    let counts = inline::call_counts(module);
    let sites: Vec<i64> = callees.iter().map(|(callee, _)| counts.get(callee).copied().unwrap_or(0)).collect();
    // Bodies held only to inline from are small and meant to be copied everywhere: every site is
    // spliced and a caller's pipeline runs once, not after each, so the sites do not cost compile time.
    let held = callees.iter().all(|(callee, _)| module.global(*callee).linkage == Linkage::AvailableExternally);
    if !held && sites.iter().any(|&count| count > TRIED_SITES) {
        return Ok(false);
    }
    // Nothing of such a body goes with its last call, so each caller is judged on its own.
    if held && only.is_none() {
        let mut stayed = false;
        for &caller in &callers {
            stayed |= trial(module, modules, layout, private, recursive, callees, Some(caller), costs, credit, reoptimised)?;
        }
        return Ok(stayed);
    }
    let sizes = |module: &Module| callers.iter().map(|&id| inline::size(module, id, costs)).sum::<Option<i64>>();
    let owns: Option<Vec<i64>> = callees.iter().map(|(callee, _)| inline::size(module, *callee, costs)).collect();
    let (Some(before), Some(owns)) = (sizes(module), owns) else { return Ok(false) };
    let kept: Vec<(GlobalId, llrm_mir::module::Function)> = callers.iter().map(|&id| (id, module.global(id).function().expect("a procedure").clone())).collect();
    let available: llrm_support::hash::IndexMap<GlobalId, inline::Candidate> = callees.iter().map(|(callee, candidate)| (*callee, (*candidate).clone())).collect();
    let mut done = false;
    for &id in &callers {
        let (mut context, mut function) = function_mut(module, id);
        let by = inline::Caller { layout, recursive: recursive.contains(&id) };
        // One site a time, as the rounds do, the body through the pipeline after each.
        let mut spliced = false;
        while inline::expanded(context, function, &by, &available, None).map_err(E::from)? {
            spliced = true;
            if !held {
                modules.changed(id);
                modules.invalidate(&PreservedAnalyses::none());
                reoptimised(module, modules, id, "inline-trial.")?;
            }
            let (next_context, next) = function_mut(module, id);
            context = next_context;
            function = next;
        }
        if spliced && held {
            modules.changed(id);
            modules.invalidate(&PreservedAnalyses::none());
            reoptimised(module, modules, id, "inline-trial.")?;
        }
        done |= spliced;
    }
    if !done {
        return Ok(false);
    }
    let now = inline::call_counts(module);
    let gone: i64 = callees.iter().zip(&owns).filter(|((callee, _), _)| private.contains(callee) && !now.contains_key(callee)).map(|(_, own)| own).sum();
    let moved: i64 = owns.iter().sum();
    let allowed: i64 = callees.iter().zip(&sites).map(|((callee, _), &count)| allowance(module, *callee, count - now.get(callee).copied().unwrap_or(0), credit.0, credit.1)).sum();
    llrm_support::debug!("inline", "trial of {}: {before} bytes before, {:?} after, {gone} gone", callees.iter().map(|(callee, _)| module.global(*callee).name.as_deref().unwrap_or("?")).collect::<Vec<_>>().join(" + "), sizes(module));
    if sizes(module).is_some_and(|after| stays(after, gone, before, moved, allowed)) {
        return Ok(true);
    }
    for (id, function) in kept {
        *function_mut(module, id).1 = function;
        modules.changed(id);
    }
    modules.invalidate(&PreservedAnalyses::none());
    Ok(false)
}

/// Whether `id`, after inlining and the pipeline, comes to more than it did (`before`), less the
/// callees nothing calls now and nothing outside reaches, which go.
fn grew(module: &Module, id: GlobalId, counts: &inline::Counter, private: &BTreeSet<GlobalId>, costs: &OperationCosts, before: i64, allowance: i64) -> bool {
    let Some(after) = inline::size(module, id, costs) else { return false };
    let now = inline::call_counts(module);
    let gone: i64 = counts.iter().filter(|(callee, was)| **was > 0 && private.contains(*callee) && now.get(*callee).copied().unwrap_or(0) == 0 && **callee != id).filter_map(|(&callee, _)| inline::size(module, callee, costs)).sum();
    after - gone > before + allowance
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
    loose: Option<&OperationCosts>,
    rate: i64,
    reach: i64,
    threshold: inline::Threshold,
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
    let mut refused: BTreeSet<(GlobalId, llrm_mir::module::InstId)> = BTreeSet::new();
    let mut inline_round = 0;
    loop {
        let mut changed = false;
        for at in 0..count {
            let module = &mut program.modules[at];
            let counts = inline::call_counts(module);
            let (available, more) = candidates(module, &program.layout, &counts, &private[at], costs, loose, reach, threshold);
            let recursive = inline::recursive(module);
            for &id in &procedures[at] {
                let caller = module.global(id).function().expect("a procedure");
                let constants = facts::current_call_constants(&module.context, caller);
                let (constant, constant_more) = constant_sites(module, &program.layout, &recursive, caller, &constants, costs, loose, reach, threshold);
                let (context, function) = function_mut(module, id);
                let by = inline::Caller { layout: &program.layout, recursive: recursive.contains(&id) };
                if inline::expanded(context, function, &by, &available, Some(&constant))? {
                    edited(&mut modules[at], &[id]);
                    let stage = format!("inline{inline_round}");
                    spliced(module, id, &stage)?;
                    reoptimised(module, &mut modules[at], id, &format!("{stage}."))?;
                    changed = true;
                    inline_round += 1;
                }
                // What only the clocks admit stays only where it comes to no more.
                if loose.is_some() && tried_sites(module, &mut modules[at], &program.layout, &private[at], &recursive, id, &constant_more, &mut refused, costs, (loose.unwrap_or(costs), rate), "inline-trial.", reoptimised)? {
                    changed = true;
                }
            }
            if loose.is_some() && tried_callees(module, &mut modules[at], &program.layout, &private[at], &recursive, &more, costs, (loose.unwrap_or(costs), rate), reoptimised)? {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // A pointer a body only reads through is given as the fields it reads, before what its callers pass
    // is propagated: a length or a segment now crosses the call as a value.
    let (priced, bytes) = match loose {
        Some(clocks) if rate > 0 => (clocks, false),
        _ => (costs, true),
    };
    for at in 0..count {
        for id in crate::argpromotion::promoted(&mut program.modules[at], &program.layout, priced, bytes) {
            edited(&mut modules[at], &[id]);
            reoptimised(&mut program.modules[at], &mut modules[at], id, "promote.")?;
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
            let (available, more) = candidates(module, &program.layout, &counts, &private[at], costs, loose, reach, threshold);
            let recursive = inline::recursive(module);
            for &id in &procedures[at] {
                let caller = module.global(id).function().expect("a procedure");
                let current = facts::current_call_constants(&module.context, caller);
                let (constant, constant_more) = constant_sites(module, &program.layout, &recursive, caller, &current, costs, loose, reach, threshold);
                let (context, function) = function_mut(module, id);
                let by = inline::Caller { layout: &program.layout, recursive: recursive.contains(&id) };
                if inline::expanded(context, function, &by, &available, Some(&constant))? {
                    edited(&mut modules[at], &[id]);
                    reoptimised(module, &mut modules[at], id, &format!("ipa-inline{argument_round}."))?;
                    inlined = true;
                }
                // What only the clocks admit stays only where it comes to no more.
                if loose.is_some() && tried_sites(module, &mut modules[at], &program.layout, &private[at], &recursive, id, &constant_more, &mut refused, costs, (loose.unwrap_or(costs), rate), "ipa-inline-trial.", reoptimised)? {
                    inlined = true;
                }
            }
            if loose.is_some() && tried_callees(module, &mut modules[at], &program.layout, &private[at], &recursive, &more, costs, (loose.unwrap_or(costs), rate), reoptimised)? {
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
    // What its callers pass bounds each parameter of a body only they call: stated as a range, which
    // the body's own proofs then read.
    for (at, id) in llrm_analysis::parameter_ranges::stamp(program, &unexported) {
        edited(&mut modules[at], &[id]);
        reoptimised(&mut program.modules[at], &mut modules[at], id, "ipa-range.")?;
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
///   states it, or where the language promises each loop ends
///   (`mustprogress` on the function, `llvm.loop.mustprogress` on each
///   loop) of a function that does nothing observable;
/// - `norecurse` where nothing can enter it again while it runs; `nounwind` where every call states it and no access can
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
    let graph = analyses.get::<CallGraphAnalysis>(module);
    let callees = llrm_mir::memory::callees(module);
    for id in graph.bottom_up() {
        let global = module.global(id);
        let exact = matches!(global.linkage, Linkage::External | Linkage::Internal | Linkage::Private);
        let (Some(name), Some(function), true) = (global.name.as_ref(), global.function(), exact) else { continue };
        let Some(summary) = known.get(name) else { continue };
        let shape = analyses.function::<Shape>(module, id);
        let procedure = Procedure::of(Unit { program: Some(&program), ..Unit::of(module, layout, function) }.with_globals_aa(globals).with_shape(&shape));
        let initialized = alias::initialized(&procedure, known)?;
        let calls = function.walk().map(|(_, inst)| inst).filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_))).collect::<Vec<_>>();
        let states = |fact: Fact| calls.iter().all(|&inst| effects::states(&module.context, &declarations, function, inst, fact));
        let volatile = function.walk().any(|(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }));
        let norecurse = graph.cannot_reenter(module, id);
        // The language's word that its loops end holds where nothing a loop that never ended could be seen by.
        let unobserved = !volatile && calls.iter().all(|&inst| !llrm_mir::memory::of(&module.context, &callees, function, inst).writes);
        let promised = norecurse && unobserved && states(Fact::WillReturn) && llrm_mir::loops::ends_by_promise(&module.metadata, function, Facts::of(&function.attrs).must_progress());
        let counted = || {
            let shape = Shape::of(function);
            let proofs = |one| llrm_analysis::induction::counted(&unit_of(module, layout, function), one, None, false);
            shape.loops.iter().all(|one| proofs(one).iter().any(|proof| !proof.stops && (proof.count.is_some() || proof.step.magnitude() == &num_bigint::BigUint::from(1_u8))))
        };
        let returns = ((facts::returns_without_looping(function) || counted()) && states(Fact::WillReturn)) || promised;
        let nounwind = states(Fact::NoUnwind) && facts::cannot_fault(module, layout, function);
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
        for (fact, proved) in [(Fact::WillReturn, returns), (Fact::NoUnwind, nounwind), (Fact::NoRecurse, norecurse)] {
            if proved && !Facts::of(&function.attrs).contains(fact) {
                function.attrs.extend(fact.attribute());
            }
        }
        for (index, attrs) in function.parameter_attrs.iter_mut().enumerate().filter(|(index, _)| pointers[*index]) {
            let identity = Some(Identity::Int(index as i64));
            if summary.captures.contains(&identity) {
                continue;
            }
            if !Facts::of(attrs).no_capture() {
                attrs.extend(Fact::NoCapture.attribute());
            }
            let through = |slices: &BTreeSet<Slice>| slices.iter().any(|one| one.object.kind == MemoryKind::Parameter && one.object.identity == identity);
            let (reads, writes) = (through(&summary.reads) || summary.unknown_read, through(&summary.writes) || summary.unknown_write);
            let access = match (reads, writes) {
                (false, false) => Some(Fact::ReadNone),
                (true, false) => Some(Fact::ReadOnly),
                (false, true) => Some(Fact::WriteOnly),
                (true, true) => None,
            };
            let stated = Facts::of(attrs);
            if let Some(access) = access
                && !(stated.read_none() || stated.read_only() || stated.write_only())
            {
                attrs.extend(access.attribute());
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

fn unit_of<'a>(module: &'a Module, layout: &'a llrm_mir::datalayout::DataLayout, function: &'a llrm_mir::module::Function) -> Unit<'a> {
    Unit::of(module, layout, function)
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
        // Said again by the `memory` attribute that replaces them.
        other => !matches!(Fact::of_attribute(other), Some(Fact::ReadNone | Fact::ReadOnly | Fact::WriteOnly)),
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

#[cfg(test)]
mod stays_tests {
    use super::stays;

    /// Callers of 100 bytes before, a callee of 40 copied and then going (40 gone): after less gone may be 110, a
    /// quarter of the 40 over, and not 111.
    #[test]
    fn a_change_within_a_quarter_of_the_copied_body_is_the_estimates_noise() {
        assert!(stays(140, 40, 100, 40, 0), "no more");
        assert!(stays(150, 40, 100, 40, 0), "10 over is within");
        assert!(!stays(151, 40, 100, 40, 0), "11 over is not");
        assert!(stays(90, 0, 100, 40, 0), "smaller always stays");
    }
}
