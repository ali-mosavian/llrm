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

use std::collections::BTreeSet;

use llrm_analysis::interprocedural as facts;
use llrm_mir::context::GlobalId;
use llrm_mir::passes::PreservedAnalyses;
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};

use crate::interprocedural::{function_mut, managers, private, procedures, reporting_marked, unexported, vouched};

/// What the interprocedural step does to a body before the Simplify list:
/// a parameter every call passes the same constant uses it (`ipa-args`), a
/// function called with a constant is copied for it (`ipa-cp`, the clone at
/// -O3), and a parameter nothing reads is not passed (`ipa-deadargs`).
pub struct ModuleIpo {
    pub inline: crate::inline::Threshold,
    /// The range every caller passes a parameter is stated on it (`ipa-range`,
    /// gcc's `-fipa-vrp`).
    pub ranges: bool,
}

impl ProgramPass for ModuleIpo {
    fn name(&self) -> &'static str {
        "ipo-early"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let count = program.modules.len();
        let mut procedures: Vec<Vec<GlobalId>> = (0..count).map(|at| procedures(program, at)).collect();
        let mut private: Vec<BTreeSet<GlobalId>> = (0..count).map(|at| private(program, at)).collect();
        let unexported = unexported(program);
        let costs = program.target.size_costs();
        let mut modules = managers(program, analyses);
        let mut changed = false;
        let constants = facts::program_parameters(program, &unexported);
        for at in 0..count {
            for &id in &procedures[at] {
                let Some(known) = constants[at].get(&id) else { continue };
                let (context, function) = function_mut(&mut program.modules[at], id);
                if facts::specialize_parameters(context, function, known) {
                    modules[at].changed(id);
                    changed = true;
                }
            }
        }
        let mut cloning = crate::ipacp::Cloning::default();
        for at in 0..count {
            let (made, before) = reporting_marked(&mut program.modules[at], &mut modules[at], |module| {
                crate::ipacp::cloned(
                    module,
                    &program.layout,
                    &procedures[at],
                    &private[at],
                    &costs,
                    self.inline.cp_clone,
                    &mut cloning,
                )
            });
            vouched(&program.modules[at], made.edited.iter().copied().filter(|id| !made.unsound.contains(id)), &before);
            for &id in &made.added {
                procedures[at].push(id);
                private[at].insert(id);
            }
            for &id in made.added.iter().chain(&made.edited) {
                llrm_mir::memory::unsplit(
                    &mut function_mut(&mut program.modules[at], id).1.attrs,
                    llrm_mir::memory::Moved::OutOfArguments,
                );
                modules[at].changed(id);
                changed = true;
            }
        }
        if self.ranges {
            for (at, id) in llrm_analysis::parameter_ranges::stamp(program, &unexported) {
                modules[at].changed(id);
                changed = true;
            }
        }
        for at in 0..count {
            let ((removed, unsound), before) =
                reporting_marked(&mut program.modules[at], &mut modules[at], crate::deadargs::removed_with);
            vouched(&program.modules[at], removed.iter().copied().filter(|id| !unsound.contains(id)), &before);
            changed |= !removed.is_empty();
        }
        for one in &mut modules {
            one.invalidate_bodies(&PreservedAnalyses::none());
        }
        for (at, one) in modules.into_iter().enumerate() {
            analyses.carry(at, one.carried());
        }
        if changed {
            analyses.invalidate();
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;
