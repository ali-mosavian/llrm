//! `LoopTermFold`: the loop's last induction variable use, its exit compare, is
//! given to another induction variable (`LoopTermFold.cpp` at `d1106deb71cc`).
//!
//! # How it works
//! For an innermost loop in simplify form with a known trip count whose latch
//! branches on `icmp (%i.next = op %i, step), invariant`, where the counter
//! `%i` is read only by its increment and that compare: another header phi
//! that is an affine add recurrence (no self wrap, non-zero step) gets the
//! test. Its value after the last trip is expanded once in the preheader, and
//! the latch tests `%j.next == that`, leaving `%i` and its increment dead.
//! The expansion must cost at most `min(2 * 4, the small constant maximum trip
//! count, the estimated trip count)` operations.
//!
//! ```text
//!  loop:  %i = phi [0, pre], [%i.next, loop]        loop:  %p = phi [%a, pre], [%p.next, loop]
//!         %p = phi [%a, pre], [%p.next, loop]  ==>         %p.next = getelementptr i8, ptr %p, 4
//!         %p.next = getelementptr i8, ptr %p, 4            %done = icmp eq ptr %p.next, %end
//!         %i.next = add nuw i32 %i, 1                      br i1 %done, label %exit, label %loop
//!         %c = icmp ne i32 %i.next, %n
//!         br i1 %c, label %loop, label %exit      pre:   %end = (value of %p after the last trip)
//! ```
//!
//! # Runs
//! In the strength slot right after `lsr`, for a target whose TTI enables it
//! ([`llrm_mir::tti::Tti::enables_loop_term_fold`]); the pin enables it for
//! RISCV, AVR and Mips.
//!
//! DEVIATE LTF1: `getLoopEstimatedTripCount` reads the latch branch's weights
//! only; the pin's `llvm.loop.estimated_trip_count` attribute is not read, no
//! pass here writes it. Owner m68k. Exit: when a pass writes the attribute.
//!
//! # See also
//! `canFoldTermCondOfLoop`, `RunTermFold`, `isAlmostDeadIV`.

use llrm_analysis::occurrence::arms;
use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::analysis::{ScevSession, Scevs};
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expand_cost::{ExpansionTti, MachineCosts};
use llrm_analysis::scev::expander::{Env, Expander};
use llrm_analysis::scev::expr::{Kind, Scev, Ty};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Attribute, Flags, IntPredicate, Opcode};
use llrm_mir::types::Type as MirType;
use llrm_mir::passes::{Analyses, Declared, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::valuetracking::guarantee::{Operator, has_poison_generating_flags, must_execute_ub_if_poison_on_path_to};
use llrm_mir::valuetracking::pattern::match_simple_recurrence_of;

use crate::lftr::{branch_of, def_of, is_almost_dead_iv};

/// The pin's `SCEVCheapExpansionBudget` (`cl::opt`, default 4).
const SCEV_CHEAP_EXPANSION_BUDGET: u32 = 4;

/// `LLRM_NO_LOOP_TERM_FOLD`, read once; set, the pass is skipped (an A/B of one
/// build).
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_LOOP_TERM_FOLD")
}

/// `LoopTermFoldPass`, for the targets whose TTI enables it.
pub struct LoopTermFold {
    /// The compile is for size (`-Os`).
    pub size: bool,
}

impl FunctionPass for LoopTermFold {
    fn name(&self) -> &'static str {
        "looptermfold"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let Some(tti) = outer.target().tti() else { return PreservedAnalyses::all() };
        let optsize = self.size
            || unit.function.attrs.iter().any(|attr| {
                matches!(
                    attr,
                    Attribute::Flag(flag) if flag == "minsize" || flag == "optsize"
                )
            });
        if !enabled() || !tti.enables_loop_term_fold(optsize) {
            return PreservedAnalyses::all();
        }
        let held = analyses.get::<Scevs>(unit.context, unit.layout, unit.function);
        let (changed, scev) =
            folded_held(unit.context, unit.layout, unit.function, unit.declared, &outer, Some(held.take()));
        held.put(scev);
        if changed { PreservedAnalyses::none().preserve::<Scevs>() } else { PreservedAnalyses::all() }
    }
}

/// What `canFoldTermCondOfLoop` finds: the phi to test, the value it holds
/// after the last trip, and whether its increment must lose its poison flags.
struct Fold {
    help: InstId,
    term_value: Scev,
    ty: Ty,
    must_drop_poison: bool,
}

