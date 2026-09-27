//! The memory analyses as entries of llrm-mir's analysis manager
//! (`passes::Analyses`), the role llrm-core's `analysis/manager.rs` played:
//! a pass asks, the manager computes once and drops what a pass did not
//! preserve. An entry reads its module and target through the outer proxy
//! (`passes::Outer`). Module analyses there: `GlobalsAA`, which globals no
//! outside code reaches but by name, and alias's callee summaries.

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, InstId, Module, ValueId};
use llrm_mir::passes::{Analyses, Analysis, ModuleAnalyses, ModuleAnalysis, Outer};
use llrm_support::hash::IndexMap;

use crate::alias::{self, Effect, PointsTo, Procedure, Summary};
use crate::consts::{self, Calls, Known};
use crate::globalsaa::{self, Globals};
use crate::floatfacts;
use crate::memory::{MemRef, Unit};
use crate::ranges::{self, Interval};

impl<'a> Unit<'a> {
    /// `function` as the manager's analyses see it: its module and target
    /// as `outer` holds them.
    pub fn within(context: &'a Context, layout: &'a DataLayout, function: &'a Function, outer: &'a Outer) -> Self {
        let globals_aa = outer.cached_ref::<GlobalsAA>().and_then(|one| one.as_ref().ok());
        Self { program: Some(outer.program()), context, layout, metadata: &outer.metadata, globals: &outer.globals, function, globals_aa, references: None }
    }
}

/// Which globals no code outside the module reaches but by name:
/// `globalsaa::analysis`.
pub struct GlobalsAA;

impl ModuleAnalysis for GlobalsAA {
    type Result = Result<Globals, String>;
    const NAME: &'static str = "globals-aa";
    fn run(module: &Module, analyses: &mut ModuleAnalyses) -> Self::Result {
        globalsaa::analysis(module, analyses.program())
    }
}

/// Each defined function's memory effects, by name: `alias::summaries` of
/// the whole module, its globals as `GlobalsAA` finds them.
pub struct Summaries;

impl ModuleAnalysis for Summaries {
    type Result = Result<IndexMap<String, Summary>, String>;
    const NAME: &'static str = "summaries";
    fn run(module: &Module, analyses: &mut ModuleAnalyses) -> Self::Result {
        // GlobalsAA's answer is found again.
        let program = analyses.program();
        let globals = globalsaa::analysis(module, program)?;
        let procedures = module
            .functions()
            .filter(|(_, _, function)| !function.is_declaration())
            .filter_map(|(_, global, function)| {
                Some((global.name.clone()?, Procedure::of(Unit { program: Some(program), ..Unit::of(module, &program.layout, function) }.with_globals_aa(&globals))))
            })
            .collect();
        alias::summaries(&procedures, None)
    }
}

/// Every pointer's objects, with no caller context: `alias::pointers`.
pub struct Pointers;

impl Analysis for Pointers {
    type Result = Result<PointsTo, String>;
    const NAME: &'static str = "points-to";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        alias::points_to(&Unit::within(context, layout, function, analyses.outer()), None, None)
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
        alias::annotated_with(&Unit::within(context, layout, function, analyses.outer()), pointers, &registers)
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
        let outer = analyses.outer();
        call_effects(&Unit::within(context, layout, function, outer), outer)
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
        consts::known(&Unit::within(context, layout, function, analyses.outer()), None, None, None)
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
        floatfacts::solved_with(&Unit::within(context, layout, function, analyses.outer()), &calls, None)
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
        Ok(consts::known(&Unit::within(context, layout, function, analyses.outer()).with_references(references), Some(&calls), None, None))
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
        ranges::dominated_edges_with(&Unit::within(context, layout, function, analyses.outer()), &registers)
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
