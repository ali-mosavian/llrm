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
//! `Walk` is a program pass placed after `Phase::Cleanup`, in place of the
//! module-wide Simplify stage. Callees' components first, it splices the
//! calls `inlinecost::sites` accepts into each function, then runs the
//! Simplify list over it, as LLVM's CGSCC walk does: a loop around a call is
//! unrolled after the call is inlined (keybind `_action_for`, 8 inlined copies
//! of `_text_eq`, 325 instructions against 55).
//!
//! # Deviations
//! - W1: `deadargelim` runs before the Simplify list, LLVM's after the CGSCC
//!   walk (`PassBuilderPipelines.cpp` l.1335). Fact: nothing here cleans a body
//!   after the walk (LLVM's later module optimization pipeline does), and the
//!   argument a dropped parameter leaves behind is dead code only the list
//!   removes. Issue: it sees bodies simplified by Early only. Exit: a post-walk
//!   cleanup stage, then `deadargelim` after the walk.
//! - W2a: the walk is per function in callee-first order, not per SCC with the
//!   inliner's own updates; the clocks inliner still runs afterwards;
//!   `inlinecost::sites` stands for `decide` until it lands. Exit: W2.
//! - W2: `ipa-cp` clones with the clocks' cost model (`ipacp`), not
//!   `FunctionSpecialization`; exit W5.
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
//! From unit test
//! `test_a_body_changed_by_an_interprocedural_stage_does_not_rerun_the_list` in
//! `walk_tests.rs`.
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
use llrm_analysis::manager::GlobalsAA;
use llrm_mir::context::GlobalId;
use llrm_mir::module::Module;
use llrm_mir::passes::ModuleAnalyses;
use llrm_mir::passes::PreservedAnalyses;
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};

use crate::interprocedural::{
    declared, function_mut, managers, private, procedures, reporting_marked, unexported, vouched,
};

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

/// LLVM's CGSCC walk: callees' components first, each function's calls decided
/// and spliced (`inlinecost::sites`), then the Simplify list over it, so that a
/// loop is unrolled (`unroll@753`) with the calls in it already inlined.
pub struct Walk {
    pub inline: crate::inline::Threshold,
    /// The Simplify list over one body, as the interprocedural step reruns it.
    pub simplify: Box<dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str)>,
}

impl ProgramPass for Walk {
    fn name(&self) -> &'static str {
        "walk"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let mut modules = managers(program, analyses);
        let params = crate::inlinecost::InlineParams::from_level(if self.inline.limit >= 250 { 3 } else { 2 });
        let size = if self.inline.single {
            crate::inlinecost::CallerSize::OptSize
        } else {
            crate::inlinecost::CallerSize::Speed
        };
        for at in 0..program.modules.len() {
            let wanted: BTreeSet<GlobalId> = procedures(program, at).into_iter().collect();
            let module = &mut program.modules[at];
            let recursive = crate::inline::recursive(module);
            let addressed = llrm_mir::callgraph::addressed(module);
            let counts = crate::inline::call_counts(module);
            let components: Vec<Vec<GlobalId>> = llrm_mir::callgraph::CallGraph::new(module)
                .bottom_up_components()
                .into_iter()
                .map(|(members, _)| members.into_iter().filter(|id| wanted.contains(id)).collect())
                .collect();
            let remarks = llrm_support::env_flag!("LLRM_INLINE_REMARKS");
            let attributes = llrm_analysis::attrsummary::enabled();
            for members in components {
                for &id in &members {
                    let mut decided = remarks.then(Vec::new);
                    // `sites` prices every block's frequency before it looks at
                    // a call.
                    let calls_a_body = module
                        .global(id)
                        .function()
                        .is_some_and(
                            |caller| caller.walk().any(|(_, inst)| {
                                llrm_mir::memory::callee(&module.context, caller, inst).is_some_and(|callee| {
                                    module.global(callee).function().is_some_and(|one| !one.layout().is_empty())
                                })
                            }),
                        );
                    let constant = if !calls_a_body {
                        Default::default()
                    } else {
                        crate::inlinecost::sites(
                            module,
                            &program.layout,
                            id,
                            &counts,
                            &addressed,
                            &recursive,
                            params,
                            size,
                            self.inline.limit == 0,
                            program.target.tti().as_deref(),
                            decided.as_mut(),
                        )
                    };
                    let mut declared = declared(&mut modules[at], module);
                    let (context, function) = function_mut(module, id);
                    let by = crate::inline::Caller { layout: &program.layout, recursive: recursive.contains(&id) };
                    let spliced = crate::inline::expanded_all(
                        context,
                        function,
                        &by,
                        &Default::default(),
                        Some(&constant),
                        &mut declared,
                    )? > 0;
                    for (call, callee, cost) in decided.iter().flatten() {
                        let name = |id: GlobalId| module.global(id).name.as_deref().unwrap_or("?");
                        let gone = module.global(id).function().is_some_and(|one| one.parent(*call).is_none());
                        eprintln!("{}", crate::inlinecost::remark(name(id), name(*callee), cost, gone));
                    }
                    if declared.place(module)? > 0 {
                        modules[at].invalidate(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                    }
                    if spliced {
                        modules[at].changed(id);
                        modules[at].invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                    }
                    (self.simplify)(module, &mut modules[at], id, "walk.");
                }
                // `PostOrderFunctionAttrsPass`: once per component, after its
                // bodies are simplified. Only the attribute-fed `Summaries`
                // reads them (`attrsummary::enabled`).
                if attributes && !members.is_empty() {
                    for id in llrm_analysis::functionattrs::post_order(
                        module,
                        &program.layout,
                        program.target.spaces(),
                        &members,
                    ) {
                        modules[at].changed(id);
                    }
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

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;
