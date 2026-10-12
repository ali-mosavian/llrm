//! Whole-loop peeling: clone every iteration of a loop with a known trip
//! count ahead of it, priced before anything is cloned, and let Fold, Decide
//! and Dead prove the residual loop unreachable.
//!
//! # How it works
//! Peeling is the CFG counterpart of full straight-line unrolling. For each
//! loop of a copy of the body, closed to LCSSA (`lcssa::closed`), the pass
//! - takes the trip count (`induction::trip_count`, else `trip_bound`) and
//!   skips the loop below two trips;
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

use llrm_analysis::manager::Registers;
use llrm_analysis::peelsize::{self, Limits};
use llrm_analysis::{cfg, induction, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Function;
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::profit;
use crate::{lcssa, loopclone};

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
    for loop_ in shape.loops.clone() {
        let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else {
            continue;
        };
        if skip.contains(&latch) {
            continue;
        }
        let unit =
            memory::Unit::within(context, layout, &closed, analyses.outer()).with_registers(&facts).with_shape(&shape);
        let Some(count) =
            induction::trip_count(&unit, &loop_, &facts).or_else(|| induction::trip_bound(&unit, &loop_, &facts))
        else {
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
            return Ok(Some((closed, latch)));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "peel_tests.rs"]
mod tests;
