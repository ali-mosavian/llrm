//! The memory and range analyses as entries of llrm-mir's analysis manager:
//! asked for by name, cached, kept or brought up to date after a pass.
//!
//! # How it works
//! A pass asks `analyses.get::<A>(context, layout, function)` and receives a
//! shared result. The manager computes it once, hands the same `Rc` to every
//! later asker, and decides after each pass what to do with it. The entries
//! here declare how, through the `Analysis` trait of `llrm_mir::passes`:
//!
//! - A pass returns the `PreservedAnalyses` it left true. An entry not named is
//!   dropped, unless `READS_OUTER` is false (it reads only the function, as
//!   `DemandedBits` and `Shape`) and the function is unchanged.
//! - `SKIPS` with `depends`: the entry is kept past an invalidation if the
//!   change log cannot have altered it. `Depends` names the instructions the
//!   result models, those it keys entries by, and the users a change flows
//!   through. `ExposedFrames`, `PointerValues`, `Pointers` and `Writes` model
//!   pointers, aggregates and calls only, so a change to integer arithmetic
//!   leaves them standing.
//! - `INCREMENTAL` with `update`: the entry is brought up to date from the
//!   change log. `DominatedEdges` and `Counted` redo only the blocks or loops
//!   `Touched::reach` the changes (a change to a terminator or to the CFG
//!   redoes all); `Accesses` in [`crate::memoryssa`] survives pure moves.
//! - `LLRM_CHECK_REPLAY` asserts that every kept or updated result equals a
//!   fresh `run`.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef warn fill:#dc2626,stroke:#991b1b,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   run["<b>pass runs</b><br/><small>PreservedAnalyses</small>"]:::mid
//!   q1["<b>entry named?</b>"]:::mid
//!   q2["<b>READS_OUTER=false and</b><br/><small>function unchanged?</small>"]:::mid
//!   q3["<b>SKIPS and depends</b><br/><small>.unaffected(changes)?</small>"]:::mid
//!   q4["<b>INCREMENTAL and</b><br/><small>update(previous, changes)?</small>"]:::mid
//!   kept["<b>kept</b>"]:::opt
//!   kept2["<b>kept, stands again</b><br/><small>if changes undone</small>"]:::opt
//!   upd["<b>updated</b>"]:::opt
//!   drop["<b>dropped#59; recomputed</b><br/><small>on the next get</small>"]:::warn
//!   run --> q1
//!   q1 -->|"yes"| kept
//!   q1 -->|"no"| q2
//!   q2 -->|"yes"| kept
//!   q2 -->|"no"| q3
//!   q3 -->|"yes"| kept2
//!   q3 -->|"no"| q4
//!   q4 -->|"Some"| upd
//!   q4 -->|"no"| drop
//! ```
//!
//! *What happens to a cached analysis when a pass runs*
//!
//! Function entries and what each asks for (every one also asks `Shape`, and
//! most `AssumptionCache` and `ExposedFrames`, which are omitted):
//!
//! ```text
//!  PointerValues    -
//!  Pointers         PointerValues
//!  Writes           PointerValues
//!  TableLoads       -
//!  Registers        TableLoads (updates)
//!  Followers        Registers
//!  Counted          Registers
//!  DominatedEdges   Registers
//!  Annotated        PointerValues, Registers, Counted, DominatedEdges
//!  Accesses         Annotated, CallFootprints, PointerValues (memoryssa)
//!  PlainAccesses    Writes, Annotated                       (the graph's structure, once)
//!  ThroughMemory    PlainAccesses, Writes, Annotated, Registers
//!  FloatFacts       ThroughMemory, PlainAccesses, Writes, Annotated, Registers
//! ```
//!
//! Module entries read the program through `ModuleAnalyses` and are dropped
//! unless every run of a pass preserved them. `GlobalsAA` (which globals no
//! outside code reaches but by name) and `Summaries` (each defined function's
//! memory effects, by name) can be held past passes that add no memory
//! operation when `covers` says the stale result states no more than a fresh
//! one. `Summaries` keeps per-body facts and recomputes only bodies whose
//! history moved. `ProgramSummaries` is a program entry: one fixed point over
//! all modules, each module's `Summaries` given the others'.
//!
//! `Unit::within` builds the `Unit` that analyses read from `Outer` (the
//! module, target and program), carrying whichever cached results the caller
//! holds. `Held` and `Ranges` bundle the cached results a pass passes on to a
//! `Unit`, so the facts are the manager's and not a solve of the pass's own.
//!
//! # Example
//! A pass replaces the uses of `%x` by `%a`, an integer edit:
//!
//! ```mir
//! declare void @use(ptr)
//! define i32 @f(ptr %p, i32 %a) {
//! entry:
//!   %x = add i32 %a, 1
//!   %y = mul i32 %x, 3
//!   %q = getelementptr i32, ptr %p, i32 1
//!   call void @use(ptr %q)
//!   ret i32 %y
//! }
//! ```
//!
//! | change                       | `Pointers`, `Writes`, `ExposedFrames`                |
//! |------------------------------|------------------------------------------------------|
//! | `%x` replaced by `%a`        | kept (`unaffected` is true)                          |
//! | `%q`, the address, rewritten | `Writes::unaffected` is false: recomputed             |
//!
//! From unit test
//! `an_integer_edit_leaves_the_pointer_analyses_as_they_were_and_a_pointer_edit_does_not`
//! in
//! `manager_tests.rs`.
//!
//! # Order check
//! A result a pass reads is a function of the body, not of when something first
//! asked for it (#1909: a `ThroughMemory` asked early and kept past sound edits
//! made `fold` weaker, 18 KB of asm against 34 KB). `LLRM_CHECK_ORDER=SEED`
//! asks some analyses early at each body pass's entry, which ones by the seed,
//! and the object must be the lazy compile's. `tests/analysis_order.rs` runs it
//! on matmul. The wide sweep is a command, not a gate; run it before a release
//! and when an analysis starts to be kept:
//!
//! ```text
//! for s in 1 2 3 4 5 6 7 8; do
//!   LLRM_CHECK_ORDER=$s tools/identical.py target/release/llrm-c target/release/llrm-c
//! done
//! ```
//!
//! `tools/identical.py` keeps a `LLRM_CHECK_*` value already set; the bench
//! BASIC and Nib programs under the harness flags are compiled the same way
//! with and without the seed.
//!
//! # Provides
//! The entries: `ExposedFrames`, `PointerValues`, `Pointers`, `Annotated`,
//! `Writes`, `Registers`, `DemandedBits`, `AssumptionCache`,
//! `Followers`, `Counted`, `ThroughMemory`, `FloatFacts`, `DominatedEdges`;
//! module entries `GlobalsAA`, `Summaries`; program
//! entry `ProgramSummaries`; `Held`, `Ranges`, `Touched::reach`,
//! `writes`. [`crate::cfg`]'s `Shape`, [`crate::memoryssa`]'s
//! `Accesses` is an entry defined beside its analysis.
//!
//! # Needs
//! [`crate::alias`], [`crate::consts`], [`crate::globalsaa`],
//! [`crate::induction`], [`crate::ranges`], [`crate::floatfacts`],
//! [`crate::frameescape`].
//!
//! # See also
//! LLVM's new pass manager `AnalysisManager` and `PreservedAnalyses`; the trait
//! is in `crates/ir/llrm-mir/src/passes.rs`.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::context::{Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::depends::Depends;
use llrm_mir::module::{Change, Function, InstId, Linkage, Mark, Module, Operand, ValueId};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::passes::{Analyses, Analysis, ModuleAnalyses, ModuleAnalysis, Outer};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramAnalysis, ProgramProxy};
use llrm_support::hash::IndexMap;

