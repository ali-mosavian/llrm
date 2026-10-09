//! The MIR fixed point every driver runs: llrm-core's `optimize/transform.rs`
//! `pipeline`, `applied`, `recorded`, `Applied` and `_Transaction` (with
//! `_applied` and `_transacted`), on llrm-mir's pass manager. The
//! transaction is `Fixed`, one function pass; `applied` runs it under a
//! manager that requires `Summaries` and verifies the module after it,
//! then the whole-module step, `Interprocedural`, which runs it again on
//! each body it changes, then `Rotate`.
//!
//! What changed with the IR:
//! - A pass reports a change by preserving less than every analysis; the old
//!   compared bodies. Arenas never shrink, so the repeated-state check compares
//!   printed bodies instead.
//! - The stage records are the manager's change log (`recorded`); `watch` is
//!   `Applied::dump`, a file per changed step of each body.
//! - `Where`'s segment, BC blocks and object file went with the BC frontend.
//!   The machine's facts are the program's target's.
//! - PointerProvenance, SplitPointers and Place have no rich-MIR meaning.
//!   Hoist's store sinking is loopmotion's pass.
//! - `flow::optimized`'s rule that an irreducible body is not promoted is here:
//!   promotion needs dominators.
//! - The old `_Transaction` verified nothing; `applied`'s manager verifies the
//!   module after the pipeline.

use std::collections::BTreeSet;
use std::path::PathBuf;

use llrm_analysis::cfg;
use llrm_analysis::manager::{Callbacks, GlobalsAA, ProgramSummaries, Summaries};
use llrm_analysis::peelsize::Limits;
use llrm_mir::context::GlobalId;
use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Module, UnnamedAddr};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, ModuleAnalyses, PassManager, PreservedAnalyses, Stage, Unit};
use llrm_mir::print;
use llrm_mir::program::Program;

use crate::interprocedural::Interprocedural;
use crate::{
    addresssink, algebraic, availableexternally, calleepop, dead, decide, dse, fill, fixednarrow, floatloop, fold,
    gepoffset, globaldce, globalopt, gvn, hoist, indvars, inferspace, inline, jumpthread, lcssa, loopmotion,
    loopsimplify, lsr, peel, ports, promote, rotate, tailrec, trivialunswitch, unroll, unswitch, window,
};

/// Which passes run, and the copy budgets: the old `Options`. The default
/// is -O2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    /// Off at -O0: no pass runs.
    pub optimize: bool,
    pub limits: Limits,
    pub inline: inline::Threshold,
    /// Whether the callers' arguments are stated as ranges on a body's
    /// parameters: gcc's `-fipa-vrp`, -O2 and up.
    pub ipa_ranges: bool,
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
    pub sibcalls: bool,
    pub unswitch: bool,
    /// Loops not proven to run are entered behind a copy of their test, before
    /// the loop passes (gcc's `-ftree-ch`; not at -Os).
    pub copy_headers: bool,
    /// Code size outranks speed where they conflict: -Os and -Oz. (Whether a
    /// complete copy of a loop may grow the code is `limits.grows`, which
    /// gcc lets only -O3 do.)
    pub for_size: bool,
    /// The allocator tries other shapes of a body and keeps the cheapest
    /// (`-fallocation-search`).
    pub search: bool,
    /// Both routes through the machine phases are made and the cheaper kept
    /// (`-fallocation-routes`); else the allocator alone.
    pub routes: bool,
    /// With `search`, every shape rather than the one the spills suggest
    /// (`-fallocation-search-all`; -Omax).
    pub exhaustive: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            optimize: true,
            limits: Limits { target_percent: 100, ..Limits::default() },
            inline: inline::Threshold::default(),
            lcssa: true,
            floatloop: true,
            fold: true,
            decide: true,
            dead: true,
            hoist: true,
            ipa_ranges: true,
            forward: true,
            drop_loads: true,
            drop_stores: true,
            promote: true,
            strength: true,
            unroll: true,
            peel: true,
            fill: true,
            sibcalls: true,
            unswitch: false,
            copy_headers: false,
            for_size: false,
            search: true,
            routes: true,
            exhaustive: false,
        }
    }
}

impl Options {
    /// -O0.
    pub fn none() -> Self {
        Self { optimize: false, search: false, routes: false, ..Self::default() }
    }

