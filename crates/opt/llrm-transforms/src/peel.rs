//! Whole-loop peeling: clone every iteration of a loop with a known trip
//! count ahead of it, priced before anything is cloned, and let Fold, Decide
//! and Dead prove the residual loop unreachable.
//!
//! # How it works
//! Peeling is the CFG counterpart of full straight-line unrolling. For each
//! loop of a copy of the body, closed to LCSSA (`lcssa::closed`), the pass
//! - takes the trip count (ScalarEvolution's `getSmallConstantTripCount`, else
//!   `getSmallConstantMaxTripCount`) and
//!   skips the loop below two trips; with neither, the most backedges the
//!   objects its `inbounds` accesses walk allow (`induction::object_backedges`,
//!   GCC's `infer_loop_bounds_from_array`), after which the residual loop's
//!   latch is made `unreachable`, as GCC's `try_unroll_loop_completely` does
//!   with a bound it cannot fold
//!   ([tree-ssa-loop-ivcanon.cc:686](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-loop-ivcanon.cc#L686)):
//!   `k < n` over `B[k][j]` of a 12 x 12 array is twelve copies, each with its
//!   exit test. A function of more than 150 instructions keeps such loops
//!   rolled: nothing folds in their copies, and the passes that compare memory
//!   accesses in pairs grow faster than the code (qcport/mod_tex -O3: 2.3 times
//!   the instructions for 1.6 times the code);
//! - asks `llrm_analysis::peelsize::admitted` whether the copies are worth
//!   their size: the budgets are `peelsize::Limits`, counted in instructions,
//!   and the loop's `Site` comes from `profit::site`;
//! - clones the iterations with [`crate::loopclone::peeled`].
//!
//! The copy is kept, and the next loop looked for, until none is admitted.
//! Each loop is peeled once, named by its latch. The residual loop stays as a
//! correctness fallback: the copies' header tests fold to constants, so the
//! branch into it is decided and `Dead` removes it.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>pre</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   b["<b>B</b>"]:::mid
//!   ex["<b>exit</b>"]:::mid
//!   pre --> h
//!   h --> b
//!   b --> h
//!   b --> ex
//! ```
//!
//! *Loop of 3 trips*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>pre</b>"]:::mid
//!   h1["<b>H1</b>"]:::mid
//!   b1["<b>B1</b>"]:::mid
//!   h2["<b>H2</b>"]:::mid
//!   b2["<b>B2</b>"]:::mid
//!   h3["<b>H3</b>"]:::mid
//!   b3["<b>B3</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   b["<b>B</b>"]:::mid
//!   pre --> h1
//!   h1 --> b1
//!   b1 --> h2
//!   h2 --> b2
//!   b2 --> h3
//!   h3 --> b3
//!   b3 --> h
//!   h --> b
//!   b --> h
//! ```
//!
//! *After peel*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>pre</b>"]:::mid
//!   b1["<b>B1</b>"]:::mid
//!   b2["<b>B2</b>"]:::mid
//!   b3["<b>B3</b>"]:::mid
//!   ex["<b>exit</b>"]:::mid
//!   pre --> b1
//!   b1 --> b2
//!   b2 --> b3
//!   b3 --> ex
//! ```
//!
//! *After fold, decide and dead*
//!
//! The one machine number is the target's: a function with an instruction the
//! target does not price (`profit::priced`) is left alone, since
//! `peelsize` cannot size it. The target's `unroll_budget` is scaled by
//! `Limits::target_percent`.
//!
//! # Example
//! Before:
//! ```mir
//! define ... @pipeline.body() ... {
//!   ; ...
//! b2:
//!   %0 = phi i16 [ 0, %b1 ], [ %6, %b5 ]
//!   %1 = icmp slt i16 %0, 2
//!   br i1 %1, label %b4, label %b3
//!
//!   ; ...
//! b4:
//!   %2 = mul i16 %0, 2
//!   %3 = getelementptr inbounds i8, ptr @_a, i16 %2
//!   %4 = load i16, ptr %3, !tbaa !9
//!   %5 = icmp slt i16 %4, 0
//!   br i1 %5, label %b6, label %b5
//!
//! b5:
//!   %6 = add nsw i16 %0, 1
//!   br label %b2
//!
//!   ; ...
//! b7:
//!   %7 = phi i16 [ 0, %b3 ], [ 1, %b6 ]
//!   ret i16 %7
//! }
//! ```
//! After:
//! ```mir
//! define ... @pipeline.body() ... {
//! b1:
//!   %0 = getelementptr inbounds i8, ptr @_a, i16 0
//!   %1 = load i16, ptr %0, !tbaa !9
//!   %2 = icmp slt i16 %1, 0
//!   br i1 %2, label %b6, label %4
//!
//! b6:
//!   br label %b7
//!
//! b7:
//!   %3 = phi i16 [ 0, %8 ], [ 1, %b6 ]
//!   ret i16 %3
//!
//! 4:
//!   %5 = getelementptr inbounds i8, ptr @_a, i16 2
//!   %6 = load i16, ptr %5, !tbaa !9
//!   %7 = icmp slt i16 %6, 0
//!   br i1 %7, label %b6, label %8
//!
//! 8:
//!   br label %b7
//! }
//! ```
//! The loop of two trips is two copies of its test and body: the branch back
//! into the loop is gone.
//!
//! From `--dump` of `hit` (`for (i = 0; i < 2; i++) if (a[i] < 0) return 1;`)
//! at -O3 -m16: stage `dead` before `peel`, and `peeled-r01-dead`, which is
//! `peel`'s copy after `fold`, `decide` and `dead` have cleaned it.
//!
//! # Runs
//! In [`crate::pipeline`]'s per-body fixed point `Fixed`, after the scalar
//! passes settle: while a peel leaves fewer loops, the scalar rounds run
//! again and the pass is asked once more, so an outer loop whose inner loop
//! was peeled may be peeled in turn. On at -O3, -Omax and -Os (`peel`,
//! `-fpeel-loops`); off at -O1, -O2, -Og and -Oz. Budgets per level:
//! `docs/reference/levels.md`.
//!
//! # Needs
//! `Registers` facts, `induction`, `peelsize`, `lcssa`, `loopclone`.
//!
//! # Keeps
//! Nothing when it peels.
//!
//! # See also
//! LLVM's `peelLoop` (`LoopPeel.cpp`) priced by `LoopUnrollPass`'s full-unroll
//! cost model; gcc's `try_unroll_loop_completely`, `-fpeel-loops`.
//! [`crate::unroll`] copies a loop in straight line instead.