use crate::alias::{self, PointsTo, Procedure, Summary};
use crate::cfg::Shape;
use crate::consts::{self, Calls, Known};
use crate::floatfacts;
use crate::globalsaa::{self, Globals, ProgramGlobals};
use crate::induction;
use crate::memory::{Identity, MemRef, MemoryKind, MemoryObject, ObjectInterner, Slice, Unit};
use crate::memoryssa;

impl<'a> Unit<'a> {
    /// `function` as the manager's analyses see it: its module and target
    /// as `outer` holds them.
    pub fn within(
        context: &'a Context,
        layout: &'a DataLayout,
        function: &'a Function,
        outer: &'a Outer,
    ) -> Self {
        let globals_aa = outer.cached_ref::<GlobalsAA>().and_then(|one| one.as_ref().ok());
        let globals_mr = outer.cached_ref::<GlobalsModRefAnalysis>();
        Self {
            program: Some(outer.program()),
            spaces: outer.target().spaces(),
            context,
            layout,
            metadata: &outer.metadata,
            tbaa: Some(outer.tbaa()),
            globals: &outer.globals,
            function,
            globals_aa,
            globals_mr,
            outer: Some(outer),
            references: None,
            shape: None,
            registers: None,
            pointers: None,
            annotated: None,
            assumptions: None,
            counted: None,
            followers: None,
            edges: None,
            exposed: None,
            plain: None,
            point_values: None,
            congruent: None,
            proved: None,
        }
    }
}

/// Which globals no code outside the module reaches but by name:
/// `globalsaa::analysis`.
pub struct GlobalsAA;

impl ModuleAnalysis for GlobalsAA {
    type Result = Result<Globals, String>;
    const NAME: &'static str = "globals-aa";
    fn covers(
        stale: &Self::Result,
        fresh: &Self::Result,
    ) -> bool {
        match (stale, fresh) {
            (Ok(stale), Ok(fresh)) => stale.covers(fresh),
            _ => stale == fresh,
        }
    }
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        globalsaa::analysis(module, analyses)
    }
}

/// `GlobalsAAResult` (GlobalsModRef.cpp): what each function does to memory
/// and to each global nothing outside the module reaches but by name.
pub struct GlobalsModRefAnalysis;

impl ModuleAnalysis for GlobalsModRefAnalysis {
    type Result = crate::globalsmodref::GlobalsModRef;
    const NAME: &'static str = "globals-modref";
    fn covers(
        stale: &Self::Result,
        fresh: &Self::Result,
    ) -> bool {
        stale.covers(fresh)
    }
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        crate::globalsmodref::compute_with(module, globalsaa::unexported(module, analyses))
    }
}

/// Whether the module forges a pointer into the data segment
/// (`forging::forges_selector`): worked out once for the module and read by
/// `GlobalsAA`.
pub struct ForgedSelectors;

impl ModuleAnalysis for ForgedSelectors {
    type Result = bool;
    const NAME: &'static str = "forged-selectors";
    /// A held `true` claims more than a fresh `false`, never less.
    fn covers(
        stale: &Self::Result,
        fresh: &Self::Result,
    ) -> bool {
        *stale || !*fresh
    }
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        crate::forging::forges_selector(module, analyses.program().spaces)
    }
}

/// Each defined function's memory effects, by name: `alias::summaries` of
/// the whole module, its globals as `GlobalsAA` finds them and the bodies
/// other modules define as the cached `ProgramSummaries` says.
pub struct Summaries;

impl ModuleAnalysis for Summaries {
    type Result = Result<Rc<IndexMap<String, Summary>>, String>;
    const NAME: &'static str = "summaries";
    fn covers(
        stale: &Self::Result,
        fresh: &Self::Result,
    ) -> bool {
        match (stale, fresh) {
            (Ok(stale), Ok(fresh)) => {
                fresh.iter().all(|(name, new)| stale.get(name).is_some_and(|old| old.covers(new)))
            }
            _ => stale == fresh,
        }
    }
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        let inputs = SummaryInputs::of(module, analyses);
        let none = BTreeSet::new();
        let tracked = match &inputs.globals_held {
            Some(held) => Result::as_ref(&**held).map_err(String::clone)?.tracked_globals(),
            None => &none,
        };
        let program = Rc::clone(analyses.program());
        let layout = &program.layout;
        let then = analyses.memo::<Option<SummariesMade>>().take();
        // The result of the last run, if it is the one `then` describes; the
        // only holder of it where nothing else holds it.
        let before = match analyses.take_dropped::<Summaries>() {
            Some(held) if then.is_some() => match Rc::try_unwrap(held) {
                Ok(Ok(map)) => Some(Rc::try_unwrap(map).unwrap_or_else(|shared| (*shared).clone())),
                Ok(Err(_)) => None,
                Err(shared) => Result::as_ref(&*shared).ok().map(|map| (**map).clone()),
            },
            _ => None,
        };
        let log = (analyses.log_epoch(), analyses.edit_log().len());
        let made = match (then, before) {
            (Some(then), Some(mut before)) => {
                // Only bodies edited since the last run moved, where what a
                // summary is of besides its body is as then.
                let alone = then.epoch == log.0
                    && then.globals == module.globals.len()
                    && then.tracked == *tracked
                    && then.inputs.is_the_last_but_declarations(&inputs);
                if alone {
                    let edited: BTreeSet<GlobalId> = analyses.edit_log()[then.logged..].iter().copied().collect();
                    crate::attrsummary::resummarized(module, layout, tracked, &inputs.found, &mut before, edited);
                    before
                } else {
                    let last = crate::attrsummary::Last {
                        declared: &then.inputs.declared,
                        declared_now: &inputs.declared,
                        found: &then.inputs.found,
                        tracked: &then.tracked,
                        made: &mut before,
                    };
                    crate::attrsummary::summaries_reusing(module, layout, tracked, &inputs.found, Some(last))
                }
            }
            _ => crate::attrsummary::summaries(module, layout, tracked, &inputs.found),
        };
        let tracked = tracked.clone();
        let globals = module.globals.len();
        *analyses.memo::<Option<SummariesMade>>() =
            Some(SummariesMade { inputs, tracked, globals, epoch: log.0, logged: log.1 });
        Ok(Rc::new(made))
    }

    /// A summary is made of its function's declaration (attributes and
    /// parameters) and of what `GlobalsAA` and `GlobalsModRef` hold, and of
    /// nothing else in the body: where those are the very results the last run
    /// read, so is the result. LLVM's `PostOrderFunctionAttrs` does not visit a
    /// function whose body and callees it kept.
    fn unchanged(
        module: &Module,
        analyses: &mut ModuleAnalyses,
        _: &Self::Result,
    ) -> bool {
        let now = SummaryInputs::of(module, analyses);
        analyses.memo::<Option<SummariesMade>>().as_ref().is_some_and(|last| now.is_the_last(&last.inputs))
    }
}

/// What the last `Summaries::run` read and made.
struct SummariesMade {
    inputs: SummaryInputs,
    tracked: BTreeSet<GlobalId>,
    /// The globals, and the position in the log of edits, the run was at.
    globals: usize,
    epoch: u64,
    logged: usize,
}

/// The results `Summaries::run` read.
struct SummaryInputs {
    declared: Rc<Vec<llrm_mir::module::GlobalValue>>,
    globals_held: Option<Rc<<GlobalsAA as ModuleAnalysis>::Result>>,
    found: Rc<<GlobalsModRefAnalysis as ModuleAnalysis>::Result>,
}

