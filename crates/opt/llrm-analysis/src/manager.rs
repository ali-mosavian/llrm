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
//!   through. `ExposedFrames`, `PointerValues`, `Pointers`, `CallEffects` and
//!   `Writes` model pointers, aggregates and calls only, so a change to integer
//!   arithmetic leaves them standing.
//! - `INCREMENTAL` with `update`: the entry is brought up to date from the
//!   change log. `DominatedEdges`, `Bounded` and `Counted` redo only the blocks
//!   or loops `Touched::reach` the changes (a change to a terminator or to the
//!   CFG redoes all); `Accesses` in [`crate::memoryssa`] survives pure moves.
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
//!  CallEffects      PointerValues
//!  Writes           CallEffects
//!  TableLoads       -
//!  Registers        TableLoads (updates)
//!  Followers        Registers
//!  Counted          Registers
//!  DominatedEdges   Registers
//!  Bounded          Registers, Counted, DominatedEdges
//!  Annotated        PointerValues, Registers, Counted, DominatedEdges, Bounded
//!  Accesses         Annotated, CallEffects                  (memoryssa)
//!  PlainAccesses    Writes, Annotated                       (the graph's structure, once)
//!  ThroughMemory    PlainAccesses, Writes, Annotated, Registers
//!  FloatFacts       ThroughMemory, PlainAccesses, Writes, Annotated, Registers
//!  Published        Pointers, ExposedFrames                 (observers)
//! ```
//!
//! Module entries read the program through `ModuleAnalyses` and are dropped
//! unless every run of a pass preserved them. `GlobalsAA` (which globals no
//! outside code reaches but by name) and `Summaries` (each defined function's
//! memory effects, by name) can be held past passes that add no memory
//! operation when `covers` says the stale result states no more than a fresh
//! one. `Summaries` keeps per-body facts and recomputes only bodies whose
//! history moved. `Callbacks`, the effects of all the callbacks into the module
//! added up, is rebuilt only when `GlobalsAA` or `Summaries` is a different
//! result. `ProgramSummaries` is a program entry: one fixed point over all
//! modules, each module's `Summaries` given the others'.
//!
//! `Unit::within` builds the `Unit` that analyses read from `Outer` (the
//! module, target and program), carrying whichever cached results the caller
//! holds. `Held` and `Ranges` bundle the cached results a pass passes on to a
//! `Unit`, so the facts are the manager's and not a solve of the pass's own.
//! `Bounded` is empty where `Outer::loop_intervals` is off (-O1).
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
//! | change                       | `Pointers`, `CallEffects`, `Writes`, `ExposedFrames` |
//! |------------------------------|------------------------------------------------------|
//! | `%x` replaced by `%a`        | kept (`unaffected` is true)                          |
//! | `%q`, the address, rewritten | `CallEffects::unaffected` is false: recomputed        |
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
//! `CallEffects`, `Writes`, `Registers`, `DemandedBits`, `AssumptionCache`,
//! `Followers`, `Counted`, `ThroughMemory`, `FloatFacts`, `DominatedEdges`,
//! `Bounded`; module entries `GlobalsAA`, `Summaries`, `Callbacks`; program
//! entry `ProgramSummaries`; `Held`, `Ranges`, `Touched::reach`,
//! `call_effects`, `writes`. [`crate::cfg`]'s `Shape`, [`crate::memoryssa`]'s
//! `Accesses`, [`crate::observers`]'s `Published` and [`crate::noreturn`]'s
//! `TerminalSites` are entries defined beside their analyses.
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
use llrm_mir::module::{Change, Function, GlobalValue, InstId, Linkage, Mark, Module, Operand, ValueId};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::passes::{Analyses, Analysis, Declarations, ModuleAnalyses, ModuleAnalysis, Outer};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramAnalysis, ProgramProxy};
use llrm_support::debug::counted;
use llrm_support::hash::IndexMap;

use crate::alias::{self, Effect, PointsTo, Procedure, Summary};
use crate::cfg::Shape;
use crate::consts::{self, Calls, Known};
use crate::floatfacts;
use crate::globalsaa::{self, Globals, ProgramGlobals};
use crate::induction;
use crate::memory::{Identity, MemRef, MemoryKind, MemoryObject, ObjectInterner, Slice, Unit};
use crate::memoryssa;
use crate::ranges;

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
            references: None,
            shape: None,
            registers: None,
            pointers: None,
            annotated: None,
            assumptions: None,
            counted: None,
            followers: None,
            edges: None,
            bounds: None,
            exposed: None,
            plain: None,
            point_values: None,
            callbacks: None,
            callbacks_from: Some(outer),
            loop_intervals: outer.loop_intervals(),
            congruent: None,
            proved: None,
        }
    }
}

