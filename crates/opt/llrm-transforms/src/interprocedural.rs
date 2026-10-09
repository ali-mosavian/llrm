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
use llrm_analysis::manager::{Callbacks, GlobalsAA, ProgramSummaries, Summaries};
use llrm_analysis::memory::{Identity, MemoryKind, Slice, Unit};
use llrm_mir::callgraph::{CallGraph, CallGraphAnalysis, Defined};
use llrm_mir::context::GlobalId;
use llrm_mir::facts::{Fact, Facts};
use llrm_mir::memory::Effects;
use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Module};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::{CalleeEffects, Declarations, Declared, ModuleAnalyses, PreservedAnalyses};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};
use llrm_mir::types::Type;

use crate::inline;
use crate::profit::OperationCosts;

/// What each function does to memory, from the module's analyses: worked out
/// again only after a body changed, not for every query. `LLRM_CHECK_CALLEES=1`
/// compares it with a fresh scan each time.
fn callees(
    modules: &mut ModuleAnalyses,
    module: &Module,
) -> std::rc::Rc<llrm_mir::memory::Callees> {
    let held = modules.get::<CalleeEffects>(module);
    if checking() {
        assert!(
            *held == llrm_mir::memory::callees(module),
            "the callees' effects held by the module's analyses differ from a fresh scan"
        );
    }
    held
}

/// `module`'s declarations to declare into: the module's own `Declarations`,
/// nothing scanned unless a pass declares. `LLRM_CHECK_CALLEES=1` compares them
/// with a fresh listing.
fn declared(
    modules: &mut ModuleAnalyses,
    module: &Module,
) -> Declared {
    let held = modules.get::<Declarations>(module);
    if checking() {
        assert!(
            *held == module.declarations(),
            "the declarations held by the module's analyses differ from a fresh listing"
        );
    }
    Declared::over(held, module.metadata.len())
}

/// Places what `declared` made. The module has more functions than its
/// `Declarations` say now.
fn placed(
    declared: &mut Declared,
    modules: &mut ModuleAnalyses,
    module: &mut Module,
) -> Result<(), String> {
    if declared.place(module)? > 0 {
        modules.invalidate(&PreservedAnalyses::none());
    }
    Ok(())
}

fn checking() -> bool {
    static CHECKING: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CHECKING.get_or_init(|| llrm_support::env_set("LLRM_CHECK_CALLEES"))
}

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
    /// Inlining weighs code bytes, and what only the clocks admit stays where
    /// it comes to no more bytes, less what the clocks it saves buy at this
    /// many thousandths of a clock a byte; None weighs the clocks alone.
    pub rate: Option<i64>,
    /// Runs the pipeline over a copy of a callee that is in no module, for the
    /// constants a site passes: what its copy there would come to.
    pub specialise:
        Box<dyn FnMut(&mut Module, &mut ModuleAnalyses, llrm_mir::module::Function) -> llrm_mir::module::Function>,
    /// Whether what a body's callers pass is stated as a range on its
    /// parameters (gcc's `-fipa-vrp`, -O2 and up).
    pub ranges: bool,
}

impl ProgramPass for Interprocedural {
    fn name(&self) -> &'static str {
        "interprocedural"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let pipeline = &mut self.pipeline;
        let specialise = &mut self.specialise;
        let costs = if self.rate.is_some() { program.target.size_costs() } else { program.target.costs() };
        let clocks = program.target.costs();
        let loose = self.rate.map(|_| &clocks);
        let rate = self.rate.unwrap_or(0);
        // The op budget stays the clocks': what it bounds is the body, not its
        // price.
        let reach = program.target.costs().call;
        let roots = roots(program);
        let mut modules = managers(program, analyses);
        let proved = optimized_with::<String>(
            program,
            &mut modules,
            &roots,
            &costs,
            loose,
            rate,
            reach,
            self.inline,
            self.ranges,
            &mut |module, analyses, id, stage| {
                pipeline(module, analyses, id, stage);
                Ok(())
            },
            &mut |module, analyses, function| Ok(specialise(module, analyses, function)),
            &mut |_, _, _| Ok(()),
        )?;
        self.proved = Some(proved);
        analyses.invalidate();
        Ok(())
    }
}

/// What each body does to memory, stated on it before any body's pipeline runs,
/// as LLVM's PostOrderFunctionAttrs runs before the loop passes: a loop that
/// calls a function is judged on what it does, not on a declaration nothing yet
/// describes.
pub struct Stamp;

impl ProgramPass for Stamp {
    fn name(&self) -> &'static str {
        "stamp"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let mut modules = managers(program, analyses);
        stamped_all(program, &mut modules)?;
        analyses.invalidate();
        Ok(())
    }
}