impl SummaryInputs {
    fn of(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self {
        // Where GlobalsAA is read cached (`PassManager::require_cached`) and
        // not yet made, no global is tracked.
        let globals_held = match analyses.cached::<GlobalsAA>() {
            Some(held) => Some(held),
            None if analyses.is_stateless::<GlobalsAA>() => None,
            None => Some(analyses.get::<GlobalsAA>(module)),
        };
        // The one `GlobalsAAResult` of the module, as the pin has: the held
        // `GlobalsModRef`, not another made for each run.
        let found = analyses.get::<GlobalsModRefAnalysis>(module);
        let declared = analyses.get::<llrm_mir::passes::Declarations>(module);
        Self { declared, globals_held, found }
    }

    /// As `is_the_last`, the declarations left to the log of edits.
    fn is_the_last_but_declarations(
        &self,
        now: &Self,
    ) -> bool {
        Rc::ptr_eq(&self.found, &now.found)
            && match (&self.globals_held, &now.globals_held) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }

    fn is_the_last(
        &self,
        last: &Self,
    ) -> bool {
        Rc::ptr_eq(&self.declared, &last.declared)
            && Rc::ptr_eq(&self.found, &last.found)
            && match (&self.globals_held, &last.globals_held) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

/// `module`'s defined functions, each with its id.
fn bodies(module: &Module) -> impl Iterator<Item = (GlobalId, &Function)> {
    module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration() && !llrm_mir::facts::opaque(function))
        .map(|(id, _, function)| (id, function))
}

/// What a body gives `Summaries` that is of the body, the declarations and (for
/// its calls) the globals' facts alone, as of the body's history then.
#[derive(Clone)]
struct BodyFacts {
    mark: Mark,
    exposed: Rc<BTreeSet<ValueId>>,
    shape: Option<Rc<Shape>>,
    calls: Option<Rc<alias::CallFacts>>,
    /// Where each pointer points, found once for the direct summary, each
    /// call's and the calls' facts, which all ask it; under the globals' facts
    /// as `calls` is.
    values: Option<Rc<alias::PointValues>>,
}

/// The exposed frames of each of `module`'s bodies, found once for the
/// summaries that ask of every access; those of a body whose history is where
/// `kept` left it are `kept`'s.
fn body_facts(
    module: &Module,
    layout: &DataLayout,
    spaces: llrm_mir::spaces::Spaces,
    mut kept: IndexMap<GlobalId, BodyFacts>,
    marks: &IndexMap<GlobalId, Mark>,
) -> IndexMap<GlobalId, BodyFacts> {
    bodies(module)
        .map(|(id, function)| {
            let mark = marks[&id].clone();
            let facts = match kept.swap_remove(&id) {
                Some(then) if then.mark == mark => then,
                _ => BodyFacts {
                    mark,
                    exposed: Rc::new(crate::memory::exposed_frames(
                        &Unit::of(module, layout, function).with_spaces(spaces),
                    )),
                    shape: None,
                    calls: None,
                    values: None,
                },
            };
            (id, facts)
        })
        .collect()
}

/// `function` as `Summaries` sees it: its program, the globals' facts, its
/// shape and its exposed frames.
fn summarized_in<'a>(
    module: &'a Module,
    program: &'a ProgramProxy,
    globals: &'a Globals,
    shape: &'a Shape,
    exposed: &'a BTreeSet<ValueId>,
    values: Option<&'a alias::PointValues>,
    function: &'a llrm_mir::module::Function,
) -> Unit<'a> {
    let unit = Unit { program: Some(program), ..Unit::of(module, &program.layout, function) }
        .with_globals_aa(globals)
        .with_shape(shape)
        .with_exposed(exposed);
    match values {
        Some(values) => unit.with_point_values(values),
        None => unit,
    }
}

/// The calls of each body `facts` has none for, as the unit it is summarized in
/// sees them.
fn calls_found(
    module: &Module,
    program: &ProgramProxy,
    globals: &Globals,
    ids: impl IntoIterator<Item = GlobalId>,
    facts: &mut IndexMap<GlobalId, BodyFacts>,
) {
    for id in ids {
        let function = module.global(id).function().expect("a body");
        let one = facts.get_mut(&id).expect("facts for every body");
        let shape = one.shape.clone().expect("the shape is kept with the facts");
        if one.values.is_none() {
            let unit = summarized_in(module, program, globals, &shape, &one.exposed, None, function);
            one.values =
                llrm_support::debug::timed("summaries values", || alias::point_values(&unit).ok().map(Rc::new));
        }
        if one.calls.is_none() {
            one.calls = Some(Rc::new(alias::CallFacts::of(&summarized_in(
                module,
                program,
                globals,
                &shape,
                &one.exposed,
                one.values.as_deref(),
                function,
            ))));
        }
    }
}

/// `module`'s named bodies as alias summarizes them.
fn procedures<'a>(
    module: &'a Module,
    program: &'a ProgramProxy,
    globals: &'a Globals,
    shapes: &'a IndexMap<GlobalId, Rc<Shape>>,
    facts: &'a IndexMap<GlobalId, BodyFacts>,
) -> IndexMap<String, Procedure<'a>> {
    bodies(module)
        .filter_map(|(id, function)| {
            let name = module.global(id).name.owned()?;
            let one = &facts[&id];
            Some((
                name,
                Procedure::with(
                    summarized_in(
                        module,
                        program,
                        globals,
                        &shapes[&id],
                        &one.exposed,
                        one.values.as_deref(),
                        function,
                    ),
                    Rc::clone(one.calls.as_ref().expect("found before")),
                ),
            ))
        })
        .collect()
}

/// For each module, the summary of each of its declarations another
/// module defines, as the module names its globals: one fixed point over
/// the program, each module's `Summaries` in turn given the others'.
pub struct ProgramSummaries;

impl ProgramAnalysis for ProgramSummaries {
    type Result = Result<Vec<IndexMap<String, Summary>>, String>;
    const NAME: &'static str = "program-summaries";
    fn run(
        program: &Program,
        analyses: &mut ProgramAnalyses,
    ) -> Self::Result {
        let count = program.modules.len();
        let mut known = vec![IndexMap::default(); count];
        if count < 2 {
            return Ok(known);
        }
        let elsewhere = analyses.get::<ProgramGlobals>(program);
        let elsewhere = Result::as_ref(&*elsewhere).map_err(String::clone)?;
        let proxies: Vec<_> = (0..count).map(|at| analyses.proxy(program, at)).collect();
        let shapes: Vec<IndexMap<GlobalId, Rc<Shape>>> = program
            .modules
            .iter()
            .map(|module| bodies(module).map(|(id, function)| (id, Rc::new(Shape::of(function)))).collect())
            .collect();
        let mut exposures: Vec<IndexMap<GlobalId, BodyFacts>> = program
            .modules
            .iter()
            .map(|module| {
                let marks = bodies(module).map(|(id, function)| (id, function.mark())).collect();
                body_facts(module, &program.layout, program.target.spaces(), IndexMap::default(), &marks)
            })
            .enumerate()
            .map(|(at, mut facts)| {
                facts.iter_mut().for_each(|(id, one)| one.shape = Some(Rc::clone(&shapes[at][id])));
                facts
            })
            .collect();
        let globals = (0..count)
            .map(|at| globalsaa::found(&program.modules[at], &proxies[at], &elsewhere[at]))
            .collect::<Result<Vec<_>, String>>()?;
        // A body defined elsewhere starts as a call no summary describes,
        // the most any call does: each round only narrows.
        let mut own = vec![IndexMap::default(); count];
        loop {
            let mut changed = false;
            for at in 0..count {
                let ids: Vec<GlobalId> = bodies(&program.modules[at]).map(|(id, _)| id).collect();
                calls_found(&program.modules[at], &proxies[at], &globals[at], ids, &mut exposures[at]);
                let procedures =
                    procedures(&program.modules[at], &proxies[at], &globals[at], &shapes[at], &exposures[at]);
                let found = alias::summaries(&procedures, Some(&known[at]))?;
                let mine: IndexMap<String, Summary> =
                    found.into_iter().filter(|(name, _)| procedures.contains_key(name)).collect();
                if mine != own[at] {
                    own[at] = mine;
                    changed = true;
                    known = imported(program, &own);
                }
            }
            if !changed {
                return Ok(known);
            }
        }
    }
}

