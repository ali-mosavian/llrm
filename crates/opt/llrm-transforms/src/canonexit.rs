//! A loop's exit compare of a zero-extended counter is made unsigned, and the
//! extension is moved off the counter onto the invariant side. LLVM's
//! `canonicalizeExitCondition` (`IndVarSimplify.cpp`).
//!
//! # How it works
//! For each loop, innermost first, over its exiting blocks that end in a
//! one-use `icmp`:
//!
//! - A signed compare of `zext X` with a loop-invariant `Y` whose unsigned
//!   range (under the loop's guards) fits `X`'s width becomes the unsigned
//!   compare: both sides are non-negative.
//! - An unsigned compare of `zext X` with such a `Y` becomes `X` against `trunc
//!   Y`, the truncation written in the preheader, when the `zext` has no other
//!   use or `X` is an add recurrence. `samesign` is dropped.
//!
//! # Example
//! `icmp slt (zext i16 %i to i32), %n` with `%n` known below 65536 becomes
//! `icmp ult (zext i16 %i to i32), %n`, then
//! `icmp ult i16 %i, %n.trunc` with `%n.trunc = trunc i32 %n to i16` in the
//! preheader.
//!
//! # Used by
//! Not wired in: [`crate::indvars`] will run [`canonicalized`] after the
//! congruent IVs, as `IndVarSimplify::run` does.
//!
//! # See also
//! `canonicalizeExitCondition`, `docs/architecture/scev-port.md`.

use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expr::Kind;
use llrm_mir::constantrange::ConstantRange;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::types::Type as MirType;

/// `LLRM_NO_SCEV_CANONEXIT`, read once; set, the step is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_SCEV_CANONEXIT")
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1527
/// `canonicalizeExitCondition` over every loop of `function`, innermost
/// first; whether anything changed.
pub fn canonicalized(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    canonicalized_in(&mut session, context, layout, function, outer)
}

/// [`canonicalized`] over `session`, whose expressions are valid for
/// `function`.
pub fn canonicalized_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let headers: Vec<BlockId> = {
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        crate::loopfilter::retain(&mut headers);
        headers
    };
    let mut changed = false;
    for header in headers {
        // `IndVarSimplify::run`: stay out of trouble without LoopSimplify
        // form.
        let (simple, exiting) = {
            let cx = env.cx(&*context, &*function);
            match cx.loops.get_loop_for(header) {
                Some(found) => (found.is_loop_simplify_form(cx.function), found.get_exiting_blocks(cx.function)),
                None => continue,
            }
        };
        if !simple {
            continue;
        }

        // Note: This is duplicating a particular part on SimplifyIndVars
        // reasoning. We need to duplicate it because given icmp
        // zext(small-iv), C, IVUsers never reaches the icmp since the zext
        // doesn't fold to an AddRec unless it already has flags. The concern
        // about caching sub-optimal results is why we only query SCEVs of the
        // loop invariant RHS here.
        for &exiting_bb in &exiting {
            let Some((icmp, mut lhs, mut rhs)) = one_use_compare(function, exiting_bb) else { continue };
            let cx = env.cx(&*context, &*function);
            let l = cx.loops.get_loop_for(header).expect("a loop");
            // For the range reasoning, avoid computing SCEVs in the loop to
            // avoid poisoning cache with sub-optimal results. For the
            // must-execute case, this is a neccessary precondition for
            // correctness.
            if !l.is_loop_invariant(function, rhs) {
                if !l.is_loop_invariant(function, lhs) {
                    continue;
                }
                // Same logic applies for the inverse case.
                std::mem::swap(&mut lhs, &mut rhs);
            }
            // Match (icmp signed-cond zext, RHS).
            let Some(lhs_op) = zext_operand(function, lhs) else { continue };
            let Opcode::ICmp(pred) = function.instruction(icmp).opcode else { unreachable!("an icmp") };
            if !is_signed(pred) {
                continue;
            }
            let (inner, outer_bits) = (int_bits(context, function, lhs_op), int_bits(context, function, rhs));
            let full = ConstantRange::full(inner).zero_extend(outer_bits);
            let guarded = {
                let s = se.get_scev(&cx, rhs);
                se.apply_loop_guards(&cx, s, header)
            };
            let rhs_range = se.get_unsigned_range(&cx, guarded);
            if full.contains_range(&rhs_range) {
                // We have now matched icmp signed-cond zext(X), zext(Y'), and
                // can thus replace the signed condition with the unsigned
                // version. No SCEV invalidation needed: neither exit counts
                // nor the values the compare produces changed.
                function.set_icmp_predicate(icmp, unsigned(pred));
                changed = true;
            }
        }

        // Now that we've canonicalized the condition to match the extend, see
        // if we can rotate the extend out of the loop.
        for &exiting_bb in &exiting {
            let Some((icmp, mut lhs, mut rhs)) = one_use_compare(function, exiting_bb) else { continue };
            let Opcode::ICmp(pred) = function.instruction(icmp).opcode else { unreachable!("an icmp") };
            if !is_unsigned(pred) {
                continue;
            }
            let cx = env.cx(&*context, &*function);
            let l = cx.loops.get_loop_for(header).expect("a loop");
            let mut swapped = false;
            if l.is_loop_invariant(function, lhs) == l.is_loop_invariant(function, rhs) {
                // Nothing to rotate.
                continue;
            }
            if l.is_loop_invariant(function, lhs) {
                // Same logic applies for the inverse case until we actually
                // pick which operand of the compare to update.
                swapped = true;
                std::mem::swap(&mut lhs, &mut rhs);
            }
            // Match (icmp unsigned-cond zext, RHS).
            // TODO in LLVM: extend to the sext/signed-cmp case and to other
            // invertible functions.
            let Some(lhs_op) = zext_operand(function, lhs) else { continue };
            // In general, we only rotate if we can do so without increasing
            // the number of instructions. The exception is when we have an
            // zext(add-rec). The reason for allowing this exception is that we
            // know we need to get rid of the zext for SCEV to be able to
            // compute a trip count for said loops; we consider the new trip
            // count valuable enough to increase instruction count by one.
            let lhs_inst = match lhs {
                Operand::Value(value) => value,
                _ => continue,
            };
            let lhs_one_use = function.users(lhs_inst).iter().count() == 1;
            if !lhs_one_use {
                let s = se.get_scev(&cx, lhs_op);
                if !matches!(se.node(s).kind, Kind::AddRec(..)) {
                    continue;
                }
            }
            // Given a icmp unsigned-cond zext(Op) where zext(trunc(RHS)) == RHS
            // replace with an icmp of the form icmp unsigned-cond Op,
            // trunc(RHS) when zext is loop varying and RHS is loop
            // invariant. This converts loop varying work to
            // loop-invariant work.
            let (inner, outer_bits) = (int_bits(context, function, lhs_op), int_bits(context, function, rhs));
            let full = ConstantRange::full(inner).zero_extend(outer_bits);
            let guarded = {
                let s = se.get_scev(&cx, rhs);
                se.apply_loop_guards(&cx, s, header)
            };
            let rhs_range = se.get_unsigned_range(&cx, guarded);
            if !full.contains_range(&rhs_range) {
                continue;
            }
            let preheader = l.get_loop_preheader(function).expect("a preheader");
            let preheader_term = function.terminator(preheader).expect("a terminator");
            let narrow = operand_type(context, function, lhs_op);
            let new_rhs =
                function.create_instruction(Opcode::Cast(CastOp::Trunc), narrow, vec![rhs], Flags::default(), None);
            function.insert(new_rhs, Position::Before(preheader_term)).expect("a placed terminator");
            let new_rhs = Operand::Value(function.instruction(new_rhs).result.expect("a trunc value"));
            let (lhs_at, rhs_at) = if swapped { (1, 0) } else { (0, 1) };
            function.set_operand(icmp, lhs_at, lhs_op);
            function.set_operand(icmp, rhs_at, new_rhs);
            // Samesign flag cannot be preserved after narrowing the compare.
            let mut flags = function.instruction(icmp).flags;
            flags.remove(Flags::SAMESIGN);
            function.set_flags(icmp, flags);
            // Note, we are leaving SCEV in an unfortunately imprecise case
            // here as rotation tends to reveal information about trip counts
            // not previously visible.
            changed = true;
        }
    }
    changed
}

