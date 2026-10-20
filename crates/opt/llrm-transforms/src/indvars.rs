//! Settles what a loop's counters are worth: a counter's value after its loop
//! becomes a number, and a narrow counter read as a wider index becomes a
//! counter of that width. LLVM's IndVarSimplify (`rewriteLoopExitValues`,
//! `widenIV`). Which counters a
//! loop keeps, and which ends it, is `lsr`'s.
//!
//! # How it works
//! One run is these steps.
//!
//! 0. `exitvalues::rewritten`: a value read after a loop becomes an expression
//!    of the loop's count where the loop is left (`rewriteLoopExitValues`,
//!    ScalarEvolution's exit values, from every exit of the loop, a rotated
//!    loop's latch included); `LLRM_NO_SCEV_EXITVALUES` turns it off.
//! 1. `simplifyindvar::simplified_and_extended` (`simplifyAndExtend`): the IV
//!    users simplified and a counter read through `zext` or `sext` at a wider
//!    index becomes a counter of that width (`WidenIV`). C's `unsigned short i`
//!    reads `a[zext i]`: a conversion per trip, which hides the loop from
//!    idioms and strength reduction.
//! 2. `congruentiv::replaced` (`SCEVExpander::replaceCongruentIVs`;
//!    `LLRM_NO_SCEV_CONGRUENT`): header phis SCEV proves equal, a narrow one a
//!    truncation of a wide one included, are one, and `dead` removes the rest.
//! 3. `canonexit::canonicalized` (`canonicalizeExitCondition`;
//!    `LLRM_NO_SCEV_CANONEXIT`), `exitopt::optimized` (`optimizeLoopExits`;
//!    `LLRM_NO_SCEV_EXITOPT`), `exitpredicate::predicated`
//!    (`predicateLoopExits`; `LLRM_NO_SCEV_PREDICATE`), and the exits their
//!    counts decide are folded by those.
//! 4. `lftr::replaced` (`linearFunctionTestReplace`; `LLRM_NO_SCEV_LFTR`): an
//!    exit tests a counter against the value it has at the last trip.
//! 5. `exitsink::sunk`: final updates only exit phis read move to the exits.
//!
//! # Example
//! Step 1, the widening, on `for (unsigned short i = 0; i < n; ++i) a[i] = 1`
//! with 32-bit addresses. Before:
//!
//! ```mir
//! target datalayout = "e-p:32:32-n8:16:32"
//!
//! define i32 @f(i16 %n) {
//! b0:
//!   %buf = alloca [64 x i8]
//!   br label %b1
//!
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
//!   %go = icmp ult i16 %i, %n
//!   br i1 %go, label %b2, label %b3
//!
//! b2:
//!   %w = zext i16 %i to i32
//!   %p = getelementptr inbounds i8, ptr %buf, i32 %w
//!   store i8 1, ptr %p
//!   %next = add i16 %i, 1
//!   br label %b1
//!
//! b3:
//!   ret i32 0
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! b0:
//!   %buf = alloca [64 x i8]
//!   %0 = zext i16 %n to i32
//!   br label %b1
//!
//! b1:
//!   %widen.iv = phi i32 [ 0, %b0 ], [ %widen.iv.next, %b2 ]
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
//!   %1 = icmp ult i32 %widen.iv, %0
//!   br i1 %1, label %b2, label %b3
//!
//! b2:
//!   %p = getelementptr inbounds i8, ptr %buf, i32 %widen.iv
//!   store i8 1, ptr %p
//!   %widen.iv.next = add i32 %widen.iv, 1
//!   %next = add i16 %i, 1
//!   br label %b1
//! ; ... b3 unchanged
//! ```
//!
//! The loop now ends against `%n` extended once in the preheader, and the
//! address reads the wide counter: the `zext` a trip is gone.
//!
//! From unit test `a_counter_extended_to_the_index_width_is_that_wide` in
//! `indvars_tests.rs`.
//!
//! # Runs
//! In the body pipeline after `promote`. With `-ftree-ch` (not at -Os) it runs
//! again after the loop-header copy, where the copy made a counted exit's test
//! the one that leaves (a number is a `mov` where the counter's register
//! already holds it).
//!
//! # Needs
//! `llrm_analysis::scev`, [`crate::exitvalues`], [`crate::exitsink`].
//!
//! # Keeps
//! Nothing after steps 1 to 3; dominators and loops after the others.
//!
//! # See also
//! LLVM `IndVarSimplify.cpp`. Tests: `indvars_tests.rs`.