/// What calling back into the module may do: the effects of GlobalsAA's
/// entries added up. Every body that calls something unknown reads it, and it
/// depends on the entries and the summaries alone, so it is made once for them
/// and kept while both are the same results.
pub struct Callbacks;

/// The `GlobalsAA` and `Summaries` results `Callbacks` was last made from.
#[derive(Default)]
struct CallbacksSeen(Option<(Rc<Result<Globals, String>>, Rc<Result<IndexMap<String, Summary>, String>>)>);

impl ModuleAnalysis for Callbacks {
    type Result = Result<Option<Summary>, String>;
    const NAME: &'static str = "callbacks";
    fn run(
        module: &Module,
        analyses: &mut ModuleAnalyses,
    ) -> Self::Result {
        let globals = analyses.get::<GlobalsAA>(module);
        let known = analyses.get::<Summaries>(module);
        analyses.memo::<CallbacksSeen>().0 = Some((Rc::clone(&globals), Rc::clone(&known)));
        Ok(alias::callbacks_over(
            Result::as_ref(&*globals).map_err(String::clone)?,
            Result::as_ref(&*known).map_err(String::clone)?,
        ))
    }
    fn unchanged(
        module: &Module,
        analyses: &mut ModuleAnalyses,
        _: &Self::Result,
    ) -> bool {
        let globals = analyses.get::<GlobalsAA>(module);
        let known = analyses.get::<Summaries>(module);
        analyses
            .memo::<CallbacksSeen>()
            .0
            .as_ref()
            .is_some_and(|(then, before)| Rc::ptr_eq(then, &globals) && Rc::ptr_eq(before, &known))
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

/// Each defined function's memory effects, by name: `alias::summaries` of
/// the whole module, its globals as `GlobalsAA` finds them and the bodies
/// other modules define as the cached `ProgramSummaries` says.
pub struct Summaries;

impl ModuleAnalysis for Summaries {
    type Result = Result<IndexMap<String, Summary>, String>;
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
        let globals_held = analyses.get::<GlobalsAA>(module);
        let globals = Result::as_ref(&*globals_held).map_err(String::clone)?;
        let program = Rc::clone(analyses.program());
        let known = match program.cached::<ProgramSummaries>() {
            Some(all) => Some(Result::as_ref(&*all).map_err(String::clone)?[program.module].clone()),
            None => None,
        };
        let declarations = analyses.get::<Declarations>(module);
        let scratch = analyses.from_scratch();
        let logged = analyses.edit_log().len();
        let epoch = analyses.log_epoch();
        // Bodies reported edited since the last run, where the globals' facts
        // and the declarations are what it was made under and no body was
        // added, removed or renamed: only those are looked at. The full scan
        // of marks below is the reference (`LLRM_CHECK_SUMMARIES`).
        if !scratch
            && let Some(found) =
                Self::brought_up(module, analyses, &program, &globals_held, &declarations, &known, logged)
        {
            return found;
        }
        let marks: IndexMap<GlobalId, Mark> = bodies(module).map(|(id, function)| (id, function.mark())).collect();
        // What a body's calls and exposed frames are depends on the body and
        // the declarations: kept while neither moved.
        let memo = analyses.memo::<SummariesMemo>();
        let kept = if scratch || !memo.declarations.as_ref().is_some_and(|then| Rc::ptr_eq(then, &declarations)) {
            IndexMap::default()
        } else {
            std::mem::take(&mut memo.facts)
        };
        // A body's shape is of its body alone: the one kept with its facts
        // stands while its history is where they left it.
        let shapes: IndexMap<GlobalId, Rc<Shape>> = bodies(module)
            .map(|(id, _)| {
                let held = kept.get(&id).filter(|one| one.mark == marks[&id]).and_then(|one| one.shape.clone());
                (id, held.unwrap_or_else(|| analyses.function::<Shape>(module, id)))
            })
            .collect();
        let memo = analyses.memo::<SummariesMemo>();
        // The calls were found under the globals' facts of the run before:
        // other facts, the calls are found again.
        let mut body_facts = body_facts(module, &program.layout, program.target.spaces(), kept, &marks);
        for (id, one) in body_facts.iter_mut() {
            one.shape = Some(Rc::clone(&shapes[id]));
        }
        if !memo.globals.as_ref().is_some_and(|then| Rc::ptr_eq(then, &globals_held)) {
            body_facts.values_mut().for_each(|one| (one.calls, one.values) = (None, None));
        }
        calls_found(module, &program, globals, bodies(module).map(|(id, _)| id), &mut body_facts);
        let procedures = Table::of(module, &program, globals, &body_facts, None);
        // Bodies edited since the last run: those whose history is not where
        // the last run left it. What else the summaries read, the
        // globals' facts and the declarations, either is the same result as
        // then or the whole is worked out again.
        let memo = analyses.memo::<SummariesMemo>();
        let dirty = memo
            .globals
            .as_ref()
            .zip(memo.declarations.as_ref())
            .filter(|(then, _)| Rc::ptr_eq(then, &globals_held))
            .and_then(|(_, then)| {
                // A declaration that differs is the function's own to answer
                // for, and its callers' (the closure does
                // that); a variable that differs, a global added or removed, is
                // the whole's.
                let mut names: BTreeSet<String> = BTreeSet::new();
                if !Rc::ptr_eq(then, &declarations) {
                    if then.len() != declarations.len() {
                        return None;
                    }
                    for (before, now) in then.iter().zip(declarations.iter()).filter(|(before, now)| before != now) {
                        if before.function().is_none() || now.function().is_none() {
                            return None;
                        }
                        names.insert(now.name.clone()?);
                    }
                }
                names.extend(
                    bodies(module)
                        .filter(|(id, function)| memo.marks.get(id) != Some(&function.mark()))
                        .filter_map(|(id, _)| module.global(id).name.clone()),
                );
                Some(names)
            });
        let found = Self::solved(&procedures, known.as_ref(), &mut memo.summaries, dirty.as_ref());
        memo.marks = marks;
        memo.globals = Some(Rc::clone(&globals_held));
        memo.declarations = Some(declarations);
        memo.slots = Some(Rc::clone(&procedures.slots));
        memo.logged = logged;
        memo.log_epoch = epoch;
        drop(procedures);
        memo.facts = body_facts;
        found
    }
}

impl Summaries {
    /// The summaries brought up to date from the edit log: none where the
    /// run is not one that a log can serve (the globals' facts or the
    /// declarations moved, a body was added, removed or renamed, nothing
    /// kept), and the full scan is made.
    fn brought_up(
        module: &Module,
        analyses: &mut ModuleAnalyses,
        program: &Rc<llrm_mir::program::ProgramProxy>,
        globals_held: &Rc<Result<Globals, String>>,
        declarations: &Rc<Vec<GlobalValue>>,
        known: &Option<IndexMap<String, Summary>>,
        logged: usize,
    ) -> Option<Result<IndexMap<String, Summary>, String>> {
        let epoch = analyses.log_epoch();
        let memo = analyses.memo::<SummariesMemo>();
        let since = memo.logged;
        let steady = memo.slots.is_some()
            && memo.log_epoch == epoch
            && since <= logged
            && memo.globals.as_ref().is_some_and(|then| Rc::ptr_eq(then, globals_held))
            && memo.declarations.as_ref().is_some_and(|then| Rc::ptr_eq(then, declarations));
        if !steady {
            return None;
        }
        let slots = Rc::clone(memo.slots.as_ref()?);
        let moved: Vec<GlobalId> = {
            let mut seen = BTreeSet::new();
            analyses.edit_log()[since..].iter().copied().filter(|id| seen.insert(*id)).collect()
        };
        let memo = analyses.memo::<SummariesMemo>();
        let mut refresh = Vec::new();
        for id in moved {
            let global = module.global(id);
            let body = global.function().filter(|one| !one.is_declaration());
            match (body, slots.by_id.get(&id)) {
                (Some(function), Some(&slot)) if global.name.as_deref() == Some(slots.names[slot].as_str()) => {
                    if memo.facts[&id].mark != function.mark() {
                        refresh.push(id);
                    }
                }
                (None, None) => {}
                (Some(_), None) if global.name.is_none() => {}
                _ => {
                    counted("summaries from the log", false);
                    return None;
                }
            }
        }
        let mut facts = std::mem::take(&mut memo.facts);
        let layout = &program.layout;
        let spaces = program.target.spaces();
        let mut dirty = BTreeSet::new();
        for &id in &refresh {
            let function = module.global(id).function().expect("a body");
            let shape = analyses.function::<Shape>(module, id);
            facts.insert(
                id,
                BodyFacts {
                    mark: function.mark(),
                    exposed: Rc::new(crate::memory::exposed_frames(
                        &Unit::of(module, layout, function).with_spaces(spaces),
                    )),
                    shape: Some(shape),
                    calls: None,
                    values: None,
                },
            );
            dirty.extend(module.global(id).name.clone());
        }
        let globals = Result::as_ref(globals_held).ok()?;
        calls_found(module, program, globals, refresh.iter().copied(), &mut facts);
        let table = Table::of(module, program, globals, &facts, Some(slots));
        let memo = analyses.memo::<SummariesMemo>();
        if llrm_support::env_flag!("LLRM_CHECK_SUMMARIES") {
            for (id, function) in bodies(module) {
                assert!(
                    refresh.contains(&id) || facts[&id].mark == function.mark(),
                    "LLRM_CHECK_SUMMARIES: {:?} was edited and not reported",
                    module.global(id).name
                );
            }
        }
        FROM_LOG.with(|runs| runs.set(runs.get() + 1));
        counted("summaries from the log", true);
        let found = Self::solved(&table, known.as_ref(), &mut memo.summaries, Some(&dirty));
        for &id in &refresh {
            memo.marks.insert(id, facts[&id].mark);
        }
        memo.logged = logged;
        memo.log_epoch = epoch;
        drop(table);
        memo.facts = facts;
        Some(found)
    }