    /// -O1: gcc's: the scalar passes and `-finline-functions-called-once`; a
    /// loop is copied out completely only where the code does not grow;
    /// nothing is inlined that `early-inlining-insns` (6) over
    /// `max-inline-insns-auto` (15) of the -O2 threshold does not admit,
    /// and no gcse, sibling calls, pattern fill, peeling or unswitching.
    pub fn basic() -> Self {
        Self {
            limits: Limits { grows: false, ..Self::default().limits },
            inline: inline::Threshold::new(Self::default().inline.limit * 6 / 15),
            ipa_ranges: false,
            forward: false,
            drop_loads: false,
            fill: false,
            sibcalls: false,
            peel: false,
            unswitch: false,
            ..Self::default()
        }
    }

    /// -O2: gcc's: -O1 with inlining, gcse, sibling calls and pattern fill; a
    /// complete copy of a loop still must not grow the code
    /// (`flag_cunroll_grow_size` is on at -O3, `-funroll-loops` and
    /// `-fpeel-loops` only).
    pub fn standard() -> Self {
        Self {
            limits: Limits { grows: false, ..Self::default().limits },
            peel: false,
            unswitch: false,
            ..Self::default()
        }
    }

    /// -O3: gcc's: -O2 with peeling, unswitching, complete copies that grow the
    /// code, and the larger inline threshold.
    pub fn speed() -> Self {
        Self {
            inline: inline::Threshold { cp_clone: true, ..inline::Threshold::new(250) },
            unswitch: true,
            ..Self::default()
        }
    }

    /// -Omax: every pass the default has on, LLVM's -O3 budgets, twice the
    /// target's unroll budget and a 250 inline threshold.
    pub fn aggressive() -> Self {
        Self {
            limits: Limits { target_percent: 200, ..Limits::default() },
            inline: inline::Threshold { cp_clone: true, ..inline::Threshold::new(250) },
            exhaustive: true,
            ..Self::default()
        }
    }

    /// -Os: no copy grows the code. Inlining keeps -O2's threshold: the
    /// threshold bounds a body's size, and what it admits, a body called
    /// once or one whose copies cost less than the calls they remove,
    /// shrinks the code here. A lower one would also refuse a constant-site
    /// clone that folds away.
    pub fn size() -> Self {
        Self {
            limits: Limits { grows: false, target_percent: 100, ..Limits::default() },
            inline: inline::Threshold::default().for_size(),
            for_size: true,
            ..Self::default()
        }
    }

    /// Whether the allocator tries other shapes of a body and keeps the
    /// cheapest.
    pub fn searches(&self) -> bool {
        self.search
    }

    /// Whether both routes are made and the cheaper kept.
    pub fn compares_routes(&self) -> bool {
        self.routes
    }

    /// Whether the search tries every shape of a body.
    pub fn searches_all(&self) -> bool {
        self.exhaustive
    }

    /// Whether code size outranks speed where they conflict: -Os and -Oz.
    pub fn prefers_size(&self) -> bool {
        self.for_size
    }

    /// -Oz: no loop is copied.
    pub fn min_size() -> Self {
        Self { unroll: false, peel: false, ..Self::size() }
    }

