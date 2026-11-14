//! The pass: InstCombine as a function pass of the pipeline.
//!
//! [`combine::combine`](crate::combine::combine) over the function, with the
//! module's globals and metadata from the pass manager. It runs only where
//! `Options::combine` is set (`LLRM_NO_COMBINE` clears it).
//!
//! DEVIATE: none for `LastRunTrackingAnalysis`
//! ([`LastRunTracking`]): `InstCombinePass::run` starts with
//! `LRT.shouldSkip(&ID)` and ends with `LRT.update(&ID, Changed)`; here the
//! pass does the same on a per-function analysis that it preserves and every
//! changing pass drops.
//!
//! # Example
//! With `Options::combine` set, `%y = add i32 %x, 0` is gone and its uses read
//! `%x`; with it clear, the function is returned unchanged.
//!
//! # Used by
//! `pipeline` (`combine@N`, in front of the old `fold@N` of each InstCombine
//! slot of LLVM's order).

use std::cell::Cell;

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Function;
use llrm_mir::passes::{Analyses, Analysis, FunctionPass, PreservedAnalyses, Unit};

use crate::combine;

/// [`LastRunTrackingAnalysis`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/LastRunTrackingAnalysis.cpp#L1):
/// whether InstCombine has run over the function as it stands. A pass that
/// changes the function and does not preserve it drops it.
pub struct LastRunTracking;

#[derive(Debug, Default)]
pub struct LastRun {
    instcombine: Cell<bool>,
}

/// What it holds is bookkeeping of the pass manager, not a fact about the
/// function: two results are never told apart.
impl PartialEq for LastRun {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl Analysis for LastRunTracking {
    type Result = LastRun;
    const NAME: &'static str = "last-run-tracking";
    const READS_OUTER: bool = false;

    fn run(
        _: &Context,
        _: &DataLayout,
        _: &Function,
        _: &mut Analyses,
    ) -> LastRun {
        LastRun::default()
    }
}

pub struct Combine {
    /// `Options::combine`: off, the pass changes nothing.
    pub enabled: bool,
    /// DEVIATE PHICONST: where bytes are priced (-O2, -O3) and the target has
    /// a rate (`Tti::speed_milliclocks_per_byte`).
    pub phi_constant_users: bool,
}

impl FunctionPass for Combine {
    fn name(&self) -> &'static str {
        "combine"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if !self.enabled {
            return PreservedAnalyses::all();
        }
        // No changes since the last InstCombine pass: exit early.
        let last = analyses.get::<LastRunTracking>(unit.context, unit.layout, unit.function);
        if last.instcombine.get() {
            llrm_support::debug::counted("combine skipped", true);
            return PreservedAnalyses::all();
        }
        llrm_support::debug::counted("combine skipped", false);
        let outer = std::rc::Rc::clone(analyses.outer());
        let changed = combine::combine_on_target(
            unit.context,
            &outer.globals,
            outer.callees(),
            &outer.metadata,
            unit.layout,
            unit.function,
            unit.declared,
            combine::TargetHooks {
                fixed_shift_count: outer.target().tti().is_some_and(|tti| tti.variable_shift_count_in_fixed_register()),
                select_to_cmp: outer.target().tti().is_some_and(|tti| tti.folds_select_to_cmp()),
                icmp_of_constant_phi: outer.target().tti().is_some_and(|tti| tti.folds_icmp_of_constant_phi()),
                keeps_swapped_select_compare: outer.target().tti().is_some_and(|tti| tti.keeps_icmp_of_swapped_selects()),
                keeps_shared_division_compare: outer
                    .target()
                    .tti()
                    .is_some_and(|tti| tti.keeps_icmp_of_shared_division()),
                // Where the target trades bytes for clocks (wasm and the
                // targets without a rate are strict at every level).
                phi_constant_users: self.phi_constant_users
                    && outer.target().tti().is_some_and(|tti| tti.speed_milliclocks_per_byte() != 0),
                phi_constant_chains: self.phi_constant_users
                    && outer.target().tti().is_some_and(|tti| tti.folds_phi_constant_chains()),
            },
        );
        last.instcombine.set(true);
        if changed {
            PreservedAnalyses::none().preserve_cfg().preserve::<LastRunTracking>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "combine_pass_tests.rs"]
mod tests;
