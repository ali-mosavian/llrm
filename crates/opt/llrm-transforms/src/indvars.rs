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
//! 5. `exitsink::sunk`: final updates only exit phis read move to the exits;
//!    `sinkinvariants::sunk` (`sinkUnusedInvariants`): preheader values the
//!    loop never reads move to its exit block; `firstiterexit::rewritten`
//!    (`rewriteFirstIterationLoopExitValues`): an exit on a loop-invariant
//!    condition reads a header phi's preheader value.
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
use llrm_analysis::scev::analysis::{ScevSession, Scevs};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};

use crate::dead;

/// What IndVarSimplify leaves a loop's counters: values after a loop
/// computed from its trips (`exitvalues`), final updates sunk to
/// the exits (`exitsink`). Which counters the loop keeps is `lsr`'s.
pub struct IndVars {
    /// Whether LFTR runs: on in the pipeline; `false` is `indvars<no-lftr>`.
    pub lftr: bool,
    /// Whether code bytes are what counts (`-Os`, `-Oz`).
    pub size: bool,
}

/// LFTR runs in the pipeline, as in the pin (`indvars<no-lftr>` is a pass
/// parameter no pipeline sets). The exit test becomes `eq`/`ne` for the
/// translated LSR to take against zero.
pub const fn lftr_default() -> bool {
    true
}

/// Whether the edits since `mark` made a phi: a counter was widened.
fn made_a_phi(
    function: &llrm_mir::module::Function,
    mark: llrm_mir::module::Mark,
) -> bool {
    use llrm_mir::module::ValueDef;
    function
        .touched_since(mark)
        .is_some_and(
            |touched| touched.values_added.iter().any(|value| match function.value(value).def {
                ValueDef::Instruction(inst) => function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Phi,
                _ => false,
            }),
        )
}

/// After a step: what its edits touched is dropped from the expressions.
fn settled(
    session: &mut ScevSession,
    mark: &mut llrm_mir::module::Mark,
    unit: &passes::Unit,
    outer: &passes::Outer,
    changed: bool,
    keep_counts: bool,
) -> bool {
    if changed {
        session.forget_since_keeping(
            unit.context,
            unit.layout,
            &outer.globals,
            &outer.metadata,
            unit.function,
            *mark,
            keep_counts,
        );
        *mark = unit.function.mark();
        // The pin's `SE.verify()` after a pass that kept it: what is kept is
        // what a new one would work out.
        if llrm_support::debug::verifying()
            && let Err(why) = session.verify(unit.context, unit.layout, &outer.globals, &outer.metadata, unit.function)
        {
            panic!("indvars left a stale expression: {why}");
        }
    }
    changed
}

/// `IndVarSimplifyPass` on the loops the last `LoopFullUnroll` made, where the
/// loop pass manager's worklist hands them to the passes after it (LPM2 in
/// `buildFunctionSimplificationPipeline`, `LPMUpdater::addSiblingLoops`).
/// Nothing runs when it made none.
pub struct IndVarsOnCreated {
    pub created: std::rc::Rc<std::cell::RefCell<Vec<llrm_mir::module::BlockId>>>,
}

