//! `AlwaysInlinerPass(InsertLifetimeIntrinsics=true)`: every call of an
//! `alwaysinline` callee is replaced by a copy of it, before the inliner
//! prices anything.
//!
//! # How it works
//! `PassBuilderPipelines.cpp` l.1326 runs it as a module pass just ahead of the
//! CGSCC inliner. Each body is gone through once for the direct calls the
//! callee's or the call's `alwaysinline` names (`inline_viable`: no recursion,
//! no `returns_twice`), and the copies' own such calls follow. A callee left
//! with no call and no address is dropped, as the pass deletes it.
//!
//! What the copy leaves is the point: `hot_op(S, hot_i_and)` passes
//! `@hot_i_and` as a value, and only the copy makes the call to it direct. The
//! inliner then prices that call with the callee's other uses already gone
//! (its last call, `isSoleCallToLocalFunction`) and walks the callees first.
//!
//! # Runs
//! Once per compile, before `Walk`.
//!
//! # Used by
//! `pipeline`.
//!
//! # Example
//! A call of an `alwaysinline` function is replaced by a copy of its body, and the
//! function is dropped once nothing calls or addresses it.
//!
//! ```text
//! define internal i32 @f(i32 %x) alwaysinline { %r = add i32 %x, 1 ; ret i32 %r }
//! %v = call i32 @f(i32 %a)      ; becomes  %v = add i32 %a, 1, and @f is deleted
//! ```

use std::rc::Rc;

use llrm_mir::context::GlobalId;
use llrm_mir::module::{InstId, Module};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::PreservedAnalyses;
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};
use llrm_support::hash::{IndexMap, SparseIdMap};

use crate::inline::{Caller, Candidate};
use crate::inlinecost::{has_flag, inline_viable};
use crate::interprocedural::{
    Liveness, calls_of, declared, function_and_metadata_mut, managers, private, procedures, roots, roots_here,
};

/// Rounds of copies of one body: the calls a round brings are the next's.
const ROUNDS: usize = 8;

/// The calls of `id` the pass replaces, each with the copy to splice.
fn sites(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    id: GlobalId,
) -> SparseIdMap<InstId, Candidate> {
    let mut out = SparseIdMap::default();
    let Some(caller) = module.global(id).function() else { return out };
    for (_, call) in caller.walk() {
        let Opcode::Call(info) = &caller.instruction(call).opcode else { continue };
        let Some(callee_id) = llrm_mir::memory::callee(&module.context, caller, call) else { continue };
        let Some(callee) = module.global(callee_id).function() else { continue };
        let named = has_flag(&info.attrs, "alwaysinline") || has_flag(&callee.attrs, "alwaysinline");
        if !named
            || callee_id == id
            || has_flag(&info.attrs, "noinline")
            || callee.layout().is_empty()
            || llrm_mir::facts::opaque(callee)
            || !crate::inline::copyable(module, callee)
            || inline_viable(module, callee, callee_id).is_err()
        {
            continue;
        }
        out.insert(
            call,
            Candidate {
                body: Rc::new(callee.clone()),
                frame: crate::inline::grown(&module.context, layout, callee),
                moved: false,
                last_call: false,
                callee: Some(callee_id),
                self_calls: None,
                self_sites: 0,
                hot: false,
            },
        );
    }
    out
}

pub struct AlwaysInline;

impl ProgramPass for AlwaysInline {
    fn name(&self) -> &'static str {
        "always-inline"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let mut modules = managers(program, analyses);
        let roots = roots(program);
        let mut changed = false;
        for at in 0..program.modules.len() {
            let wanted = procedures(program, at);
            let private = private(program, at);
            let module = &mut program.modules[at];
            if !wanted.iter().any(|&id| !sites(module, &program.layout, id).is_empty()) {
                continue;
            }
            let addressed = llrm_mir::callgraph::addressed_by_uses(module);
            let mut live = Liveness::new(module, private, &addressed, &roots_here(&roots, at));
            for id in wanted {
                for _ in 0..ROUNDS {
                    let sites = sites(module, &program.layout, id);
                    if sites.is_empty() {
                        break;
                    }
                    let mut declared = declared(&mut modules[at], module);
                    let (context, function, metadata) = function_and_metadata_mut(module, id);
                    let was = calls_of(context, function);
                    let known = modules[at].cached::<llrm_analysis::manager::GlobalsModRefAnalysis>();
                    let by = Caller {
                        layout: &program.layout,
                        effects: known.as_deref(),
                        recursive: false,
                        self_inlining: None,
                        peeling: false,
                        depth: 0,
                        callee: i64::MAX,
                        growth: 0,
                    };
                    let spliced = crate::inline::expanded_all(
                        context,
                        function,
                        &by,
                        &IndexMap::default(),
                        Some(&sites),
                        &mut declared,
                        metadata,
                    )? > 0;
                    if declared.place(module)? > 0 {
                        modules[at].invalidate(
                            &PreservedAnalyses::none().preserve_module::<llrm_analysis::manager::GlobalsAA>(),
                        );
                    }
                    if !spliced {
                        break;
                    }
                    changed = true;
                    modules[at].changed(id);
                    modules[at].invalidate_bodies(
                        &PreservedAnalyses::none().preserve_module::<llrm_analysis::manager::GlobalsAA>(),
                    );
                    let now = calls_of(&module.context, module.global(id).function().expect("a procedure"));
                    live.spliced(module, &mut modules[at], was, now);
                }
            }
        }
        // What the managers worked out is the next pass's start, not lost: the
        // summaries and globals' facts are made from nothing once for a program.
        for (at, one) in modules.into_iter().enumerate() {
            analyses.carry(at, one.carried());
        }
        if changed {
            analyses.invalidate();
        }
        Ok(())
    }
}
