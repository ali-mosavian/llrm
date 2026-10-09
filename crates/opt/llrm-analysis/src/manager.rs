//! The memory analyses as entries of llrm-mir's analysis manager
//! (`passes::Analyses`), the role llrm-core's `analysis/manager.rs` played:
//! a pass asks, the manager computes once and drops what a pass did not
//! preserve. An entry reads its module and target through the outer proxy
//! (`passes::Outer`). Module analyses there: `GlobalsAA`, which globals no
//! outside code reaches but by name, and alias's callee summaries, which
//! ask it.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::context::{Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::depends::Depends;
use llrm_mir::module::{BlockId, Change, Function, GlobalValue, InstId, Linkage, Mark, Module, ValueId};
use llrm_mir::opcode::Opcode;
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
            edges: None,
            bounds: None,
            exposed: None,
            point_values: None,
            callbacks: outer.cached_ref::<Callbacks>().and_then(|one| one.as_ref().ok()),
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
        let shapes = bodies(module).map(|(id, _)| (id, analyses.function::<Shape>(module, id))).collect();
        // What a body's calls and exposed frames are depends on the body and
        // the declarations: kept while neither moved.
        let scratch = analyses.from_scratch();
        let memo = analyses.memo::<SummariesMemo>();
        let kept = if scratch || !memo.declarations.as_ref().is_some_and(|then| Rc::ptr_eq(then, &declarations)) {
            IndexMap::default()
        } else {
            std::mem::take(&mut memo.facts)
        };
        // The calls were found under the globals' facts of the run before:
        // other facts, the calls are found again.
        let mut body_facts = body_facts(module, &program.layout, program.target.spaces(), kept);
        if !memo.globals.as_ref().is_some_and(|then| Rc::ptr_eq(then, &globals_held)) {
            body_facts.values_mut().for_each(|one| (one.calls, one.values) = (None, None));
        }
        calls_found(module, &program, globals, &shapes, &mut body_facts);
        let procedures = procedures(module, &program, globals, &shapes, &body_facts);
        // Bodies edited since the last run: those whose history is not where
        // the last run left it. What else the summaries read, the
        // globals' facts and the declarations, either is the same result as
        // then or the whole is worked out again.
        let memo = analyses.memo::<SummariesMemo>();
        let marks: IndexMap<GlobalId, Mark> = bodies(module).map(|(id, function)| (id, function.mark())).collect();
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
        counted("summaries updated", dirty.is_some());
        let found = alias::summaries_updating(&procedures, known.as_ref(), &mut memo.summaries, dirty.as_ref());
        memo.marks = marks;
        memo.facts = body_facts.clone();
        memo.globals = Some(Rc::clone(&globals_held));
        memo.declarations = Some(declarations);
        if llrm_support::env_set("LLRM_CHECK_MODULES") {
            let fresh = alias::summaries(&procedures, known.as_ref());
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

/// What `Summaries` keeps for its next run.
#[derive(Default)]
struct SummariesMemo {
    summaries: alias::SummaryMemo,
    marks: IndexMap<GlobalId, Mark>,
    globals: Option<Rc<Result<Globals, String>>>,
    declarations: Option<Rc<Vec<GlobalValue>>>,
    facts: IndexMap<GlobalId, BodyFacts>,
}

/// `module`'s defined functions, each with its id.
fn bodies(module: &Module) -> impl Iterator<Item = (GlobalId, &Function)> {
    module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(id, _, function)| (id, function))
}

