//! SimplifyIndVar's comparison elimination: a comparison with an induction
//! variable that ScalarEvolution proves true or false is that constant.
//!
//! # How it works
//! For each loop, innermost first, and each phi of its header that
//! ScalarEvolution models, `simplifyUsers` walks the users of the phi that are
//! in the loop: a user is queued once, with the value it was reached from; a
//! comparison is asked `evaluatePredicateAt` of the values of its two operands
//! at the comparison's own loop (`getSCEVAtScope`), at the common dominator of
//! its users; a user that ScalarEvolution models as a recurrence of the loop
//! is walked through in turn. A comparison with an answer has every use
//! replaced by it; `decide` and `dead` then do the rest.
//!
//! Not translated, each its own step: `foldIVUser`,
//! `replaceIVUserWithLoopInvariant`, `makeIVComparisonInvariant` (needs the
//! expander), the `samesign` canonicalisation, `eliminateSDiv`,
//! `simplifyIVRemainder`, `strengthenBinaryOp`, `eliminateTrunc` and the
//! float-induction replacement.
//!
//! ```text
//!  loop (n <= 100):  %i = phi [0, ...], [%i.next, ...]
//!                    %big = icmp ult i16 %i, 200      ==> true
//!                    br i1 %big, ...                      br i1 true, ...
//! ```
//!
//! # Example
//! Before, `n <= 100` is checked on entry:
//!
//! ```mir
//! body:
//!   %i = phi i16 [ 0, %entry ], [ %next, %body ]
//!   %big = icmp ult i16 %i, 200
//!   br i1 %big, label %ok, label %crash
//! ```
//!
//! After, `%big` is `true` wherever it is read.
//!
//! From unit test `test_a_compare_the_trip_count_decides_is_a_constant` in
//! `simplifyindvar_tests.rs`.
//!
//! # Runs
//! Step 0 of [`crate::indvars`], with `LLRM_EXP_SCEV_IVCMP`; off by default.
//!
//! # Needs
//! `llrm_analysis::scev` (`ScevSession`): `evaluatePredicateAt`,
//! `getSCEVAtScope`, trip counts and guards.
//!
//! # Keeps
//! Blocks and edges are as they were: only the uses of compares change.
//!
//! # See also
//! [`crate::exitfold`], [`crate::decide`]; LLVM SimplifyIndVar
//! (`simplifyUsers`, `eliminateIVComparison`).

use std::collections::HashSet;

use llrm_analysis::num::Num as BigInt;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expr::Kind;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Outer;

use crate::counting;

/// `LLRM_EXP_SCEV_IVCMP`, read once.
pub fn enabled() -> bool {
    llrm_support::env_flag!("LLRM_EXP_SCEV_IVCMP")
}