    /// `procedures`' summaries, brought up to date for `dirty` (all where it
    /// is none), and checked against a run from nothing where asked.
    fn solved(
        procedures: &Table,
        known: Option<&IndexMap<String, Summary>>,
        memo: &mut alias::SummaryMemo,
        dirty: Option<&BTreeSet<String>>,
    ) -> Result<IndexMap<String, Summary>, String> {
        counted("summaries updated", dirty.is_some());
        let found = alias::summaries_updating(procedures, known, memo, dirty);
        if llrm_support::env_flag!("LLRM_CHECK_SUMMARIES") {
            procedures.check_against_scratch();
        }
        if llrm_support::env_flag!("LLRM_CHECK_MODULES") {
            let fresh = alias::summaries(procedures, known);
            if found != fresh {
                let (now, then) = (found.as_ref().ok(), fresh.as_ref().ok());
                let names: Vec<String> = match (now, then) {
                    (Some(now), Some(then)) => now
                        .iter()
                        .filter(|(name, one)| then.get(*name) != Some(one))
                        .map(|(name, _)| name.clone())
                        .chain(then.keys().filter(|name| !now.contains_key(*name)).cloned())
                        .collect(),
                    _ => Vec::new(),
                };
                panic!(
                    "summaries brought up to date differ from summaries worked out again: {names:?} (dirty {dirty:?})"
                );
            }
        }
        found
    }
}

thread_local! {
    static FROM_LOG: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many runs of `Summaries` on this thread were brought up to date from
/// the edit log alone, for a test that an edit reported to one body does not
/// look at the rest.
pub fn runs_from_the_log() -> usize {
    FROM_LOG.with(std::cell::Cell::get)
}

/// What `Summaries` keeps for its next run.
#[derive(Default)]
struct SummariesMemo {
    summaries: alias::SummaryMemo,
    marks: IndexMap<GlobalId, Mark>,
    globals: Option<Rc<Result<Globals, String>>>,
    declarations: Option<Rc<Vec<GlobalValue>>>,
    facts: IndexMap<GlobalId, BodyFacts>,
    /// The bodies, by number, the last run was over.
    slots: Option<Rc<Slots>>,
    /// How much of the module analyses' edit log the last run had read.
    logged: usize,
    /// The `ModuleAnalyses::log_epoch` that `logged` is a position in.
    log_epoch: u64,
}

/// The named bodies of a module, in the order the summaries number them.
struct Slots {
    /// Which table of bodies this is, as `alias::Bodies::layout` asks.
    layout: u64,
    ids: Vec<GlobalId>,
    names: Vec<String>,
    by_name: llrm_support::hash::HashMap<String, usize>,
    by_id: llrm_support::hash::HashMap<GlobalId, usize>,
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
            let name = module.global(id).name.clone()?;
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

/// The bodies `Summaries` is over, as `alias::Bodies`: each body's calls as
/// kept, and its `Procedure` made when a visit asks for it.
struct Table<'a> {
    module: &'a Module,
    program: &'a ProgramProxy,
    globals: &'a Globals,
    facts: &'a IndexMap<GlobalId, BodyFacts>,
    slots: Rc<Slots>,
}

impl<'a> Table<'a> {
    /// `slots` where the bodies are those of the run before, else made.
    fn of(
        module: &'a Module,
        program: &'a ProgramProxy,
        globals: &'a Globals,
        facts: &'a IndexMap<GlobalId, BodyFacts>,
        slots: Option<Rc<Slots>>,
    ) -> Self {
        let slots = slots.unwrap_or_else(|| {
            let named: Vec<(GlobalId, String)> =
                bodies(module).filter_map(|(id, _)| module.global(id).name.clone().map(|name| (id, name))).collect();
            thread_local! {
                static MADE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
            }
            Rc::new(Slots {
                layout: MADE.with(|made| {
                    made.set(made.get() + 1);
                    made.get()
                }),
                by_name: named.iter().enumerate().map(|(at, (_, name))| (name.clone(), at)).collect(),
                by_id: named.iter().enumerate().map(|(at, (id, _))| (*id, at)).collect(),
                ids: named.iter().map(|(id, _)| *id).collect(),
                names: named.into_iter().map(|(_, name)| name).collect(),
            })
        });
        Self { module, program, globals, facts, slots }
    }

    /// What `LLRM_CHECK_SUMMARIES` asserts: the facts kept for every body are
    /// those a run from nothing finds (exposed frames, point values, calls),
    /// and so are the shapes' bodies' marks.
    fn check_against_scratch(&self) {
        let (module, program, globals) = (self.module, self.program, self.globals);
        let mut fresh = body_facts(
            module,
            &program.layout,
            program.target.spaces(),
            IndexMap::default(),
            &bodies(module).map(|(id, function)| (id, function.mark())).collect(),
        );
        fresh.iter_mut().for_each(|(id, one)| one.shape = self.facts[id].shape.clone());
        calls_found(module, program, globals, bodies(module).map(|(id, _)| id), &mut fresh);
        for (&id, kept) in self.facts {
            let then = &fresh[&id];
            let name = module.global(id).name.clone();
            assert!(
                kept.exposed == then.exposed,
                "LLRM_CHECK_SUMMARIES: exposed frames of {name:?} differ from scratch"
            );
            assert!(
                kept.values.as_deref() == then.values.as_deref(),
                "LLRM_CHECK_SUMMARIES: point values of {name:?} differ from scratch"
            );
            assert!(
                format!("{:?}", kept.calls) == format!("{:?}", then.calls),
                "LLRM_CHECK_SUMMARIES: calls of {name:?} differ from scratch"
            );
        }
    }
}

impl alias::Bodies for Table<'_> {
    fn len(&self) -> usize {
        self.slots.ids.len()
    }

    fn name(
        &self,
        at: usize,
    ) -> &str {
        &self.slots.names[at]
    }

    fn position(
        &self,
        name: &str,
    ) -> Option<usize> {
        self.slots.by_name.get(name).copied()
    }

    fn facts(
        &self,
        at: usize,
    ) -> &Rc<alias::CallFacts> {
        self.facts[&self.slots.ids[at]].calls.as_ref().expect("found before")
    }

    fn globals(&self) -> Option<&Globals> {
        (!self.slots.ids.is_empty()).then_some(self.globals)
    }

    fn layout(&self) -> u64 {
        self.slots.layout
    }

    fn procedure(
        &self,
        at: usize,
    ) -> Procedure<'_> {
        let id = self.slots.ids[at];
        let one = &self.facts[&id];
        Procedure::with(
            summarized_in(
                self.module,
                self.program,
                self.globals,
                one.shape.as_ref().expect("the shape is kept with the facts"),
                &one.exposed,
                one.values.as_deref(),
                self.module.global(id).function().expect("a body"),
            ),
            Rc::clone(self.facts(at)),
        )
    }
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
            .map(|at| {
                globalsaa::found(&program.modules[at], &proxies[at], &elsewhere[at], &mut |id| {
                    Rc::clone(&shapes[at][&id])
                })
            })
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
                    Some((global.name.clone()?, moved(summary, from, module)))
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

/// What the pointer analyses (`ExposedFrames`, `Pointers`, `CallEffects`,
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
                Change::BlockCreated(_) | Change::BlockErased(_) | Change::Parameters => return None,
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
            crate::frameescape::exposed_renewed(function, previous.iter().copied(), &touched, |inst| {
                crate::memory::is_lifetime_marker(&unit, inst)
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
/// `Pointers` and `CallEffects` ask this one and each runs only its own escape
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
        let bounds = loop_bounds(context, layout, function, analyses);
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
        if let Ok(bounds) = &*bounds {
            unit = unit.with_bounds(bounds);
        }
        alias::annotated_with(&unit, pointers, &registers)
    }
}

/// What each call reads and writes, its callee as `Summaries` says:
/// `alias::calls_annotated`. Where `Summaries` was not required, each
/// call is to an unknown callee. Either way a call does no more than its
/// attributes state, as alias reads them.
pub struct CallEffects;

impl Analysis for CallEffects {
    type Result = Result<IndexMap<InstId, Effect>, String>;
    const NAME: &'static str = "call-effects";
    const SKIPS: bool = true;
    const SOUND: bool = true;
    /// Its references name the pointers of the call's arguments: past a
    /// substitution they would name a value the edit erased.
    const NAMES_VALUES: bool = true;
    /// What it says of a call is what the call and its arguments' meanings
    /// give, which edits that keep every value's meaning leave (no call is
    /// made or moved). The calls still there are the calls it has an entry
    /// for; an erased one is not asked.
    fn covers(
        old: &Self::Result,
        fresh: &Self::Result,
        _context: &Context,
        function: &Function,
    ) -> bool {
        match (old, fresh) {
            (Ok(old), Ok(fresh)) => old.iter().all(|(inst, effect)| {
                function.is_erased(*inst) || fresh.get(inst).is_some_and(|found| effect.covers(found))
            }),
            (old, fresh) => old == fresh,
        }
    }
    fn depends() -> Option<Depends> {
        Some(Depends { models: models_wide, keyed: calls_only, flows: Some(stops_at), edges: true })
    }
    /// Each effect with the values it names followed through the substitution:
    /// the same bytes by the value that replaced one found equal.
    fn remapped(
        old: &Self::Result,
        substituted: &dyn Fn(ValueId) -> Option<llrm_mir::module::Operand>,
        _changes: &[llrm_mir::module::Change],
        _context: &Context,
        function: &Function,
    ) -> Option<Self::Result> {
        let old = old.as_ref().ok()?;
        // A reference holds its pointer's decomposition (root, base, scale),
        // read down the chain of instructions below it. Renaming a value to one
        // that decomposes otherwise (a load forwarded to the GEP it loaded)
        // gives a reference that knows less than a fresh one: derived again.
        if renamed_into_a_chain(function, substituted, old) {
            return None;
        }
        Some(Ok(alias::effects_renamed(old, substituted)?))
    }
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let values = analyses.get::<PointerValues>(context, layout, function);
        let outer = analyses.outer();
        let mut unit = Unit::within(context, layout, function, outer).with_shape(&shape).with_exposed(&exposed);
        if let Ok(values) = &*values {
            unit = unit.with_point_values(values);
        }
        call_effects(&unit, outer)
    }
}

