//! A loop counter whose range provably fits a native integer width becomes a
//! counter of that width. Beyond the pin: IndVarSimplify only widens
//! counters, so `for (long i = 0; i < 12; ++i)` keeps four bytes on an 8-bit
//! target, a four-byte compare for each test of `i`.
//!
//! DEVIATE IV-NARROW (owner z80-b, review 2026-12-20): no pin pass narrows a
//! counter; `WidenIV`
//! ([SimplifyIndVar.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1855))
//! only widens, and `opt -passes=default<O2>` on this IR at `n8:16` keeps the
//! `i32` phi. `TruncInstCombine` and instcombine's trunc demotion start from a
//! `trunc` and cannot reach a phi cycle that has other users.
//!
//! # How it works
//! For each loop in LoopSimplify form, a header phi `%i` of an integer width
//! the datalayout does not hold natively (`legal_integer`, the test of LLVM's
//! `shouldChangeType`), with the preheader's start and `%next = add/sub %i, X`
//! from the latch (`X` loop-invariant), is narrowed when ScalarEvolution's
//! range of `%i` and of `%next` fits a native width `W` below it: unsigned
//! (`zext`) or signed (`sext`), whichever is narrower. The loop then carries a
//! phi and an increment of `W` bits; every use of `%i` and `%next` reads a
//! `zext`/`sext` of them, which the combiner folds into the uses.
//!
//! # Example
//! `%i = phi i32 [0, %entry], [%next, %loop]`, `%next = add i32 %i, 1`, exit on
//! `%next == 12` (range 0..=12 for both) becomes
//! `%i.n = phi i8 [0, %entry], [%next.n, %loop]`, `%next.n = add i8 %i.n, 1`,
//! `%i = zext i8 %i.n to i32`, `%next = zext i8 %next.n to i32`.
//!
//! # Used by
//! [`crate::indvars`] runs [`narrowed_in`] after LFTR where
//! `Tti::narrows_counters`.
//!
//! # See also
//! [`crate::widen`] (the other direction), `docs/architecture/scev-port.md`.

use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expr::Kind;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::types::TypeId;

/// `LLRM_NO_SCEV_NARROWIV`, read once; set, the step is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_SCEV_NARROWIV")
}

/// What to narrow: the counter, its increment and the width.
struct Plan {
    preheader: BlockId,
    phi: InstId,
    increment: InstId,
    /// The increment's operand that is not the counter, and whether the
    /// counter is its first operand (a `sub` needs it so).
    step: Operand,
    narrow_bits: u32,
    signed: bool,
}

/// [`narrowed_in`] over a session of its own.
pub fn narrowed(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    size: bool,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    narrowed_in(&mut session, context, layout, function, outer, size)
}

/// The counters of `function` narrowed where the target asks
/// (`Tti::narrows_counters`); whether anything changed. The replaced phis and
/// increments are left to `dead`.
pub fn narrowed_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    size: bool,
) -> bool {
    if !outer.target().tti().is_some_and(|tti| tti.narrows_counters() && (size || tti.narrows_counters_at_speed())) {
        return false;
    }
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
    crate::loopfilter::retain(&mut headers);
    let mut plans = Vec::new();
    {
        let cx = env.cx(&*context, &*function);
        for header in headers {
            plan_loop(se, &cx, layout, header, &mut plans);
        }
    }
    for plan in &plans {
        narrow(context, layout, function, plan);
    }
    !plans.is_empty()
}