    /// Whether pass `name` is on. Every pass can be turned off, which is
    /// how a miscompile is bisected.
    fn wanted(
        &self,
        name: &str,
    ) -> bool {
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
            "indvars" | "lsr" | "window" | "gepoffset" | "addresssink" => self.strength,
            "unroll" => self.unroll,
            "peel" => self.peel,
            "fill" | "merge" => self.fill,
            "tailrec" => self.sibcalls,
            "ipa-ranges" => self.ipa_ranges,
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
    let mut every: Vec<Box<dyn FunctionPass>> = vec![
        // Aggregate/object leaves become ordinary SSA before any scalar or
        // CFG pass asks what is constant, redundant, or loop invariant.
        Box::new(promote::Sroa),
        Box::new(fold::Fold),
        // Before anything asks what a port call does to memory.
        Box::new(ports::Ports),
        Box::new(decide::Decide),
        // Once the arguments are values rather than frame cells; the loop it
        // makes goes to the loop passes below.
        Box::new(tailrec::TailRecursion),
        Box::new(loopsimplify::LoopSimplify),
        Box::new(lcssa::LoopClosedSSA),
        // Strict floating recurrences must retain their original iteration
        // order; LICM may move invariant preparation out afterwards.
        Box::new(floatloop::FloatLoop),
        Box::new(hoist::Hoist { size: applied.options.prefers_size() }),
        Box::new(loopmotion::LoopMotion),
        // The loop's exit tests that nothing in it changes are made once, with
        // the loop's entry.
        Box::new(trivialunswitch::TrivialUnswitch),
        // Before gvn: a far pointer cast from a near one is read as the near
        // one.
        Box::new(inferspace::InferAddressSpaces),
        Box::new(dse::Dse),
        Box::new(gvn::Gvn),
        // Ordinary scalar write-through promotion remains after memory GVN.
        Box::new(promote::Promote),
        Box::new(indvars::IndVars),
        Box::new(algebraic::Algebraic { size: applied.options.prefers_size() }),
        Box::new(dead::Dead),
        Box::new(unroll::Unroll { limits: limits() }),
        Box::new(peel::Peel { limits: limits() }),
        Box::new(fill::Fill { size: applied.options.prefers_size() }),
        Box::new(fill::Merge),
    ];
    every.into_iter().filter(|one| applied.options.wanted(one.name())).collect()
}

/// The order, from the pipeline itself rather than beside it.
pub fn passes() -> Vec<&'static str> {
    pipeline(&Applied::default()).iter().map(|one| one.name()).collect()
}

/// Plugs the pass manager's steps into `LLRM_DEBUG=time` and `runs`, if either
/// is on: `time` times them, `runs` tells a pass's own work from the analyses
/// it computed.
pub fn timed() {
    use llrm_support::debug;
    if !debug::enabled("time") && !debug::enabled("runs") {
        return;
    }
    llrm_mir::passes::observe(llrm_mir::passes::Observer {
        span: |kind, name, run| {
            let timed = |run: &mut dyn FnMut()| {
                if debug::enabled("time") { debug::timed_by(|| format!("{kind} {name}"), run) } else { run() }
            };
            if kind == "analysis" { debug::analysed(|| timed(run)) } else { timed(run) }
        },
        function: |name, run| debug::in_function(name, run),
        count: |what, hit| debug::counted(what, hit),
    });
}

/// `program` through the pipeline.
pub fn applied(
    program: &mut Program,
    applied: &Applied,
) -> Result<(), String> {
    recorded(program, applied).map(|_| ())
}