/// Each module's declarations another module defines, with that body's
/// summary in `own` as the module names its globals.
fn imported(
    program: &Program,
    own: &[IndexMap<String, Summary>],
) -> Vec<IndexMap<String, Summary>> {
    program
        .modules
        .iter()
        .enumerate()
        .map(|(at, module)| {
            module
                .functions()
                .filter(|(_, _, function)| function.is_declaration())
                .filter_map(|(id, global, _)| {
                    let (there, defined) = program.definition(at, id).filter(|&(there, _)| there != at)?;
                    let from = &program.modules[there];
                    let summary = own[there].get(from.global(defined).name.as_deref()?)?;
                    Some((global.name.owned()?, moved(summary, from, module)))
                })
                .collect()
        })
        .collect()
}

/// `summary` of a body of `from` as `to` names its globals. A global `to`
/// cannot name it cannot reach but through a pointer: one captured is
/// reached as unknown memory, and one not is not reached.
fn moved(
    summary: &Summary,
    from: &Module,
    to: &Module,
) -> Summary {
    let mut out = summary.clone();
    out.reads = carried(&summary.reads, from, to, &mut out.unknown_read).into();
    out.writes = carried(&summary.writes, from, to, &mut out.unknown_write).into();
    // A global reached as unknown memory has no access type here.
    if out.unknown_write {
        out.unknown_write_types = None;
    }
    out
}

fn carried(
    slices: &crate::sliceset::SliceSet,
    from: &Module,
    to: &Module,
    unknown: &mut bool,
) -> BTreeSet<Slice> {
    // The objects are numbered by their module's interner: each is named again
    // in the other's.
    let (source, target) = (ObjectInterner::of(&from.context), ObjectInterner::of(&to.context));
    let mut out = BTreeSet::new();
    for one in slices {
        let object = source.object(one.object);
        let (MemoryKind::Global, Some(Identity::Global(id))) = (object.kind, &object.identity) else {
            out.insert(Slice { object: target.intern(object), ..*one });
            continue;
        };
        let global = from.global(GlobalId(*id));
        let there = global
            .name
            .as_deref()
            .filter(|_| !matches!(global.linkage, Linkage::Internal | Linkage::Private))
            .and_then(|name| to.named(name));
        match there {
            Some(there) => {
                out.insert(Slice {
                    object: target.intern(MemoryObject { identity: Some(Identity::Global(there.0)), ..object }),
                    ..*one
                });
            }
            None => *unknown |= one.object.captured(),
        }
    }
    out
}

/// What the pointer analyses (`ExposedFrames`, `Pointers`,
/// `Writes`) model: an instruction that can make, move or name a pointer or an
/// aggregate, or is a call. An integer loaded, computed, stored and compared is
/// nothing to them; one that reaches an address or a call is. Where a result is
/// not so, `LLRM_CHECK_REPLAY` says.
fn models(
    context: &Context,
    function: &Function,
    inst: InstId,
    scalars_matter: bool,
) -> bool {
    let wide = |ty| {
        !matches!(
            context.types.get(ty),
            llrm_mir::types::Type::Int(_) | llrm_mir::types::Type::Float(_) | llrm_mir::types::Type::Void
        )
    };
    let one = function.instruction(inst);
    match &one.opcode {
        Opcode::Load { .. } if !scalars_matter => wide(one.ty),
        Opcode::Store { .. } if !scalars_matter => {
            one.operands.first().and_then(|value| function.operand_type(context, *value)).is_none_or(wide)
        }
        Opcode::Load { .. }
        | Opcode::Store { .. }
        | Opcode::Alloca { .. }
        | Opcode::GetElementPtr { .. }
        | Opcode::Call(_)
        | Opcode::Invoke(_)
        | Opcode::LandingPad { .. }
        | Opcode::Resume => true,
        Opcode::Cast(
            llrm_mir::opcode::CastOp::PtrToInt
            | llrm_mir::opcode::CastOp::IntToPtr
            | llrm_mir::opcode::CastOp::BitCast
            | llrm_mir::opcode::CastOp::AddrSpaceCast,
        ) => true,
        Opcode::Phi | Opcode::Select | Opcode::Freeze | Opcode::ExtractValue(_) | Opcode::InsertValue(_) => {
            wide(one.ty)
        }
        // A branch picks between blocks; none made or erased (those are not
        // asked of here) changes what a pointer reaches.
        Opcode::Br | Opcode::Switch => false,
        op => op.is_terminator(),
    }
}

fn models_scalars(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    models(context, function, inst, true)
}

fn models_wide(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    models(context, function, inst, false)
}

/// A scalar a call, a branch, a return or a store takes is nothing to a
/// pointer.
fn stops_at(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Br | Opcode::Switch | Opcode::Ret | Opcode::Store { .. }
    )
}

fn calls_only(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    let _ = context;
    matches!(
        function.instruction(inst).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    )
}

fn allocas_only(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    let _ = context;
    matches!(function.instruction(inst).opcode, Opcode::Alloca { .. })
}

/// The allocas whose address is exposed: `frameescape::exposed_allocas`, once
/// for the function where each access asked of its own alloca's uses
/// (`memory::object_of`).
pub struct ExposedFrames;

impl Analysis for ExposedFrames {
    type Result = BTreeSet<ValueId>;
    const NAME: &'static str = "exposed-frames";
    const SKIPS: bool = true;
    const INCREMENTAL: bool = true;
    fn depends() -> Option<Depends> {
        Some(Depends { models: models_scalars, keyed: allocas_only, flows: Some(stops_at), edges: false })
    }
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        crate::memory::exposed_frames(&Unit::within(context, layout, function, analyses.outer()))
    }

    /// An alloca's answer reads the instructions its address moves through
    /// and uses: the allocas behind the values the edits read are asked
    /// again, the others stand (`frameescape::exposed_renewed`).
    fn update(
        previous: &Self::Result,
        changes: &[Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        let mut touched = Vec::<ValueId>::new();
        let mut read = |operand: &Operand| {
            if let Operand::Value(value) = operand {
                touched.push(*value);
            }
        };
        for change in changes {
            match change {
                Change::Parameters => return None,
                // An exposed alloca is one of its address's uses, and a block
                // has none: an erased one was emptied by `Erased` changes, a
                // created one is filled by `Inserted` ones.
                Change::BlockCreated(_) | Change::BlockErased(_) => {}
                Change::Operand { was, .. } => read(was),
                Change::Operands { was, .. } => was.iter().for_each(&mut read),
                Change::Erased { inst, .. } => function.instruction(*inst).operands.iter().for_each(&mut read),
                Change::Inserted { inst, .. }
                | Change::Moved { inst, .. }
                | Change::Cloned { to: inst, .. }
                | Change::Rewritten(inst) => {
                    if function.is_erased(*inst) {
                        continue;
                    }
                    let instruction = function.instruction(*inst);
                    instruction.operands.iter().for_each(&mut read);
                    if let Some(result) = instruction.result {
                        read(&Operand::Value(result));
                    }
                }
                Change::Substituted { .. } => {}
            }
        }
        let unit = Unit::within(context, layout, function, analyses.outer());
        Some(
            crate::frameescape::exposed_renewed(function, previous.iter().copied(), &touched, |inst, at| {
                crate::memory::kept_by(&unit, inst, at)
            })
            .into_iter()
            .collect(),
        )
    }
}