/// Every comparison with an induction variable that ScalarEvolution decides,
/// replaced by what it decides; whether any was.
pub fn simplified(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let decided = {
        let mut session = ScevSession::of(context, function, outer);
        session.ask(context, layout, function, outer, |scev, cx| {
            let mut decided: Vec<(InstId, bool)> = Vec::new();
            let mut done: HashSet<InstId> = HashSet::new();
            let loops = cx.loops.get_loops_in_preorder();
            for loop_ in loops.into_iter().rev() {
                let header = loop_.get_header();
                let phis: Vec<InstId> = cx
                    .function
                    .block(header)
                    .instructions()
                    .iter()
                    .copied()
                    .take_while(|inst| cx.function.instruction(*inst).opcode == Opcode::Phi)
                    .collect();
                for phi in phis {
                    let Some(result) = cx.function.instruction(phi).result else { continue };
                    if !scev.is_value_scevable(cx, Operand::Value(result)) {
                        continue;
                    }
                    simplify_users(scev, cx, &loop_, phi, &mut decided, &mut done);
                }
            }
            decided
        })
    };
    let mut changed = false;
    for (compare, value) in decided {
        let Some(result) = function.instruction(compare).result else { continue };
        let constant = counting::constant(context, &BigInt::from(u8::from(value)), 1);
        function.replace_all_uses_with(result, constant);
        changed = true;
    }
    changed
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L923
/// `SimplifyIndvar::simplifyUsers` for the comparison elimination.
fn simplify_users(
    scev: &mut llrm_analysis::scev::ScalarEvolution,
    cx: &llrm_analysis::scev::Cx,
    loop_: &llrm_mir::loopinfo::Loop,
    curr_iv: InstId,
    decided: &mut Vec<(InstId, bool)>,
    done: &mut HashSet<InstId>,
) {
    // Instructions processed for CurrIV.
    let mut simplified: HashSet<InstId> = HashSet::new();
    // Use-def pairs of IV users waiting to be processed for CurrIV.
    let mut simple_iv_users: Vec<(InstId, InstId)> = Vec::new();

    push_iv_users(cx, loop_, curr_iv, &mut simplified, &mut simple_iv_users);

    while let Some((use_inst, iv_operand)) = simple_iv_users.pop() {
        let instruction = cx.function.instruction(use_inst);
        // If a user of the IndVar is trivially dead, we prefer just to mark it
        // dead rather than try to do some complex analysis or transformation.
        if instruction.result.is_none_or(|result| cx.function.users(result).is_empty())
            && matches!(
                instruction.opcode,
                Opcode::ICmp(_) | Opcode::Binary(_) | Opcode::Cast(_) | Opcode::Select
            )
        {
            continue;
        }

        // Bypass back edges to avoid extra work.
        if use_inst == curr_iv {
            continue;
        }

        if let Opcode::ICmp(_) = instruction.opcode {
            if done.insert(use_inst)
                && let Some(value) = eliminate_iv_comparison(scev, cx, use_inst, iv_operand)
            {
                decided.push((use_inst, value));
            }
            push_iv_users(cx, loop_, iv_operand, &mut simplified, &mut simple_iv_users);
            continue;
        }

        if is_simple_iv_user(scev, cx, loop_, use_inst) {
            push_iv_users(cx, loop_, use_inst, &mut simplified, &mut simple_iv_users);
        }
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1003
/// `pushIVUsers`: the users of `def` in the loop not queued yet.
fn push_iv_users(
    cx: &llrm_analysis::scev::Cx,
    loop_: &llrm_mir::loopinfo::Loop,
    def: InstId,
    simplified: &mut HashSet<InstId>,
    simple_iv_users: &mut Vec<(InstId, InstId)>,
) {
    let Some(result) = cx.function.instruction(def).result else { return };
    for used in cx.function.users(result) {
        let ui = used.user;
        // Avoid infinite or exponential worklist processing. Also ensure
        // unique worklist users. If Def is a LoopPhi, it may not be in the
        // Simplified set, so check for self edges first.
        if ui == def {
            continue;
        }

        // Only change the current Loop, do not change the other parts (e.g.
        // other Loops).
        if !loop_.contains_instruction(cx.function, ui) {
            continue;
        }

        // Do not push the same instruction more than once.
        if !simplified.insert(ui) {
            continue;
        }

        simple_iv_users.push((ui, def));
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1032
/// `isSimpleIVUser`: an instruction ScalarEvolution models as a recurrence of
/// the loop.
fn is_simple_iv_user(
    scev: &mut llrm_analysis::scev::ScalarEvolution,
    cx: &llrm_analysis::scev::Cx,
    loop_: &llrm_mir::loopinfo::Loop,
    inst: InstId,
) -> bool {
    let Some(result) = cx.function.instruction(inst).result else { return false };
    let value = Operand::Value(result);
    if !scev.is_value_scevable(cx, value) {
        return false;
    }

    // Get the symbolic expression for this instruction.
    let s = scev.get_scev(cx, value);

    // Only consider affine recurrences.
    matches!(
        scev.node(s).kind,
        Kind::AddRec(_, header) if header == loop_.get_header()
    )
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L251
/// `eliminateIVComparison`: the value of the comparison `icmp` where
/// ScalarEvolution knows it.
fn eliminate_iv_comparison(
    scev: &mut llrm_analysis::scev::ScalarEvolution,
    cx: &llrm_analysis::scev::Cx,
    icmp: InstId,
    iv_operand: InstId,
) -> Option<bool> {
    let instruction = cx.function.instruction(icmp);
    let Opcode::ICmp(mut pred) = instruction.opcode else { unreachable!("a comparison") };
    let iv_value = Operand::Value(cx.function.instruction(iv_operand).result?);
    let mut iv_oper_idx = 0;
    if iv_value != instruction.operands[0] {
        // Swapped
        debug_assert!(iv_value == instruction.operands[1], "Can't find IVOperand");
        iv_oper_idx = 1;
        pred = pred.swapped();
    }

    // Get the SCEVs for the ICmp operands (in the specific context of the
    // current loop)
    let icmp_block = cx.function.parent(icmp)?;
    let icmp_loop = cx.loops.get_loop_for(icmp_block).map(|one| one.get_header());
    let (iv_op, other) = (instruction.operands[iv_oper_idx], instruction.operands[1 - iv_oper_idx]);
    if !scev.is_value_scevable(cx, iv_op) || !scev.is_value_scevable(cx, other) {
        return None;
    }
    let s = scev.get_scev_at_scope_of_value(cx, iv_op, icmp_loop).scev;
    let x = scev.get_scev_at_scope_of_value(cx, other, icmp_loop).scev;

    // If the condition is always true or always false in the given context,
    // replace it with a constant value.
    let result = instruction.result?;
    let users: Vec<InstId> = cx.function.users(result).iter().map(|used| used.user).collect();
    let ctx_block = common_dominator(cx, &users)?;
    scev.evaluate_predicate_at(cx, pred, s, x, ctx_block)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L118
/// `findCommonDominator`: the block of the nearest common dominator of the
/// instructions (only its block is read, by `evaluatePredicateAt`).
fn common_dominator(
    cx: &llrm_analysis::scev::Cx,
    instructions: &[InstId],
) -> Option<BlockId> {
    let mut common: Option<BlockId> = None;
    for &inst in instructions {
        let block = cx.function.parent(inst)?;
        common = Some(match common {
            None => block,
            Some(one) => nearest_common_dominator(cx, one, block),
        });
    }
    common
}

/// `DominatorTree::findNearestCommonDominator(BB1, BB2)`.
fn nearest_common_dominator(
    cx: &llrm_analysis::scev::Cx,
    a: BlockId,
    b: BlockId,
) -> BlockId {
    if !cx.tree.is_reachable(b) {
        return a;
    }
    if !cx.tree.is_reachable(a) {
        return b;
    }
    let mut at = a;
    while !cx.tree.dominates(at, b) {
        match cx.tree.immediate_dominator(at) {
            Some(up) => at = up,
            None => break,
        }
    }
    at
}

#[cfg(test)]
#[path = "simplifyindvar_tests.rs"]
mod tests;
