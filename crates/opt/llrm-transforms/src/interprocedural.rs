//! The whole-program step run once every body has reached its own fixed point:
//! states what each body does, inlines, carries constants across direct calls,
//! drops dead pure calls and the tails of calls that never return, and sends
//! each changed body back through its pipeline. What one body proves about its
//! callees or callers is invisible to a pass that sees one body; this step
//! carries it across the calls.
//!
//! # How it works
//! A frontend hands in its pipeline; nothing here names a language or a
//! machine. A body changes in place and is reported with
//! `ModuleAnalyses::changed`, so the module analyses are updated for the
//! bodies that differ rather than recomputed
//! (`docs/optimizations/module-analyses.md`). A procedure has every caller in
//! the program when the program does not export it (`unexported`); the roots
//! are the exported bodies. Each stage re-runs the pipeline over the bodies it
//! changed under a stage name (`LLRM_DEBUG=runs` and `--dump` use it). A pass
//! that found nothing on a body starts again only where what it read has
//! changed ([`crate::settled`]); `ipa-cp.` and `ipa-deadargs.` vouch for their
//! edits of a caller that are no more than a call's new name or one argument
//! fewer.
//!
//! The stages, in order:
//!
//! 1. `stamped_all` (the `stamp` slot, before the walk): each exactly defined
//!    body is stamped with what is proved of it, callees first: `memory(...)`,
//!    `nocapture`/`readonly`/`readnone`/ `writeonly`/`initializes` on pointer
//!    parameters, `willreturn`, `norecurse`, `nounwind`. A stated attribute is
//!    only narrowed. The same stamp is stated on the declarations of the body
//!    in other modules.
//! 2. The private bodies nothing reaches any more, the ones the walk
//!    ([`crate::walk::Walk`]) inlined into every caller, lose their bodies
//!    (`bare_the_unreached`). Inlining is the walk's alone.
//! 3. `promote.` ([`crate::argpromotion`]), `narrow.` ([`crate::narrowspace`]),
//!    `globalsra.` ([`crate::globalsra`]): a pointer a body only reads is
//!    passed as the fields it reads; a far pointer every call fills from DGROUP
//!    is passed as its offset.
//! 4. `ipa<N>.`: a body whose every `ret` returns one integer has its calls'
//!    results replaced by it (`llrm_analysis::interprocedural`); the call
//!    stays.
//! 5. A loop of `ipa-cp.` ([`crate::ipacp`] cloning) and return propagation,
//!    until nothing changes. What every caller passes is IPSCCP's, before the
//!    walk ([`crate::walk::ModuleIpo`]); the pin does not run it again here, but
//!    it runs once more after `promote.` changed a body (DEVIATE, below it).
//! 6. `ipa-deadargs.` ([`crate::deadargs`]): parameters nothing reads go.
//! 7. `ipa-pure.` removes unused calls to bodies proved pure and terminating,
//!    and `ipa-noreturn.` cuts the code after a call to a private body every
//!    path of which stops, keeping the call.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef note fill:transparent,stroke:transparent,color:#64748b
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   st1["<b>stamp</b>"]:::mid
//!   inl["<b>inline*</b>"]:::mid
//!   pn["<b>promote/narrow</b>"]:::mid
//!   rc["<b>ret consts</b>"]:::mid
//!   lp["<b>cp*</b>"]:::mid
//!   da["<b>deadargs</b>"]:::mid
//!   st2["<b>stamp</b>"]:::mid
//!   pu["<b>pure</b>"]:::mid
//!   nr["<b>noreturn</b>"]:::mid
//!   re["each changed body re-enters<br/>the per-body pipeline"]:::note
//!   st1 --> inl
//!   inl --> pn
//!   pn --> rc
//!   rc --> lp
//!   lp --> da
//!   da --> st2
//!   st2 --> pu
//!   pu --> nr
//!   nr -.-> re
//! ```
//!
//! *Interprocedural stages, from bodies at their fixed points*
//!
//! The step returns `Proved`: the bodies that cannot return, and the ones a
//! root still reaches.
//!
//! # Example
//! The stage `ipa-noreturn.` on a call to a body every path of which stops.
//! Before:
//!
//! ```mir
//! define internal void @spin() {
//! b:
//!   br label %l
//!
//! l:
//!   br label %l
//! }
//!
//! define i16 @f(i16 %a) {
//! b:
//!   call void @spin()
//!   %s = add i16 %a, 1
//!   ret i16 %s
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define internal void @spin() memory(none) nounwind norecurse {
//! b:
//!   br label %l
//!
//! l:
//!   br label %l
//! }
//!
//! define i16 @f(i16 %a) memory(none) nounwind norecurse {
//! b:
//!   call void @spin()
//!   unreachable
//! }
//! ```
//!
//! The stamp states each body's attributes, and the code after the call that
//! never returns is cut; the call stays. `@f` is run through its pipeline again
//! as `ipa-noreturn.`.
//!
//! From unit test `test_a_private_terminal_body_cuts_its_callers_tail` in
//! `interprocedural_tests.rs`.
//!
//! # Runs
//! A `ProgramPass` after the per-body fixed point and before `globaldce`,
//! and not at -O0. `Stamp` is the same stamping run earlier, before any body's
//! pipeline, so loops are judged on what their callees do. Which stages run
//! follows the level: `inline` and `ipa-cp` thresholds, `ipa-cp-clone` (-O3
//! and up). See
//! `docs/reference/levels.md`.
//!
//! # Needs
//! [`crate::inline`], [`crate::ipacp`], [`crate::profit`], and
//! `llrm_analysis`' `Summaries`, `GlobalsAA` and `interprocedural`.
//!
//! # See also
//! LLVM's FunctionAttrs and IPSCCP; GCC's `ipa-inline.cc`, `ipa-cp.cc` and
//! `ipa-modref.cc`.

use std::collections::BTreeSet;

use llrm_analysis::aa::ModRefInfo;
use llrm_analysis::functionattrs;
use llrm_analysis::interprocedural as facts;
use llrm_analysis::manager::{GlobalsAA, GlobalsModRefAnalysis, ProgramSummaries, Summaries};
use llrm_analysis::memory::{Identity, MemoryKind};
use llrm_mir::callgraph::{CallGraph, CallGraphAnalysis, Defined};
use llrm_mir::context::GlobalId;
use llrm_mir::facts::{Fact, Facts};
use llrm_mir::module::{GlobalKind, Linkage, Module};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Declarations, Declared, ModuleAnalyses, PreservedAnalyses};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};
use llrm_mir::types::Type;

