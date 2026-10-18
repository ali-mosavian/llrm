//! A value a loop computes and the code after it reads becomes an expression
//! of the loop's trip count, written where the loop is left. LLVM's
//! `rewriteLoopExitValues` (`LoopUtils.cpp`), which IndVarSimplify calls with
//! its default policy `OnlyCheapRepl`.
//!
//! # How it works
//! For each loop, innermost first, and each phi of its exit blocks (loop-closed
//! SSA puts every value read after the loop there), the incoming value defined
//! in the loop is asked its value at the exit (`getSCEVAtExit`). It is
//! rewritten when that is invariant in the loop and safe to expand, unless the
//! loop itself reads the value in a way that cannot go away (an instruction
//! with side effects reached from it: `hasHardUserWithinLoop`). The expansion
//! is written before the defining instruction (the first non-phi of its block
//! for a phi) and must be cheap (`isHighCostExpansion`, budget 4), unless the
//! loop can then be deleted (`canLoopBeDeleted`: one exiting block, one exit,
//! nothing in the loop with side effects, the exit's other phis invariant).
//! The phi reads the expansion; a phi left with one incoming value is replaced
//! by it.
//!
//! # Example
//! `s = 0; for (i = 0; i < n; i++) s += 2; return s` returns `2 * n`: the exit
//! phi reads `shl n, 1` written in the exit block, and the loop's `s` is dead.
//!
//! # Used by
//! [`crate::indvars`], first of its steps. It replaced indvars' own
//! `exit_values`, which asked only the header for its exit.
//!
//! # See also
//! `rewriteLoopExitValues`, `canLoopBeDeleted`, `hasHardUserWithinLoop`,
//! `docs/architecture/scev-port.md` (D11, D12, D13).

use llrm_analysis::effects::Declarations;
use llrm_analysis::occurrence::arms;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expand_cost::{ExpansionTti, UnitCosts};
use llrm_analysis::scev::expander::Expander;
use llrm_analysis::scev::expr::Scev;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::{Declared, Outer};
use llrm_support::hash::HashSet;

use crate::edges;
use crate::local;

/// `SCEVCheapExpansionBudget`: its `cl::opt` default.
const CHEAP_EXPANSION_BUDGET: u32 = 4;

/// `LLRM_NO_SCEV_EXITVALUES`, read once: the exit values come from
/// ScalarEvolution; set, the step is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_SCEV_EXITVALUES")
}

/// `RewritePhi`: an exit phi's incoming value to rewrite.
struct RewritePhi {
    /// `PN`.
    phi: InstId,
    /// `Ith`.
    index: usize,
    /// `ExpansionSCEV`.
    expansion: Scev,
    /// `ExpansionPoint`.
    point: InstId,
    /// `HighCost`.
    high_cost: bool,
}

