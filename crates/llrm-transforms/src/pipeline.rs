//! The MIR fixed point every driver runs: llrm-core's `optimize/transform.rs`
//! `pipeline`, `applied`, `recorded`, `Applied` and `_Transaction` (with
//! `_applied` and `_transacted`), on llrm-mir's pass manager. The
//! transaction is `Fixed`, one function pass; `applied` runs it under a
//! manager that requires `Summaries` and verifies the module after it,
//! then the whole-module step, `Interprocedural`, which runs it again on
//! each body it changes, then `Rotate`.
//!
//! What changed with the IR:
//! - A pass reports a change by preserving less than every analysis; the
//!   old compared bodies. Arenas never shrink, so the repeated-state check
//!   compares printed bodies instead.
//! - The stage records are the manager's change log (`recorded`); `watch`
//!   is `Applied::dump`, a file per changed step of each body.
//! - `Where`'s segment, BC blocks and object file went with the BC
//!   frontend; its index scales and address forms were strength's, which
//!   prices neither here. The machine's facts are `target`'s.
//! - PointerProvenance, SplitPointers and Place have no rich-MIR meaning.
//!   Hoist's store sinking is loopmotion's pass.
//! - `flow::optimized`'s rule that an irreducible body is not promoted is
//!   here: promotion needs dominators.
//! - The old `_Transaction` verified nothing; `applied`'s manager verifies
//!   the module after the pipeline.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use llrm_analysis::cfg;
use llrm_analysis::manager::Summaries;
use llrm_analysis::peelsize::Limits;
use llrm_graph::loops;
use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Module, UnnamedAddr};
use llrm_mir::print;
use llrm_mir::passes::{Analyses, Declared, FunctionPass, Outer, PassManager, PreservedAnalyses, Stage, Unit};
use llrm_mir::target::Machine;

use crate::interprocedural::Interprocedural;
use crate::{
    affine, algebraic, dead, decide, dse, fill, floatloop, fold, gvn, hoist, indvars, lcssa, loopmotion, loopsimplify, peel, promote,
    rotate, strength, unroll, unswitch,
};

/// Which passes run, and the copy budgets: the old `Options`. The default
/// is -O2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    pub limits: Limits,
    pub lcssa: bool,
    pub floatloop: bool,
    pub fold: bool,
    pub decide: bool,
    pub dead: bool,
    pub hoist: bool,
    pub forward: bool,
    pub drop_loads: bool,
    pub drop_stores: bool,
    pub promote: bool,
    pub strength: bool,
    pub unroll: bool,
    pub peel: bool,
    pub fill: bool,
    pub unswitch: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            lcssa: true,
            floatloop: true,
            fold: true,
            decide: true,
            dead: true,
            hoist: true,
            forward: true,
            drop_loads: true,
            drop_stores: true,
            promote: true,
            strength: true,
            unroll: true,
            peel: true,
            fill: true,
            unswitch: false,
        }
    }
}

impl Options {
    /// -Os: no copy grows the code.
    pub fn size() -> Self {
        Self { limits: Limits { grows: false, ..Limits::default() }, ..Self::default() }
    }

    /// Whether pass `name` is on. Every pass can be turned off, which is
    /// how a miscompile is bisected.
    fn wanted(&self, name: &str) -> bool {
        match name {
            "lcssa" => self.lcssa,
            "floatloop" => self.floatloop,
            "fold" => self.fold,
            "decide" => self.decide,
            "dead" => self.dead,
            "hoist" | "loopmotion" => self.hoist,
            "gvn" => self.forward && self.drop_loads,
            "dse" => self.drop_stores,
            "sroa" | "promote" => self.promote,
            "strength" | "zeroed" => self.strength,
            "unroll" => self.unroll,
            "peel" => self.peel,
            "fill" => self.fill,
            _ => true,
        }
    }
}

/// `applied`'s arguments: the old ones less the BC frontend's.
#[derive(Clone, Default)]
pub struct Applied {
    pub options: Options,
    /// The one pass to run, by name.
    pub only: Option<String>,
    /// What analyses ask of the machine: its prices and registers.
    pub target: Option<Rc<dyn Machine>>,
    /// Where each body's changed steps are written, `N/NNN-stage.ll`, the
    /// Nth body run; the manager writes the module after it there.
    pub dump: Option<PathBuf>,
}