/// `applied`, with what the pipeline did to each function.
pub fn recorded(
    program: &mut Program,
    applied: &Applied,
) -> Result<Vec<Stage>, String> {
    timed();
    let mut manager = PassManager::default();
    manager.verify_each = llrm_support::debug::verifying();
    manager.dump = applied.dump.clone();
    if !applied.options.optimize {
        manager.add_program(availableexternally::EliminateAvailableExternally);
        return manager.run(program);
    }
    // As LLVM's O2 requires GlobalsAA before the function pipeline.
    manager.require::<GlobalsAA>();
    manager.require::<Summaries>();
    manager.require::<Callbacks>();
    manager.require_program::<ProgramSummaries>();
    // As LLVM's O2 runs GlobalOpt before the function pipeline.
    manager.add_module(globalopt::GlobalOpt);
    // Over every body first, so a caller's pipeline sees each callee's
    // port calls narrowed.
    if applied.options.wanted("ports") {
        manager.add(ports::Ports);
    }
    manager.add_program(crate::interprocedural::Stamp);
    manager.add(Fixed::new(applied));
    // Once every body has reached its own fixed point, as the old Nib
    // driver's whole-module step: a body it changes goes back through.
    let mut again =
        Fixed::new(&Applied { dump: applied.dump.as_ref().map(|one| one.join("interprocedural")), ..applied.clone() });
    manager.add_program(Interprocedural {
        pipeline: Box::new(move |module, analyses, id, stage| {
            TRIGGER.with(|trigger| *trigger.borrow_mut() = stage.to_owned());
            let done = rerun(module, analyses, id, &mut again).unwrap_or_else(|error| panic!("pipeline: {error}"));
            TRIGGER.with(|trigger| trigger.borrow_mut().clear());
            done
        }),
        specialise: {
            let mut alone = Fixed::new(&Applied { dump: None, ..applied.clone() });
            Box::new(move |module, analyses, mut function| {
                let layout = analyses.program().layout.clone();
                let outer = analyses.outer(module);
                let mut declared = Declared::over(std::rc::Rc::clone(&outer.globals), module.metadata.len());
                let Module { context, metadata, .. } = &mut *module;
                let mut unit = Unit {
                    context,
                    layout: &layout,
                    function: &mut function,
                    id: None,
                    metadata,
                    declared: &mut declared,
                };
                alone.run(&mut unit, &mut Analyses::new(std::rc::Rc::clone(&outer)));
                function
            })
        },
        proved: None,
        inline: applied.options.inline,
        ranges: applied.options.ipa_ranges,
        rate: Some(if applied.options.prefers_size() { 0 } else { applied.options.limits.milliclocks_per_byte }),
    });
    // What no live code names any more goes before selection, as LLVM runs
    // GlobalDCE after inlining.
    manager.add_program(globaldce::GlobalDce);
    manager.add_program(availableexternally::EliminateAvailableExternally);
    // Once the callers that remain are the ones that stay: an internal function
    // they all call directly pops its own arguments.
    if applied.options.wanted("calleepop") {
        manager.add_module(calleepop::CalleePop { size: applied.options.prefers_size() });
    }
    // The summaries are made. The passes below move and rewrite what a function
    // does and add nothing to it, so they read these and do not make them
    // again after each edit, as gcc's passes after IPA read the modref
    // summaries it made there.
    manager.freeze::<Summaries>();
    manager.freeze::<GlobalsAA>();
    // gcc's `pass_ch` (passes.def:232, in `pass_all_optimizations`) runs after
    // `pass_ipa_inline`: the inliner sizes a body before its header is copied.
    // The loops it guards are re-simplified for the passes that follow. Not at
    // -Os (`optimize_loop_for_size_p`): there only a loop proven to run is
    // entered at its body, which the last `Rotate` does.
    if applied.options.copy_headers && !applied.options.prefers_size() {
        manager.add(rotate::Rotate { proven: false, copy: true });
        manager.add(loopsimplify::LoopSimplify);
        manager.add(lcssa::LoopClosedSSA);
    }
    // Before LSR: a factor of two or a scale the product carries still shows as
    // a shift.
    if applied.options.wanted("fixednarrow") {
        manager.add(fixednarrow::FixedNarrow);
    }
    // Each loop's counters chosen once, on the loop the passes above leave.
    if applied.options.wanted("lsr") {
        manager.add(lsr::Lsr {
            size: applied.options.prefers_size(),
            bounds: if applied.options.searches_all() { lsr::Bounds::NONE } else { lsr::Bounds::GCC },
        });
        // What the counters it chose leave behind (a bound subtracted from a
        // counter rebased by it), as LLVM's LSR cleans with
        // SimplifyInstructions.
        manager.add(algebraic::Differences);
    }
    // On the pointers LSR chose: a huge one a loop keeps in one window is far
    // there.
    if applied.options.wanted("window") {
        manager.add(window::Window { size: applied.options.prefers_size() });
    }
    // Last, as the old drivers rotated in lowering: unroll and peel refuse
    // a rotated loop.
    manager.add(rotate::Rotate { proven: true, copy: false });
    // After the loop passes: the cycles it makes between the cases are no
    // natural loop. LLVM's DFAJumpThreading, gcc's FSM threader.
    if applied.options.wanted("jumpthread") {
        manager.add(jumpthread::JumpThread {
            size: applied.options.prefers_size(),
            correlated: applied.options.copy_headers,
        });
    }
    // A loop entered at its body runs it at least once: what it loads
    // unchanged may now leave it, as MachineLICM follows LLVM's LSR.
    if applied.options.wanted("hoist") {
        manager.add(hoist::Hoist { size: applied.options.prefers_size() });
    }
    // After hoist, which would move a constant `gep` out of its loop.
    if applied.options.wanted("gepoffset") {
        manager.add(gepoffset::GepOffset);
    }
    // After gvn's last partial redundancy elimination, which makes the phis it
    // sinks.
    if applied.options.wanted("addresssink") {
        manager.add(addresssink::AddressSink);
    }
    // Last: what it names are the instructions selection sees.
    manager.add_module(crate::spares::Spares);
    manager.add_module(crate::homes::Homes);
    manager.run(program)
}