/// Every pointer's objects, with no caller context: `alias::pointers`.
pub struct Pointers;

/// What the pointer solve finds of a body before it asks what escapes:
/// `alias::point_values`. The call arguments and captures never enter it, so
/// `Pointers` and `Writes` ask this one and each runs only its own escape
/// phase.
pub struct PointerValues;

impl Analysis for PointerValues {
    type Result = Result<alias::PointValues, String>;
    const NAME: &'static str = "pointer-values";
    const SKIPS: bool = true;
    fn depends() -> Option<Depends> {
        Some(Depends { models: models_scalars, keyed: models_scalars, flows: Some(stops_at), edges: false })
    }
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        alias::point_values(
            &Unit::within(context, layout, function, analyses.outer()).with_shape(&shape).with_exposed(&exposed),
        )
    }
}

impl Analysis for Pointers {
    type Result = Result<PointsTo, String>;
    const NAME: &'static str = "points-to";
    const SKIPS: bool = true;
    const AUDITED: bool = true;
    fn audit(
        result: &Self::Result,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) {
        if let Ok(facts) = result {
            let unit = Unit::within(context, layout, function, analyses.outer());
            // A callee the module defines is read in its body as it stood
            // before this pass.
            let snapshot = llrm_mir::passes::MODULE_SNAPSHOT.with(|held| held.borrow().clone());
            let verify = snapshot
                .as_ref()
                .map(
                    |module| {
                        let mut verify = crate::capturecheck::Verify::new(module, layout, 8);
                        verify.trust_external = true;
                        verify
                    },
                );
            crate::capturecheck::check_with(&unit, facts, verify.as_ref());
        }
    }
    fn depends() -> Option<Depends> {
        Some(Depends { models: models_scalars, keyed: models_scalars, flows: Some(stops_at), edges: true })
    }
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let values = analyses.get::<PointerValues>(context, layout, function);
        let mut unit = Unit::within(context, layout, function, analyses.outer())
            .with_shape(&shape)
            .with_assumptions(&assumptions)
            .with_exposed(&exposed);
        if let Ok(values) = &*values {
            unit = unit.with_point_values(values);
        }
        alias::points_to(&unit, None, None)
    }
}

/// Each load's and store's reference: `alias::annotated`.
pub struct Annotated;

impl Analysis for Annotated {
    type Result = Result<IndexMap<InstId, MemRef>, String>;
    const NAME: &'static str = "annotated";
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let pointers = analyses.get::<PointerValues>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let pointers = Result::as_ref(&*pointers).map_err(String::clone)?;
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let counted = analyses.get::<Counted>(context, layout, function);
        let edges = analyses.get::<DominatedEdges>(context, layout, function);
        let mut unit = Unit::within(context, layout, function, analyses.outer())
            .with_shape(&shape)
            .with_assumptions(&assumptions)
            .with_registers(&registers)
            .with_point_values(pointers)
            .with_exposed(&exposed)
            .with_counted(&counted);
        if let Ok(edges) = &*edges {
            unit = unit.with_edges(edges);
        }
        alias::annotated_with(&unit, pointers, &registers)
    }
}

/// The constants the body holds as values, which is none: see `Registers::run`.
pub struct Registers;

/// The loads from a constant global by an index (`consts::is_table_load`), in
/// order of instruction. A change reaches a load by the instruction itself or
/// by the values its address is made of, so those loads, and only those, are
/// asked again.
pub struct TableLoads;

impl Analysis for TableLoads {
    type Result = Vec<InstId>;
    const NAME: &'static str = "table-loads";
    const INCREMENTAL: bool = true;

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let mut found = consts::table_loads(&Unit::within(context, layout, function, analyses.outer()));
        found.sort_unstable();
        found
    }

    fn update(
        previous: &Self::Result,
        changes: &[Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        let unit = Unit::within(context, layout, function, analyses.outer());
        let mut work: Vec<InstId> = Vec::new();
        for change in changes {
            match *change {
                Change::Inserted { inst, .. }
                | Change::Erased { inst, .. }
                | Change::Moved { inst, .. }
                | Change::Rewritten(inst) => work.push(inst),
                Change::Cloned { to, .. } => work.push(to),
                // Each use of a substituted value is rewritten, in an entry of
                // its own.
                Change::BlockCreated(_)
                | Change::BlockErased(_)
                | Change::Operand { .. }
                | Change::Operands { .. }
                | Change::Substituted { .. } => {}
                Change::Parameters => return None,
            }
        }
        let mut seen: llrm_mir::dense::IdSet<InstId> = work.iter().copied().collect();
        let mut asked = llrm_mir::dense::IdSet::<InstId>::new();
        while let Some(inst) = work.pop() {
            if function.parent(inst).is_none() {
                continue;
            }
            if matches!(function.instruction(inst).opcode, Opcode::Load { .. }) {
                asked.insert(inst);
                continue;
            }
            // `MemRef::of` reads a load's address through bitcasts and address
            // arithmetic alone (`memory::step`): a change reaches a load by the
            // users that are loads or those, and by no other use.
            if let Some(result) = function.instruction(inst).result {
                for user in function.users(result) {
                    if matches!(
                        function.instruction(user.user).opcode,
                        Opcode::Load { .. }
                            | Opcode::GetElementPtr { .. }
                            | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast | CastOp::IntToPtr)
                    ) && seen.insert(user.user)
                    {
                        work.push(user.user);
                    }
                }
            }
        }
        let mut found: llrm_mir::dense::IdSet<InstId> = previous
            .iter()
            .copied()
            .filter(|&inst| function.parent(inst).is_some() && !asked.contains(&inst))
            .collect();
        found.extend(asked.iter().filter(|&inst| consts::is_table_load(&unit, inst)));
        Some(found.iter().collect())
    }
}

impl Analysis for Registers {
    type Result = IndexMap<ValueId, Known>;
    const NAME: &'static str = "registers";
    const READS_OUTER: bool = false;

    /// Empty: a constant is folded into the body where it appears (SCCP,
    /// InstSimplify, `fold`), as the pin does, so no map of them is kept. The
    /// fixed point this used to derive (`consts::known`) found 72 values in
    /// 25033 instructions of d_faces -O2, and every object, row, run and
    /// QCport module was the same without it.
    fn run(
        _: &Context,
        _: &DataLayout,
        _: &Function,
        _: &mut Analyses,
    ) -> Self::Result {
        IndexMap::default()
    }
}

/// The bits of each integer value that anything reads (`demanded::demanded`),
/// LLVM's DemandedBits.
pub struct DemandedBits;

impl Analysis for DemandedBits {
    type Result = llrm_mir::dense::IdMap<ValueId, u128>;
    const NAME: &'static str = "demanded";
    // The function alone: no module or target is read.
    const READS_OUTER: bool = false;
    fn run(
        context: &Context,
        _layout: &DataLayout,
        function: &Function,
        _analyses: &mut Analyses,
    ) -> Self::Result {
        crate::demanded::demanded(context, function)
    }
}

/// What each block assumes, LLVM's AssumptionCache: a unit that carries none
/// found them again, a walk of the whole body, at every `guards` and `ranges`
/// query.
pub struct AssumptionCache;

impl Analysis for AssumptionCache {
    type Result = crate::assumptions::Assumptions;
    const NAME: &'static str = "assumptions";
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        crate::assumptions::Assumptions::of(&Unit::within(context, layout, function, analyses.outer()))
    }
}

