//! The module passes that LLVM runs between the early function pipeline and
//! the function simplification (`ipsccp`, `deadargelim`), so that a body they
//! change goes through the Simplify list once and not again afterwards.
//!
//! # How it works
//! `ModuleIpo` is a program pass placed after `Phase::Early`. It applies the
//! edits of the interprocedural step that used to run after the whole list
//! and send each body through the Simplify list again (`ipa-deadargs.`). The
//! bodies it changes are reported to their module's analyses; no pipeline
//! runs here.
//!
//! # Example
//! `@f` is called with an argument its second parameter never reads; it is
//! dropped before the Simplify list, which then runs once over `@f` and
//! `@top`:
//!
//! ```mir
//! define internal i16 @f(i16 %a, i16 %unused) noinline {
//! b0:
//!   %r = add i16 %a, 1
//!   ret i16 %r
//! }
//!
//! define i16 @top(i16 %x) {
//! b0:
//!   %r = call i16 @f(i16 %x, i16 12)
//!   ret i16 %r
//! }
//! ```
//!
//! From unit test `test_a_body_changed_by_an_interprocedural_stage_does_not_rerun_the_list`
//! in `walk_tests.rs`.
//!
//! # Runs
//! Once per compile under `Options::llvm_order`, after the Early stage.
//!
//! # See also
//! LLVM `PassBuilderPipelines.cpp` (`buildModuleSimplificationPipeline`:
//! `IPSCCPPass`, `GlobalOptPass`, `DeadArgumentEliminationPass` before the
//! CGSCC walk), `DeadArgumentElimination.cpp`.

use llrm_mir::passes::PreservedAnalyses;
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};

use crate::interprocedural::{managers, reporting_marked, vouched};

/// Parameters nothing reads are not passed, before the Simplify list.
pub struct ModuleIpo;

impl ProgramPass for ModuleIpo {
    fn name(&self) -> &'static str {
        "ipo-early"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        // A program pass drops what the function stages hold (GlobalsAA): it
        // runs only where a parameter goes.
        if !program.modules.iter().any(crate::deadargs::any_dead) {
            return Ok(());
        }
        let mut modules = managers(program, analyses);
        let mut changed = false;
        for at in 0..program.modules.len() {
            let ((removed, unsound), before) =
                reporting_marked(&mut program.modules[at], &mut modules[at], crate::deadargs::removed_with);
            vouched(&program.modules[at], removed.iter().copied().filter(|id| !unsound.contains(id)), &before);
            if !before.is_empty() {
                changed = true;
                modules[at].invalidate_bodies(&PreservedAnalyses::none());
            }
        }
        for (at, one) in modules.into_iter().enumerate() {
            analyses.carry(at, one.carried());
        }
        // The held GlobalsAA stays where nothing changed.
        if changed {
            analyses.invalidate();
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;