/// The `icmp` an exiting block branches on, if it has one use, with its
/// operands.
fn one_use_compare(
    function: &Function,
    exiting: BlockId,
) -> Option<(InstId, Operand, Operand)> {
    let term = function.terminator(exiting)?;
    let branch = function.instruction(term);
    if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
        return None;
    }
    let Operand::Value(cond) = branch.operands[0] else { return None };
    let ValueDef::Instruction(icmp) = function.value(cond).def else { return None };
    if !matches!(function.instruction(icmp).opcode, Opcode::ICmp(_)) || function.users(cond).iter().count() != 1 {
        return None;
    }
    let operands = &function.instruction(icmp).operands;
    Some((icmp, operands[0], operands[1]))
}

/// `X` of `zext X`.
fn zext_operand(
    function: &Function,
    v: Operand,
) -> Option<Operand> {
    let Operand::Value(value) = v else { return None };
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    let instruction = function.instruction(inst);
    (instruction.opcode == Opcode::Cast(CastOp::ZExt)).then(|| instruction.operands[0])
}

fn operand_type(
    context: &Context,
    function: &Function,
    v: Operand,
) -> llrm_mir::types::TypeId {
    match v {
        Operand::Value(value) => function.value(value).ty,
        Operand::Constant(constant) => context.get(constant).ty,
        _ => unreachable!("a value"),
    }
}

fn int_bits(
    context: &Context,
    function: &Function,
    v: Operand,
) -> u32 {
    match context.types.get(operand_type(context, function, v)) {
        MirType::Int(bits) => *bits,
        _ => unreachable!("an integer"),
    }
}

fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

fn is_unsigned(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
    )
}

/// `getUnsignedPredicate`.
fn unsigned(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Sgt => IntPredicate::Ugt,
        IntPredicate::Sge => IntPredicate::Uge,
        IntPredicate::Slt => IntPredicate::Ult,
        IntPredicate::Sle => IntPredicate::Ule,
        same => same,
    }
}

#[cfg(test)]
#[path = "canonexit_tests.rs"]
mod tests;
