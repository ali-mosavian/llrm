//! Exact CFG loop peeling, priced before anything is cloned: llrm-core's
//! `optimize/peel.rs`, the port of `qbopt/optimize/peel.py`, adapted to the
//! rich MIR.
//! LLVM: LoopPeel (`peelLoop`) peeling every trip, priced by LoopUnrollPass's full-unroll cost model.
//!
//! Peeling is the CFG counterpart of full straight-line unrolling: clone
//! every block of a proven exact loop once per trip, keep the residual loop
//! as a correctness fallback, and let Fold, Decide and Dead prove it
//! unreachable.
//!
//! The copy budgets are peelsize's `Limits`, which count instructions. The
//! one machine number is the target's costs: a function with an instruction
//! the target does not price is left alone (`profit::priced`).
//!
//! What changed with the IR: the old stage records and `watch` hook are the
//! pass manager's dump and change log. Dropped:
//! `MAX_CONDITIONAL_FLOAT_OPERATIONS`, a limit on the x87 strict-FP fixed
//! point a floating branch crossed; a float is an ordinary value here.

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