thread_local! {
    /// What made the interprocedural step run a body's pipeline again (empty:
    /// the first run), for `LLRM_DEBUG=runs`.
    static TRIGGER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// `fixed` over body `id` alone, as the manager runs a function pass, its
/// module's analyses those `analyses` holds.
fn rerun(
    module: &mut Module,
    analyses: &mut ModuleAnalyses,
    id: GlobalId,
    fixed: &mut Fixed,
) -> Result<(), String> {
    let layout = analyses.program().layout.clone();
    let outer = analyses.outer(module);
    let mut declared = Declared::over(std::rc::Rc::clone(&outer.globals), module.metadata.len());
    let Module { context, globals, metadata, .. } = &mut *module;
    let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else {
        return Err(format!("@{}: not a function", id.0));
    };
    let mut unit = Unit { context, layout: &layout, function, id: Some(id), metadata, declared: &mut declared };
    let preserved = fixed.run(&mut unit, analyses.manager(id, &outer));
    analyses.invalidate(&preserved);
    if declared.place(module)? > 0 {
        analyses.invalidate(&PreservedAnalyses::none());
    }
    Ok(())
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
    /// Once, after everything else.
    last: Vec<Box<dyn FunctionPass>>,
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
        let (last, passes): (Vec<_>, Vec<_>) = passes.into_iter().partition(|one| one.name() == "merge");
        // A candidate is judged after the whole pipeline, unswitching off.
        let unswitch = applied
            .options
            .unswitch
            .then(
                || {
                    let options = Options { unswitch: false, ..applied.options.clone() };
                    let reoptimize = Applied { options, only: None, dump: None, ..applied.clone() };
                    unswitch::Unswitch { passes: vec![Box::new(Fixed::new(&reoptimize))] }
                },
            );
        Self {
            boundary,
            passes,
            unrollers,
            peelers,
            last,
            unswitch,
            only: only.is_some(),
            dump: applied.dump.clone(),
            runs: 0,
        }
    }

    fn transacted(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
        run: &mut Run,
    ) -> Result<(), String> {
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
        // Again while a peel leaves fewer loops, as LLVM's loop pass manager
        // revisits a parent once its child is gone: a loop whose inner loop
        // was peeled may then be peeled itself.
        let loops = |unit: &Unit| cfg::Shape::of(unit.function).loops.len();
        let mut before = loops(unit);
        while !self.peelers.is_empty() && run.step(&mut *self.peelers[0], "peel", unit, analyses) {
            self.scalarized(unit, analyses, run, "peeled");
            self.fixed(unit, analyses, run, "peeled-")?;
            let after = loops(unit);
            if after >= before {
                break;
            }
            before = after;
        }
        if let Some(unswitch) = &mut self.unswitch {
            run.step(unswitch, "unswitch", unit, analyses);
        }
        for one in &mut self.last {
            let stage = one.name();
            run.step(&mut **one, stage, unit, analyses);
        }
        Ok(run.settled(unit, analyses))
    }

    fn scalarized(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
        run: &mut Run,
        stage: &str,
    ) {
        for one in &mut self.boundary {
            let stage = format!("{stage}-{}", one.name());
            run.step(&mut **one, &stage, unit, analyses);
        }
    }

    fn fixed(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
        run: &mut Run,
        prefix: &str,
    ) -> Result<(), String> {
        // A monotone chain may expose one simplification per operation.
        let size = unit.function.layout().len() + unit.function.walk().count();
        let limit = std::cmp::max(16, size + 1);
        // The change each pass last left the body at, unchanged: handed that
        // body again it would change nothing, so it is skipped.
        let mut settled: Vec<Option<usize>> = vec![None; self.passes.len()];
        let mut unroll_settled = None;
        // Separately reject a repeated state, so an oscillator fails at once
        // instead of consuming the limit.
        let mut history = BTreeSet::from([print::body(unit.context, unit.function)]);
        run.fixed += 1;
        for iteration in 0..limit {
            let before = run.version;
            run.rounds += 1;
            for (one, settled) in self.passes.iter_mut().zip(&mut settled) {
                if *settled == Some(run.version) {
                    run.skipped += 1;
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

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        self.runs += 1;
        let promotes = analyses
            .get::<cfg::Shape>(unit.context, unit.layout, unit.function)
            .dominance
            .irreducible(unit.function)
            .is_empty();
        let dump = self.dump.as_ref().map(|directory| directory.join(format!("{:02}", self.runs)));
        let mut run = Run {
            version: 0,
            promotes,
            dump,
            rounds: 0,
            steps: 0,
            skipped: 0,
            fixed: 0,
            billing: llrm_support::debug::enabled("runs"),
            idle: 0,
            useful: 0,
            idle_with_analyses: 0,
            useful_with_analyses: 0,
        };
        self.transacted(unit, analyses, &mut run).unwrap_or_else(|error| panic!("pipeline: {error}"));
        llrm_support::debug!(
            "runs",
            "body {} trigger {:?}: {} fixed points, {} rounds, {} pass runs, {} skipped as settled, {} changes, work idle {} useful {} (with the analyses the passes computed first: idle {} useful {})",
            unit.id.map_or(-1, |id| i64::from(id.0)),
            TRIGGER.with(|trigger| trigger.borrow().clone()),
            run.fixed,
            run.rounds,
            run.steps,
            run.skipped,
            run.version,
            run.idle,
            run.useful,
            run.idle_with_analyses,
            run.useful_with_analyses
        );
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
    /// For `LLRM_DEBUG=runs`: fixed points reached, rounds in them, passes run,
    /// passes skipped as settled.
    fixed: usize,
    rounds: usize,
    steps: usize,
    skipped: usize,
    /// The work of passes that changed nothing, and of those that did (only
    /// where `runs` is on).
    billing: bool,
    idle: u64,
    useful: u64,
    /// The same two with the analyses the passes computed first: what skipping
    /// a pass would not save, since the next pass to ask would pay it.
    idle_with_analyses: u64,
    useful_with_analyses: u64,
}

impl Run {
    /// `pass` over the body; whether it changed it.
    fn step(
        &mut self,
        pass: &mut dyn FunctionPass,
        stage: &str,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> bool {
        if !self.promotes && matches!(pass.name(), "sroa" | "promote") {
            return false;
        }
        self.steps += 1;
        let before = unit.function.mark();
        let billed = self.billing.then(|| (llrm_support::debug::work(), llrm_support::debug::analysed_work()));
        let preserved =
            llrm_mir::passes::spanned(pass.name(), || pass.run(unit, analyses)).unless_unchanged(unit.function, before);
        if let Some((billed, analysed)) = billed {
            let with_analyses = llrm_support::debug::work() - billed;
            let own = with_analyses.saturating_sub(llrm_support::debug::analysed_work() - analysed);
            let idle = preserved.are_all_preserved();
            *(if idle { &mut self.idle } else { &mut self.useful }) += own;
            *(if idle { &mut self.idle_with_analyses } else { &mut self.useful_with_analyses }) += with_analyses;
            llrm_support::debug!(
                "runs",
                "step {stage} {} {own} with-analyses {with_analyses}",
                if idle { "idle" } else { "changed" }
            );
        }
        if preserved.are_all_preserved() {
            return false;
        }
        llrm_mir::passes::note_pass(pass.name());
        llrm_mir::passes::spanned("invalidate", || analyses.invalidate(&preserved));
        analyses.check_kept(pass.name(), unit.context, unit.layout, unit.function);
        self.changed(stage, unit, analyses);
        true
    }

    /// The body a pipeline hands out, with its unreachable blocks settled.
    fn settled(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) {
        if cfg::_unreachable(unit.context, unit.function) {
            analyses.invalidate(&PreservedAnalyses::none());
            self.changed("unreachable", unit, analyses);
        }
    }

    fn changed(
        &mut self,
        stage: &str,
        unit: &Unit,
        analyses: &Analyses,
    ) {
        self.version += 1;
        let Some(directory) = &self.dump else { return };
        // The body beside every global's declaration, which is what it names.
        let mut globals = analyses.outer().globals.to_vec();
        globals.push(GlobalValue {
            name: Some("pipeline.body".to_owned()),
            linkage: Linkage::Internal,
            unnamed_addr: UnnamedAddr::default(),
            address_space: 0,
            kind: GlobalKind::Function(Box::new(unit.function.clone())),
        });
        let module = Module {
            context: unit.context.clone(),
            datalayout: None,
            globals,
            metadata: unit.metadata.to_vec(),
            named_metadata: Vec::new(),
        };
        let file = directory.join(format!("{:03}-{stage}.ll", self.version));
        std::fs::create_dir_all(directory)
            .and_then(|()| std::fs::write(&file, llrm_mir::print::module(&module)))
            .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod pipeline_tests;