/// Whether a value replaced by another is read the same by a decomposition:
/// both stop it (a load, a call, an allocation, a phi, a parameter, a
/// constant), or both are one instruction (opcode, operands and flags: an
/// inbounds gep goes on through its index where a plain one does not).
pub(crate) fn decomposes_alike(
    function: &Function,
    was: ValueId,
    now: ValueId,
) -> bool {
    use llrm_mir::module::ValueDef;
    let ends = |value: ValueId| match function.value(value).def {
        ValueDef::Instruction(inst) => matches!(
            function.instruction(inst).opcode,
            Opcode::Load { .. } | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Alloca { .. } | Opcode::Phi
        ),
        _ => true,
    };
    match (function.value(was).def, function.value(now).def) {
        (ValueDef::Instruction(a), ValueDef::Instruction(b)) => {
            let (a, b) = (function.instruction(a), function.instruction(b));
            (ends(was) && ends(now))
                || (!ends(was) && !ends(now) && a.opcode == b.opcode && a.operands == b.operands && a.flags == b.flags)
        }
        _ => ends(was) && ends(now),
    }
}

/// Whether a value some reference of `effects` names is replaced by one that a
/// fresh decomposition reads differently: the reference would name the new
/// value as its root, where a decomposition goes on through the definition of a
/// GEP, a cast, an add, and stops at a parameter, a constant, a load, a call,
/// an allocation, a phi. A load forwarded to the pointer it loaded (a GEP) is
/// the case; a value replaced by an equal instruction (the same opcode and
/// operands) decomposes alike.
fn renamed_into_a_chain(
    function: &Function,
    substituted: &dyn Fn(ValueId) -> Option<llrm_mir::module::Operand>,
    effects: &IndexMap<InstId, alias::Effect>,
) -> bool {
    use llrm_mir::module::Operand;
    let alike = |was: ValueId, now: ValueId| decomposes_alike(function, was, now);
    let differs = |one: &crate::memory::MemRef| {
        [one.pointer, one.root, one.base.map(Operand::Value), one.segment].into_iter().any(|named| {
            matches!(
                named,
                Some(Operand::Value(was)) if matches!(substituted(was), Some(Operand::Value(now)) if !alike(was, now))
            )
        })
    };
    // The lists calls share are looked at once.
    let mut seen: std::collections::HashSet<*const [crate::memory::MemRef]> = std::collections::HashSet::new();
    effects.values().any(|effect| {
        let mut lists = [&effect.loads, &effect.stores].into_iter().filter(|list| seen.insert(Rc::as_ptr(list)));
        lists.any(|list| list.iter().any(&differs)) || effect.fills.iter().any(&differs)
    })
}