/// [`LoopTermFold`] over every loop of `function`, innermost first, reading
/// `held` (valid for `function`); whether anything changed, and the
/// expressions, current again.
pub fn folded_held(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    held: Option<ScalarEvolution>,
) -> (bool, ScalarEvolution) {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    if let Some(scev) = held {
        session.adopt(scev);
    }
    let mut mark = function.mark();
    let headers: Vec<BlockId> = {
        let (_, env) = session.split(layout, &outer.globals, &outer.metadata);
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        headers
    };
    let minimum_size = function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(flag) if flag == "minsize"));
    let costs = MachineCosts { tti: outer.target().tti(), minimum_size };
    let mut changed = false;
    for header in headers {
        session.forget_since_keeping(context, layout, &outer.globals, &outer.metadata, function, mark, true);
        mark = function.mark();
        let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
        let mut rewriter = Expander::new("lsr_fold_term_cond");
        let Some(fold) =
            can_fold_term_cond_of_loop(context, function, declared, outer, &costs, se, &env, &rewriter, header)
        else {
            continue;
        };
        run_term_fold(context, function, declared, outer, se, &env, &mut rewriter, header, &fold);
        changed = true;
    }
    session.forget_since_keeping(context, layout, &outer.globals, &outer.metadata, function, mark, true);
    (changed, session.into_scev())
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopTermFold.cpp#L51
/// `canFoldTermCondOfLoop`: the phi that can take the loop's exit test, if
/// the loop is of the required form.
#[allow(clippy::too_many_arguments)]
fn can_fold_term_cond_of_loop(
    context: &Context,
    function: &Function,
    declared: &Declared,
    outer: &Outer,
    tti: &dyn ExpansionTti,
    se: &mut ScalarEvolution,
    env: &Env,
    rewriter: &Expander,
    header: BlockId,
) -> Option<Fold> {
    let cx = env.cx(context, function);
    let l = cx.loops.get_loop_for(header)?;
    if !l.is_innermost() {
        return None;
    }
    // Only inspect on simple loop structure.
    if !l.is_loop_simplify_form(function) {
        return None;
    }
    if !se.has_loop_invariant_backedge_taken_count(&cx, header) {
        return None;
    }
    let latch = l.get_loop_latch(function)?;
    let (latch_branch, cond) = branch_of(function, latch)?;
    let term_cond =
        def_of(function, cond).filter(|&one| matches!(function.instruction(one).opcode, Opcode::ICmp(_)))?;
    let cond_value = function.instruction(term_cond).result?;
    if function.users(cond_value).iter().count() != 1 {
        return None;
    }
    let (lhs, rhs) = (function.instruction(term_cond).operands[0], function.instruction(term_cond).operands[1]);
    let lhs = def_of(function, lhs).filter(|&one| matches!(function.instruction(one).opcode, Opcode::Binary(_)))?;
    // We could pattern match the inverse form of the icmp, but that is
    // non-canonical, and this pass is running *very* late in the pipeline.
    if !l.is_loop_invariant(function, rhs) {
        return None;
    }
    // Find the IV used by the current exit condition.
    let query = cx.query(None);
    let (to_fold, _, _) = match_simple_recurrence_of(&query, lhs)?;
    // Ensure the simple recurrence is a part of the current loop.
    if function.parent(to_fold) != Some(header) {
        return None;
    }
    // If that IV isn't dead after we rewrite the exit condition in terms of
    // another IV, there's no point in doing the transform.
    if !is_almost_dead_iv(function, to_fold, latch, cond) {
        return None;
    }
    // Inserting instructions in the preheader has a runtime cost, scale the
    // allowed cost with the loops trip count as best we can.
    let budget = {
        let budget = 2 * SCEV_CHEAP_EXPANSION_BUDGET;
        match se.get_small_constant_max_trip_count(&cx, header) {
            0 => match estimated_trip_count(context, outer, declared, function, &l, latch) {
                Some(count) => budget.min(count),
                // Unknown trip count, assume long running by default.
                None => budget,
            },
            small => budget.min(small),
        }
    };
    let be_count = se.get_backedge_taken_count(&cx, header, ExitCountKind::Exact);
    let preheader = l.get_loop_preheader(function)?;
    let insert_pt = function.terminator(preheader)?;
    let latch_term = function.terminator(latch)?;
    let phis: Vec<InstId> =
        function.iter_block(header).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect();
    let mut found: Option<Fold> = None;
    for pn in phis {
        if pn == to_fold {
            continue;
        }
        let value = Operand::Value(function.instruction(pn).result?);
        if !se.is_value_scevable(&cx, value) {
            continue;
        }
        let ar = se.get_scev(&cx, value);
        // Only speculate on affine AddRec.
        if !matches!(se.node(ar).kind, Kind::AddRec(..)) || !se.is_affine(ar) {
            continue;
        }
        // Check that we can compute the value of AddRec on the exiting
        // iteration without soundness problems. evaluateAtIteration
        // internally needs to multiply the stride of the iteration number -
        // which may wrap around. The issue here is subtle because computing
        // the result accounting for wrap is insufficient. In order to use the
        // result in an exit test, we must also know that AddRec doesn't take
        // the same value on any previous iteration. The simplest case to
        // consider is a candidate IV which is narrower than the trip count
        // (and thus original IV), but this can also happen due to non-unit
        // strides on the candidate IVs.
        let step = se.get_step_recurrence(&cx, ar);
        if !se.has_no_self_wrap(ar) || !se.is_known_non_zero(&cx, step) {
            continue;
        }
        let post_inc = se.get_post_inc_expr(&cx, ar);
        let term_local = se.evaluate_at_iteration(&cx, post_inc, be_count);
        if !rewriter.is_safe_to_expand(&cx, se, term_local) {
            continue;
        }
        if rewriter.is_high_cost_expansion(&cx, se, &[term_local], header, budget, tti, insert_pt) {
            continue;
        }
        // The candidate IV may have been otherwise dead and poison from the
        // very first iteration. If we can't disprove that, we can't use the IV.
        if !must_execute_ub_if_poison_on_path_to(&query, pn, latch_term, cx.tree) {
            continue;
        }
        // The candidate IV may become poison on the last iteration. If this
        // value is not branched on, this is a well defined program. We're
        // about to add a new use to this IV, and we have to ensure we don't
        // insert UB which didn't previously exist.
        let mut must_drop_poison = false;
        let Some(post_inc_v) = arms(function, pn)
            .into_iter()
            .find(|(_, from)| *from == latch)
            .and_then(|(incoming, _)| def_of(function, incoming))
        else {
            continue;
        };
        if !must_execute_ub_if_poison_on_path_to(&query, post_inc_v, latch_term, cx.tree) {
            // If this is a complex recurrence with multiple instructions
            // computing the backedge value, we might need to strip poison
            // flags from all of them.
            if function.instruction(post_inc_v).operands.first() != Some(&value) {
                continue;
            }
            // In order to perform the transform, we need to drop the poison
            // generating flags on this instruction (if any).
            must_drop_poison = has_poison_generating_flags(&query, Operator::Instruction(post_inc_v));
        }
        // We pick the last legal alternate IV. We could explore choosing an
        // optimal alternate IV if we had a decent heuristic to do so.
        found = Some(Fold { help: pn, term_value: term_local, ty: se.type_of(ar), must_drop_poison });
    }
    let _ = latch_branch;
    found
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L862
/// `getLoopEstimatedTripCount` (DEVIATE LTF1): from the latch branch's
/// weights, `divideNearest(loop, exit) + 1`.
fn estimated_trip_count(
    context: &Context,
    outer: &Outer,
    declared: &Declared,
    function: &Function,
    l: &Loop,
    latch: BlockId,
) -> Option<u32> {
    let (branch, _) = branch_of(function, latch)?;
    let (Operand::Block(first), Operand::Block(second)) =
        (function.instruction(branch).operands[1], function.instruction(branch).operands[2])
    else {
        return None;
    };
    // The loop must have an exit through this branch.
    if l.contains(first) == l.contains(second) {
        return None;
    }
    let (mut loop_weight, mut exit_weight) =
        crate::profswap::weights_of(context, &outer.metadata, declared, function, branch)?;
    if l.contains(second) {
        std::mem::swap(&mut loop_weight, &mut exit_weight);
    }
    if exit_weight == 0 {
        return None;
    }
    // Estimated exit count is a ratio of the loop weight by the weight of the
    // edge exiting the loop, rounded to nearest.
    let exit_count = loop_weight.saturating_add(exit_weight / 2) / exit_weight;
    // When ExitCount + 1 would wrap in unsigned, saturate at UINT_MAX.
    Some(u32::try_from(exit_count).ok().and_then(|count| count.checked_add(1)).unwrap_or(u32::MAX))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopTermFold.cpp#L229
/// `RunTermFold`: the latch tests the found phi's increment against its final
/// value, expanded in the preheader.
#[allow(clippy::too_many_arguments)]
fn run_term_fold(
    context: &mut Context,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    se: &mut ScalarEvolution,
    env: &Env,
    rewriter: &mut Expander,
    header: BlockId,
    fold: &Fold,
) {
    let (preheader, latch) = {
        let cx = env.cx(&*context, &*function);
        let l = cx.loops.get_loop_for(header).expect("a loop");
        (l.get_loop_preheader(function).expect("a preheader"), l.get_loop_latch(function).expect("a latch"))
    };
    let loop_value = arms(function, fold.help).into_iter().find(|(_, from)| *from == latch).expect("a latch case").0;
    // See comment in canFoldTermCondOfLoop on why this is sufficient.
    if fold.must_drop_poison
        && let Some(inc) = def_of(function, loop_value)
    {
        crate::local::drop_poison_generating_flags(context, function, inc);
    }
    // Create new terminating value at loop preheader.
    rewriter.set_insert_point(function.terminator(preheader).expect("a terminator"));
    if outer.target().tti().is_some_and(|tti| tti.ltf_reuses_dominating_values()) {
        prime_dominating_values(context, function, se, env, preheader, fold.ty);
    }
    let term_value = rewriter.expand_code_for(env, context, declared, function, se, fold.term_value, Some(fold.ty));
    // Create new terminating condition at loop latch.
    let (branch, old_cond) = branch_of(function, latch).expect("a conditional branch");
    let old_cond = def_of(function, old_cond).expect("an icmp");
    let i1 = context.types.int(1);
    let new_cond = function.create_instruction(
        Opcode::ICmp(IntPredicate::Eq),
        i1,
        vec![loop_value, term_value],
        Flags::default(),
        Some("lsr_fold_term_cond.replaced_term_cond"),
    );
    function.insert(new_cond, Position::Before(branch)).expect("a placed branch");
    let new_value = Operand::Value(function.instruction(new_cond).result.expect("a compare value"));
    // Swap successors to exit loop body if IV equals to new TermValue.
    if function.instruction(branch).operands[1] == Operand::Block(header) {
        crate::profswap::swap_arms(context, &outer.metadata, declared, function, branch);
    }
    function.set_operand(branch, 0, new_value);
    function.erase(old_cond).expect("the old compare");
    // `DeleteDeadPHIs`: the phis nothing reads any more, those of a cycle
    // included.
    let phis: Vec<_> = function
        .iter_block(header)
        .take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .filter_map(|inst| function.instruction(inst).result)
        .collect();
    crate::dead::dead_from(context, outer.callees(), function, phis);
}

#[cfg(test)]
#[path = "looptermfold_tests.rs"]
mod tests;

/// DEVIATE LTF-1: the SCEV of each value of `ty` that dominates the end of
/// `preheader` (the first `PRIME_LIMIT` of them), so the expander finds it.
const PRIME_LIMIT: usize = 256;

fn prime_dominating_values(
    context: &Context,
    function: &Function,
    se: &mut ScalarEvolution,
    env: &Env,
    preheader: BlockId,
    ty: Ty,
) {
    let cx = env.cx(context, function);
    let at = function.terminator(preheader).expect("a terminator");
    let mut seen = 0;
    for &block in function.layout() {
        for inst in function.iter_block(block) {
            let Some(result) = function.instruction(inst).result else { continue };
            let value = Operand::Value(result);
            let same = match (cx.context.types.get(function.value(result).ty), ty) {
                (MirType::Int(bits), Ty::Int(want)) => *bits == want,
                (MirType::Pointer(space), Ty::Ptr(want)) => *space == want,
                _ => false,
            };
            if !same || !cx.tree.instruction_dominates(function, inst, at) || !se.is_value_scevable(&cx, value) {
                continue;
            }
            se.get_scev(&cx, value);
            seen += 1;
            if seen == PRIME_LIMIT {
                return;
            }
        }
    }
}
