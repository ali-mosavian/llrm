//! The memory analyses as entries of llrm-mir's analysis manager
//! (`passes::Analyses`), the role llrm-core's `analysis/manager.rs` played:
//! a pass asks, the manager computes once and drops what a pass did not
//! preserve. An entry reads its module and target through the outer proxy
//! (`passes::Outer`), and alias's callee summaries are a module analysis
//! there, as LLVM's `GlobalsAA`.

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
        Self { machine: outer.target.as_deref(), context, layout, metadata: &outer.metadata, globals: &outer.globals, function, references: None }
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
        let pointers = analyses.get::<Pointers>(context, layout, function);
        let registers = analyses.get::<Registers>(context, layout, function);
        let pointers = Result::as_ref(&*pointers).map_err(String::clone)?;
        alias::annotated_with(&Unit::within(context, layout, function, analyses.outer()), pointers, &registers)
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
        outer.cached::<Summaries>().ok_or("call effects need the module's summaries: require `Summaries`")?;
        call_effects(&Unit::within(context, layout, function, outer), outer)
    }
}

/// What each call of `unit` reads and writes, its callee as `Summaries`
/// says where `outer` holds it, and otherwise as its attributes alone do.
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

/// Every value known, memory solved alongside and each call writing what
/// `CallEffects` says.
pub struct ThroughMemory;

impl Analysis for ThroughMemory {
    type Result = Result<IndexMap<ValueId, Known>, String>;
    const NAME: &'static str = "through-memory";
    fn run(context: &Context, layout: &DataLayout, function: &Function, analyses: &mut Analyses) -> Self::Result {
        let effects = analyses.get::<CallEffects>(context, layout, function);
        let calls: Calls = Result::as_ref(&*effects).map_err(String::clone)?.iter().map(|(&at, effect)| (at, effect.stores.clone())).collect();
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