use llrm_analysis::cfg;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};

use crate::dead;

/// What IndVarSimplify leaves a loop's counters: values after a loop
/// computed from its trips (`exitvalues`), final updates sunk to
/// the exits (`exitsink`). Which counters the loop keeps is `lsr`'s.
pub struct IndVars {
    /// Whether LFTR runs: on in the pipeline, `LLRM_NO_LFTR` turns it off.
    pub lftr: bool,
}

/// `LLRM_NO_LFTR`, read once: LFTR runs in the pipeline, as the pin's
/// IndVarSimplify does. The exit test becomes `eq`/`ne` for the translated LSR
/// to take against zero (it was opt-in while the old `lsr` could not reduce
/// the counter LFTR leaves, D22).
pub fn lftr_default() -> bool {
    !llrm_support::env_flag!("LLRM_NO_LFTR")
}

impl FunctionPass for IndVars {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "indvars"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("indvars")
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        // One session for the run, as LLVM holds one ScalarEvolution for the
        // pass: a loop's trip count is worked out once, not once for each step.
        // A step that changed the body drops the expressions; the blocks and
        // edges stay, so the tree and the loops do not.
        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        let mut settled = |session: &mut ScevSession, changed: bool| {
            if changed {
                session.forget_expressions();
            }
            changed
        };
        let compared = crate::simplifyindvar::simplified_and_extended_in(
            &mut session,
            unit.context,
            unit.layout,
            unit.function,
            unit.declared,
            &outer,
        );
        let compared = settled(&mut session, compared);
        let rewrote = crate::exitvalues::enabled() && {
            let changed = crate::exitvalues::rewritten_in(
                &mut session,
                unit.context,
                unit.layout,
                unit.function,
                unit.declared,
                &outer,
            );
            settled(&mut session, changed)
        };
        let merged = crate::congruentiv::enabled() && {
            let changed =
                crate::congruentiv::replaced_in(&mut session, unit.context, unit.layout, unit.function, &outer);
            settled(&mut session, changed)
        };
        let canonical = crate::canonexit::enabled() && {
            let changed =
                crate::canonexit::canonicalized_in(&mut session, unit.context, unit.layout, unit.function, &outer);
            settled(&mut session, changed)
        };
        let optimized = crate::exitopt::enabled() && {
            let changed = crate::exitopt::optimized_in(
                &mut session,
                unit.context,
                unit.layout,
                unit.function,
                unit.declared,
                &outer,
            );
            settled(&mut session, changed)
        };
        let predicated = crate::exitpredicate::enabled() && {
            let changed = crate::exitpredicate::predicated_in(
                &mut session,
                unit.context,
                unit.layout,
                unit.function,
                unit.declared,
                &outer,
            );
            settled(&mut session, changed)
        };
        let tested = self.lftr
            && crate::lftr::enabled()
            && crate::lftr::replaced_in(&mut session, unit.context, unit.layout, unit.function, unit.declared, &outer);
        let found = analyses.get::<passes::Loops>(unit.context, unit.layout, unit.function);
        let loops = cfg::natural(unit.function, &found);
        let sunk = crate::exitsink::sunk(unit.function, &loops);
        let dead = (compared || rewrote || canonical || optimized || predicated || tested || sunk || merged)
            && dead::dead(unit.context, outer.callees(), unit.function);
        if compared || rewrote || canonical || optimized || predicated || tested || sunk || merged || dead {
            // Blocks and edges are as they were.
            PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "indvars_tests.rs"]
pub(crate) mod tests;