/// Each loop's followers of its counters, under `Registers`:
/// `induction::followers_all`.
pub struct Followers;

impl Analysis for Followers {
    type Result = induction::LoopFollowers;
    const NAME: &'static str = "followers";

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        induction::followers_all(
            &Unit::within(context, layout, function, analyses.outer())
                .with_shape(&shape)
                .with_assumptions(&assumptions)
                .with_registers(&registers)
                .with_exposed(&exposed),
        )
    }
}

/// Each loop's counted proofs, under `Registers`: `induction::counted_all`.
pub struct Counted;

impl Analysis for Counted {
    type Result = induction::Counted;
    const NAME: &'static str = "counted";
    const INCREMENTAL: bool = true;

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        induction::counted_all(
            &Unit::within(context, layout, function, analyses.outer())
                .with_shape(&shape)
                .with_assumptions(&assumptions)
                .with_registers(&registers)
                .with_exposed(&exposed),
        )
    }

    /// A loop's proofs read its own blocks, and the values its operands come
    /// from, which `Registers` also derives from their operands alone. So
    /// they hold until a change to an instruction reaches the loop through the
    /// uses of the values it makes: a loop none reaches keeps its proofs. A
    /// change to the CFG, or to the loops themselves, derives them afresh.
    fn update(
        previous: &Self::Result,
        changes: &[Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        let reach = function.reach_of(changes)?;
        let (reached, cfg_changed) = (reach.blocks.clone(), reach.cfg_changed());
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        if !shape.loops.iter().map(|one| one.header).eq(previous.keys().copied()) {
            return None;
        }
        let registers = analyses.get::<Registers>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer())
            .with_shape(&shape)
            .with_assumptions(&assumptions)
            .with_registers(&registers)
            .with_exposed(&exposed);
        // A change to the CFG moves a loop's blocks or what enters it only by
        // a terminator it rewrites, in the loop or in a block that enters it;
        // and the guards over its entry (`guards::guards`) are the
        // branches of the blocks that dominate it.
        Some(induction::counted_renewed(&unit, previous, |one| {
            one.body.iter().any(|at| reached.contains(&crate::cfg::block(*at)))
                || (cfg_changed
                    && (function.predecessors(crate::cfg::block(one.header)).iter().any(|pred| reached.contains(pred))
                        || reached.iter().any(|r| shape.dominance.dominates(function, crate::cfg::id(r), one.header))))
        }))
    }
}

/// What each call reads and writes, as its attributes and `GlobalsModRef` say
/// (`alias::calls_by_attributes`), made once for the body: `Writes` and
/// `Accesses` take it. Each made it afresh (39 and 11 walks of the body's
/// calls on d_faces -O2, 32 M Ir) where `CallEffects` was kept past the edits
/// that reach no call.
pub struct CallFootprints;

impl Analysis for CallFootprints {
    type Result = Result<std::rc::Rc<llrm_mir::dense::IdMap<InstId, alias::Effect>>, String>;
    const NAME: &'static str = "call-footprints";
    const SKIPS: bool = true;
    fn depends() -> Option<Depends> {
        Some(Depends { models: models_wide, keyed: calls_only, flows: Some(stops_at), edges: true })
    }
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        // The unit `Accesses` made them under: without the shape and the
        // exposed frames each argument's object was worked out from the
        // body again (3x the Ir a walk).
        let values = analyses.get::<PointerValues>(context, layout, function);
        let shape = analyses.get::<Shape>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let mut unit =
            Unit::within(context, layout, function, analyses.outer()).with_shape(&shape).with_exposed(&exposed);
        if let Ok(values) = &*values {
            unit = unit.with_point_values(values);
        }
        Ok(std::rc::Rc::new(alias::calls_by_attributes(&unit)))
    }
}

/// What each call writes: the stores of `CallFootprints`, the `Calls` consts
/// and floatfacts take.
pub struct Writes;

impl Analysis for Writes {
    type Result = Result<Calls, String>;
    const NAME: &'static str = "writes";
    const SKIPS: bool = true;
    fn depends() -> Option<Depends> {
        Some(Depends { models: models_wide, keyed: calls_only, flows: Some(stops_at), edges: true })
    }
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let footprints = analyses.get::<CallFootprints>(context, layout, function);
        let footprints = Result::as_ref(&*footprints).map_err(String::clone)?;
        Ok(footprints.iter().map(|(at, effect)| (at, effect.stores.clone())).collect())
    }
}

/// `Writes`, or where it failed none: each call then writes what
/// `memory::unmodeled_write` says.
pub fn writes(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    analyses: &mut Analyses,
) -> Calls {
    Result::as_ref(&*analyses.get::<Writes>(context, layout, function)).cloned().unwrap_or_default()
}

/// The body's accesses as `Accesses::plain` makes them, each call writing what
/// `Writes` says, with the graph they make kept in them: one per body state for
/// `ThroughMemory`, `FloatFacts` and each walk they make, where each of them
/// made the graph again (a whole-function pass of the body, 147 times for
/// d_faces' 48 float solves and 63 through-memory ones).
pub struct PlainAccesses;

impl Analysis for PlainAccesses {
    type Result = Result<std::rc::Rc<memoryssa::Accesses>, String>;
    const NAME: &'static str = "plain-accesses";
    const INCREMENTAL: bool = true;

    /// The accesses as they were, with the structure they made, when the
    /// edits made or moved no memory op, no terminator and no block, and what
    /// the accesses read is as it was.
    fn update(
        previous: &Self::Result,
        changes: &[llrm_mir::module::Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        let held = previous.as_ref().ok()?;
        let was = held.plain_source()?;
        let writes = analyses.get::<Writes>(context, layout, function);
        let annotated = analyses.get::<Annotated>(context, layout, function);
        let same = std::rc::Rc::ptr_eq(analyses.outer(), &was.outer)
            && (std::rc::Rc::ptr_eq(&writes, &was.writes) || *writes == *was.writes)
            && (std::rc::Rc::ptr_eq(&annotated, &was.annotated) || *annotated == *was.annotated);
        (same && memoryssa::structure_kept(changes, function)).then(|| Ok(std::rc::Rc::new(held.carried(function))))
    }
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let writes_rc = analyses.get::<Writes>(context, layout, function);
        let calls = Result::as_ref(&*writes_rc).map_err(String::clone)?.clone();
        let annotated_rc = analyses.get::<Annotated>(context, layout, function);
        let references = Result::as_ref(&*annotated_rc).map_err(String::clone)?;
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer())
            .with_references(references)
            .with_exposed(&exposed);
        let source = memoryssa::PlainSource {
            annotated: std::rc::Rc::clone(&annotated_rc),
            writes: std::rc::Rc::clone(&writes_rc),
            outer: std::rc::Rc::clone(analyses.outer()),
        };
        Ok(std::rc::Rc::new(memoryssa::Accesses::plain(&unit, &calls).with_plain_source(source)))
    }
}

/// The body's accesses with each reference as it stands (`MemRef::of`), no
/// provenance annotated: what MemorySSA's clients read once the AA stack
/// answers every pair (`Annotated` and its inputs are not asked).
pub struct BaseAccesses;

impl Analysis for BaseAccesses {
    type Result = Result<std::rc::Rc<memoryssa::Accesses>, String>;
    const NAME: &'static str = "base-accesses";

    fn run(
        _: &Context,
        _: &DataLayout,
        _: &Function,
        _: &mut Analyses,
    ) -> Self::Result {
        // Nothing is made up front: a reference, and what an instruction
        // touches, are asked of the unit that reads them (`Accesses::lazy`).
        Ok(std::rc::Rc::new(memoryssa::Accesses::lazy()))
    }
}