use crate::inline;
use crate::profit::OperationCosts;

/// `edit` run on `module`, and every body it changed said to have: a pass that
/// rewrites the calls of a function it changes (its callers) reports the
/// function, and the callers' analyses were read as they stood before
/// (`ModuleAnalyses::function` brings them up to date, and
/// `LLRM_CHECK_UNREPORTED` fails the read).
pub(crate) fn reporting<T>(
    module: &mut Module,
    modules: &mut ModuleAnalyses,
    edit: impl FnOnce(&mut Module) -> T,
) -> T {
    reporting_marked(module, modules, edit).0
}

/// `reporting`, and where each body it changed was before: what a stage that
/// vouches for its edit (`Settled::vouch`) says it from.
pub(crate) fn reporting_marked<T>(
    module: &mut Module,
    modules: &mut ModuleAnalyses,
    edit: impl FnOnce(&mut Module) -> T,
) -> (T, llrm_support::hash::IndexMap<GlobalId, llrm_mir::module::Mark>) {
    let before: Vec<(GlobalId, llrm_mir::module::Mark)> = module
        .functions()
        .filter(|(_, _, body)| !body.is_declaration())
        .map(|(id, _, body)| (id, body.mark()))
        .collect();
    let out = edit(module);
    let mut changed = llrm_support::hash::IndexMap::default();
    for (id, mark) in before {
        if module.global(id).function().map(llrm_mir::module::Function::mark) != Some(mark) {
            modules.changed(id);
            changed.insert(id, mark);
        }
    }
    (out, changed)
}

