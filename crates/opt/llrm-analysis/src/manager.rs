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
use llrm_mir::module::{Function, InstId, Linkage, Module, ValueId};
use llrm_mir::passes::{Analyses, Analysis, ModuleAnalyses, ModuleAnalysis, Outer};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramAnalysis, ProgramProxy};
use llrm_support::hash::IndexMap;

use crate::cfg::Shape;
use crate::alias::{self, Effect, PointsTo, Procedure, Summary};
use crate::consts::{self, Calls, Known};
use crate::globalsaa::{self, Globals, ProgramGlobals};
use crate::floatfacts;
use crate::memory::{Identity, MemRef, MemoryKind, MemoryObject, Slice, Unit};
use crate::ranges::{self, Interval};

impl<'a> Unit<'a> {
    /// `function` as the manager's analyses see it: its module and target
    /// as `outer` holds them.
    pub fn within(context: &'a Context, layout: &'a DataLayout, function: &'a Function, outer: &'a Outer) -> Self {
        let globals_aa = outer.cached_ref::<GlobalsAA>().and_then(|one| one.as_ref().ok());
        Self { program: Some(outer.program()), context, layout, metadata: &outer.metadata, tbaa: Some(outer.tbaa()), globals: &outer.globals, function, globals_aa, references: None, shape: None, registers: None, pointers: None, annotated: None }
    }
}

/// Which globals no code outside the module reaches but by name:
/// `globalsaa::analysis`.
pub struct GlobalsAA;

impl ModuleAnalysis for GlobalsAA {
    type Result = Result<Globals, String>;
    const NAME: &'static str = "globals-aa";
    fn run(module: &Module, analyses: &mut ModuleAnalyses) -> Self::Result {
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
    fn run(module: &Module, analyses: &mut ModuleAnalyses) -> Self::Result {
        let globals = analyses.get::<GlobalsAA>(module);
        let globals = Result::as_ref(&*globals).map_err(String::clone)?;
        let program = Rc::clone(analyses.program());
        let known = match program.cached::<ProgramSummaries>() {
            Some(all) => Some(Result::as_ref(&*all).map_err(String::clone)?[program.module].clone()),
            None => None,
        };
        let shapes = bodies(module).map(|(id, _)| (id, analyses.function::<Shape>(module, id))).collect();
        alias::summaries(&procedures(module, &program, globals, &shapes), known.as_ref())
    }
}

/// `module`'s defined functions, each with its id.
fn bodies(module: &Module) -> impl Iterator<Item = (GlobalId, &Function)> {
    module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(id, _, function)| (id, function))
}

