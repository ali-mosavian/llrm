//! `LoopIdiomRecognizePass` as a function pass of the pipeline: every loop,
//! the innermost first, through [`LoopIdiomRecognize::run_on_loop`], with
//! BasicAA.
//!
//! # How it works
//! The pass takes the manager's dominators and loops, makes the assumptions
//! and a `ScalarEvolution` for the function, and runs the loops in post-order.
//! A loop it changes has its expressions forgotten (`forgetLoop`). The pass
//! adds instructions inside existing blocks, so the loops and dominators stay
//! true: it keeps both.
//!
//! # Example
//! A loop that stores a zero byte to `%p + %i` for `%n` trips becomes
//! `call void @llvm.memset.p0.i32(ptr %p, i8 0, i32 %n, i1 false)` in its
//! preheader; the loop is left for later passes to delete.
//!
//! # Used by
//! The `loopidiom@737` slot of `pipeline`.
//!
//! # See also
//! [`super`] (the translation), `loopidioms` (the older pass that runs after
//! it for what this one does not take).

use llrm_analysis::aa::AAResults;
use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::scev::expander::Env;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, GlobalValue, MetadataNode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::valuetracking::AssumptionCache;

use super::process::{AaProvider, Run};
use super::{Library, LoopIdiomRecognize, Options};

/// BasicAA over the function as it stands at each query.
pub struct BasicAaProvider<'a> {
    pub layout: &'a DataLayout,
    pub metadata: &'a [MetadataNode],
    pub globals: &'a [GlobalValue],
}

impl AaProvider for BasicAaProvider<'_> {
    fn with(
        &self,
        context: &Context,
        function: &Function,
        query: &mut dyn FnMut(&mut dyn AAResults) -> bool,
    ) -> bool {
        let unit = llrm_analysis::memory::Unit::over(context, self.layout, self.metadata, self.globals, function);
        query(&mut BasicAA::new(&unit))
    }
}

/// `LoopIdiomRecognizePass`.
pub struct LoopIdiom {
    pub options: Options,
    pub library: Library,
    /// The routines the compile states the program links.
    pub routines: Vec<llrm_mir::libfunc::Routine>,
    /// -Os or -Oz.
    pub for_size: bool,
}

impl LoopIdiom {
    /// The library calls a DOS target has: `memset` and `memcpy`, no
    /// `memset_pattern16`.
    pub fn new() -> Self {
        Self::with_routines(Vec::new())
    }

    /// The pass over a compile that states the library `routines` its program
    /// links (the pin's `TargetLibraryInfo`).
    pub fn with_routines(routines: Vec<llrm_mir::libfunc::Routine>) -> Self {
        Self {
            options: Options::default(),
            library: Library { memset: true, memset_pattern16: false, memcpy: true },
            routines,
            for_size: false,
        }
    }
}

impl Default for LoopIdiom {
    fn default() -> Self {
        Self::new()
    }
}

/// The headers of `loops`' loops, the innermost first (`LoopPassManager`'s
/// order).
fn post_order(loops: &llrm_mir::loopinfo::LoopInfo) -> Vec<BlockId> {
    fn walk(
        one: llrm_mir::loopinfo::Loop,
        out: &mut Vec<BlockId>,
    ) {
        for sub in one.get_sub_loops() {
            walk(sub, out);
        }
        out.push(one.get_header());
    }
    let mut out = Vec::new();
    for top in loops.top_level_loops() {
        walk(top, &mut out);
    }
    out
}

impl FunctionPass for LoopIdiom {
    fn name(&self) -> &'static str {
        "loopidiom"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("loop-idiom")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let loops = analyses.get::<Loops>(unit.context, unit.layout, unit.function);
        let headers = post_order(&loops);
        if headers.is_empty() {
            return PreservedAnalyses::all();
        }
        let outer = std::rc::Rc::clone(analyses.outer());
        let name = unit
            .id
            .and_then(|id| outer.globals.get(id.0 as usize))
            .and_then(|global| global.name.owned())
            .unwrap_or_default();
        let assumptions = AssumptionCache::of(unit.context, &outer.globals, unit.metadata, unit.function);
        let env = Env {
            globals: &outer.globals,
            metadata: unit.metadata,
            layout: unit.layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let tti = outer.target().tti();
        let provider = BasicAaProvider { layout: unit.layout, metadata: unit.metadata, globals: &outer.globals };
        let mark = unit.function.mark();
        let held = analyses.get::<llrm_analysis::scev::analysis::Scevs>(unit.context, unit.layout, unit.function);
        let mut se = held.take();
        let size_costs = self.for_size.then(|| outer.target().size_costs());
        let clock_costs = self
            .for_size
            .then(|| (outer.target().costs(), outer.target().size_milliclocks_per_byte()))
            .filter(|(_, rate)| *rate > 0);
        let speed_costs = (!self.for_size).then(|| outer.target().costs());
        let mut changed = false;
        for header in headers {
            let cur_loop = loops.get_loop_for(header).expect("a loop");
            let mut run = Run {
                name: &name,
                env: &env,
                context: &mut *unit.context,
                function: &mut *unit.function,
                se: &mut se,
                aa: &provider,
                declared: &mut *unit.declared,
                cur_loop,
                bytes: size_costs.as_ref().map(|costs| (costs, outer.callees())),
                clocks: clock_costs.as_ref().map(|(costs, rate)| (costs, *rate)),
                speed: speed_costs.as_ref(),
                tti: tti.as_deref(),
            };
            let mut options = self.options;
            options.force_memset_pattern_intrinsic |= self.for_size && outer.target().fills_by_pattern();
            let mut idiom = LoopIdiomRecognize::new(options, self.library);
            idiom.routines = self.routines.clone();
            idiom.for_size = self.for_size;
            if idiom.run_on_loop(&mut run) == Some(true) {
                changed = true;
                // The pass reports a change when it expanded code it then
                // removed (the pin does): nothing was edited, the counts stand.
                if unit.function.mark() != mark {
                    let cx = env.cx(&*unit.context, &*unit.function);
                    se.forget_loop(&cx, header);
                }
            }
        }
        if changed && unit.function.mark() != mark {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            held.put(se);
            PreservedAnalyses::all()
        }
    }
}
