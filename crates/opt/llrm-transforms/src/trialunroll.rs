//! DEVIATE UNROLL-TRIAL (beyond the pin; owner 6502, exit 2026-12-05): a loop
//! the unroll threshold refused is unrolled on trial.
//!
//! The pin unrolls a loop by a threshold in its own units and by what the
//! loop's body folds (`analyzeLoopUnrollCost`, at most ten trips). An array
//! filled by one loop and read by the next folds only once the first loop is
//! gone, which neither sees: the pin takes that unroll by its large threshold
//! alone. Here every function with a loop the target's threshold refused but
//! the trial size admits (`Tti::unroll_trial_threshold`) is unrolled whole, the
//! simplification list runs over it, and the result is priced as a function:
//! the code bytes and the clocks of every instruction at its block's
//! frequency. It stays only if it pays at the target's rate
//! (`unroll_milliclocks_per_byte`; at size, if it is not larger), else the
//! body is put back. A trial costs the list over the function once more, so it
//! is made for a function of at most `TRIAL_FUNCTION` instructions.

use llrm_analysis::manager::GlobalsAA;
use llrm_mir::module::Module;
use llrm_mir::passes::{ModuleAnalyses, PreservedAnalyses};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};

use crate::interprocedural::{function_mut, managers};
use crate::loopunrollpass::trial;

/// The most instructions a function may have to be tried.
const TRIAL_FUNCTION: usize = 2000;

pub struct TrialUnroll {
    /// The Simplify list over one body.
    pub simplify: Box<dyn FnMut(&mut Module, &mut ModuleAnalyses, llrm_mir::context::GlobalId, &str)>,
    /// Sized for bytes: the trial stays only if the function is not larger.
    pub for_size: bool,
}

/// What a body costs on the target's own tables (`DEVIATE 6502-OPCOSTS`, the
/// ones the per-loop unroll price reads): its code bytes, and its clocks with
/// each block weighted by how often it runs for one entry (`profit::UNIT`
/// `UNIT`ths of an execution, so a cold block still counts).
fn price(
    layout: &llrm_mir::datalayout::DataLayout,
    target: &dyn llrm_mir::target::Machine,
    module: &Module,
    function: &llrm_mir::module::Function,
    trips: &llrm_support::hash::IndexMap<i64, i64>,
) -> Option<(i64, i64)> {
    // The stores to globals nothing loads go with `WriteOnly`, which follows:
    // a fold that leaves a table's every load folded leaves its stores dead.
    let found = crate::globalopt::never_loaded(module);
    let mut cleaned = function.clone();
    crate::globalopt::without_stores_to(&module.context, &mut cleaned, &found, &mut Default::default());
    let function = &cleaned;
    let callees = llrm_mir::memory::callees(module);
    let bytes = crate::profit::r#static(&module.context, layout, function, &callees, &target.byte_costs())?;
    let frequency =
        crate::profit::_frequencies(&module.context, &module.metadata, &module.globals, function, Some(trips))?;
    let clocks = crate::profit::weighted(&module.context, layout, function, &callees, &target.costs(), &frequency)?;
    Some((bytes, clocks))
}

impl ProgramPass for TrialUnroll {
    fn name(&self) -> &'static str {
        "trial-unroll"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let eligible = trial::take();
        if llrm_support::env_flag!("LLRM_DEBUG_TRIAL") {
            eprintln!("trial: eligible {eligible:?}");
        }
        let Some(limit) = program.target.tti().and_then(|tti| tti.unroll_trial_threshold()) else {
            return Ok(());
        };
        if eligible.is_empty() {
            return Ok(());
        }
        let rate = program.target.tti().map_or(0, |tti| tti.unroll_milliclocks_per_byte());
        let remarks = llrm_support::env_flag!("LLRM_DEBUG_TRIAL");
        let mut modules = managers(program, analyses);
        for at in 0..program.modules.len() {
            for name in &eligible {
                let Some(id) = program.modules[at]
                    .functions()
                    .find(|(_, global, _)| global.name.as_deref() == Some(name.as_str()))
                    .map(|(id, _, _)| id)
                else {
                    continue;
                };
                let module = &mut program.modules[at];
                let kept = function_mut(module, id).1.clone();
                if kept.walk().count() > TRIAL_FUNCTION {
                    continue;
                }
                // The body is as the last run left it, so the passes that found
                // nothing then are recorded as settled and skipped; the trial
                // threshold changes what the unroll finds without changing
                // what it reads of the body.
                modules[at].changed(id);
                modules[at].invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                trial::take_trips();
                trial::with(limit, || (self.simplify)(module, &mut modules[at], id, "trial-unroll."));
                // The loops are priced at the trips the run proved of them, in
                // the body before and after alike.
                let trips = trial::take_trips();
                let now = module.global(id).function().expect("a body");
                if *now == kept {
                    if remarks {
                        eprintln!("trial: '{name}' unrolled nothing");
                    }
                    continue;
                }
                let (Some((kept_bytes, kept_clocks)), Some((bytes, clocks))) = (
                    price(&program.layout, &*program.target, module, &kept, &trips),
                    price(&program.layout, &*program.target, module, now, &trips),
                ) else {
                    if remarks {
                        eprintln!("trial: '{name}' is not priced");
                    }
                    *function_mut(module, id).1 = kept;
                    modules[at].changed(id);
                    modules[at].invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                    continue;
                };
                let saved = kept_clocks - clocks;
                let added = bytes - kept_bytes;
                let pays = if self.for_size || rate <= 0 {
                    added <= 0 && saved >= 0
                } else {
                    saved * 1000 >= added * rate * crate::profit::UNIT
                };
                if remarks {
                    eprintln!(
                        "trial unroll of '{name}': {added} bytes, {} clocks saved: {}",
                        saved / crate::profit::UNIT,
                        if pays { "kept" } else { "put back" }
                    );
                }
                if !pays {
                    *function_mut(module, id).1 = kept;
                    modules[at].changed(id);
                    modules[at].invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                }
            }
        }
        for (at, one) in modules.into_iter().enumerate() {
            analyses.carry(at, one.carried());
        }
        analyses.invalidate();
        Ok(())
    }
}