/// The counters of the loop at `header` to narrow.
fn plan_loop(
    se: &mut ScalarEvolution,
    cx: &Cx,
    layout: &DataLayout,
    header: BlockId,
    plans: &mut Vec<Plan>,
) {
    let Some(l) = cx.loops.get_loop_for(header) else { return };
    if !l.is_loop_simplify_form(cx.function) {
        return;
    }
    let (Some(preheader), Some(latch)) = (l.get_loop_preheader(cx.function), l.get_loop_latch(cx.function)) else {
        return;
    };
    let phis: Vec<InstId> =
        cx.function.iter_block(header).take_while(|&inst| cx.function.instruction(inst).opcode == Opcode::Phi).collect();
    for phi in phis {
        let inst = cx.function.instruction(phi);
        let Some(bits) = cx.context.types.int_bits(inst.ty) else { continue };
        if layout.legal_integer(bits) {
            continue;
        }
        // The counter: one input from the preheader, one from the latch.
        let [first, Operand::Block(first_from), second, Operand::Block(second_from)] = inst.operands[..] else {
            continue;
        };
        let from_latch = if first_from == latch && second_from == preheader {
            first
        } else if second_from == latch && first_from == preheader {
            second
        } else {
            continue;
        };
        let Operand::Value(next) = from_latch else { continue };
        let ValueDef::Instruction(increment) = cx.function.value(next).def else { continue };
        let Some(own) = inst.result else { continue };
        let step = {
            let add = cx.function.instruction(increment);
            let Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub)) = add.opcode else { continue };
            if !l.contains(cx.function.parent(increment).expect("placed")) {
                continue;
            }
            let [lhs, rhs] = add.operands[..] else { continue };
            let counter = Operand::Value(own);
            match (lhs == counter, rhs == counter, op) {
                (true, false, _) => rhs,
                (false, true, BinaryOp::Add) => lhs,
                _ => continue,
            }
        };
        if !l.is_loop_invariant(cx.function, step) {
            continue;
        }
        let widths: Vec<u32> = (1..bits).filter(|&w| layout.legal_integer(w)).collect();
        let (counter, advanced) = (se.get_scev(cx, Operand::Value(own)), se.get_scev(cx, Operand::Value(next)));
        // A counter that is not an affine recurrence of this loop has a range
        // no trip count bounds.
        let Kind::AddRec(_, rec_header) = se.node(counter).kind.clone() else { continue };
        if rec_header != header {
            continue;
        }
        let (a, b) = (se.get_unsigned_range(cx, counter), se.get_unsigned_range(cx, advanced));
        let unsigned = a.get_active_bits().max(b.get_active_bits());
        let (a, b) = (se.get_signed_range(cx, counter), se.get_signed_range(cx, advanced));
        let signed = a.get_min_signed_bits().max(b.get_min_signed_bits());
        let by_unsigned = widths.iter().copied().find(|&w| unsigned <= w);
        let by_signed = widths.iter().copied().find(|&w| signed <= w);
        let (narrow_bits, signed) = match (by_unsigned, by_signed) {
            (Some(u), Some(s)) if s < u => (s, true),
            (Some(u), _) => (u, false),
            (None, Some(s)) => (s, true),
            (None, None) => continue,
        };
        plans.push(Plan { preheader, phi, increment, step, narrow_bits, signed });
    }
}

/// `value` as a `to` value, before `before`; a constant folds.
fn cast(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    op: CastOp,
    value: Operand,
    to: TypeId,
    before: InstId,
) -> Operand {
    if let Operand::Constant(constant) = value
        && let Some(folded) = llrm_mir::constantfolding::constant_fold_cast_operand(context, layout, op, constant, to)
    {
        return Operand::Constant(folded);
    }
    let inst = function.create_instruction(Opcode::Cast(op), to, vec![value], Flags::default(), None);
    function.insert(inst, Position::Before(before)).expect("a placed instruction");
    Operand::Value(function.instruction(inst).result.expect("a cast has a value"))
}

/// The edit of one counter: a narrow phi and increment, and the old values
/// read through an extension of them.
fn narrow(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    plan: &Plan,
) {
    let wide = function.instruction(plan.phi).ty;
    let small = context.types.int(plan.narrow_bits);
    let opcode = function.instruction(plan.increment).opcode.clone();
    let extension = if plan.signed { CastOp::SExt } else { CastOp::ZExt };
    let result_of = |function: &Function, inst: InstId| function.instruction(inst).result.expect("a value");

    // The start and the step, truncated where the loop is entered.
    let entry = function.terminator(plan.preheader).expect("a terminated preheader");
    let (start, preheader_arm) = {
        let operands = &function.instruction(plan.phi).operands;
        let at = operands.iter().position(|&one| one == Operand::Block(plan.preheader)).expect("the preheader arm");
        (operands[at - 1], at)
    };
    let latch_arm = 4 - preheader_arm; // arms at 1 and 3: the other one
    let start = cast(context, layout, function, CastOp::Trunc, start, small, entry);
    let step = cast(context, layout, function, CastOp::Trunc, plan.step, small, entry);

    // The narrow phi, its latch input made once the increment exists.
    let latch = {
        let operands = &function.instruction(plan.phi).operands;
        match operands[latch_arm] {
            Operand::Block(block) => block,
            other => unreachable!("a phi's block operand, not {other:?}"),
        }
    };
    let phi = function.create_instruction(
        Opcode::Phi,
        small,
        vec![start, Operand::Block(plan.preheader), start, Operand::Block(latch)],
        Flags::default(),
        None,
    );
    function.insert(phi, Position::Before(plan.phi)).expect("a placed phi");
    let phi_value = Operand::Value(result_of(function, phi));
    let increment = function.create_instruction(opcode, small, vec![phi_value, step], Flags::default(), None);
    function.insert(increment, Position::Before(plan.increment)).expect("a placed increment");
    let increment_value = Operand::Value(result_of(function, increment));
    function.set_operand(phi, 2, increment_value);

    // The old values, read through the narrow ones.
    let header = function.parent(plan.phi).expect("placed");
    let first = crate::widen::first_insertion_point(function, header);
    let counter = cast(context, layout, function, extension, phi_value, wide, first);
    let advanced = cast(context, layout, function, extension, increment_value, wide, plan.increment);
    let (own, next) = (result_of(function, plan.phi), result_of(function, plan.increment));
    function.replace_all_uses_with(own, counter);
    function.replace_all_uses_with(next, advanced);
}

#[cfg(test)]
#[path = "narrowiv_tests.rs"]
mod tests;