/// The passes, in order, that `applied` leaves on.
///
/// A pass that is off is not in it, rather than in it and skipped, so what
/// runs is what this returns.
pub fn pipeline(applied: &Applied) -> Vec<Box<dyn FunctionPass>> {
    let limits = || applied.options.limits.clone();
    let every: Vec<Box<dyn FunctionPass>> = vec![
        // Aggregate/object leaves become ordinary SSA before any scalar or
        // CFG pass asks what is constant, redundant, or loop invariant.
        Box::new(promote::Sroa),
        Box::new(fold::Fold),
        Box::new(decide::Decide),
        Box::new(loopsimplify::LoopSimplify),
        Box::new(lcssa::LoopClosedSSA),
        // Strict floating recurrences must retain their original iteration
        // order; LICM may move invariant preparation out afterwards.
        Box::new(floatloop::FloatLoop),
        Box::new(affine::Affine),
        Box::new(hoist::Hoist),
        Box::new(loopmotion::LoopMotion),
        Box::new(dse::Dse),
        Box::new(gvn::Gvn),
        // Ordinary scalar write-through promotion remains after memory GVN.
        Box::new(promote::Promote),
        Box::new(strength::Strength),
        Box::new(algebraic::Algebraic),
        Box::new(dead::Dead),
        Box::new(unroll::Unroll { limits: limits() }),
        Box::new(peel::Peel { limits: limits() }),
        Box::new(fill::Fill),
        Box::new(indvars::CountToZero),
    ];
    every.into_iter().filter(|one| applied.options.wanted(one.name())).collect()
}

/// The order, from the pipeline itself rather than beside it.
pub fn passes() -> Vec<&'static str> {
    pipeline(&Applied::default()).iter().map(|one| one.name()).collect()
}

/// The settled round after which a pass is first admitted: strength once
/// the scalar passes settle, counting to zero once strength has.
fn settles_after(name: &str) -> u8 {
    match name {
        "strength" => 1,
        "zeroed" => 2,
        _ => 0,
    }
}

/// `module` through the pipeline.
pub fn applied(module: &mut Module, applied: &Applied) -> Result<(), String> {
    recorded(module, applied).map(|_| ())
}

/// `applied`, with what the pipeline did to each function.
pub fn recorded(module: &mut Module, applied: &Applied) -> Result<Vec<Stage>, String> {
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.dump = applied.dump.clone();
    manager.target = applied.target.clone();
    manager.require::<Summaries>();
    manager.add(Fixed::new(applied));
    // Once every body has reached its own fixed point, as the old Nib
    // driver's whole-module step: a body it changes goes back through.
    let mut again = Fixed::new(&Applied { dump: applied.dump.as_ref().map(|one| one.join("interprocedural")), ..applied.clone() });
    let target = applied.target.clone();
    manager.add_module(Interprocedural {
        target: applied.target.clone(),
        roots: roots(module),
        pipeline: Box::new(move |module, id, _| rerun(module, id, &mut again, target.clone()).unwrap_or_else(|error| panic!("pipeline: {error}"))),
        proved: None,
    });
    // Last, as the old drivers rotated in lowering: unroll and peel refuse
    // a rotated loop.
    manager.add(rotate::Rotate);
    manager.run(module)
}

/// The bodies something outside the module may call.
fn roots(module: &Module) -> BTreeSet<GlobalId> {
    module
        .functions()
        .filter(|(_, global, function)| !function.is_declaration() && !matches!(global.linkage, Linkage::Internal | Linkage::Private))
        .map(|(id, _, _)| id)
        .collect()
}

/// `fixed` over body `id` alone, as the manager runs a function pass.
fn rerun(module: &mut Module, id: GlobalId, fixed: &mut Fixed, target: Option<Rc<dyn Machine>>) -> Result<(), String> {
    let layout = match &module.datalayout {
        Some(text) => DataLayout::parse(text)?,
        None => DataLayout::default(),
    };
    let mut outer = Outer::of(module, target);
    outer.require::<Summaries>(module);
    let callees = llrm_mir::memory::callees(module);
    let sizes = llrm_mir::valuetracking::sizes(module, &layout);
    let mut declared = Declared::of(module);
    let Module { context, globals, metadata, .. } = &mut *module;
    let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else {
        return Err(format!("@{}: not a function", id.0));
    };
    let mut unit = Unit { context, layout: &layout, function, callees: &callees, metadata, sizes: &sizes, declared: &mut declared };
    fixed.run(&mut unit, &mut Analyses::new(Rc::new(outer)));
    declared.place(module)
}

