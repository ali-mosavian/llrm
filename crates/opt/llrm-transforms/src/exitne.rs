//! A loop's strict exit compare of a unit-stride counter becomes `!=` where the
//! counter provably never passes the bound. Beyond the pin: no LLVM pass
//! rewrites `ugt`/`ult` to `ne` on its own (`canonicalizeExitCondition` and
//! `CorrelatedValuePropagation` only turn signed compares unsigned).
//!
//! # How it works
//! For each loop whose latch ends in a conditional branch that stays in the
//! loop on one side and leaves it on the other, with a compare of `X` against
//! a loop-invariant `C` that continues the loop on `X >u C`, `X >s C`,
//! `X <u C` or `X <s C`:
//!
//! - `X` is `{S,+,-1}` for `>` (`{S,+,1}` for `<`) in this loop.
//! - The loop entry is guarded by `S >= C` (`S <= C`), same signedness.
//!
//! Then `X >= C` holds on every trip: it holds on the first, and the loop goes
//! round again only when `X > C`, so the step cannot cross `C`. The compare
//! equals `X != C` there, and exits exactly when `X == C`. Nothing is poison:
//! the compare is the same bit before and after.
//!
//! # Example
//! `n` is positive; `%i = phi [n, entry], [%i.next, loop]`, `%i.next = add %i,
//! -1`, `br (icmp ugt %i, 1), loop, exit` becomes `icmp ne %i, 1`.
//!
//! # Used by
//! [`crate::indvars`] runs [`strengthened_in`] after LFTR. The m68k mid-end
//! runs [`strengthened`] before loop strength reduction.
//!
//! # See also
//! [`crate::lftr`], `docs/architecture/scev-port.md`.

use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expr::Kind;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{IntPredicate, Opcode};
use llrm_mir::passes::Outer;

/// `LLRM_NO_SCEV_EXITNE`, read once; set, the step is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_SCEV_EXITNE")
}

/// [`strengthened_in`] over a session of its own.
pub fn strengthened(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    strengthened_in(&mut session, context, layout, function, outer)
}

/// The latch compare of every loop of `function` made `!=`/`==` where it
/// qualifies; whether anything changed. The compare's value does not change,
/// so no expression is stale.
pub fn strengthened_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
    crate::loopfilter::retain(&mut headers);
    let mut edits = Vec::new();
    {
        let cx = env.cx(&*context, &*function);
        for header in headers {
            if let Some(edit) = never_passes(se, &cx, header) {
                edits.push(edit);
            }
        }
    }
    for &(icmp, pred) in &edits {
        function.set_icmp_predicate(icmp, pred);
    }
    !edits.is_empty()
}

/// The latch compare of the loop at `header` and the predicate it becomes,
/// where `X` never passes the bound.
fn never_passes(
    se: &mut ScalarEvolution,
    cx: &Cx,
    header: BlockId,
) -> Option<(InstId, IntPredicate)> {
    let l = cx.loops.get_loop_for(header)?;
    let latch = l.get_loop_latch(cx.function)?;
    // The latch belongs to this loop, not an inner one.
    if cx.loops.get_loop_for(latch)?.get_header() != header {
        return None;
    }
    let branch = cx.function.instruction(cx.function.terminator(latch)?);
    if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
        return None;
    }
    let (Operand::Value(cond), Operand::Block(on_true), Operand::Block(on_false)) =
        (branch.operands[0], branch.operands[1], branch.operands[2])
    else {
        return None;
    };
    // The loop goes round on one side and is left on the other.
    let goes_round_on_true = if on_true == header && !l.contains(on_false) {
        true
    } else if on_false == header && !l.contains(on_true) {
        false
    } else {
        return None;
    };
    let ValueDef::Instruction(icmp) = cx.function.value(cond).def else { return None };
    let Opcode::ICmp(pred) = cx.function.instruction(icmp).opcode else { return None };
    let (lhs, rhs) = (cx.function.instruction(icmp).operands[0], cx.function.instruction(icmp).operands[1]);
    let (x, bound, pred) = if l.is_loop_invariant(cx.function, rhs) && !l.is_loop_invariant(cx.function, lhs) {
        (lhs, rhs, pred)
    } else if l.is_loop_invariant(cx.function, lhs) && !l.is_loop_invariant(cx.function, rhs) {
        (rhs, lhs, pred.swapped())
    } else {
        return None;
    };
    let goes_round_on = if goes_round_on_true { pred } else { pred.inverse() };

    let x = se.get_scev(cx, x);
    let Kind::AddRec(operands, rec_header) = se.node(x).kind.clone() else { return None };
    if rec_header != header || operands.len() != 2 {
        return None;
    }
    let (start, step) = (operands[0].scev, se.as_constant(operands[1].scev)?);
    // The side of the bound the counter starts on, which it never leaves.
    let held = match goes_round_on {
        IntPredicate::Ugt if step.is_all_ones() => IntPredicate::Uge,
        IntPredicate::Sgt if step.is_all_ones() => IntPredicate::Sge,
        IntPredicate::Ult if step.is_one() => IntPredicate::Ule,
        IntPredicate::Slt if step.is_one() => IntPredicate::Sle,
        _ => return None,
    };
    let bound = se.get_scev(cx, bound);
    if !se.is_available_at_loop_entry(cx, start, header) || !se.is_available_at_loop_entry(cx, bound, header) {
        return None;
    }
    if !se.is_loop_entry_guarded_by_cond_pred(cx, header, held, start, bound) {
        return None;
    }
    Some((icmp, if goes_round_on_true { IntPredicate::Ne } else { IntPredicate::Eq }))
}

#[cfg(test)]
#[path = "exitne_tests.rs"]
mod tests;