/// Each module's analyses under `analyses`' program results, summaries
/// kept for the function passes to read.
pub fn managers(
    program: &Program,
    analyses: &mut ProgramAnalyses,
) -> Vec<ModuleAnalyses> {
    analyses.get::<ProgramSummaries>(program);
    (0..program.modules.len())
        .map(|at| {
            let mut one = ModuleAnalyses::new(analyses.proxy(program, at));
            one.require::<Summaries>();
            // As the first run's: a body sent back through reads what no code
            // outside reaches.
            one.require::<GlobalsAA>();
            one.require::<Callbacks>();
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
    program.modules.iter().enumerate().flat_map(|(at, module)| {
        module.functions().filter(|(_, _, function)| !function.is_declaration()).map(move |(id, _, _)| (at, id))
    })
}

/// Defined procedures of module `at`, in module order.
fn procedures(
    program: &Program,
    at: usize,
) -> Vec<GlobalId> {
    defined(program).filter(|&(one, _)| one == at).map(|(_, id)| id).collect()
}

/// Defined procedures no outside code calls: every caller is in the program,
/// and a call through a pointer is not one, so none has its address taken.
fn unexported(program: &Program) -> BTreeSet<Defined> {
    let addressed: BTreeSet<Defined> = program
        .modules
        .iter()
        .enumerate()
        .flat_map(|(at, module)| {
            llrm_mir::callgraph::addressed(module).into_iter().filter_map(move |id| program.definition(at, id))
        })
        .collect();
    defined(program)
        .filter(|&one @ (at, id)| {
            !program.exports.exported(program.modules[at].global(id)) && !addressed.contains(&one)
        })
        .collect()
}

/// The calls to inline: those `costs` admits and, tuned for size, those `loose`
/// (the clocks) does too, which the estimate then checks against what the copy
/// comes to once its constants are known: a body that folds on known actuals is
/// nothing the byte price can see.
fn candidates(
    module: &Module,
    callees: &llrm_mir::memory::Callees,
    layout: &llrm_mir::datalayout::DataLayout,
    counts: &inline::Counter,
    private: &BTreeSet<GlobalId>,
    costs: &OperationCosts,
    loose: Option<&OperationCosts>,
    reach: i64,
    threshold: inline::Threshold,
) -> (
    llrm_support::hash::IndexMap<GlobalId, inline::Candidate>,
    llrm_support::hash::IndexMap<GlobalId, inline::Candidate>,
) {
    let mut found = inline::candidates(module, callees, layout, counts, private, costs, reach, threshold);
    // Held only to inline from, a body's price is what the trial finds, never
    // its size.
    found.retain(|id, _| module.global(*id).linkage != Linkage::AvailableExternally);
    let more: llrm_support::hash::IndexMap<GlobalId, inline::Candidate> =
        loose.map_or_else(Default::default, |loose| {
            inline::candidates(module, callees, layout, counts, private, loose, reach, threshold)
                .into_iter()
                .filter(|(id, _)| !found.contains_key(id))
                .collect()
        });
    // A callee at many sites is not tried: its copies add up, and a caller's
    // gain from one is no tolerance for so many (savegame.c's took 4x the
    // compile time; part.c's `_part_rand` at 35 sites came to 13% more bytes).
    // A body held only to inline from is small and meant to be copied
    // everywhere.
    let more = more
        .into_iter()
        .filter(|(id, _)| {
            counts.get(id).copied().unwrap_or(0) <= TRIED_SITES
                || module.global(*id).linkage == Linkage::AvailableExternally
        })
        .collect();
    (found, more)
}

fn constant_sites(
    module: &Module,
    callees: &llrm_mir::memory::Callees,
    layout: &llrm_mir::datalayout::DataLayout,
    recursive: &BTreeSet<GlobalId>,
    caller: &llrm_mir::module::Function,
    constants: &llrm_support::hash::IndexMap<llrm_mir::module::InstId, Vec<Option<llrm_mir::context::ConstantId>>>,
    costs: &OperationCosts,
    loose: Option<&OperationCosts>,
    reach: i64,
    threshold: inline::Threshold,
) -> (
    llrm_support::hash::IndexMap<llrm_mir::module::InstId, inline::Candidate>,
    llrm_support::hash::IndexMap<llrm_mir::module::InstId, inline::Candidate>,
) {
    let found = inline::constant_sites(module, callees, layout, recursive, caller, constants, costs, reach, threshold);
    let more = loose.map_or_else(Default::default, |loose| {
        inline::constant_sites(module, callees, layout, recursive, caller, constants, loose, reach, threshold)
            .into_iter()
            .filter(|(at, _)| !found.contains_key(at))
            .collect()
    });
    (found, more)
}

/// What of a body's size the byte estimate may be off by: 28% in the median
/// over QCport's 532 functions (calibration of `bytes_in_code`), taken as a
/// quarter. A change in bytes within it of the body copied is the estimate's
/// noise, and what the clocks admitted stays.
/// The most sites of a callee that are tried.
const TRIED_SITES: i64 = 4;

const ESTIMATE_ERROR: (i64, i64) = (1, 4);

/// Whether callers coming to `after` bytes, less `gone` for the callee that
/// goes, come to no more than `before`, within the estimate's error of the body
/// (`moved` bytes) that was copied.
fn stays(
    after: i64,
    gone: i64,
    before: i64,
    moved: i64,
    allowance: i64,
) -> bool {
    after - gone <= before + moved * ESTIMATE_ERROR.0 / ESTIMATE_ERROR.1 + allowance
}

/// The bytes `sites` calls of `callee` removed may add: what their overhead
/// saves in clocks, at `rate` clocks a byte (`clocks` prices the calls), where
/// 0 allows none.
fn allowance(
    module: &Module,
    callee: GlobalId,
    sites: i64,
    clocks: &OperationCosts,
    rate: i64,
) -> i64 {
    let arguments = module.global(callee).function().map_or(0, |body| module.signature(body.ty).1.len());
    if rate <= 0 { 0 } else { 1000 * sites * inline::call_overhead(clocks, arguments) / rate }
}

/// What a callee comes to once the constants a site passes are known and the
/// pipeline has run over it alone, by (callee, its body's state, the
/// constants): one fact for every site that passes them, and for any later
/// decision about a copy for them.
#[derive(Default)]
pub(crate) struct Specialisations {
    sizes: llrm_support::hash::HashMap<
        (GlobalId, llrm_mir::module::Mark, Vec<Option<llrm_mir::context::ConstantId>>),
        Option<i64>,
    >,
}

/// Runs the pipeline over a function that is in no module, in `module`'s
/// context: what a copy specialised for its constants comes to.
pub(crate) type Specialiser<'a, E> = &'a mut dyn FnMut(
    &mut Module,
    &mut ModuleAnalyses,
    llrm_mir::module::Function,
) -> Result<llrm_mir::module::Function, E>;

impl Specialisations {
    /// `callee`'s bytes (priced by `costs`) with `constants` for its
    /// parameters.
    fn bytes<E: From<String>>(
        &mut self,
        module: &mut Module,
        modules: &mut ModuleAnalyses,
        callee: GlobalId,
        constants: &[Option<llrm_mir::context::ConstantId>],
        costs: &OperationCosts,
        specialised: Specialiser<E>,
    ) -> Result<Option<i64>, E> {
        let Some(body) = module.global(callee).function() else { return Ok(None) };
        let key = (callee, body.mark(), constants.to_vec());
        if let Some(found) = self.sizes.get(&key) {
            return Ok(*found);
        }
        let mut copy = body.clone();
        for (&parameter, constant) in body.parameters().iter().zip(constants) {
            if let Some(constant) = constant {
                copy.replace_value(parameter, llrm_mir::module::Operand::Constant(*constant));
            }
        }
        let copy = specialised(module, modules, copy)?;
        let size = inline::size_of(module, &callees(modules, module), &copy, costs);
        self.sizes.insert(key, size);
        Ok(size)
    }
}

/// What a call to `callee` costs in `costs` that a copy of its body does not:
/// the call, each argument pushed, and the return the copy has no use for.
fn call_bytes(
    module: &Module,
    callee: GlobalId,
    costs: &OperationCosts,
) -> i64 {
    let arguments = module.global(callee).function().map_or(0, |body| module.signature(body.ty).1.len());
    inline::call_overhead(costs, arguments)
}

/// Each of `more`, the callees the clocks admit and the bytes do not, inlined
/// at every call of them where the callers, with each copy as the pipeline over
/// it alone leaves it for the constants its site passes, and the callee that
/// goes, come to no more than the estimate's error of the body copied. The
/// callers that took one are put through the pipeline once, after all of them
/// (a callee at a time, a caller with many of them ran it that many times).
/// Whether any did.
fn estimated_callees<E: From<String>>(
    module: &mut Module,
    modules: &mut ModuleAnalyses,
    layout: &llrm_mir::datalayout::DataLayout,
    private: &BTreeSet<GlobalId>,
    recursive: &BTreeSet<GlobalId>,
    bases: &llrm_support::hash::IndexMap<GlobalId, i64>,
    more: &llrm_support::hash::IndexMap<GlobalId, inline::Candidate>,
    counts: &inline::Counter,
    costs: &OperationCosts,
    credit: (&OperationCosts, i64),
    memo: &mut Specialisations,
    specialised: Specialiser<E>,
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
) -> Result<bool, E> {
    type Calls = Vec<(GlobalId, Vec<Option<llrm_mir::context::ConstantId>>)>;
    // Every direct call of each, in the callers that are not it: one scan.
    let mut calls: llrm_support::hash::IndexMap<GlobalId, Calls> = more.keys().map(|one| (*one, Vec::new())).collect();
    for (caller, _, body) in module.functions().filter(|(_, _, body)| !body.is_declaration()) {
        let mut constants = None;
        for (_, inst) in body.walk() {
            let Some(callee) = llrm_mir::memory::callee(&module.context, body, inst).filter(|one| *one != caller)
            else {
                continue;
            };
            let Some(list) = calls.get_mut(&callee) else { continue };
            let constants = constants
                .get_or_insert_with(|| llrm_analysis::interprocedural::current_call_constants(&module.context, body));
            list.push((caller, constants.get(&inst).cloned().unwrap_or_default()));
        }
    }
    let mut touched: BTreeSet<GlobalId> = BTreeSet::new();
    for (&callee, candidate) in more {
        let sites = counts.get(&callee).copied().unwrap_or(0);
        let calls = &calls[&callee];
        if sites == 0 || calls.is_empty() || recursive.contains(&callee) {
            continue;
        }
        let Some(own) = inline::size(module, &callees(modules, module), callee, costs) else { continue };
        let mut grown = 0;
        let mut priced = true;
        for (_, constants) in calls {
            match memo.bytes(module, modules, callee, constants, costs, specialised)? {
                Some(copy) => grown += copy - call_bytes(module, callee, costs),
                None => priced = false,
            }
        }
        if !priced {
            continue;
        }
        let gone = if private.contains(&callee) && calls.len() as i64 == sites { own } else { 0 };
        let allowed = allowance(module, callee, calls.len() as i64, credit.0, credit.1);
        let stays_now = stays(grown, gone, 0, own, allowed);
        llrm_support::debug!(
            "trial",
            "estimate callee {} sites {} grown {grown} gone {gone} moved {own} allowed {allowed} -> {}",
            module.global(callee).name.as_deref().unwrap_or("?"),
            calls.len(),
            if stays_now { "kept" } else { "refused" }
        );
        if !stays_now {
            continue;
        }
        let available = llrm_support::hash::IndexMap::from_iter([(callee, candidate.clone())]);
        for caller in calls.iter().map(|(caller, _)| *caller).collect::<BTreeSet<_>>() {
            let by = inline::Caller {
                layout,
                recursive: recursive.contains(&caller),
                base: bases.get(&caller).copied().unwrap_or(0),
            };
            loop {
                let mut declared = declared(modules, module);
                let (context, function) = function_mut(module, caller);
                let more =
                    inline::expanded(context, function, &by, &available, None, &mut declared).map_err(E::from)?;
                placed(&mut declared, modules, module).map_err(E::from)?;
                if !more {
                    break;
                }
                touched.insert(caller);
            }
        }
    }
    for &caller in &touched {
        modules.changed(caller);
        modules.invalidate(&PreservedAnalyses::none());
        reoptimised(module, modules, caller, "inline-estimate.")?;
    }
    Ok(!touched.is_empty())
}

/// The constant `sites` of `caller` the clocks admit and the bytes do not,
/// inlined where the copy, as the pipeline over it alone leaves it for the
/// constants its site passes, adds no more than the call it replaces and what
/// the clocks saved pay (the callee that goes with its last call counted).
/// Whether any was.
fn estimated_sites<E: From<String>>(
    module: &mut Module,
    modules: &mut ModuleAnalyses,
    layout: &llrm_mir::datalayout::DataLayout,
    private: &BTreeSet<GlobalId>,
    recursive: &BTreeSet<GlobalId>,
    bases: &llrm_support::hash::IndexMap<GlobalId, i64>,
    caller: GlobalId,
    sites: &llrm_support::hash::SparseIdMap<llrm_mir::module::InstId, inline::Candidate>,
    counts: &inline::Counter,
    costs: &OperationCosts,
    credit: (&OperationCosts, i64),
    memo: &mut Specialisations,
    specialised: Specialiser<E>,
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
) -> Result<(bool, llrm_support::hash::SparseIdMap<llrm_mir::module::InstId, inline::Candidate>), E> {
    let mut chosen: llrm_support::hash::SparseIdMap<llrm_mir::module::InstId, inline::Candidate> = Default::default();
    let mut left: llrm_support::hash::SparseIdMap<llrm_mir::module::InstId, inline::Candidate> = Default::default();
    let constants = {
        let body = module.global(caller).function().expect("a procedure");
        llrm_analysis::interprocedural::current_call_constants(&module.context, body)
    };
    for (&site, candidate) in sites {
        let body = module.global(caller).function().expect("a procedure");
        let Some(callee) = llrm_mir::memory::callee(&module.context, body, site) else { continue };
        let known = constants.get(&site).cloned().unwrap_or_default();
        let Some(copy) = memo.bytes(module, modules, callee, &known, costs, specialised)? else { continue };
        let own = inline::size(module, &callees(modules, module), callee, costs).unwrap_or(0);
        let gone = if private.contains(&callee) && counts.get(&callee).copied() == Some(1) { own } else { 0 };
        let bought = allowance(module, callee, 1, credit.0, credit.1);
        let stays_now = copy - call_bytes(module, callee, costs) - gone <= bought;
        llrm_support::debug!(
            "trial",
            "estimate site caller {} callee {} copy {copy} gone {gone} bought {bought} -> {}",
            module.global(caller).name.as_deref().unwrap_or("?"),
            module.global(callee).name.as_deref().unwrap_or("?"),
            if stays_now { "kept" } else { "refused" }
        );
        if stays_now {
            chosen.insert(site, candidate.clone());
        } else {
            left.insert(site, candidate.clone());
        }
    }
    if chosen.is_empty() {
        return Ok((false, left));
    }
    let by = inline::Caller {
        layout,
        recursive: recursive.contains(&caller),
        base: bases.get(&caller).copied().unwrap_or(0),
    };
    let mut spliced = false;
    loop {
        let mut declared = declared(modules, module);
        let (context, function) = function_mut(module, caller);
        let more = inline::expanded(context, function, &by, &Default::default(), Some(&chosen), &mut declared)
            .map_err(E::from)?;
        placed(&mut declared, modules, module).map_err(E::from)?;
        if !more {
            break;
        }
        spliced = true;
    }
    if spliced {
        modules.changed(caller);
        modules.invalidate(&PreservedAnalyses::none());
        reoptimised(module, modules, caller, "inline-estimate.")?;
    }
    Ok((spliced, left))
}

/// The sites of `caller` the estimates refused, spliced together, the caller
/// put through the pipeline once, and kept where it comes to no more (`stays`,
/// as the callee trials judged). What a site alone shows no gain for can pay
/// beside its neighbours: one call's result folding another's (B$FMID into
/// B$FASC), the memory the caller has just stored to, the code a copy shrinks
/// into. The pipeline over the caller costs the same whichever sites it holds,
/// so one trial a caller a round is the caller's size, where a trial of each
/// site was its size by the sites. A set that does not pay is not tried again
/// in that body (mdl_ai.c re-ran its pipeline some 600 times for 20 sites).
/// Whether the sites were spliced.
fn together_trial<E: From<String>>(
    module: &mut Module,
    modules: &mut ModuleAnalyses,
    layout: &llrm_mir::datalayout::DataLayout,
    private: &BTreeSet<GlobalId>,
    recursive: &BTreeSet<GlobalId>,
    bases: &llrm_support::hash::IndexMap<GlobalId, i64>,
    caller: GlobalId,
    sites: &llrm_support::hash::SparseIdMap<llrm_mir::module::InstId, inline::Candidate>,
    counts: &inline::Counter,
    costs: &OperationCosts,
    credit: (&OperationCosts, i64),
    refused: &mut BTreeSet<(GlobalId, llrm_mir::module::InstId)>,
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
) -> Result<bool, E> {
    let chosen: llrm_support::hash::IndexMap<_, _> =
        sites.iter().filter(|(site, _)| !refused.contains(&(caller, **site))).map(|(s, c)| (*s, c.clone())).collect();
    if chosen.is_empty() {
        return Ok(false);
    }
    let kept = module.global(caller).function().expect("a procedure").clone();
    let before = inline::size(module, &callees(modules, module), caller, costs);
    let by = inline::Caller {
        layout,
        recursive: recursive.contains(&caller),
        base: bases.get(&caller).copied().unwrap_or(0),
    };
    let mut spliced = false;
    loop {
        let mut declared = declared(modules, module);
        let (context, function) = function_mut(module, caller);
        let more = inline::expanded(context, function, &by, &Default::default(), Some(&chosen), &mut declared)
            .map_err(E::from)?;
        placed(&mut declared, modules, module).map_err(E::from)?;
        if !more {
            break;
        }
        spliced = true;
    }
    if !spliced {
        refused.extend(chosen.keys().map(|site| (caller, *site)));
        return Ok(false);
    }
    modules.changed(caller);
    modules.invalidate(&PreservedAnalyses::none());
    reoptimised(module, modules, caller, "inline-trial.")?;
    let callees_of: BTreeSet<GlobalId> =
        chosen.keys().filter_map(|site| llrm_mir::memory::callee(&module.context, &kept, *site)).collect();
    // The calls of each in the module after: what they were, less this caller's
    // before and plus its now (a scan of the one body, not of the module).
    let in_body = |function: &llrm_mir::module::Function, callee: GlobalId| {
        function
            .walk()
            .filter(|&(_, inst)| llrm_mir::memory::callee(&module.context, function, inst) == Some(callee))
            .count() as i64
    };
    let held = callees(modules, module);
    let (mut moved, mut gone, mut allowed) = (0, 0, 0);
    for callee in callees_of {
        let own = inline::size(module, &held, callee, costs).unwrap_or(0);
        moved += own;
        let was = counts.get(&callee).copied().unwrap_or(0);
        let left =
            was - in_body(&kept, callee) + in_body(module.global(caller).function().expect("a procedure"), callee);
        if private.contains(&callee) && was > 0 && left == 0 {
            gone += own;
        }
        allowed += allowance(module, callee, was - left, credit.0, credit.1);
    }
    let after = inline::size(module, &held, caller, costs);
    let stays_now = before.zip(after).is_some_and(|(before, after)| stays(after, gone, before, moved, allowed));
    llrm_support::debug!(
        "trial",
        "together caller {} sites {} before {before:?} after {after:?} gone {gone} moved {moved} allowed {allowed} -> {}",
        module.global(caller).name.as_deref().unwrap_or("?"),
        chosen.len(),
        if stays_now { "kept" } else { "refused" }
    );
    if stays_now {
        return Ok(true);
    }
    *function_mut(module, caller).1 = kept;
    modules.changed(caller);
    modules.invalidate(&PreservedAnalyses::none());
    refused.extend(chosen.keys().map(|site| (caller, *site)));
    Ok(false)
}

/// Module `at`'s procedures every caller of which is in the module: no
/// outside code, and no other module, calls them.
fn private(
    program: &Program,
    at: usize,
) -> BTreeSet<GlobalId> {
    let others: BTreeSet<Defined> = program
        .modules
        .iter()
        .enumerate()
        .filter(|&(other, _)| other != at)
        .flat_map(|(other, module)| {
            (0..module.globals.len() as u32).filter_map(move |id| program.definition(other, GlobalId(id)))
        })
        .collect();
    unexported(program)
        .into_iter()
        .filter(|&(one, id)| one == at && !others.contains(&(at, id)))
        .map(|(_, id)| id)
        .collect()
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
    specialised: Specialiser<E>,
    spliced: &mut dyn FnMut(&Module, GlobalId, &str) -> Result<(), E>,
) -> Result<Proved, E> {
    optimized_with::<E>(
        program,
        modules,
        roots,
        costs,
        loose,
        rate,
        reach,
        threshold,
        true,
        reoptimised,
        specialised,
        spliced,
    )
}

pub fn optimized_with<E: From<String>>(
    program: &mut Program,
    modules: &mut [ModuleAnalyses],
    roots: &BTreeSet<Defined>,
    costs: &OperationCosts,
    loose: Option<&OperationCosts>,
    rate: i64,
    reach: i64,
    threshold: inline::Threshold,
    ranges: bool,
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
    specialised: Specialiser<E>,
    spliced: &mut dyn FnMut(&Module, GlobalId, &str) -> Result<(), E>,
) -> Result<Proved, E> {
    let count = program.modules.len();
    let mut procedures: Vec<Vec<GlobalId>> = (0..count).map(|at| procedures(program, at)).collect();
    let mut private: Vec<BTreeSet<GlobalId>> = (0..count).map(|at| private(program, at)).collect();
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
    // How large each body is before anything is inlined into it: what its
    // growth is measured against.
    let mut bases: Vec<llrm_support::hash::IndexMap<GlobalId, i64>> = (0..count)
        .map(|at| {
            procedures[at]
                .iter()
                .filter_map(|&id| Some((id, inline::operations(program.modules[at].global(id).function()?))))
                .collect()
        })
        .collect();
    let mut specialisations = Specialisations::default();
    let mut refused_together: BTreeSet<(GlobalId, llrm_mir::module::InstId)> = BTreeSet::new();
    let mut inline_round = 0;
    loop {
        let mut changed = false;
        for at in 0..count {
            let module = &mut program.modules[at];
            let counts = inline::call_counts(module);
            let (available, more) = candidates(
                module,
                &callees(&mut modules[at], module),
                &program.layout,
                &counts,
                &private[at],
                costs,
                loose,
                reach,
                threshold,
            );
            let recursive = inline::recursive(module);
            // What only the clocks admit of a callee stays where its copies,
            // as the pipeline over each alone leaves them, come to no more.
            if loose.is_some()
                && estimated_callees(
                    module,
                    &mut modules[at],
                    &program.layout,
                    &private[at],
                    &recursive,
                    &bases[at],
                    &more,
                    &counts,
                    costs,
                    (loose.unwrap_or(costs), rate),
                    &mut specialisations,
                    specialised,
                    reoptimised,
                )?
            {
                changed = true;
            }
            for &id in &procedures[at] {
                let caller = module.global(id).function().expect("a procedure");
                let constants = facts::current_call_constants(&module.context, caller);
                let (constant, constant_more) = constant_sites(
                    module,
                    &callees(&mut modules[at], module),
                    &program.layout,
                    &recursive,
                    caller,
                    &constants,
                    costs,
                    loose,
                    reach,
                    threshold,
                );
                let mut declared = declared(&mut modules[at], module);
                let (context, function) = function_mut(module, id);
                let by = inline::Caller {
                    layout: &program.layout,
                    recursive: recursive.contains(&id),
                    base: bases[at].get(&id).copied().unwrap_or(0),
                };
                let spliced_now = inline::expanded(context, function, &by, &available, Some(&constant), &mut declared)?;
                placed(&mut declared, &mut modules[at], module)?;
                if spliced_now {
                    edited(&mut modules[at], &[id]);
                    let stage = format!("inline{inline_round}");
                    spliced(module, id, &stage)?;
                    reoptimised(module, &mut modules[at], id, &format!("{stage}."))?;
                    changed = true;
                    inline_round += 1;
                }
                // What only the clocks admit stays where the copy, with the
                // constants its site passes, comes to no more;
                // and what that refuses is tried with its
                // neighbours, together, once.
                if loose.is_some() {
                    let (stayed, mut together) = estimated_sites(
                        module,
                        &mut modules[at],
                        &program.layout,
                        &private[at],
                        &recursive,
                        &bases[at],
                        id,
                        &constant_more,
                        &counts,
                        costs,
                        (loose.unwrap_or(costs), rate),
                        &mut specialisations,
                        specialised,
                        reoptimised,
                    )?;
                    if stayed {
                        changed = true;
                    } else {
                        let body = module.global(id).function().expect("a procedure");
                        for (_, inst) in body.walk() {
                            if let Some(candidate) = llrm_mir::memory::callee(&module.context, body, inst)
                                .and_then(|callee| more.get(&callee))
                            {
                                together.entry(inst).or_insert_with(|| candidate.clone());
                            }
                        }
                        if together_trial(
                            module,
                            &mut modules[at],
                            &program.layout,
                            &private[at],
                            &recursive,
                            &bases[at],
                            id,
                            &together,
                            &counts,
                            costs,
                            (loose.unwrap_or(costs), rate),
                            &mut refused_together,
                            reoptimised,
                        )? {
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    // A pointer a body only reads through is given as the fields it reads,
    // before what its callers pass is propagated: a length or a segment now
    // crosses the call as a value.
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

    // A far pointer every call fills from DGROUP is passed as its offset.
    for at in 0..count {
        for id in crate::narrowspace::narrowed(&mut program.modules[at], &program.layout, program.target.spaces()) {
            edited(&mut modules[at], &[id]);
            reoptimised(&mut program.modules[at], &mut modules[at], id, "narrow.")?;
        }
    }

    let mut return_round = 0;

    // Materialize every newly constant result.
    let propagate_constant_returns =
        |procedures: &[Vec<GlobalId>],
         program: &mut Program,
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
    propagate_constant_returns(&procedures, program, modules, &mut return_round, reoptimised)?;
    let mut argument_round = 0;
    let mut cloning = crate::ipacp::Cloning::default();
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
            propagate_constant_returns(&procedures, program, modules, &mut return_round, reoptimised)?;
        }

        // A function called with a constant is copied for it (gcc's ipa-cp),
        // and the calls go to the copy.
        let mut cloned_now = false;
        for at in 0..count {
            let made = crate::ipacp::cloned(
                &mut program.modules[at],
                &program.layout,
                &procedures[at],
                &private[at],
                costs,
                threshold.cp_clone,
                &mut cloning,
            );
            for &id in &made.added {
                let size = inline::operations(program.modules[at].global(id).function().expect("a procedure"));
                procedures[at].push(id);
                private[at].insert(id);
                bases[at].insert(id, size);
            }
            for &id in made.added.iter().chain(&made.edited) {
                edited(&mut modules[at], &[id]);
                reoptimised(&mut program.modules[at], &mut modules[at], id, "ipa-cp.")?;
                cloned_now = true;
            }
        }
        if cloned_now {
            propagate_constant_returns(&procedures, program, modules, &mut return_round, reoptimised)?;
            changed = true;
        }

        // A single current-MIR constant may be worth cloning even where
        // another caller keeps the private body dynamic.
        let mut inlined = false;
        for at in 0..count {
            let module = &mut program.modules[at];
            let counts = inline::call_counts(module);
            let (available, more) = candidates(
                module,
                &callees(&mut modules[at], module),
                &program.layout,
                &counts,
                &private[at],
                costs,
                loose,
                reach,
                threshold,
            );
            let recursive = inline::recursive(module);
            // What only the clocks admit of a callee stays where its copies,
            // as the pipeline over each alone leaves them, come to no more.
            if loose.is_some()
                && estimated_callees(
                    module,
                    &mut modules[at],
                    &program.layout,
                    &private[at],
                    &recursive,
                    &bases[at],
                    &more,
                    &counts,
                    costs,
                    (loose.unwrap_or(costs), rate),
                    &mut specialisations,
                    specialised,
                    reoptimised,
                )?
            {
                inlined = true;
            }
            for &id in &procedures[at] {
                let caller = module.global(id).function().expect("a procedure");
                let current = facts::current_call_constants(&module.context, caller);
                let (constant, constant_more) = constant_sites(
                    module,
                    &callees(&mut modules[at], module),
                    &program.layout,
                    &recursive,
                    caller,
                    &current,
                    costs,
                    loose,
                    reach,
                    threshold,
                );
                let mut declared = declared(&mut modules[at], module);
                let (context, function) = function_mut(module, id);
                let by = inline::Caller {
                    layout: &program.layout,
                    recursive: recursive.contains(&id),
                    base: bases[at].get(&id).copied().unwrap_or(0),
                };
                let spliced_now = inline::expanded(context, function, &by, &available, Some(&constant), &mut declared)?;
                placed(&mut declared, &mut modules[at], module)?;
                if spliced_now {
                    edited(&mut modules[at], &[id]);
                    reoptimised(module, &mut modules[at], id, &format!("ipa-inline{argument_round}."))?;
                    inlined = true;
                }
                // What only the clocks admit stays where the copy, with the
                // constants its site passes, comes to no more;
                // and what that refuses is tried with its
                // neighbours, together, once.
                if loose.is_some() {
                    let (stayed, mut together) = estimated_sites(
                        module,
                        &mut modules[at],
                        &program.layout,
                        &private[at],
                        &recursive,
                        &bases[at],
                        id,
                        &constant_more,
                        &counts,
                        costs,
                        (loose.unwrap_or(costs), rate),
                        &mut specialisations,
                        specialised,
                        reoptimised,
                    )?;
                    if stayed {
                        inlined = true;
                    } else {
                        let body = module.global(id).function().expect("a procedure");
                        for (_, inst) in body.walk() {
                            if let Some(candidate) = llrm_mir::memory::callee(&module.context, body, inst)
                                .and_then(|callee| more.get(&callee))
                            {
                                together.entry(inst).or_insert_with(|| candidate.clone());
                            }
                        }
                        if together_trial(
                            module,
                            &mut modules[at],
                            &program.layout,
                            &private[at],
                            &recursive,
                            &bases[at],
                            id,
                            &together,
                            &counts,
                            costs,
                            (loose.unwrap_or(costs), rate),
                            &mut refused_together,
                            reoptimised,
                        )? {
                            inlined = true;
                        }
                    }
                }
            }
        }
        if inlined {
            propagate_constant_returns(&procedures, program, modules, &mut return_round, reoptimised)?;
        }
        if !changed && !inlined {
            break;
        }
    }
    // What its callers pass bounds each parameter of a body only they call:
    // stated as a range, which the body's own proofs then read.
    for round in 0..if ranges { 4 } else { 0 } {
        let stamped = llrm_analysis::parameter_ranges::stamp(program, &unexported);
        if stamped.is_empty() {
            break;
        }
        for (at, id) in stamped {
            edited(&mut modules[at], &[id]);
            reoptimised(&mut program.modules[at], &mut modules[at], id, &format!("ipa-range{round}."))?;
        }
    }
    // What the constants and ranges left unread is not pushed.
    for at in 0..count {
        for id in crate::deadargs::removed(&mut program.modules[at]) {
            edited(&mut modules[at], &[id]);
            // A body with fewer values to carry is another body to the loop
            // passes and the recursion's.
            reoptimised(&mut program.modules[at], &mut modules[at], id, "ipa-deadargs.")?;
        }
    }
    // GCC's recursive inlining: a function that calls itself is given copies of
    // itself (`inline::inlined_into_itself`), as -finline-functions does,
    // so not at -O1's none or -Os (the recursive call is cold there). After the
    // parameters nothing reads are gone and the tail calls are loops, as GCC's
    // early passes have made them: a body with two calls would be a tree,
    // not a chain.
    if let Some(budget) = threshold.budget(reach).filter(|_| !threshold.single) {
        for at in 0..count {
            for &id in &procedures[at] {
                let module = &mut program.modules[at];
                let Some(original) = module.global(id).function().cloned() else { continue };
                if !original
                    .walk()
                    .any(|(_, inst)| llrm_mir::memory::callee(&module.context, &original, inst) == Some(id))
                {
                    continue;
                }
                let mut work = original.clone();
                let (metadata, globals) =
                    (module.metadata.clone(), module.globals.iter().map(GlobalValue::declaration).collect::<Vec<_>>());
                let made = inline::inlined_into_itself(
                    id,
                    &mut work,
                    &original,
                    budget,
                    &|context, function| {
                        crate::profit::_frequencies(context, &metadata, &globals, function, None).unwrap_or_default()
                    },
                    &mut module.context,
                );
                if made == 0 {
                    continue;
                }
                *function_mut(module, id).1 = work;
                edited(&mut modules[at], &[id]);
                reoptimised(&mut program.modules[at], &mut modules[at], id, "ipa-recursive.")?;
            }
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
        let noreturn = facts::program_noreturn(
            program,
            &declarations.iter().map(|one| one.as_slice()).collect::<Vec<_>>(),
            &unexported,
        );
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
pub fn stamped_all(
    program: &mut Program,
    modules: &mut [ModuleAnalyses],
) -> Result<(), String> {
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
fn published(
    program: &mut Program,
    at: usize,
    id: GlobalId,
) -> Vec<usize> {
    let mut changed = Vec::new();
    for other in (0..program.modules.len()).filter(|&other| other != at) {
        let declared: Vec<GlobalId> = (0..program.modules[other].globals.len() as u32)
            .map(GlobalId)
            .filter(|&one| program.definition(other, one) == Some((at, id)))
            .collect();
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
///   states it, or where the language promises each loop ends (`mustprogress`
///   on the function, `llvm.loop.mustprogress` on each loop) of a function that
///   does nothing observable;
/// - `norecurse` where nothing can enter it again while it runs; `nounwind`
///   where every call states it and no access can fault
///   (`interprocedural::cannot_fault`).
///
/// Any other attribute already stated stays. The bodies stamped; the
/// caller drops `analyses` when there are any.
pub fn stamped(
    module: &mut Module,
    analyses: &mut ModuleAnalyses,
) -> Result<Vec<GlobalId>, String> {
    let program = std::rc::Rc::clone(analyses.program());
    let layout = &program.layout;
    let known = analyses.get::<Summaries>(module);
    let known = Result::as_ref(&*known).map_err(String::clone)?;
    let globals = analyses.get::<GlobalsAA>(module);
    let globals = Result::as_ref(&*globals).map_err(String::clone)?;
    let callbacks = analyses.get::<Callbacks>(module);
    let callbacks = Result::as_ref(&*callbacks).map_err(String::clone)?;
    let mut declarations = (*analyses.get::<Declarations>(module)).clone();
    let mut changed = Vec::new();
    let graph = analyses.get::<CallGraphAnalysis>(module);
    let callees = llrm_mir::memory::callees(module);
    for id in graph.bottom_up() {
        let global = module.global(id);
        let exact = matches!(
            global.linkage,
            Linkage::External | Linkage::Internal | Linkage::Private
        );
        let (Some(name), Some(function), true) = (global.name.as_ref(), global.function(), exact) else { continue };
        let Some(summary) = known.get(name) else { continue };
        let shape = analyses.function::<Shape>(module, id);
        let exposed = llrm_analysis::memory::exposed_frames(
            &Unit::of(module, layout, function).with_spaces(program.target.spaces()),
        );
        let procedure = Procedure::of(
            Unit { program: Some(&program), ..Unit::of(module, layout, function) }
                .with_globals_aa(globals)
                .with_shape(&shape)
                .with_exposed(&exposed)
                .with_callbacks(callbacks),
        );
        let initialized = alias::initialized(&procedure, known)?;
        let calls = function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|&inst| matches!(
                function.instruction(inst).opcode,
                Opcode::Call(_) | Opcode::Invoke(_)
            ))
            .collect::<Vec<_>>();
        let states = |fact: Fact| {
            calls.iter().all(|&inst| effects::states(&module.context, &declarations, function, inst, fact))
        };
        let volatile = function
            .walk()
            .any(
                |(_, inst)| matches!(
                    function.instruction(inst).opcode,
                    Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }
                ),
            );
        let norecurse = graph.cannot_reenter(module, id);
        // The language's word that its loops end holds where nothing a loop
        // that never ended could be seen by.
        let unobserved = !volatile
            && calls.iter().all(|&inst| !llrm_mir::memory::of(&module.context, &callees, function, inst).writes);
        let promised = norecurse
            && unobserved
            && states(Fact::WillReturn)
            && llrm_mir::loops::ends_by_promise(&module.metadata, function, Facts::of(&function.attrs).must_progress());
        let counted = || {
            let shape = Shape::of(function);
            // Found once for the function, not for each loop: the proofs of
            // every loop ask the same.
            let registers = llrm_analysis::consts::known(
                &unit_of(module, layout, function).with_spaces(program.target.spaces()),
                None,
                None,
                None,
            );
            let proofs = |one| {
                llrm_analysis::induction::counted(
                    &unit_of(module, layout, function)
                        .with_spaces(program.target.spaces())
                        .with_registers(&registers)
                        .with_shape(&shape),
                    one,
                    Some(&registers),
                    false,
                )
            };
            shape
                .loops
                .iter()
                .all(
                    |one| proofs(one).iter().any(|proof| {
                        !proof.stops
                            && (proof.count.is_some() || proof.step.magnitude() == &num_bigint::BigUint::from(1_u8))
                    }),
                )
        };
        let returns = ((facts::returns_without_looping(function) || counted()) && states(Fact::WillReturn)) || promised;
        let nounwind = states(Fact::NoUnwind) && facts::cannot_fault(module, layout, program.target.spaces(), function);
        let mut hidden = if volatile { Effects::ANY } else { Effects::NONE };
        for &inst in &calls {
            let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { continue };
            let declared = llrm_mir::memory::callee(&module.context, function, inst)
                .and_then(|one| declarations.get(one.0 as usize))
                .and_then(GlobalValue::function)
                .map_or(Effects::ANY, |one| llrm_mir::memory::inaccessible(&one.attrs));
            hidden = _either(hidden, _both(declared, llrm_mir::memory::inaccessible(&info.attrs)));
        }
        let pointers = function
            .parameters()
            .iter()
            .map(|&one| matches!(
                module.context.types.get(function.value(one).ty),
                Type::Pointer(_)
            ))
            .collect::<Vec<_>>();
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
            let through = |slices: &BTreeSet<Slice>| {
                slices
                    .iter()
                    .any(
                        |one| one.object.kind == MemoryKind::Parameter
                            && one.object.key == llrm_analysis::memory::Key::Int(index as i64),
                    )
            };
            let (reads, writes) =
                (through(&summary.reads) || summary.unknown_read, through(&summary.writes) || summary.unknown_write);
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

fn unit_of<'a>(
    module: &'a Module,
    layout: &'a llrm_mir::datalayout::DataLayout,
    function: &'a llrm_mir::module::Function,
) -> Unit<'a> {
    Unit::of(module, layout, function)
}

fn _both(
    one: Effects,
    other: Effects,
) -> Effects {
    Effects { reads: one.reads && other.reads, writes: one.writes && other.writes }
}

fn _either(
    one: Effects,
    other: Effects,
) -> Effects {
    Effects { reads: one.reads || other.reads, writes: one.writes || other.writes }
}

/// `attrs`' `memory(...)` narrowed to what `summary` reads and writes
/// through the pointer parameters and elsewhere, its own frame aside, and
/// to `hidden` on inaccessible memory.
fn _narrowed(
    attrs: &mut Vec<Attribute>,
    summary: &Summary,
    hidden: Effects,
) {
    let local = |one: &Slice| matches!(one.object.kind, MemoryKind::Frame | MemoryKind::Stack);
    let found = |slices: &BTreeSet<Slice>, parameter: bool| {
        slices.iter().filter(|one| !local(one)).any(|one| (one.object.kind == MemoryKind::Parameter) == parameter)
    };
    let (stated_arguments, stated_other) = llrm_mir::memory::located(attrs);
    let arguments =
        _both(stated_arguments, Effects { reads: found(&summary.reads, true), writes: found(&summary.writes, true) });
    let other = _both(
        stated_other,
        Effects {
            reads: found(&summary.reads, false) || summary.unknown_read,
            writes: found(&summary.writes, false) || summary.unknown_write,
        },
    );
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
    let named = [("argmem", arguments), ("inaccessiblemem", hidden)]
        .into_iter()
        .filter(|(_, one)| *one != other)
        .map(|(location, one)| (Some(location.to_owned()), access(one)))
        .collect::<Vec<_>>();
    let mut locations = Vec::new();
    if other != Effects::NONE || named.is_empty() {
        locations.push((None, access(other)));
    }
    locations.extend(named);
    // What `readnone`, `readonly` or `writeonly` said, it now says.
    attrs.retain(|one| match one {
        Attribute::Memory(_) => false,
        // Said again by the `memory` attribute that replaces them.
        other => !matches!(
            Fact::of_attribute(other),
            Some(Fact::ReadNone | Fact::ReadOnly | Fact::WriteOnly)
        ),
    });
    attrs.push(Attribute::Memory(locations));
}

/// The procedure `id`, with the context its types and constants live in.
pub(crate) fn function_mut(
    module: &mut Module,
    id: GlobalId,
) -> (&mut llrm_mir::context::Context, &mut llrm_mir::module::Function) {
    let Module { context, globals, .. } = module;
    match &mut globals[id.0 as usize].kind {
        GlobalKind::Function(function) => (context, function),
        GlobalKind::Variable(_) => unreachable!("a procedure is a function"),
    }
}

/// Procedures a surviving direct call reaches from `roots`; all of them
/// when there are no roots.
fn reachable(
    program: &Program,
    roots: &BTreeSet<Defined>,
) -> BTreeSet<Defined> {
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
                .filter(|&(_, inst)| matches!(
                    function.instruction(inst).opcode,
                    Opcode::Call(_) | Opcode::Invoke(_)
                ))
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

    /// Callers of 100 bytes before, a callee of 40 copied and then going (40
    /// gone): after less gone may be 110, a quarter of the 40 over, and not
    /// 111.
    #[test]
    fn a_change_within_a_quarter_of_the_copied_body_is_the_estimates_noise() {
        assert!(stays(140, 40, 100, 40, 0), "no more");
        assert!(stays(150, 40, 100, 40, 0), "10 over is within");
        assert!(!stays(151, 40, 100, 40, 0), "11 over is not");
        assert!(stays(90, 0, 100, 40, 0), "smaller always stays");
    }
}
