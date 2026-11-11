//! `SelectOptimize` as a pipeline slot, for a target that asks for it.
//!
//! # How it works
//! The pin's `llc` leaves the pass off (`-disable-select-optimize`, true by
//! default); DEVIATE SELECT-OPT-I486 runs it where the target states a
//! misprediction penalty (`Tti::select_optimize_penalty`: the i486, which has no
//! conditional move). The slot is the pin's, after `ExpandReductions` and before
//! CodeGenPrepare. The pass is [`crate::selectoptimize`].

use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::selectoptimize::{Env, Target, select_optimize};

pub struct SelectOptimizePass;

impl FunctionPass for SelectOptimizePass {
    fn name(&self) -> &'static str {
        "selectoptimize"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectOptimize.cpp#L357")
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer();
        let Some(tti) = outer.target().tti() else { return PreservedAnalyses::all() };
        let Some(penalty) = tti.select_optimize_penalty() else { return PreservedAnalyses::all() };
        let target = Target { misprediction_penalty: penalty, ..Target::base(&*tti) };
        let env = Env {
            context: &*unit.context,
            globals: &outer.globals,
            metadata: unit.metadata,
            layout: unit.layout,
            callees: outer.callees(),
        };
        if select_optimize(&env, &target, unit.declared, unit.function) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}