/// What a body gives `Summaries` that is of the body, the declarations and (for
/// its calls) the globals' facts alone, as of the body's history then.
#[derive(Clone)]
struct BodyFacts {
    mark: Mark,
    exposed: Rc<BTreeSet<ValueId>>,
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
) -> IndexMap<GlobalId, BodyFacts> {
    bodies(module)
        .map(|(id, function)| {
            let mark = function.mark();
            let facts = match kept.swap_remove(&id) {
                Some(then) if then.mark == mark => then,
                _ => BodyFacts {
                    mark,
                    exposed: Rc::new(crate::memory::exposed_frames(
                        &Unit::of(module, layout, function).with_spaces(spaces),
                    )),
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
    shapes: &IndexMap<GlobalId, Rc<Shape>>,
    facts: &mut IndexMap<GlobalId, BodyFacts>,
) {
    for (id, function) in bodies(module) {
        let one = facts.get_mut(&id).expect("facts for every body");
        if one.values.is_none() {
            let unit = summarized_in(module, program, globals, &shapes[&id], &one.exposed, None, function);
            one.values =
                llrm_support::debug::timed("summaries values", || alias::point_values(&unit).ok().map(Rc::new));
        }
        if one.calls.is_none() {
            one.calls = Some(Rc::new(alias::CallFacts::of(&summarized_in(
                module,
                program,
                globals,
                &shapes[&id],
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
            .map(|module| body_facts(module, &program.layout, program.target.spaces(), IndexMap::default()))
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
                calls_found(&program.modules[at], &proxies[at], &globals[at], &shapes[at], &mut exposures[at]);
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
    out.reads = carried(&summary.reads, from, to, &mut out.unknown_read);
    out.writes = carried(&summary.writes, from, to, &mut out.unknown_write);
    // A global reached as unknown memory has no access type here.
    if out.unknown_write {
        out.unknown_write_types = None;
    }
    out
}

fn carried(
    slices: &BTreeSet<Slice>,
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
            None => *unknown |= one.object.captured,
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
    fn depends() -> Option<Depends> {
        Some(Depends { models: models_scalars, keyed: allocas_only, flows: Some(stops_at) })
    }
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        crate::memory::exposed_frames(&Unit::within(context, layout, function, analyses.outer()))
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
        Some(Depends { models: models_scalars, keyed: models_scalars, flows: Some(stops_at) })
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
        Some(Depends { models: models_scalars, keyed: models_scalars, flows: Some(stops_at) })
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
        let pointers = analyses.get::<Pointers>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let pointers = Result::as_ref(&*pointers).map_err(String::clone)?;
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let counted = analyses.get::<Counted>(context, layout, function);
        let edges = analyses.get::<DominatedEdges>(context, layout, function);
        let bounds = analyses.get::<Bounded>(context, layout, function);
        let mut unit = Unit::within(context, layout, function, analyses.outer())
            .with_shape(&shape)
            .with_assumptions(&assumptions)
            .with_registers(&registers)
            .with_pointers(pointers)
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
    fn depends() -> Option<Depends> {
        Some(Depends { models: models_wide, keyed: calls_only, flows: Some(stops_at) })
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

impl Analysis for Registers {
    type Result = IndexMap<ValueId, Known>;
    const NAME: &'static str = "registers";
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
            None,
            None,
            None,
        )
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
        let reached = reached_by(function, changes)?.blocks;
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
        Some(induction::counted_renewed(&unit, previous, |one| {
            one.body.iter().any(|at| reached.contains(&crate::cfg::block(*at)))
        }))
    }
}

/// What a change reached: the blocks holding an instruction it touched, or one
/// that reads a value a touched instruction makes, or one that reads the value
/// such a reader makes, and so on; and of those the blocks whose branch, or
/// assume, or call, is one that reads a value the change made, which can alter
/// what holds below them.
pub struct Reach {
    pub blocks: BTreeSet<BlockId>,
    pub conditions: BTreeSet<BlockId>,
}

/// What `changes` reached; none where a change was to the CFG.
pub fn reached_by(
    function: &Function,
    changes: &[Change],
) -> Option<Reach> {
    let mut blocks = BTreeSet::new();
    let mut work: Vec<InstId> = Vec::new();
    for change in changes {
        let (inst, placed) = match *change {
            Change::BlockCreated(_) | Change::BlockErased(_) => return None,
            Change::Inserted { inst, block, .. } | Change::Erased { inst, block, .. } => (inst, vec![block]),
            Change::Moved { inst, block, from, .. } => (inst, vec![block, from]),
            Change::Rewritten(inst) => (inst, Vec::new()),
            Change::Cloned { to, .. } => (to, Vec::new()),
        };
        if function.instruction(inst).opcode.is_terminator() {
            return None;
        }
        blocks.extend(placed);
        work.push(inst);
    }
    let mut seen: BTreeSet<InstId> = work.iter().copied().collect();
    let mut conditions = BTreeSet::new();
    while let Some(inst) = work.pop() {
        blocks.extend(function.parent(inst));
        if let Some(block) = function.parent(inst)
            && (function.instruction(inst).opcode.is_terminator()
                || matches!(
                    function.instruction(inst).opcode,
                    Opcode::Call(_) | Opcode::Invoke(_)
                ))
        {
            conditions.insert(block);
        }
        if let Some(result) = function.instruction(inst).result {
            for user in function.users(result) {
                if seen.insert(user.user) {
                    work.push(user.user);
                }
            }
        }
    }
    Some(Reach { blocks, conditions })
}

/// What each call writes, as `CallEffects` says: the `Calls` consts and
/// floatfacts take.
pub struct Writes;

impl Analysis for Writes {
    type Result = Result<Calls, String>;
    const NAME: &'static str = "writes";
    const SKIPS: bool = true;
    fn depends() -> Option<Depends> {
        Some(Depends { models: models_wide, keyed: calls_only, flows: Some(stops_at) })
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
        let through = analyses.get::<ThroughMemory>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer())
            .with_shape(&shape)
            .with_assumptions(&assumptions)
            .with_registers(&registers)
            .with_annotated(&references)
            .with_exposed(&exposed);
        match &*through {
            Ok(integers) => {
                if llrm_support::env_set("LLRM_CHECK_FACTS") {
                    assert!(
                        *integers == consts::known(&unit, Some(&calls), None, None),
                        "ThroughMemory's integers are not those the float solve derives for itself"
                    );
                }
                floatfacts::solved_over(&unit, &calls, None, integers)
            }
            Err(_) => floatfacts::solved_with(&unit, &calls, None),
        }
    }
}

/// Every value known, memory solved alongside and each call writing what
/// `CallEffects` says.
pub struct ThroughMemory;

impl Analysis for ThroughMemory {
    type Result = Result<IndexMap<ValueId, Known>, String>;
    const NAME: &'static str = "through-memory";
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
        let unit = Unit::within(context, layout, function, analyses.outer())
            .with_references(references)
            .with_shape(&shape)
            .with_assumptions(&assumptions)
            .with_registers(&registers)
            .with_exposed(&exposed);
        Ok(consts::known(&unit, Some(&calls), None, None))
    }
}

/// What unavoidable branch edges bound at each block:
/// `ranges::dominated_edges`.
pub struct DominatedEdges;

impl Analysis for DominatedEdges {
    type Result = Result<ranges::EdgeStates, String>;
    const NAME: &'static str = "dominated-edges";
    const INCREMENTAL: bool = true;

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        Self::solved(context, layout, function, analyses, None)
    }

    /// A block's state is of its operations, the state it starts from and the
    /// facts of the values they read; those of the blocks a change reaches,
    /// and those that start from a state that changed, are worked again.
    fn update(
        previous: &Self::Result,
        changes: &[Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        let states = previous.as_ref().ok()?;
        let reached = reached_by(function, changes)?.blocks;
        Some(Self::solved(context, layout, function, analyses, Some((states, &reached))))
    }
}

impl DominatedEdges {
    fn solved(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
        before: Option<(&ranges::EdgeStates, &BTreeSet<BlockId>)>,
    ) -> <Self as Analysis>::Result {
        let registers = analyses.get::<Registers>(context, layout, function);
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        ranges::edges_solved(
            &Unit::within(context, layout, function, analyses.outer())
                .with_shape(&shape)
                .with_assumptions(&assumptions)
                .with_registers(&registers)
                .with_exposed(&exposed),
            &registers,
            before,
        )
    }
}

/// What each memory cell holds before each instruction, where it is a number,
/// of the body alone (no callee's writes, no facts from outside):
/// `consts::cells`.
pub struct MemoryCells;

impl Analysis for MemoryCells {
    type Result = consts::HeldCells;
    const NAME: &'static str = "memory-cells";

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        consts::cells(
            &Unit::within(context, layout, function, analyses.outer()).with_shape(&shape).with_exposed(&exposed),
            &consts::Calls::default(),
            None,
            None,
            None,
            None,
            None,
        )
    }
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
        let reach = reached_by(function, changes)?;
        let shape = analyses.get::<Shape>(context, layout, function);
        let edges = analyses.get::<DominatedEdges>(context, layout, function);
        let dirty = ranges::loops_reached(
            function,
            &shape,
            &reach.blocks,
            &reach.conditions,
            bounds,
            edges.as_ref().as_ref().ok()?,
        );
        Some(Self::solved(context, layout, function, analyses, Some((bounds, &dirty))))
    }
}

impl Bounded {
    fn solved(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
        prior: Option<(&ranges::Bounds, &BTreeSet<i64>)>,
    ) -> <Self as Analysis>::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        let assumptions = analyses.get::<AssumptionCache>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let exposed = analyses.get::<ExposedFrames>(context, layout, function);
        let counted = analyses.get::<Counted>(context, layout, function);
        let edges = analyses.get::<DominatedEdges>(context, layout, function);
        let mut unit = Unit::within(context, layout, function, analyses.outer())
            .with_shape(&shape)
            .with_assumptions(&assumptions)
            .with_registers(&registers)
            .with_exposed(&exposed)
            .with_counted(&counted);
        if let Ok(edges) = &*edges {
            unit = unit.with_edges(edges);
        }
        ranges::bounded_solved(&unit, &registers, prior)
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
    pointers: Option<Rc<<Pointers as Analysis>::Result>>,
    annotated: Option<Rc<<Annotated as Analysis>::Result>>,
    counted: Option<Rc<<Counted as Analysis>::Result>>,
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
            pointers: alias.then(|| analyses.get::<Pointers>(context, layout, function)),
            annotated: alias.then(|| analyses.get::<Annotated>(context, layout, function)),
            // Annotated has proved them already.
            counted: alias.then(|| analyses.get::<Counted>(context, layout, function)),
            bounded: None,
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
        self.bounded = Some(analyses.get::<Bounded>(context, layout, function));
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
        if let Some(Ok(pointers)) = self.pointers.as_deref() {
            unit = unit.with_pointers(pointers);
        }
        if let Some(annotated) = self.annotated.as_deref() {
            unit = unit.with_annotated(annotated);
        }
        if let Some(counted) = self.counted.as_deref() {
            unit = unit.with_counted(counted);
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