/// What each call of `unit` reads and writes, its callee as `Summaries`
/// says where `outer` holds it, and otherwise an unknown one, as LLVM's
/// function passes read an outer result only if cached.
pub fn call_effects(
    unit: &Unit,
    outer: &Outer,
) -> Result<IndexMap<InstId, Effect>, String> {
    let summaries = outer.cached::<Summaries>();
    let none = IndexMap::default();
    let known = match summaries.as_deref() {
        Some(found) => found.as_ref().map_err(String::clone)?,
        None => &none,
    };
    alias::calls_annotated(&Procedure::of(*unit), known)
}

/// Every value known without solving memory: `consts::known` of no calls.
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
    const INCREMENTAL: bool = true;

    /// A fact is of the values its value is made from alone (the edges of the
    /// CFG are not read: a phi meets what its arms are), so the facts of
    /// the values a change did not reach, by the uses of what it
    /// touched, stand; the others are worked out again from them.
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
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let tables = analyses.get::<TableLoads>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer()).with_exposed(&exposed);
        // A load from a table is known by the bounds of its whole index,
        // whatever it is made of: it is asked again with the values a
        // change touched.
        let mut kept = Rc::try_unwrap(previous).unwrap_or_else(|shared| (*shared).clone());
        // The values touched, and the facts that were made from them, which no
        // longer stand.
        let mut touched = llrm_mir::dense::IdSet::<ValueId>::new();
        let mut work = Vec::<ValueId>::new();
        for &inst in tables.iter() {
            if let Some(result) = function.instruction(inst).result
                && touched.insert(result)
            {
                work.push(result);
            }
        }
        for change in changes {
            let inst = match *change {
                Change::Inserted { inst, .. }
                | Change::Erased { inst, .. }
                | Change::Moved { inst, .. }
                | Change::Rewritten(inst) => inst,
                Change::Cloned { to, .. } => to,
                Change::BlockCreated(_)
                | Change::BlockErased(_)
                | Change::Operand { .. }
                | Change::Operands { .. }
                | Change::Substituted { .. } => {
                    continue;
                }
                // An argument's fact is of its position.
                Change::Parameters => return None,
            };
            if let Some(result) = function.instruction(inst).result
                && touched.insert(result)
            {
                work.push(result);
            }
        }
        // Every value to settle again: the touched, those whose fact was made
        // from them, and the users of each, whose operands may now be
        // known.
        let mut dirty = touched.clone();
        let mut seen = touched;
        while let Some(value) = work.pop() {
            kept.swap_remove(&value);
            for user in function.users(value) {
                if function.parent(user.user).is_none() {
                    continue;
                }
                let Some(result) = function.instruction(user.user).result else { continue };
                dirty.insert(result);
                if kept.contains_key(&result) && seen.insert(result) {
                    work.push(result);
                }
            }
        }
        Some(consts::known_from(&unit, kept, Some(&dirty), &tables))
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
        consts::known(
            &Unit::within(context, layout, function, analyses.outer())
                .with_shape(&shape)
                .with_assumptions(&assumptions)
                .with_exposed(&exposed),
        )
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
                        || reached.iter().any(|r| shape.dominance.dominates(crate::cfg::id(r), one.header))))
        }))
    }
}

