//! The pass: InstCombine as a function pass of the pipeline.
//!
//! [`combine::combine`](crate::combine::combine) over the function, with the
//! module's globals and metadata from the pass manager. It runs only where
//! `Options::combine` is set (`LLRM_NO_COMBINE` clears it).
//!
//! # Example
//! With `Options::combine` set, `%y = add i32 %x, 0` is gone and its uses read
//! `%x`; with it clear, the function is returned unchanged.
//!
//! # Used by
//! `pipeline` (`combine@N`, in front of the old `fold@N` of each InstCombine
//! slot of LLVM's order).

use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::combine;

pub struct Combine {
    /// `Options::combine`: off, the pass changes nothing.
    pub enabled: bool,
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
        let changed = combine::combine(unit.context, &outer.globals, &outer.metadata, unit.layout, unit.function);
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

#[cfg(test)]
#[path = "combine_pass_tests.rs"]
mod tests;