/// `function`'s loops' exit values rewritten (`IndVarSimplify::run`'s call of
/// `rewriteLoopExitValues` over every loop, the innermost first); whether any
/// was.
pub fn rewritten(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    let headers: Vec<BlockId> = {
        let (_, env) = session.split(layout, &outer.globals, &outer.metadata);
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        headers
    };
    let minimum_size = function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(flag) if flag == "minsize"));
    let tti = UnitCosts { minimum_size };
    let mut rewriter = Expander::new("indvars");
    let mut changed = false;
    for header in headers {
        // `IndVarSimplify::run`: stay out of trouble without LoopSimplify
        // form.
        let simple = {
            let (_, env) = session.split(layout, &outer.globals, &outer.metadata);
            let cx = env.cx(context, function);
            let Some(found) = cx.loops.get_loop_for(header) else { continue };
            found.get_loop_preheader(function).is_some()
                && found.get_loop_latch(function).is_some()
                && found.has_dedicated_exits(function)
        };
        if !simple {
            continue;
        }
        changed |= rewrite_loop_exit_values(
            context,
            layout,
            function,
            declared,
            outer,
            &mut session,
            &mut rewriter,
            &tti,
            header,
        ) > 0;
    }
    changed
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L1876
/// `rewriteLoopExitValues(L, ...)` for the loop headed by `header`: the number
/// of exit values replaced. The policy is `OnlyCheapRepl` (DEVIATE 11).
#[allow(clippy::too_many_arguments)]
fn rewrite_loop_exit_values(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    session: &mut ScevSession,
    rewriter: &mut Expander,
    tti: &dyn ExpansionTti,
    header: BlockId,
) -> usize {
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let declarations: &Declarations = &outer.globals;
    let mut rewrite_phi_set: Vec<RewritePhi> = Vec::new();
    {
        let cx = env.cx(context, function);
        let found = cx.loops.get_loop_for(header).expect("a loop");
        // Find all values that are computed inside the loop, but used outside
        // of it. Because of LCSSA, these values will only occur in LCSSA PHI
        // Nodes. Scan the exit blocks of the loop to find them.
        for exit in found.get_unique_exit_blocks(function) {
            // If there are no PHI nodes in this exit block, then no values
            // defined inside the loop are used on this path, skip it.
            for phi in edges::phis(function, exit) {
                let Some(result) = function.instruction(phi).result else { continue };
                if function.users(result).is_empty() {
                    continue; // dead use, don't replace it
                }
                if !se.is_value_scevable(&cx, Operand::Value(result)) {
                    continue;
                }
                // Iterate over all of the values in all the PHI nodes.
                for (index, (value, from)) in arms(function, phi).into_iter().enumerate() {
                    // If the value being merged in is not integer or is not
                    // defined in the loop, skip it.
                    let Operand::Value(in_value) = value else { continue };
                    let ValueDef::Instruction(inst) = function.value(in_value).def else { continue };
                    // If this pred is for a subloop, not L itself, skip it.
                    if cx.loops.get_loop_for(from).map(|one| one.get_header()) != Some(header) {
                        continue; // The Block is in a subloop, skip it.
                    }
                    // Check that InVal is defined in the loop.
                    if !found.contains_instruction(function, inst) {
                        continue;
                    }
                    // Okay, this instruction has a user outside of the current
                    // loop and varies predictably *inside* the loop. Evaluate
                    // the value it contains when the loop exits, if possible.
                    // We prefer to start with expressions which are true for
                    // all exits (so as to maximize expression reuse by the
                    // SCEVExpander), but resort to per-exit evaluation if that
                    // fails.
                    let scev = se.get_scev(&cx, value);
                    let exit_value = se.get_scev_at_exit(&cx, scev, header, from).scev;
                    if !se.is_loop_invariant(&cx, exit_value, Some(header))
                        || !rewriter.is_safe_to_expand(&cx, se, exit_value)
                    {
                        continue;
                    }
                    // DEVIATE 14: the expander does not yet write an add
                    // recurrence (`visitAddRecExpr`), which an enclosing
                    // loop's value in the exit value would need.
                    if llrm_analysis::scev::guards::contains_add_recurrence(se, exit_value) {
                        continue;
                    }
                    // DEVIATE 15: the closed form of a sum over the count
                    // (`n(n-1)/2`) is computed in a type one bit wider
                    // (`i33`), which the back end does not legalize (#2497).
                    if has_odd_width(se, exit_value) {
                        continue;
                    }
                    // Computing the value outside of the loop brings no
                    // benefit if it is definitely used inside the loop in a
                    // way which can not be optimized away. Avoid doing so
                    // unless we know we have a value which computes the
                    // ExitValue already. TODO: This should be merged into SCEV
                    // expander to leverage its knowledge of existing
                    // expressions.
                    let plain = matches!(
                        se.node(exit_value).kind,
                        llrm_analysis::scev::expr::Kind::Constant(_) | llrm_analysis::scev::expr::Kind::Unknown(_)
                    );
                    if !plain && has_hard_user_within_loop(context, outer, declarations, function, &found, inst) {
                        continue;
                    }
                    // Check if expansions of this SCEV would count as being
                    // high cost.
                    let high_cost = rewriter.is_high_cost_expansion(
                        &cx,
                        se,
                        &[exit_value],
                        header,
                        CHEAP_EXPANSION_BUDGET,
                        tti,
                        inst,
                    );
                    // Note that we must not perform expansions until after we
                    // query *all* the costs, because if we perform temporary
                    // expansion inbetween, one that we might not intend to
                    // keep, said expansion *may* affect cost calculation of
                    // the next SCEV's we'll query, and next SCEV may
                    // errneously get smaller cost.

                    // Collect all the candidate PHINodes to be rewritten.
                    let point = if function.instruction(inst).opcode == Opcode::Phi {
                        first_insertion_point(function, function.parent(inst).expect("a placed instruction"))
                    } else {
                        inst
                    };
                    rewrite_phi_set.push(RewritePhi { phi, index, expansion: exit_value, point, high_cost });
                }
            }
        }
    }

    let can_be_deleted = {
        let cx = env.cx(context, function);
        let found = cx.loops.get_loop_for(header).expect("a loop");
        can_loop_be_deleted(context, outer, declarations, function, &found, &rewrite_phi_set)
    };
    let mut replaced = 0;

    // Transformation.
    for rewrite in &rewrite_phi_set {
        // Only do the rewrite when the ExitValue can be expanded cheaply. If
        // LoopCanBeDel is true, rewrite exit value aggressively.
        if !can_be_deleted && rewrite.high_cost {
            continue;
        }
        rewriter.set_insert_point(rewrite.point);
        let exit_value = rewriter.expand_code_for(&env, context, declared, function, se, rewrite.expansion, None);
        replaced += 1;
        let phi_result = function.instruction(rewrite.phi).result.expect("a phi has a value");
        function.set_operand(rewrite.phi, 2 * rewrite.index, exit_value);
        // It's necessary to tell ScalarEvolution about this explicitly so
        // that it can walk the def-use list and forget all SCEVs, as it may
        // not be watching the PHI itself. Once the new exit value is in place,
        // there may not be a def-use connection between the loop and every
        // instruction which got a SCEVAddRecExpr for that loop.
        {
            let cx = env.cx(context, function);
            se.forget_value(&cx, Operand::Value(phi_result));
        }
        // The incoming instruction may be dead now: `indvars` ends with `dead`.

        // Replace PN with ExitVal if that is legal and does not break LCSSA.
        if function.instruction(rewrite.phi).operands.len() == 2 && {
            let cx = env.cx(context, function);
            replacement_preserves_lcssa_form(&cx, rewrite.phi, exit_value)
        } {
            function.replace_all_uses_with(phi_result, exit_value);
            function.erase(rewrite.phi).expect("an unused phi");
        }
    }

    // The insertion point instruction may have been deleted; clear it out so
    // that the rewriter doesn't trip over it later.
    rewriter.clear_insert_point();
    replaced
}