/// What each call writes, as `CallEffects` says: the `Calls` consts and
/// floatfacts take.
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
        let effects = analyses.get::<CallEffects>(context, layout, function);
        Ok(Result::as_ref(&*effects)
            .map_err(String::clone)?
            .iter()
            .map(|(&at, effect)| (at, effect.stores.clone()))
            .collect())
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
/// `CallEffects` says.
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
        Some(Ok(held.keeping(|at| !dominated_by(&shape, &reached, at))))
    }
}

/// Whether `at`, or a block above it, is one of `reached`.
pub fn dominated_by(
    shape: &Shape,
    reached: &llrm_mir::dense::IdSet<i64>,
    at: i64,
) -> bool {
    let mut above = Some(at);
    while let Some(one) = above {
        if reached.contains(&one) {
            return true;
        }
        above = shape.dominance.immediate(one);
    }
    false
}

/// The facts each counted loop gives its blocks, and where there is none the
/// edges': `ranges::bounded`.
pub struct Bounded;

impl Analysis for Bounded {
    type Result = Result<ranges::Bounds, String>;
    const NAME: &'static str = "bounded";
    const INCREMENTAL: bool = true;

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        Self::solved(context, layout, function, analyses, None)
    }

    /// A loop's facts read its own blocks, the facts of the values its
    /// operations read, the edges' facts above it and the facts of the
    /// loops it starts from; the loops a change reaches by any of these are
    /// worked again.
    fn update(
        previous: &Self::Result,
        changes: &[Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        let bounds = previous.as_ref().ok()?;
        let reach = function.reach_of(changes)?;
        if reach.cfg_changed() {
            return None;
        }
        let shape = analyses.get::<Shape>(context, layout, function);
        let edges = analyses.get::<DominatedEdges>(context, layout, function);
        edges.as_ref().as_ref().ok()?;
        // A loop starts from the edges' facts of its dominator: those of a
        // block a change reached or one it dominates may have changed.
        let reached: llrm_mir::dense::IdSet<i64> = reach.blocks.iter().map(crate::cfg::id).collect();
        let dirty = ranges::loops_reached(function, &shape, &reach.blocks, &reach.conditions, &|at| {
            dominated_by(&shape, &reached, at)
        });
        Some(Self::solved(context, layout, function, analyses, Some((bounds, &dirty))))
    }
}