/// The edits `bodies` had since `before` are ones the pipeline's passes need
/// not look at again: each is a call that names another function that says the
/// same, or one that takes fewer arguments (`Settled`).
pub(crate) fn vouched(
    module: &Module,
    bodies: impl IntoIterator<Item = GlobalId>,
    before: &llrm_support::hash::IndexMap<GlobalId, llrm_mir::module::Mark>,
) {
    let settled = module.context.extension::<crate::settled::Settled>();
    for id in bodies {
        if let (Some(function), Some(&mark)) = (module.global(id).function(), before.get(&id)) {
            settled.vouch(function, mark);
        }
    }
}

/// `module`'s declarations to declare into: the module's own `Declarations`,
/// nothing scanned unless a pass declares. `LLRM_CHECK_CALLEES=1` compares them
/// with a fresh listing.
pub(crate) fn declared(
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

fn checking() -> bool {
    static CHECKING: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CHECKING.get_or_init(|| llrm_support::env_flag!("LLRM_CHECK_CALLEES"))
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
    /// The copies of a tree recursion in itself and its tail-call
    /// elimination, once the clones are made (D10).
    pub late: Option<crate::walk::SelfInline>,
}

thread_local! {
    /// `LLRM_EXP_INLINECOST` prices the sites once.
    static SHADOWED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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
        let costs = if self.rate.is_some() { program.target.size_costs() } else { program.target.costs() };
        let clocks = program.target.costs();
        let loose = self.rate.map(|_| &clocks);
        let rate = self.rate.unwrap_or(0);
        let roots = roots(program);
        let mut modules = managers(program, analyses);
        let proved = optimized_with::<String>(
            program,
            &mut modules,
            &roots,
            &costs,
            loose,
            rate,
            self.inline,
            &mut |module, analyses, id, stage| {
                pipeline(module, analyses, id, stage);
                Ok(())
            },
        )?;
        self.proved = Some(proved);
        if let Some(late) = &mut self.late {
            late.apply(program, &mut modules)?;
        }
        for (at, one) in modules.into_iter().enumerate() {
            analyses.carry(at, one.carried());
        }
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
        for (at, one) in modules.into_iter().enumerate() {
            analyses.carry(at, one.carried());
        }
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
            if let Some(before) = analyses.uncarry(at) {
                one = one.carrying(before);
            }
            one.require::<Summaries>();
            one.require::<GlobalsModRefAnalysis>();
            // As the first run's: a body sent back through reads what no code
            // outside reaches.
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
    program
        .modules
        .iter()
        .enumerate()
        .flat_map(
            |(at, module)| module
                .functions()
                .filter(|(_, _, function)| !function.is_declaration() && !function.is_deleted())
                .map(move |(id, _, _)| (at, id)),
        )
}

/// Defined procedures of module `at`, in module order.
pub(crate) fn procedures(
    program: &Program,
    at: usize,
) -> Vec<GlobalId> {
    defined(program).filter(|&(one, _)| one == at).map(|(_, id)| id).collect()
}

/// The direct calls to each function of a module as bodies change, and the
/// private functions nothing reaches any more: they lose their bodies and
/// with them the functions only they called (the pin's CGSCC inliner deletes
/// a callee at its last call, `removeDeadFunctions`; gcc removes an
/// unreachable node at once). What a deleted body said (what it writes, whom
/// it calls) stands for nothing.
pub(crate) struct Liveness {
    references: llrm_support::hash::IndexMap<GlobalId, i64>,
    held: BTreeSet<GlobalId>,
    private: BTreeSet<GlobalId>,
}

impl Liveness {
    /// A private function is held when its address is taken, a root names it
    /// or metadata lists it.
    pub(crate) fn new(
        module: &Module,
        private: BTreeSet<GlobalId>,
        addressed: &BTreeSet<GlobalId>,
        roots: &BTreeSet<GlobalId>,
    ) -> Self {
        let held = addressed.union(roots).chain(&listed(module)).copied().collect();
        Self { references: reference_counts(module), held, private }
    }

    /// The direct calls to each function now.
    pub(crate) fn references(&self) -> &llrm_support::hash::IndexMap<GlobalId, i64> {
        &self.references
    }

    fn unreached(
        &self,
        id: &GlobalId,
    ) -> bool {
        self.private.contains(id) && !self.held.contains(id) && self.references.get(id).copied().unwrap_or(0) == 0
    }

    /// The calls of `function` were `was` and are `now` (a splice): the
    /// callees left with no call lose their bodies. Whether any did.
    pub(crate) fn spliced(
        &mut self,
        module: &mut Module,
        analyses: &mut ModuleAnalyses,
        was: Vec<(GlobalId, i64)>,
        now: Vec<(GlobalId, i64)>,
    ) -> bool {
        let mut dead = Vec::new();
        for (callee, count) in now {
            *self.references.entry(callee).or_insert(0) += count;
        }
        for (callee, count) in was {
            *self.references.entry(callee).or_insert(0) -= count;
            dead.push(callee);
        }
        self.bare(module, analyses, dead)
    }

    /// Every function nothing reaches, and those only they called.
    pub(crate) fn bare_all(
        &mut self,
        module: &mut Module,
        analyses: &mut ModuleAnalyses,
    ) {
        let dead: Vec<GlobalId> = module
            .functions()
            .filter(|(_, _, function)| !function.is_declaration() && !stub(function))
            .map(|(id, _, _)| id)
            .collect();
        self.bare(module, analyses, dead);
    }

    fn bare(
        &mut self,
        module: &mut Module,
        analyses: &mut ModuleAnalyses,
        candidates: Vec<GlobalId>,
    ) -> bool {
        let mut dead: Vec<GlobalId> = candidates.into_iter().filter(|id| self.unreached(id)).collect();
        let mut bared = false;
        while let Some(id) = dead.pop() {
            let Some(function) = module.global(id).function().filter(|one| !one.is_declaration() && !stub(one)) else {
                continue;
            };
            let called = calls_of(&module.context, function);
            bare(module, analyses, id);
            bared = true;
            for (callee, count) in called {
                let left = self.references.entry(callee).or_insert(0);
                *left -= count;
                if *left == 0 && self.unreached(&callee) {
                    dead.push(callee);
                }
            }
        }
        if bared {
            analyses.invalidate_bodies(&PreservedAnalyses::none());
        }
        bared
    }
}

fn bare_the_unreached(
    module: &mut Module,
    analyses: &mut ModuleAnalyses,
    private: &BTreeSet<GlobalId>,
    addressed: &BTreeSet<GlobalId>,
    roots: &BTreeSet<GlobalId>,
) {
    Liveness::new(module, private.clone(), addressed, roots).bare_all(module, analyses);
}

/// Whether `function` is what `bare` leaves.
fn stub(function: &llrm_mir::module::Function) -> bool {
    function.is_deleted()
}

/// `id` with a body of one `unreachable`: nothing it did, wrote or called
/// stands. (A declaration of an internal function is not a module.) The
/// caller drops the analyses.
fn bare(
    module: &mut Module,
    analyses: &mut ModuleAnalyses,
    id: GlobalId,
) {
    assert!(
        !llrm_support::debug::verifying() || reference_counts(module).get(&id).copied().unwrap_or(0) == 0,
        "@{} is bared with a call to it standing",
        module.global(id).name.as_deref().unwrap_or_default()
    );
    let Some(mut stub) = module.global(id).function().map(llrm_mir::module::Function::declaration) else { return };
    let void = module.context.types.void();
    let entry = stub.create_block(None);
    stub.insert_block(entry, None).expect("a first block");
    let end = stub.create_instruction(Opcode::Unreachable, void, Vec::new(), llrm_mir::opcode::Flags::default(), None);
    stub.insert(end, llrm_mir::edit::Position::End(entry)).expect("a placed block");
    analyses.changed(id);
    module.globals[id.0 as usize].kind = GlobalKind::Function(Box::new(stub));
}

/// The roots in module `at`.
pub(crate) fn roots_here(
    roots: &BTreeSet<Defined>,
    at: usize,
) -> BTreeSet<GlobalId> {
    roots.iter().filter(|one| one.0 == at).map(|one| one.1).collect()
}

/// How often each function is called directly, by call or invoke, in `module`.
fn reference_counts(module: &Module) -> llrm_support::hash::IndexMap<GlobalId, i64> {
    let mut found = llrm_support::hash::IndexMap::default();
    for (_, _, function) in module.functions() {
        for (callee, count) in calls_of(&module.context, function) {
            *found.entry(callee).or_insert(0) += count;
        }
    }
    found
}

/// The functions an indirect call may reach by the metadata that lists them
/// (`!callees`): no direct call names them, and they are still called.
fn listed(module: &Module) -> BTreeSet<GlobalId> {
    module
        .metadata
        .iter()
        .flat_map(|node| &node.operands)
        .filter_map(|operand| match operand {
            llrm_mir::module::MetadataOperand::Constant(id) => match module.context.get(*id).kind {
                llrm_mir::context::ConstantKind::Global(global) => Some(global),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// How often `function` calls each function directly.
pub(crate) fn calls_of(
    context: &llrm_mir::context::Context,
    function: &llrm_mir::module::Function,
) -> Vec<(GlobalId, i64)> {
    let mut found: llrm_support::hash::IndexMap<GlobalId, i64> = llrm_support::hash::IndexMap::default();
    for (_, inst) in function.walk() {
        if let Some(callee) = llrm_mir::memory::callee(context, function, inst) {
            *found.entry(callee).or_insert(0) += 1;
        }
    }
    found.into_iter().collect()
}

/// Defined procedures no outside code calls: every caller is in the program,
/// and a call through a pointer is not one, so none has its address taken.
pub(crate) fn unexported(program: &Program) -> BTreeSet<Defined> {
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
            // A `noipa` routine is entered as outside code may enter it: no
            // constant is pushed into it and it is not cloned.
            !program.exports.exported(program.modules[at].global(id))
                && !addressed.contains(&one)
                && !program.modules[at].global(id).function().is_some_and(llrm_mir::facts::opaque)
        })
        .collect()
}

/// Module `at`'s procedures every caller of which is in the module: no
/// outside code, and no other module, calls them.
pub(crate) fn private(
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
/// pipeline again on a body `stage` changed.
pub fn optimized_with<E: From<String>>(
    program: &mut Program,
    modules: &mut [ModuleAnalyses],
    roots: &BTreeSet<Defined>,
    costs: &OperationCosts,
    loose: Option<&OperationCosts>,
    rate: i64,
    threshold: inline::Threshold,
    reoptimised: &mut dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str) -> Result<(), E>,
) -> Result<Proved, E> {
    let count = program.modules.len();
    let mut procedures: Vec<Vec<GlobalId>> = (0..count).map(|at| procedures(program, at)).collect();
    let mut private: Vec<BTreeSet<GlobalId>> = (0..count).map(|at| private(program, at)).collect();
    let unexported = unexported(program);
    let edited = |analyses: &mut ModuleAnalyses, bodies: &[GlobalId]| {
        for &id in bodies {
            analyses.changed(id);
        }
        analyses.invalidate_bodies(&PreservedAnalyses::none());
    };

    // The walk has run: what each body does, stated on it by the `stamp`
    // slot before it, is what the dead-call removal below reads.
    // What the walk inlined into every caller has no use left; `globaldce`
    // would drop it, the passes below skip it.
    for at in 0..count {
        let module = &mut program.modules[at];
        let addressed = llrm_mir::callgraph::addressed(module);
        bare_the_unreached(module, &mut modules[at], &private[at], &addressed, &roots_here(roots, at));
    }

    // A pointer a body only reads through is given as the fields it reads,
    // before what its callers pass is propagated: a length or a segment now
    // crosses the call as a value.
    let (priced, bytes) = match loose {
        Some(clocks) if rate > 0 => (clocks, false),
        _ => (costs, true),
    };
    let mut promoted_any = false;
    for at in 0..count {
        for id in reporting(&mut program.modules[at], &mut modules[at], |module| {
            crate::argpromotion::promoted(module, &program.layout, priced, bytes)
        }) {
            promoted_any = true;
            edited(&mut modules[at], &[id]);
            reoptimised(&mut program.modules[at], &mut modules[at], id, "promote.")?;
        }
    }
    // DEVIATE: the pin's IPSCCP runs once, before the walk. Promotion is not
    // the pin's (it refuses a descriptor read after a branch), and the lengths
    // it makes into arguments are constants only IPSCCP's lattice sees through
    // a recursion that passes them on. Exit: promotion follows the pin's
    // conditions.
    if promoted_any {
        for (at, id) in crate::ipsccp::run_ipsccp_with(program, modules) {
            edited(&mut modules[at], &[id]);
            reoptimised(&mut program.modules[at], &mut modules[at], id, "ipsccp.")?;
        }
    }

    // A far pointer every call fills from DGROUP is passed as its offset.
    for at in 0..count {
        for id in reporting(&mut program.modules[at], &mut modules[at], |module| {
            crate::narrowspace::narrowed(module, &program.layout, program.target.spaces())
        }) {
            edited(&mut modules[at], &[id]);
            reoptimised(&mut program.modules[at], &mut modules[at], id, "narrow.")?;
        }
    }

    // An internal variable is split into the pieces its loads read, and one a
    // body that does not recurse writes before it reads is that body's stack
    // slot: GlobalOpt's `SRAGlobal` and its localizing branch.
    for at in 0..count {
        let ids = reporting(&mut program.modules[at], &mut modules[at], |module| {
            crate::globalsra::split(module, &program.layout, program.target.spaces())
        });
        // Every body is told changed before the first runs again: the global
        // analyses see the new variables once, not once per body.
        edited(&mut modules[at], &ids);
        if !ids.is_empty() {
            modules[at].invalidate(&PreservedAnalyses::none());
        }
        for id in ids {
            reoptimised(&mut program.modules[at], &mut modules[at], id, "globalsra.")?;
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

    propagate_constant_returns(&procedures, program, modules, &mut return_round, reoptimised)?;
    let mut cloning = crate::ipacp::Cloning::default();
    loop {
        // A function called with a constant is copied for it (gcc's ipa-cp),
        // and the calls go to the copy.
        let mut cloned_now = false;
        for at in 0..count {
            let (made, before) = reporting_marked(&mut program.modules[at], &mut modules[at], |module| {
                crate::ipacp::cloned(
                    module,
                    &program.layout,
                    &procedures[at],
                    &private[at],
                    costs,
                    threshold.cp_clone,
                    &mut cloning,
                )
            });
            vouched(&program.modules[at], made.edited.iter().copied().filter(|id| !made.unsound.contains(id)), &before);
            for &id in &made.added {
                procedures[at].push(id);
                private[at].insert(id);
            }
            for &id in made.added.iter().chain(&made.edited) {
                llrm_mir::memory::unsplit(
                    &mut function_mut(&mut program.modules[at], id).1.attrs,
                    llrm_mir::memory::Moved::OutOfArguments,
                );
                edited(&mut modules[at], &[id]);
                reoptimised(&mut program.modules[at], &mut modules[at], id, "ipa-cp.")?;
                cloned_now = true;
            }
        }
        if !cloned_now {
            break;
        }
        propagate_constant_returns(&procedures, program, modules, &mut return_round, reoptimised)?;
    }
    // What the constants left unread is not pushed.
    for at in 0..count {
        let ((removed, unsound), before) =
            reporting_marked(&mut program.modules[at], &mut modules[at], crate::deadargs::removed_with);
        vouched(&program.modules[at], removed.iter().copied().filter(|id| !unsound.contains(id)), &before);
        for id in removed {
            edited(&mut modules[at], &[id]);
            // A body with fewer values to carry is another body to the loop
            // passes and the recursion's.
            reoptimised(&mut program.modules[at], &mut modules[at], id, "ipa-deadargs.")?;
        }
    }
    // DEVIATE F6 (`stamped`): the `stamp` slot runs on raw bodies, whose
    // pointers are spilled, so a function that writes only through its
    // parameter states `memory(write)`, not `argmem: write`. This run, on the
    // simplified bodies, states it; `ipa-pure` below reads it. Exit: the pin's
    // position, `PostOrderFunctionAttrs` of each component after its function
    // pipeline inside the walk (w-6, 2026-11-13); until then dropping this run
    // lost the nib effects test (#4202).
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

/// Each body whose definition is exact stamped as LLVM's
/// `deriveAttrsInPostOrder` does (`functionattrs::post_order`, callees first
/// so a caller sees what they state), and `initializes` on each pointer
/// parameter it keeps no copy of, from alias's summary (DEVIATE F6: it also
/// states `nocapture` and `readnone`/`readonly`/`writeonly` where `post_order`
/// did not, until bodies are simplified when they are stamped: the first run is
/// on raw bodies, whose pointers are spilled). Any other attribute
/// already stated stays. The bodies stamped; the caller drops `analyses` when
/// there are any.
pub fn stamped(
    module: &mut Module,
    analyses: &mut ModuleAnalyses,
) -> Result<Vec<GlobalId>, String> {
    let program = std::rc::Rc::clone(analyses.program());
    let layout = &program.layout;
    let spaces = program.target.spaces();
    let known = analyses.get::<Summaries>(module);
    let known = Result::as_ref(&*known).map_err(String::clone)?;
    let graph = analyses.get::<CallGraphAnalysis>(module);
    let mut changed: Vec<GlobalId> = Vec::new();
    let mut seen: BTreeSet<GlobalId> = BTreeSet::new();
    for (members, _) in graph.bottom_up_components() {
        for id in functionattrs::post_order(module, layout, spaces, &members) {
            if seen.insert(id) {
                changed.push(id);
            }
        }
    }
    for id in graph.bottom_up() {
        let global = module.global(id);
        let exact = matches!(
            global.linkage,
            Linkage::External | Linkage::Internal | Linkage::Private
        );
        let (Some(name), Some(function), true) = (global.name.as_ref(), global.function(), exact) else { continue };
        // A `noipa` body is stamped with nothing: callers read what the source
        // declared.
        if llrm_mir::facts::opaque(function) {
            continue;
        }
        let Some(summary) = known.get(name) else { continue };
        let pointers = function
            .parameters()
            .iter()
            .map(|&one| matches!(
                module.context.types.get(function.value(one).ty),
                Type::Pointer(_)
            ))
            .collect::<Vec<_>>();
        let function = function_mut(module, id).1;
        let before = function.parameter_attrs.clone();
        for (index, attrs) in function.parameter_attrs.iter_mut().enumerate().filter(|(index, _)| pointers[*index]) {
            let identity = Some(Identity::Int(index as i64));
            if summary.captures.contains(&identity) {
                continue;
            }
            // DEVIATE F6: what alias's summary proves of the parameter where
            // `post_order` did not, until it runs on simplified bodies: the
            // first run is on raw bodies, whose pointers are spilled.
            if !Facts::of(attrs).no_capture() {
                attrs.extend(Fact::NoCapture.attribute());
            }
            let through = |slices: &llrm_analysis::sliceset::SliceSet| {
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
        }
        if before != function.parameter_attrs && seen.insert(id) {
            changed.push(id);
        }
    }
    if llrm_support::env_flag!("LLRM_SHADOW_GMR") {
        shadow_globalsmodref(module, known);
        shadow_attributes(module, known);
    }
    Ok(changed)
}

/// Shadow of `globalsmodref::compute` against alias's summaries: for each
/// body both describe and each tracked global, one line per disagreement, and
/// the counts.
fn shadow_globalsmodref(
    module: &Module,
    known: &llrm_support::hash::IndexMap<String, std::rc::Rc<llrm_analysis::alias::Summary>>,
) {
    use llrm_analysis::memory::{Key, MemoryKind};
    let found = llrm_analysis::globalsmodref::compute(module);
    let tracked: Vec<GlobalId> = (0..module.globals.len() as u32)
        .map(GlobalId)
        .filter(|&id| found.is_non_address_taken(id) && module.global(id).function().is_none())
        .collect();
    let (mut same, mut looser, mut tighter, mut compared) = (0_usize, 0_usize, 0_usize, 0_usize);
    for (id, global, _) in module.functions() {
        let Some(name) = global.name.as_ref() else { continue };
        let (Some(info), Some(summary)) = (found.function_info(id), known.get(name)) else { continue };
        compared += 1;
        for &g in &tracked {
            let touches = |slices: &llrm_analysis::sliceset::SliceSet| {
                slices.iter().any(|one| one.object.kind == MemoryKind::Global && one.object.key == Key::Global(g.0))
            };
            // A tracked global is reached by name alone: unknown accesses
            // do not count.
            let (reads, writes) = (touches(&summary.reads), touches(&summary.writes));
            let theirs = info.mod_ref_for_global(g);
            let mine = match (reads, writes) {
                (false, false) => ModRefInfo::NoModRef,
                (true, false) => ModRefInfo::Ref,
                (false, true) => ModRefInfo::Mod,
                (true, true) => ModRefInfo::ModRef,
            };
            if theirs == mine {
                same += 1;
            } else if theirs.union(mine) == theirs {
                looser += 1;
            } else {
                tighter += 1;
                eprintln!(
                    "shadow-gmr tighter @{name} {:?}: globalsmodref {theirs:?} summary {mine:?}",
                    module.global(g).name
                );
            }
        }
    }
    eprintln!("shadow-gmr compared {compared} bodies: same {same} looser {looser} tighter {tighter}");
}

/// Shadow of the attributes' `memory(...)` against alias's summaries, per body:
/// the whole-memory read and write, and whether the effect is argument memory
/// alone. `tighter` is an attribute that states less than the summary found.
fn shadow_attributes(
    module: &Module,
    known: &llrm_support::hash::IndexMap<String, std::rc::Rc<llrm_analysis::alias::Summary>>,
) {
    use llrm_analysis::aa::{IRMemLocation, MemoryEffects};
    use llrm_analysis::memory::MemoryKind;
    let (mut same, mut looser, mut tighter, mut compared) = (0_usize, 0_usize, 0_usize, 0_usize);
    let (mut argmem_same, mut argmem_looser) = (0_usize, 0_usize);
    for (_, global, function) in module.functions() {
        let Some(name) = global.name.as_ref() else { continue };
        let Some(summary) = known.get(name) else { continue };
        compared += 1;
        let me = MemoryEffects::of_attrs(&function.attrs);
        let reads = !summary.reads.is_empty() || summary.unknown_read;
        let writes = !summary.writes.is_empty() || summary.unknown_write;
        let theirs = me.overall();
        let mine = match (reads, writes) {
            (false, false) => ModRefInfo::NoModRef,
            (true, false) => ModRefInfo::Ref,
            (false, true) => ModRefInfo::Mod,
            (true, true) => ModRefInfo::ModRef,
        };
        if theirs == mine {
            same += 1;
        } else if theirs.union(mine) == theirs {
            looser += 1;
            eprintln!("shadow-attrs looser @{name}: attrs {theirs:?} summary {mine:?}");
        } else {
            tighter += 1;
            eprintln!("shadow-attrs tighter @{name}: attrs {theirs:?} summary {mine:?}");
        }
        let only_arguments = |slices: &llrm_analysis::sliceset::SliceSet| {
            slices.iter().all(|one| one.object.kind == MemoryKind::Parameter)
        };
        let summary_args = !summary.unknown_read
            && !summary.unknown_write
            && only_arguments(&summary.reads)
            && only_arguments(&summary.writes);
        let attrs_args = me.mod_ref(IRMemLocation::Other).is_no_mod_ref()
            && me.mod_ref(IRMemLocation::InaccessibleMem).is_no_mod_ref();
        if summary_args == attrs_args {
            argmem_same += 1;
        } else if summary_args {
            argmem_looser += 1;
            eprintln!("shadow-attrs argument-only looser @{name}: attrs {me:?}");
        }
    }
    eprintln!(
        "shadow-attrs compared {compared} bodies: overall same {same} looser {looser} tighter {tighter}; argument-only same {argmem_same} looser {argmem_looser}"
    );
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

/// `function_mut`, with the module's metadata apart from it.
pub(crate) fn function_and_metadata_mut(
    module: &mut Module,
    id: GlobalId,
) -> (&mut llrm_mir::context::Context, &mut llrm_mir::module::Function, llrm_mir::module::Metadata<'_>) {
    let Module { context, globals, metadata, special, .. } = module;
    match &mut globals[id.0 as usize].kind {
        GlobalKind::Function(function) => (context, function, llrm_mir::module::Metadata { nodes: metadata, special }),
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