/// A body's translated MemorySSA. The accesses it is read with are the
/// manager's `BaseAccesses`, fetched by the first client that asks
/// (`accesses`): building the graph needs none of their references.
pub struct MssaGraph {
    pub mssa: crate::mssa::Mssa,
    accesses: std::cell::OnceCell<std::rc::Rc<memoryssa::Accesses>>,
}

impl MssaGraph {
    /// A graph with the accesses a pass that updated it kept, if it did.
    pub fn new(
        mssa: crate::mssa::Mssa,
        accesses: Option<std::rc::Rc<memoryssa::Accesses>>,
    ) -> Self {
        Self { mssa, accesses: accesses.map(std::cell::OnceCell::from).unwrap_or_default() }
    }

    /// The accesses of the body this graph is of: the ones kept with it, else
    /// the manager's.
    pub fn accesses(
        &self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Result<std::rc::Rc<memoryssa::Accesses>, String> {
        if let Some(held) = self.accesses.get() {
            return Ok(std::rc::Rc::clone(held));
        }
        let base = analyses.get::<BaseAccesses>(context, layout, function);
        let made = Result::as_ref(&*base).map_err(String::clone)?;
        Ok(std::rc::Rc::clone(self.accesses.get_or_init(|| std::rc::Rc::clone(made))))
    }

    /// Whether the accesses have been fetched or kept.
    pub fn has_accesses(&self) -> bool {
        self.accesses.get().is_some()
    }
}

/// Two graphs are equal when every access says the same (`shadow::said`): the
/// manager's check of a preserved set compares a graph with the one made again.
/// A use the walker answered for is optimized: its defining access is the
/// walker's result, not the nearest def ([`setDefiningAccess(DMA,
/// Optimized)`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L293),
/// [`MemoryUse::isOptimized`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L337)),
/// so those names are not compared.
impl PartialEq for MssaGraph {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        let (mut held, mut fresh) = (crate::mssa::shadow::said(&self.mssa), crate::mssa::shadow::said(&other.mssa));
        // A use the walker moved up stands for the same load as the nearest
        // def of a graph made afresh.
        for inst in crate::mssa::shadow::optimized_uses(&self.mssa) {
            if let (Some(kept), Some(now)) = (held.instructions.get_mut(&inst), fresh.instructions.get_mut(&inst))
                && !kept.0
                && !now.0
            {
                kept.1 = crate::mssa::shadow::Name::Live;
                now.1 = crate::mssa::shadow::Name::Live;
            }
        }
        held == fresh
    }
}

impl std::fmt::Debug for MssaGraph {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str("MssaGraph")
    }
}

/// `MemorySSAAnalysis`: the graph `mssa::Mssa::build` makes of a body, once per
/// body state. A pass that edits it takes a clone (`Mssa: Clone`) and updates
/// that; the manager drops the cached graph with the pass's edit.
pub struct MemorySsa;

impl Analysis for MemorySsa {
    type Result = Result<std::rc::Rc<MssaGraph>, String>;
    const NAME: &'static str = "mssa";

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let shape = analyses.get::<Shape>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer()).with_exposed(&exposed);
        // `AAResults::getModRefInfo(Call, nullptr)` over the pin's stack.
        let mut stack = crate::aggregate::AggregateAA::new().with_unit(&unit);
        stack.add_aa_result(crate::basicaa::BasicAA::new(&unit));
        if let Some(info) = unit.globals_mr {
            stack.add_aa_result(crate::globalsmodref::GlobalsAAResult::new(&unit, info));
        }
        let mssa =
            crate::mssa::Mssa::build(function, &shape.dominance, &crate::mssa::shadow::effect_from_aa(&unit, stack));
        Ok(std::rc::Rc::new(MssaGraph::new(mssa, None)))
    }
}

/// Float values, integers and memory in one solve, each call writing what
/// `writes` says: `floatfacts::solved_with`.
pub struct FloatFacts;

thread_local! {
    static FLOAT_SOLVES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has solved a function's floats.
pub fn float_solves() -> usize {
    FLOAT_SOLVES.with(std::cell::Cell::get)
}

impl Analysis for FloatFacts {
    type Result = floatfacts::Solved;
    const NAME: &'static str = "float-facts";
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        FLOAT_SOLVES.with(|solves| solves.set(solves.get() + 1));
        let calls = writes(context, layout, function, analyses);
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let references = analyses.get::<Annotated>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        // The integers under it are `ThroughMemory`'s, which others ask too.
        let through = analyses.get_exact::<ThroughMemory>(context, layout, function);
        let plain = analyses.get::<PlainAccesses>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer())
            .with_shape(&shape)
            .with_assumptions(&assumptions)
            .with_registers(&registers)
            .with_annotated(&references)
            .with_exposed(&exposed);
        let unit = match &*plain {
            Ok(plain) => unit.with_plain(plain),
            Err(_) => unit,
        };
        match &*through {
            Ok(integers) => floatfacts::solved_over(&unit, &calls, integers),
            Err(_) => floatfacts::solved_with(&unit, &calls),
        }
    }
}

/// Every value known, memory solved alongside and each call writing what
/// `Writes` says.
pub struct ThroughMemory;

impl Analysis for ThroughMemory {
    type Result = Result<IndexMap<ValueId, Known>, String>;
    const NAME: &'static str = "through-memory";
    const SOUND: bool = true;
    /// What it says of a live value is a number the value has, which edits
    /// that keep every value's meaning do not change; a fresh derivation may
    /// know more (a constant folded), never another number.
    fn covers(
        old: &Self::Result,
        fresh: &Self::Result,
        _context: &Context,
        function: &Function,
    ) -> bool {
        match (old, fresh) {
            (Ok(old), Ok(fresh)) => old.iter().all(|(value, fact)| {
                use llrm_mir::module::ValueDef;
                match function.value(*value).def {
                    ValueDef::Instruction(inst) if function.is_erased(inst) => true,
                    _ => fresh.get(value).is_none_or(|now| now == fact),
                }
            }),
            (old, fresh) => old == fresh,
        }
    }
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let calls = Result::as_ref(&*analyses.get::<Writes>(context, layout, function)).map_err(String::clone)?.clone();
        let references = analyses.get::<Annotated>(context, layout, function);
        let references = Result::as_ref(&*references).map_err(String::clone)?;
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let plain = analyses.get::<PlainAccesses>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer())
            .with_references(references)
            .with_shape(&shape)
            .with_assumptions(&assumptions)
            .with_registers(&registers)
            .with_exposed(&exposed);
        let unit = match &*plain {
            Ok(plain) => unit.with_plain(plain),
            Err(_) => unit,
        };
        Ok(consts::known_walked(&unit, &calls))
    }
}

/// What unavoidable branch edges bound at each block:
/// `ranges::dominated_edges`.
pub struct DominatedEdges;

impl Analysis for DominatedEdges {
    type Result = Result<crate::ranger::EdgeFacts, String>;
    const NAME: &'static str = "dominated-edges";
    const INCREMENTAL: bool = true;

    /// Nothing is worked out until a pass asks for a value at a block
    /// (`ranger`): GCC's `ranger_cache` starts empty too.
    fn run(
        _: &Context,
        _: &DataLayout,
        _: &Function,
        _: &mut Analyses,
    ) -> Self::Result {
        Ok(crate::ranger::EdgeFacts::default())
    }