/// What a pass asks of the ranges of a body as it stands: the manager's
/// registers, bounds and edges, and the unit that carries them.
pub struct Ranges {
    shape: Rc<Shape>,
    assumptions: Rc<<AssumptionCache as Analysis>::Result>,
    registers: Rc<<Registers as Analysis>::Result>,
    exposed: Rc<<ExposedFrames as Analysis>::Result>,
    counted: Rc<<Counted as Analysis>::Result>,
    edges: Rc<<DominatedEdges as Analysis>::Result>,
    bounds: Option<Rc<<Bounded as Analysis>::Result>>,
}

impl Ranges {
    /// With the bounds too, which `Bounded` itself works out from the rest.
    pub fn of(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
        with_bounds: bool,
    ) -> Self {
        Self {
            shape: analyses.get::<Shape>(context, layout, function),
            assumptions: analyses.get::<AssumptionCache>(context, layout, function),
            registers: analyses.get::<Registers>(context, layout, function),
            exposed: analyses.get::<ExposedFrames>(context, layout, function),
            counted: analyses.get::<Counted>(context, layout, function),
            edges: analyses.get::<DominatedEdges>(context, layout, function),
            bounds: None,
        }
        .bounded(context, layout, function, analyses, with_bounds)
    }

    fn bounded(
        mut self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
        with_bounds: bool,
    ) -> Self {
        if with_bounds {
            self.bounds = Some(analyses.get::<Bounded>(context, layout, function));
        }
        self
    }