use std::collections::BTreeSet;

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::manager::Registers;
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::peelsize::{self, Limits};
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::{cfg, induction, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::Function;
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use num_traits::ToPrimitive;

use crate::profit;
use crate::{edges, lcssa, loopclone};

/// Instructions past which a loop bounded only by its object is not copied out.
const MAX_BOUNDED_FUNCTION_OPS: usize = 150;

#[derive(Default)]
pub struct Peel {
    pub limits: Limits,
}

impl FunctionPass for Peel {
    fn name(&self) -> &'static str {
        "peel"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        match optimized(unit, analyses, &self.limits) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("peel: {error}"),
        }
    }
}

/// Every exact loop `peelsize::admitted` prices as worth it peeled, once
/// each; whether any was.
pub fn optimized(
    unit: &mut passes::Unit,
    analyses: &Analyses,
    limits: &Limits,
) -> Result<bool, String> {
    let costs = &profit::costs(analyses.outer());
    let limits = &limits.on(costs.unroll_budget);
    if !profit::priced(unit.context, unit.layout, unit.function, analyses.outer().callees(), costs) {
        return Ok(false);
    }
    let mut peeled = BTreeSet::<i64>::new();
    while let Some((candidate, latch)) =
        _candidate(unit.context, unit.layout, unit.function, analyses, limits, &peeled)?
    {
        llrm_support::debug!("peel", "peeled the loop with latch b{latch}");
        peeled.insert(latch);
        *unit.function = candidate;
    }
    Ok(!peeled.is_empty())
}