impl FunctionPass for IndVarsOnCreated {
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
        let headers = std::mem::take(&mut *self.created.borrow_mut());
        if headers.is_empty() {
            return PreservedAnalyses::all();
        }
        let _scope = crate::loopfilter::only(&headers);
        IndVars { lftr: lftr_default(), size: false }.run(unit, analyses)
    }
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
        // A step that changed the body drops the expressions its edits touched;
        // the blocks and edges stay, so the tree and the loops do not.
        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        // The expressions of the pass before, as the pin keeps one SE for the
        // loop passes; handed back below, current.
        let held = analyses.get::<Scevs>(unit.context, unit.layout, unit.function);
        session.adopt(held.take());
        let mut mark = unit.function.mark();
        let start = mark;
        let compared = crate::simplifyindvar::simplified_and_extended_in(
            &mut session,
            unit.context,
            unit.layout,
            unit.function,
            unit.declared,
            &outer,
        );
        // Widening replaces operands in place, as the pin does without telling
        // ScalarEvolution: the loop's exit count stays the one worked out
        // before, in the narrow type, and LFTR takes its limit from that
        // (`genLoopLimit`, which then extends it as `wide.trip.count`). The pin
        // forgets only a deleted or replaced value and, after
        // `optimizeLoopExits` and `predicateLoopExits`, the loop:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1933-L1968
        let widened = compared && made_a_phi(unit.function, mark);
        let compared = settled(&mut session, &mut mark, unit, &outer, compared, widened);
        let rewrote = crate::exitvalues::enabled() && {
            let changed = crate::exitvalues::rewritten_in(
                &mut session,
                unit.context,
                unit.layout,
                unit.function,
                unit.declared,
                &outer,
                self.size,
            );
            settled(&mut session, &mut mark, unit, &outer, changed, false)
        };
        let merged = crate::congruentiv::enabled() && {
            let changed =
                crate::congruentiv::replaced_in(&mut session, unit.context, unit.layout, unit.function, &outer);
            // It only replaces and erases values, which tell ScalarEvolution
            // (the value handles) of those values; no loop's count reads them.
            settled(&mut session, &mut mark, unit, &outer, changed, widened)
        };
        let canonical = crate::canonexit::enabled() && {
            let changed =
                crate::canonexit::canonicalized_in(&mut session, unit.context, unit.layout, unit.function, &outer);
            settled(&mut session, &mut mark, unit, &outer, changed, false)
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
            settled(&mut session, &mut mark, unit, &outer, changed, false)
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
            settled(&mut session, &mut mark, unit, &outer, changed, false)
        };
        let tested = self.lftr
            && crate::lftr::enabled()
            && crate::lftr::replaced_in(&mut session, unit.context, unit.layout, unit.function, unit.declared, &outer);
        let found = analyses.get::<passes::Loops>(unit.context, unit.layout, unit.function);
        let loops = cfg::natural(unit.function, &found);
        let sunk = crate::exitsink::sunk(unit.function, &loops);
        // `sinkUnusedInvariants`, the pin's last step but one.
        let sunk_invariants = crate::sinkinvariants::sunk(unit.context, outer.callees(), unit.function, &loops);
        // `rewriteFirstIterationLoopExitValues`, the last.
        let tree = analyses.get::<passes::Dominators>(unit.context, unit.layout, unit.function);
        let first_trip = crate::firstiterexit::rewritten(unit.function, &loops, &tree);
        // The pin deletes what an edit stopped reading as it goes; a sweep of
        // the function (0.8 M Ir on d_faces' largest body) found the same.
        let dead = (compared
            || rewrote
            || canonical
            || optimized
            || predicated
            || tested
            || sunk
            || sunk_invariants
            || first_trip
            || merged)
            && {
                match unit.function.touched_since(start) {
                    Some(touched) => {
                        let seeds: Vec<_> = touched.named.iter().collect();
                        dead::dead_from(unit.context, outer.callees(), unit.function, seeds)
                    }
                    None => dead::dead(unit.context, outer.callees(), unit.function),
                }
            };
        // The edits after the last step (lftr, the sinking, the dead code).
        // The sinking moves instructions no count reads, LFTR keeps the exit
        // count it was built from and the dead code is values
        // (forgotten as erased); the pin forgets a loop only after
        // `optimizeLoopExits` and `predicateLoopExits`, the steps
        // above.
        session.forget_since_keeping(
            unit.context,
            unit.layout,
            &outer.globals,
            &outer.metadata,
            unit.function,
            mark,
            true,
        );
        held.put(session.into_scev());
        if compared
            || rewrote
            || canonical
            || optimized
            || predicated
            || tested
            || sunk
            || sunk_invariants
            || first_trip
            || merged
            || dead
        {
            // Blocks and edges are as they were.
            PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>().preserve::<Scevs>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "indvars_tests.rs"]
pub(crate) mod tests;