    pub fn registers(&self) -> &Rc<<Registers as Analysis>::Result> {
        &self.registers
    }

    /// The bounds, where they were asked for.
    pub fn bounds(&self) -> Option<&<Bounded as Analysis>::Result> {
        self.bounds.as_deref()
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
        if let Some(Ok(bounds)) = self.bounds.as_deref() {
            unit = unit.with_bounds(bounds);
        }
        unit
    }
}

impl Bounded {
    fn solved(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
        prior: Option<(&ranges::Bounds, &llrm_mir::dense::IdSet<i64>)>,
    ) -> <Self as Analysis>::Result {
        let held = Ranges::of(context, layout, function, analyses, false);
        ranges::bounded_solved(&held.unit(context, layout, function, analyses.outer()), held.registers(), prior, true)
    }
}

/// The counted loops' intervals for alias and decisions: `Bounded`, or none at
/// a level without them (-O1: gcc's has value ranges only from -O2's
/// `-ftree-vrp`; its -O1 reaches an induction variable's range through SCEV and
/// the loop's iteration bound, which `indvars`, `loopexit`, `hoist` and
/// `algebraic` still ask `Bounded` for). Empty, the unit does not solve them
/// itself.
fn loop_bounds(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    analyses: &mut Analyses,
) -> Rc<<Bounded as Analysis>::Result> {
    if analyses.outer().loop_intervals() {
        analyses.get::<Bounded>(context, layout, function)
    } else {
        Rc::new(Ok(ranges::Bounds::default()))
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
    bounded: Option<Rc<<Bounded as Analysis>::Result>>,
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
            bounded: None,
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

    /// Also what the counted loops bound: for a pass that asks it, so that it
    /// is the manager's, kept and brought up to date, and not a solve of
    /// the pass's own.
    pub fn with_bounded(
        mut self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self {
        self.bounded = Some(loop_bounds(context, layout, function, analyses));
        self
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
        if let Some(Ok(bounds)) = self.bounded.as_deref() {
            unit = unit.with_bounds(bounds);
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
        analyses.get::<CallEffects>(context, layout, function);
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
        analyses.get::<Bounded>(context, layout, function);
    }
    if chosen() {
        analyses.get::<TableLoads>(context, layout, function);
    }
    if chosen() {
        let _ = memoryssa::Accesses::managed(context, layout, function, analyses);
    }
}