/// Whether `s` has a subexpression of an integer width other than 8, 16, 32
/// or 64: the expansion would write an `iN` the back end cannot select.
fn has_odd_width(
    se: &llrm_analysis::scev::ScalarEvolution,
    s: Scev,
) -> bool {
    use llrm_analysis::scev::expr::{Kind, Ty};
    let mut seen: HashSet<Scev> = HashSet::default();
    let mut work = vec![s];
    while let Some(s) = work.pop() {
        if !seen.insert(s) {
            continue;
        }
        if let Some(Ty::Int(bits)) = se.node(s).ty {
            if !matches!(bits, 1 | 8 | 16 | 32 | 64) {
                return true;
            }
        }
        match &se.node(s).kind {
            Kind::Constant(_) | Kind::Unknown(_) | Kind::CouldNotCompute => {}
            _ => work.extend(se.operands_of(s).iter().map(|op| op.scev)),
        }
    }
    false
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L1770
/// `hasHardUserWithinLoop(L, I)`.
fn has_hard_user_within_loop(
    context: &Context,
    outer: &Outer,
    declarations: &Declarations,
    function: &Function,
    l: &Loop,
    inst: InstId,
) -> bool {
    let mut visited: IdSet<InstId> = IdSet::new();
    let mut worklist = vec![inst];
    visited.insert(inst);
    while let Some(curr) = worklist.pop() {
        // This use is outside the loop, nothing to do.
        if !l.contains_instruction(function, curr) {
            continue;
        }
        // Do we assume it is a "hard" use which will not be eliminated easily?
        if local::may_have_side_effects(context, outer.callees(), declarations, function, curr) {
            return true;
        }
        // Otherwise, add all its users to worklist.
        if let Some(result) = function.instruction(curr).result {
            for user in function.users(result) {
                if visited.insert(user.user) {
                    worklist.push(user.user);
                }
            }
        }
    }
    false
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L1811
/// `canLoopBeDeleted(L, RewritePhiSet)`: whether the loop will be deletable
/// once the exit values are rewritten. If it is possible, `ReplaceExitValue`
/// is ignored and the rewriting is done aggressively.
fn can_loop_be_deleted(
    context: &Context,
    outer: &Outer,
    declarations: &Declarations,
    function: &Function,
    l: &Loop,
    rewrite_phi_set: &[RewritePhi],
) -> bool {
    // If there is no preheader, the loop will not be deleted.
    if l.get_loop_preheader(function).is_none() {
        return false;
    }
    // In LoopDeletion pass Loop can be deleted when ExitingBlocks.size() > 1.
    // We obviate multiple ExitingBlocks case for simplicity.
    // TODO: If we see testcase with multiple ExitingBlocks can be deleted
    // after exit value rewriting, we can enhance the logic here.
    let exiting_blocks = l.get_exiting_blocks(function);
    let exit_blocks = l.get_unique_exit_blocks(function);
    let ([exit_block], [exiting]) = (&exit_blocks[..], &exiting_blocks[..]) else { return false };

    for phi in edges::phis(function, *exit_block) {
        let Some((incoming, _)) = arms(function, phi).into_iter().find(|&(_, from)| from == *exiting) else {
            continue;
        };
        // If the Incoming value of P is found in RewritePhiSet, we know it
        // could be rewritten to use a loop invariant value in transformation
        // phase later. Skip it in the loop invariant check below.
        let found = rewrite_phi_set.iter().any(|rewrite| {
            rewrite.phi == phi && arms(function, phi).get(rewrite.index).is_some_and(|&(value, _)| value == incoming)
        });
        if !found {
            if let Operand::Value(value) = incoming {
                if let ValueDef::Instruction(inst) = function.value(value).def {
                    if !has_loop_invariant_operands(function, l, inst) {
                        return false;
                    }
                }
            }
        }
    }

    for &block in l.get_blocks() {
        if function
            .block(block)
            .iter()
            .any(|inst| local::may_have_side_effects(context, outer.callees(), declarations, function, inst))
        {
            return false;
        }
    }
    true
}

/// `Loop::hasLoopInvariantOperands`.
fn has_loop_invariant_operands(
    function: &Function,
    l: &Loop,
    inst: InstId,
) -> bool {
    function
        .instruction(inst)
        .operands
        .iter()
        .all(
            |&operand| match operand {
                Operand::Value(value) => match function.value(value).def {
                    ValueDef::Instruction(def) => !l.contains_instruction(function, def),
                    _ => true,
                },
                _ => true,
            },
        )
}

/// `BasicBlock::getFirstInsertionPt`: the first instruction that is neither a
/// phi nor a landing pad.
fn first_insertion_point(
    function: &Function,
    block: BlockId,
) -> InstId {
    function
        .block(block)
        .iter()
        .find(|&inst| !matches!(
            function.instruction(inst).opcode,
            Opcode::Phi | Opcode::LandingPad { .. }
        ))
        .expect("a terminated block")
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopInfo.cpp#L1036
/// `LoopInfo::replacementPreservesLCSSAForm(From, To)`.
fn replacement_preserves_lcssa_form(
    cx: &llrm_analysis::scev::Cx,
    from: InstId,
    to: Operand,
) -> bool {
    // Preserving LCSSA form is only problematic if the replacing value is an
    // instruction.
    let Operand::Value(value) = to else { return true };
    let ValueDef::Instruction(to) = cx.function.value(value).def else { return true };
    // If both instructions are defined in the same basic block then
    // replacement cannot break LCSSA form.
    let (to_block, from_block) = (cx.function.parent(to), cx.function.parent(from));
    if to_block == from_block {
        return true;
    }
    // If the instruction is not defined in a loop then it can safely replace
    // anything.
    let Some(to_loop) = to_block.and_then(|block| cx.loops.get_loop_for(block)) else { return true };
    // If the replacing instruction is defined in a loop, so must be the
    // original in the same loop for LCSSA.
    to_loop.contains_loop(from_block.and_then(|block| cx.loops.get_loop_for(block)))
}

#[cfg(test)]
#[path = "exitvalues_tests.rs"]
mod tests;