/// `function` closed and the first bounded exact loop not in `skip` peeled,
/// with that loop's latch.
fn _candidate(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    analyses: &Analyses,
    limits: &Limits,
    skip: &BTreeSet<i64>,
) -> Result<Option<(Function, i64)>, String> {
    let mut closed = function.clone();
    lcssa::closed(&mut closed)?;
    let facts = analyses.fresh().get::<Registers>(context, layout, &closed);
    let shape = cfg::Shape::of(&closed);
    let frequencies = profit::Frequencies::default();
    let mut scevs = (!shape.loops.is_empty())
        .then(|| ScevSession::within(context, &analyses.outer().globals, &analyses.outer().metadata, &closed));
    for loop_ in shape.loops.clone() {
        let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else {
            continue;
        };
        if skip.contains(&latch) {
            continue;
        }
        let unit =
            memory::Unit::within(context, layout, &closed, analyses.outer()).with_registers(&facts).with_shape(&shape);
        let exact = scevs
            .as_mut()
            .and_then(
                |session| {
                    profit::scev_trips(session, &unit, &loop_)
                        .map(BigInt::from)
                        .or_else(
                            || {
                                // A bound ScalarEvolution gives from the type's
                                // range (`n` unknown: 2^31 trips) says nothing;
                                // the object's extent, where tighter, is the
                                // bound, as GCC takes the tightest.
                                let bound = BigInt::from(profit::scev_trip_bound(session, &unit, &loop_)?);
                                induction::object_backedges(&unit, &loop_)
                                    .filter(|extent| *extent < bound)
                                    .map_or(Some(bound), |_| None)
                            },
                        )
                },
            );
        // No count: the most backedges the object an access walks allows.
        let bounded = exact.is_none();
        // Nothing folds in the copies of a loop bounded only by an object, and
        // the analyses that compare memory accesses in pairs grow faster than
        // the code does: qcport/mod_tex at -O3 took 2.3 times the instructions
        // for 1.6 times the code. A large function is left rolled.
        if bounded && closed.walk().count() > MAX_BOUNDED_FUNCTION_OPS {
            continue;
        }
        let Some(count) = exact.or_else(|| induction::object_backedges(&unit, &loop_)) else {
            continue;
        };
        if count < BigInt::from(2)
            || !peelsize::admitted(
                &unit,
                &loop_,
                &count,
                &facts,
                limits,
                profit::site(&unit, analyses.outer(), &loop_, &frequencies),
            )
        {
            continue;
        }
        let Some(count) = count.to_i64() else {
            continue;
        };
        if loopclone::peeled(&mut closed, &loop_, count)?.is_some() {
            if bounded {
                _without_back_edge(&mut closed, &loop_, latch)?;
            }
            return Ok(Some((closed, latch)));
        }
    }
    Ok(None)
}

/// `loop_`'s latch ends in `unreachable`: after `bound` backedges were peeled
/// out ahead of it, the next one is the one the bound rules out, and nothing
/// would otherwise tell `Dead` so (GCC's `remove_exits_and_undefined_stmts`).
fn _without_back_edge(
    function: &mut Function,
    loop_: &Loop,
    latch: i64,
) -> Result<(), String> {
    let (latch, header) = (cfg::block(latch), cfg::block(loop_.header));
    for phi in edges::phis(function, header) {
        let kept = lcssa::arms(function, phi).into_iter().filter(|&(_, source)| source != latch).collect::<Vec<_>>();
        function.set_operands(phi, lcssa::from_arms(&kept));
    }
    let jump = function.terminator(latch).ok_or("a latch without a terminator")?;
    let void = function.instruction(jump).ty;
    function.erase(jump)?;
    let stop = function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
    function.insert(stop, Position::End(latch))
}

#[cfg(test)]
#[path = "peel_tests.rs"]
mod tests;
