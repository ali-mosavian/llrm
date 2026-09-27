//! The memory analyses as entries of llrm-mir's analysis manager
//! (`passes::Analyses`), the role llrm-core's `analysis/manager.rs` played:
//! a pass asks, the manager computes once and drops what a pass did not
//! preserve. An entry reads its module and target through the outer proxy
//! (`passes::Outer`), and alias's callee summaries are a module analysis
//! there, as LLVM's `GlobalsAA`.
//!
//! `Annotated` and `DominatedEdges` still solve the points-to and the
//! constants they build on themselves, rather than ask `Pointers` and
//! `Registers`.

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, InstId, Module, ValueId};
use llrm_mir::passes::{Analyses, Analysis, ModuleAnalysis, Outer};
use llrm_mir::target::Machine;
use llrm_support::hash::IndexMap;

use crate::alias::{self, Effect, PointsTo, Procedure, Summary};
use crate::consts::{self, Calls, Known};
use crate::memory::{MemRef, Unit};
use crate::ranges::{self, Interval};

impl<'a> Unit<'a> {
    /// `function` as the manager's analyses see it: its module and target
    /// as `outer` holds them.
    pub fn within(context: &'a Context, layout: &'a DataLayout, function: &'a Function, outer: &'a Outer) -> Self {
        Self { machine: outer.target.as_deref(), context, layout, metadata: &outer.metadata, globals: &outer.globals, function }
    }
}

/// Each defined function's memory effects, by name: `alias::summaries` of
/// the whole module.
pub struct Summaries;

impl ModuleAnalysis for Summaries {
    type Result = Result<IndexMap<String, Summary>, String>;
    const NAME: &'static str = "summaries";
    fn run(module: &Module, layout: &DataLayout, target: Option<&dyn Machine>) -> Self::Result {
        let procedures = module
            .functions()
            .filter(|(_, _, function)| !function.is_declaration())
            .filter_map(|(_, global, function)| Some((global.name.clone()?, Procedure::of(Unit { machine: target, ..Unit::of(module, layout, function) }))))
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
        alias::annotated(&Unit::within(context, layout, function, analyses.outer()))
    }
}

/// What each call reads and writes, its callee as `Summaries` says:
/// `alias::calls_annotated`. An error where `Summaries` was not required.
pub struct CallEffects;

impl Analysis for CallEffects {
    type Result = Result<IndexMap<InstId, Effect>, String>;
    const NAME: &'static str = "call-effects";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let outer = analyses.outer();
        let summaries = outer.cached::<Summaries>().ok_or("call effects need the module's summaries: require `Summaries`")?;
        let summaries = Result::as_ref(&*summaries).map_err(String::clone)?;
        alias::calls_annotated(&Procedure::of(Unit::within(context, layout, function, outer)), summaries)
    }
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

/// `Writes`, or where `Summaries` was not required none: each call then
/// writes what `memory::unmodeled_write` says.
pub fn writes(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Calls {
    Result::as_ref(&*analyses.get::<Writes>(context, layout, function)).cloned().unwrap_or_default()
}

/// Every value known, memory solved alongside and each call writing what
/// `CallEffects` says.
pub struct ThroughMemory;

impl Analysis for ThroughMemory {
    type Result = Result<IndexMap<ValueId, Known>, String>;
    const NAME: &'static str = "through-memory";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let calls = Result::as_ref(&*analyses.get::<Writes>(context, layout, function)).map_err(String::clone)?.clone();
        Ok(consts::known(&Unit::within(context, layout, function, analyses.outer()), Some(&calls), None, None))
    }
}

/// What unavoidable branch edges bound at each block:
/// `ranges::dominated_edges`.
pub struct DominatedEdges;

impl Analysis for DominatedEdges {
    type Result = Result<IndexMap<i64, IndexMap<ValueId, Interval>>, String>;
    const NAME: &'static str = "dominated-edges";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        ranges::dominated_edges(&Unit::within(context, layout, function, analyses.outer()))
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