/// `module`'s named bodies as alias summarizes them.
fn procedures<'a>(module: &'a Module, program: &'a ProgramProxy, globals: &'a Globals, shapes: &'a IndexMap<GlobalId, Rc<Shape>>) -> IndexMap<String, Procedure<'a>> {
    bodies(module)
        .filter_map(|(id, function)| {
            let name = module.global(id).name.clone()?;
            Some((name, Procedure::of(Unit { program: Some(program), ..Unit::of(module, &program.layout, function) }.with_globals_aa(globals).with_shape(&shapes[&id]))))
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
    fn run(program: &Program, analyses: &mut ProgramAnalyses) -> Self::Result {
        let count = program.modules.len();
        let mut known = vec![IndexMap::default(); count];
        if count < 2 {
            return Ok(known);
        }
        let elsewhere = analyses.get::<ProgramGlobals>(program);
        let elsewhere = Result::as_ref(&*elsewhere).map_err(String::clone)?;
        let proxies: Vec<_> = (0..count).map(|at| analyses.proxy(program, at)).collect();
        let shapes: Vec<IndexMap<GlobalId, Rc<Shape>>> = program.modules.iter().map(|module| bodies(module).map(|(id, function)| (id, Rc::new(Shape::of(function)))).collect()).collect();
        let globals = (0..count)
            .map(|at| globalsaa::found(&program.modules[at], &proxies[at], &elsewhere[at], &mut |id| Rc::clone(&shapes[at][&id])))
            .collect::<Result<Vec<_>, String>>()?;
        // A body defined elsewhere starts as a call no summary describes,
        // the most any call does: each round only narrows.
        let mut own = vec![IndexMap::default(); count];
        loop {
            let mut changed = false;
            for at in 0..count {
                let procedures = procedures(&program.modules[at], &proxies[at], &globals[at], &shapes[at]);
                let found = alias::summaries(&procedures, Some(&known[at]))?;
                let mine: IndexMap<String, Summary> = found.into_iter().filter(|(name, _)| procedures.contains_key(name)).collect();
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
fn imported(program: &Program, own: &[IndexMap<String, Summary>]) -> Vec<IndexMap<String, Summary>> {
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
fn moved(summary: &Summary, from: &Module, to: &Module) -> Summary {
    let mut out = summary.clone();
    out.reads = carried(&summary.reads, from, to, &mut out.unknown_read);
    out.writes = carried(&summary.writes, from, to, &mut out.unknown_write);
    out
}

fn carried(slices: &BTreeSet<Slice>, from: &Module, to: &Module, unknown: &mut bool) -> BTreeSet<Slice> {
    let mut out = BTreeSet::new();
    for one in slices {
        let (MemoryKind::Global, Some(Identity::Global(id))) = (one.object.kind, &one.object.identity) else {
            out.insert(one.clone());
            continue;
        };
        let global = from.global(GlobalId(*id));
        let there = global.name.as_deref().filter(|_| !matches!(global.linkage, Linkage::Internal | Linkage::Private)).and_then(|name| to.named(name));
        match there {
            Some(there) => {
                out.insert(Slice { object: MemoryObject { identity: Some(Identity::Global(there.0)), ..one.object.clone() }, ..one.clone() });
            }
            None => *unknown |= one.object.captured,
        }
    }
    out
}

/// Every pointer's objects, with no caller context: `alias::pointers`.
pub struct Pointers;

impl Analysis for Pointers {
    type Result = Result<PointsTo, String>;
    const NAME: &'static str = "points-to";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        alias::points_to(&Unit::within(context, layout, function, analyses.outer()).with_shape(&shape), None, None)
    }
}

/// Each load's and store's reference: `alias::annotated`.
pub struct Annotated;

impl Analysis for Annotated {
    type Result = Result<IndexMap<InstId, MemRef>, String>;
    const NAME: &'static str = "annotated";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let pointers = analyses.get::<Pointers>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let pointers = Result::as_ref(&*pointers).map_err(String::clone)?;
        let shape = analyses.get::<Shape>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer()).with_shape(&shape).with_registers(&registers).with_pointers(pointers);
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
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        let outer = analyses.outer();
        call_effects(&Unit::within(context, layout, function, outer).with_shape(&shape), outer)
    }
}

/// What each call of `unit` reads and writes, its callee as `Summaries`
/// says where `outer` holds it, and otherwise an unknown one, as LLVM's
/// function passes read an outer result only if cached.
pub fn call_effects(unit: &Unit, outer: &Outer) -> Result<IndexMap<InstId, Effect>, String> {
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
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let shape = analyses.get::<Shape>(context, layout, function);
        consts::known(&Unit::within(context, layout, function, analyses.outer()).with_shape(&shape), None, None, None)
    }
}

/// What each call writes, as `CallEffects` says: the `Calls` consts and
/// floatfacts take.
pub struct Writes;

impl Analysis for Writes {
    type Result = Result<Calls, String>;
    const NAME: &'static str = "writes";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let effects = analyses.get::<CallEffects>(context, layout, function);
        Ok(Result::as_ref(&*effects).map_err(String::clone)?.iter().map(|(&at, effect)| (at, effect.stores.clone())).collect())
    }
}

/// `Writes`, or where it failed none: each call then writes what
/// `memory::unmodeled_write` says.
pub fn writes(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Calls {
    Result::as_ref(&*analyses.get::<Writes>(context, layout, function)).cloned().unwrap_or_default()
}

/// Float values, integers and memory in one solve, each call writing what
/// `writes` says: `floatfacts::solved_with`.
pub struct FloatFacts;

impl Analysis for FloatFacts {
    type Result = floatfacts::Solved;
    const NAME: &'static str = "float-facts";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let calls = writes(context, layout, function, analyses);
        let shape = analyses.get::<Shape>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let references = analyses.get::<Annotated>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer()).with_shape(&shape).with_registers(&registers).with_annotated(&references);
        floatfacts::solved_with(&unit, &calls, None)
    }
}

/// Every value known, memory solved alongside and each call writing what
/// `CallEffects` says.
pub struct ThroughMemory;

impl Analysis for ThroughMemory {
    type Result = Result<IndexMap<ValueId, Known>, String>;
    const NAME: &'static str = "through-memory";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let calls = Result::as_ref(&*analyses.get::<Writes>(context, layout, function)).map_err(String::clone)?.clone();
        let references = analyses.get::<Annotated>(context, layout, function);
        let references = Result::as_ref(&*references).map_err(String::clone)?;
        let shape = analyses.get::<Shape>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let unit = Unit::within(context, layout, function, analyses.outer()).with_references(references).with_shape(&shape).with_registers(&registers);
        Ok(consts::known(&unit, Some(&calls), None, None))
    }
}

/// What unavoidable branch edges bound at each block:
/// `ranges::dominated_edges`.
pub struct DominatedEdges;

impl Analysis for DominatedEdges {
    type Result = Result<IndexMap<i64, IndexMap<ValueId, Interval>>, String>;
    const NAME: &'static str = "dominated-edges";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let registers = analyses.get::<Registers>(context, layout, function);
        let shape = analyses.get::<Shape>(context, layout, function);
        ranges::dominated_edges_with(&Unit::within(context, layout, function, analyses.outer()).with_shape(&shape).with_registers(&registers), &registers)
    }
}

/// What the manager holds of a function that a unit over it carries, asked
/// while the body is as the manager last saw it: dominance and loops, what
/// is known without memory and, where asked for, what alias finds.
pub struct Held {
    shape: Rc<Shape>,
    registers: Rc<IndexMap<ValueId, Known>>,
    pointers: Option<Rc<<Pointers as Analysis>::Result>>,
    annotated: Option<Rc<<Annotated as Analysis>::Result>>,
}

impl Held {
    pub fn of(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses, alias: bool) -> Self {
        Self {
            shape: analyses.get::<Shape>(context, layout, function),
            registers: analyses.get::<Registers>(context, layout, function),
            pointers: alias.then(|| analyses.get::<Pointers>(context, layout, function)),
            annotated: alias.then(|| analyses.get::<Annotated>(context, layout, function)),
        }
    }

    /// A unit over `function`, the body these were found of.
    pub fn unit<'a>(&'a self, context: &'a Context, layout: &'a DataLayout, function: &'a Function, outer: &'a Outer) -> Unit<'a> {
        let mut unit = Unit::within(context, layout, function, outer).with_shape(&self.shape).with_registers(&self.registers);
        if let Some(Ok(pointers)) = self.pointers.as_deref() {
            unit = unit.with_pointers(pointers);
        }
        if let Some(annotated) = self.annotated.as_deref() {
            unit = unit.with_annotated(annotated);
        }
        unit
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