    /// An answer is of the block's own operations, the edges and assumes that
    /// dominate it and the facts of the values they read: a change drops the
    /// answers of the blocks it reaches and of those they dominate. A change to
    /// the CFG drops them all.
    fn update(
        previous: &Self::Result,
        changes: &[Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        Self::update_owned(Rc::new(previous.clone()), changes, context, layout, function, analyses)
    }

    fn update_owned(
        previous: Rc<Self::Result>,
        changes: &[Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        previous.as_ref().as_ref().ok()?;
        let reach = function.reach_of(changes)?;
        if reach.cfg_changed() {
            return Some(Ok(crate::ranger::EdgeFacts::default()));
        }
        let shape = analyses.get::<Shape>(context, layout, function);
        let reached: llrm_mir::dense::IdSet<i64> = reach.blocks.iter().map(crate::cfg::id).collect();
        let held = Rc::try_unwrap(previous).unwrap_or_else(|shared| (*shared).clone()).ok()?;
        Some(Ok(held.keeping(|at| !dominated_by(&shape, function, &reached, at))))
    }
}

/// Whether `at`, or a block above it, is one of `reached`.
pub fn dominated_by(
    shape: &Shape,
    function: &Function,
    reached: &llrm_mir::dense::IdSet<i64>,
    at: i64,
) -> bool {
    let mut above = Some(at);
    while let Some(one) = above {
        if reached.contains(&one) {
            return true;
        }
        above = shape.dominance.immediate(function, one);
    }
    false
}

/// What a pass asks of the ranges of a body as it stands: the manager's
/// registers and edges, and the unit that carries them.
pub struct Ranges {
    shape: Rc<Shape>,
    assumptions: Rc<<AssumptionCache as Analysis>::Result>,
    registers: Rc<<Registers as Analysis>::Result>,
    exposed: Rc<<ExposedFrames as Analysis>::Result>,
    counted: Rc<<Counted as Analysis>::Result>,
    edges: Rc<<DominatedEdges as Analysis>::Result>,
}

impl Ranges {
    pub fn of(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self {
        Self {
            shape: analyses.get::<Shape>(context, layout, function),
            assumptions: analyses.get::<AssumptionCache>(context, layout, function),
            registers: analyses.get::<Registers>(context, layout, function),
            exposed: analyses.get::<ExposedFrames>(context, layout, function),
            counted: analyses.get::<Counted>(context, layout, function),
            edges: analyses.get::<DominatedEdges>(context, layout, function),
        }
    }

    pub fn registers(&self) -> &Rc<<Registers as Analysis>::Result> {
        &self.registers
    }

    pub fn unit<'a>(
        &'a self,
        context: &'a Context,
        layout: &'a DataLayout,
        function: &'a Function,
        outer: &'a Outer,
    ) -> Unit<'a> {
        let mut unit = Unit::within(context, layout, function, outer)
            .with_shape(&self.shape)
            .with_assumptions(&self.assumptions)
            .with_registers(&self.registers)
            .with_exposed(&self.exposed)
            .with_counted(&self.counted);
        if let Ok(edges) = &*self.edges {
            unit = unit.with_edges(edges);
        }
        unit
    }
}

/// What the manager holds of a function that a unit over it carries, asked
/// while the body is as the manager last saw it: dominance and loops, what
/// is known without memory and, where asked for, what alias finds.
pub struct Held {
    shape: Rc<Shape>,
    exposed: Rc<BTreeSet<ValueId>>,
    registers: Rc<IndexMap<ValueId, Known>>,
    assumptions: Rc<crate::assumptions::Assumptions>,
    values: Option<Rc<<PointerValues as Analysis>::Result>>,
    annotated: Option<Rc<<Annotated as Analysis>::Result>>,
    counted: Option<Rc<<Counted as Analysis>::Result>>,
    followers: Option<Rc<<Followers as Analysis>::Result>>,
}

impl Held {
    pub fn of(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
        alias: bool,
    ) -> Self {
        Self {
            shape: analyses.get::<Shape>(context, layout, function),
            exposed: analyses.get::<ExposedFrames>(context, layout, function),
            registers: analyses.get::<Registers>(context, layout, function),
            assumptions: analyses.get::<AssumptionCache>(context, layout, function),
            values: alias.then(|| analyses.get::<PointerValues>(context, layout, function)),
            annotated: alias.then(|| analyses.get::<Annotated>(context, layout, function)),
            // Annotated has proved them already.
            counted: alias.then(|| analyses.get::<Counted>(context, layout, function)),
            followers: alias.then(|| analyses.get::<Followers>(context, layout, function)),
        }
    }

    /// `of` without the memory annotations and points-to (`PointerValues`,
    /// `Annotated`): the loop facts and what is known without memory, for a
    /// pass that decides from the values.
    pub fn of_without_memory(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self {
        Self {
            values: None,
            annotated: None,
            counted: Some(analyses.get::<Counted>(context, layout, function)),
            followers: Some(analyses.get::<Followers>(context, layout, function)),
            ..Self::of(context, layout, function, analyses, false)
        }
    }

    /// A unit over `function`, the body these were found of.
    pub fn unit<'a>(
        &'a self,
        context: &'a Context,
        layout: &'a DataLayout,
        function: &'a Function,
        outer: &'a Outer,
    ) -> Unit<'a> {
        let mut unit = Unit::within(context, layout, function, outer)
            .with_shape(&self.shape)
            .with_registers(&self.registers)
            .with_assumptions(&self.assumptions)
            .with_exposed(&self.exposed);
        if let Some(Ok(values)) = self.values.as_deref() {
            unit = unit.with_point_values(values);
        }
        if let Some(annotated) = self.annotated.as_deref() {
            unit = unit.with_annotated(annotated);
        }
        if let Some(counted) = self.counted.as_deref() {
            unit = unit.with_counted(counted);
        }
        if let Some(followers) = self.followers.as_deref() {
            unit = unit.with_followers(followers);
        }
        unit
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;

/// `LLRM_CHECK_ORDER`: some function analyses asked of `function`, each when
/// a hash of `mix` and its number falls on one in eight. A result a pass reads
/// must be a function of the body, not of when something first asked; asked
/// early and left to be kept past the edits after it, an analysis shows it.
pub fn ask_some(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    analyses: &mut Analyses,
    mix: u64,
) {
    let mut number = 0u64;
    let mut chosen = || {
        number += 1;
        (mix.wrapping_add(number).wrapping_mul(0xD6E8_FEB8_6659_FD93) >> 40) % 8 == 0
    };
    if chosen() {
        analyses.get::<Shape>(context, layout, function);
    }
    if chosen() {
        analyses.get::<ExposedFrames>(context, layout, function);
    }
    if chosen() {
        analyses.get::<PointerValues>(context, layout, function);
    }
    if chosen() {
        analyses.get::<Pointers>(context, layout, function);
    }
    if chosen() {
        analyses.get::<Annotated>(context, layout, function);
    }
    if chosen() {
        analyses.get::<Writes>(context, layout, function);
    }
    if chosen() {
        analyses.get::<Registers>(context, layout, function);
    }
    if chosen() {
        analyses.get::<DemandedBits>(context, layout, function);
    }
    if chosen() {
        analyses.get::<AssumptionCache>(context, layout, function);
    }
    if chosen() {
        analyses.get::<Followers>(context, layout, function);
    }
    if chosen() {
        analyses.get::<Counted>(context, layout, function);
    }
    if chosen() {
        analyses.get::<PlainAccesses>(context, layout, function);
    }
    if chosen() {
        analyses.get::<ThroughMemory>(context, layout, function);
    }
    if chosen() {
        analyses.get::<FloatFacts>(context, layout, function);
    }
    if chosen() {
        analyses.get::<DominatedEdges>(context, layout, function);
    }
    if chosen() {
        analyses.get::<TableLoads>(context, layout, function);
    }
    if chosen() {
        let _ = memoryssa::Accesses::managed(context, layout, function, analyses);
    }
}