/// The pipeline over one body, the old `_Transaction`: the structural
/// passes at its boundaries, the scalar ones to a fixed point with
/// unrolling after each round, peeling once that settles, then
/// unswitching.
pub struct Fixed {
    boundary: Vec<Box<dyn FunctionPass>>,
    passes: Vec<Box<dyn FunctionPass>>,
    unrollers: Vec<Box<dyn FunctionPass>>,
    peelers: Vec<Box<dyn FunctionPass>>,
    unswitch: Option<unswitch::Unswitch>,
    only: bool,
    dump: Option<PathBuf>,
    /// Bodies run, naming each one's dump.
    runs: usize,
}

impl Fixed {
    pub fn new(applied: &Applied) -> Self {
        // Public debugging selectors from before value reuse became one pass.
        let only = match applied.only.as_deref() {
            Some("forward" | "drop_loads" | "reuse" | "cse") => Some("gvn"),
            Some("drop_stores") => Some("dse"),
            other => other,
        };
        let passes = pipeline(applied).into_iter().filter(|one| only.is_none_or(|only| one.name() == only));
        // SROA establishes the scalar memory shape at structural
        // boundaries; it is no member of the scalar fixed point.
        let (boundary, passes): (Vec<_>, Vec<_>) = passes.partition(|one| one.name() == "sroa");
        let (unrollers, passes): (Vec<_>, Vec<_>) = passes.into_iter().partition(|one| one.name() == "unroll");
        let (peelers, passes): (Vec<_>, Vec<_>) = passes.into_iter().partition(|one| one.name() == "peel");
        // A candidate is judged after the whole pipeline, unswitching off.
        let unswitch = applied.options.unswitch.then(|| {
            let options = Options { unswitch: false, ..applied.options.clone() };
            let reoptimize = Applied { options, only: None, dump: None, ..applied.clone() };
            unswitch::Unswitch { passes: vec![Box::new(Fixed::new(&reoptimize))] }
        });
        Self { boundary, passes, unrollers, peelers, unswitch, only: only.is_some(), dump: applied.dump.clone(), runs: 0 }
    }

    fn transacted(&mut self, unit: &mut Unit, analyses: &mut Analyses, run: &mut Run) -> Result<(), String> {
        self.scalarized(unit, analyses, run, "r01");
        if self.only && !self.boundary.is_empty() {
            return Ok(run.settled(unit, analyses));
        }
        if self.only && !self.unrollers.is_empty() {
            run.step(&mut *self.unrollers[0], "r01-unroll", unit, analyses);
            return Ok(run.settled(unit, analyses));
        }
        if self.only && !self.peelers.is_empty() {
            run.step(&mut *self.peelers[0], "r01-peel", unit, analyses);
            return Ok(run.settled(unit, analyses));
        }
        self.fixed(unit, analyses, run, "")?;
        if !self.peelers.is_empty() && run.step(&mut *self.peelers[0], "peel", unit, analyses) {
            self.scalarized(unit, analyses, run, "peeled");
            self.fixed(unit, analyses, run, "peeled-")?;
        }
        if let Some(unswitch) = &mut self.unswitch {
            run.step(unswitch, "unswitch", unit, analyses);
        }
        Ok(run.settled(unit, analyses))
    }

    fn scalarized(&mut self, unit: &mut Unit, analyses: &mut Analyses, run: &mut Run, stage: &str) {
        for one in &mut self.boundary {
            let stage = format!("{stage}-{}", one.name());
            run.step(&mut **one, &stage, unit, analyses);
        }
    }

