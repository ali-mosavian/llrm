//! The pass: InstCombine as a function pass of the pipeline.
//!
//! [`combine::combine`](crate::combine::combine) over the function, with the
//! module's globals and metadata from the pass manager. `LLRM_NO_COMBINE`
//! turns it off.
//!
//! # Used by
//! `pipeline` (`combine@N`, in front of the old `fold@N` of each InstCombine
//! slot of LLVM's order).

use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::combine;

pub struct Combine {
    enabled: bool,
}

impl Combine {
    /// On unless `LLRM_NO_COMBINE` is set.
    pub fn from_env() -> Self {
        Self { enabled: !llrm_support::env_flag!("LLRM_NO_COMBINE") }
    }
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
        let outer = std::rc::Rc::clone(analyses.outer());
        let changed =
            combine::combine(unit.context, &outer.globals, &outer.metadata, unit.layout, unit.function);
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}