    fn fixed(&mut self, unit: &mut Unit, analyses: &mut Analyses, run: &mut Run, prefix: &str) -> Result<(), String> {
        // A monotone chain may expose one simplification per operation.
        let size = unit.function.layout().len() + unit.function.walk().count();
        let limit = std::cmp::max(16, size + 1);
        // The change each pass last left the body at, unchanged: handed that
        // body again it would change nothing, so it is skipped.
        let mut settled: Vec<Option<usize>> = vec![None; self.passes.len()];
        let mut unroll_settled = None;
        // Passes wait by stage; each settled round admits the next.
        let last = if self.only { 0 } else { self.passes.iter().map(|one| settles_after(one.name())).max().unwrap_or(0) };
        let mut stage = 0;
        // Separately reject a repeated state, so an oscillator fails at once
        // instead of consuming the limit.
        let mut history = BTreeSet::from([print::body(unit.context, unit.function)]);
        for iteration in 0..limit {
            let before = run.version;
            for (one, settled) in self.passes.iter_mut().zip(&mut settled) {
                if settles_after(one.name()) > stage || *settled == Some(run.version) {
                    continue;
                }
                let name = format!("{prefix}r{:02}-{}", iteration + 1, one.name());
                *settled = (!run.step(&mut **one, &name, unit, analyses)).then_some(run.version);
            }
            // Ask at the original pipeline boundary: fully converging the
            // scalar passes first destroys matmul's exact counted-loop shape.
            if !self.unrollers.is_empty() && unroll_settled != Some(run.version) {
                let name = format!("{prefix}r{:02}-unroll", iteration + 1);
                if run.step(&mut *self.unrollers[0], &name, unit, analyses) {
                    // A copy's constant indices are new exact leaves, so it
                    // crosses the structural boundary before the scalar
                    // passes settle it.
                    self.scalarized(unit, analyses, run, &format!("{prefix}unrolled"));
                } else {
                    unroll_settled = Some(run.version);
                }
            }
            if stage < last && run.version == before {
                stage += 1;
                continue;
            }
            if self.only || run.version == before {
                run.settled(unit, analyses);
                return Ok(());
            }
            if !history.insert(print::body(unit.context, unit.function)) {
                return Err(format!("MIR optimization did not converge: cycle after {} rounds", iteration + 1));
            }
        }
        Err(format!("MIR optimization did not converge after {limit} size-scaled rounds"))
    }
}

impl FunctionPass for Fixed {
    fn name(&self) -> &'static str {
        "pipeline"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        self.runs += 1;
        let graph = cfg::graph(unit.function);
        let promotes = loops::irreducible(&graph, unit.function.entry().map(cfg::id)).is_empty();
        let dump = self.dump.as_ref().map(|directory| directory.join(format!("{:02}", self.runs)));
        let mut run = Run { version: 0, promotes, dump };
        self.transacted(unit, analyses, &mut run).unwrap_or_else(|error| panic!("pipeline: {error}"));
        if run.version == 0 { PreservedAnalyses::all() } else { PreservedAnalyses::none() }
    }
}

/// One body's run through `Fixed`.
struct Run {
    /// Changes so far.
    version: usize,
    /// Whether promotion may run: not over an irreducible CFG.
    promotes: bool,
    dump: Option<PathBuf>,
}

impl Run {
    /// `pass` over the body; whether it changed it.
    fn step(&mut self, pass: &mut dyn FunctionPass, stage: &str, unit: &mut Unit, analyses: &mut Analyses) -> bool {
        if !self.promotes && matches!(pass.name(), "sroa" | "promote") {
            return false;
        }
        let preserved = pass.run(unit, analyses);
        if preserved.are_all_preserved() {
            return false;
        }
        analyses.invalidate(&preserved);
        self.changed(stage, unit, analyses);
        true
    }

    /// The body a pipeline hands out, with its unreachable blocks settled.
    fn settled(&mut self, unit: &mut Unit, analyses: &mut Analyses) {
        if cfg::_unreachable(unit.context, unit.function) {
            analyses.invalidate(&PreservedAnalyses::none());
            self.changed("unreachable", unit, analyses);
        }
    }

    fn changed(&mut self, stage: &str, unit: &Unit, analyses: &Analyses) {
        self.version += 1;
        let Some(directory) = &self.dump else { return };
        // The body beside every global's declaration, which is what it names.
        let mut globals = analyses.outer().globals.clone();
        globals.push(GlobalValue {
            name: Some("pipeline.body".to_owned()),
            linkage: Linkage::Internal,
            unnamed_addr: UnnamedAddr::default(),
            address_space: 0,
            kind: GlobalKind::Function(Box::new(unit.function.clone())),
        });
        let module = Module { context: unit.context.clone(), datalayout: None, globals, metadata: unit.metadata.to_vec(), named_metadata: Vec::new() };
        let file = directory.join(format!("{:03}-{stage}.ll", self.version));
        std::fs::create_dir_all(directory)
            .and_then(|()| std::fs::write(&file, llrm_mir::print::module(&module)))
            .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod pipeline_tests;
