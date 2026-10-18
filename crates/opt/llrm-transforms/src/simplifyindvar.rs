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
//! Step 0 of [`crate::indvars`], on by default; `LLRM_NO_SCEV_IVCMP` turns it
//! off.
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

use llrm_analysis::num::Num as BigInt;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expr::Kind;
use llrm_analysis::scev::expr::Ty;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{CastOp, IntPredicate, Opcode};
use llrm_mir::passes::Outer;

use crate::counting;
use crate::widen::{Edit, WideIvInfo, WidenIv};

/// Read once; `LLRM_NO_SCEV_IVCMP` turns the step off.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_SCEV_IVCMP")
}

/// `LLRM_SCEV_WIDEN`, read once: `indvars` runs `simplifyAndExtend` (the IV
/// users simplified and the counters widened, as LLVM's IndVarSimplify), not
/// `simplified` and `indvars::widened`. Off by default until #2531 is fixed.
pub fn widening_enabled() -> bool {
    #[cfg(test)]
    if let Some(forced) = tests::FORCED.with(|one| one.get()) {
        return forced;
    }
    llrm_support::env_flag!("LLRM_SCEV_WIDEN")
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
            let mut done: IdSet<InstId> = IdSet::new();
            let loops = cx.loops.get_loops_in_preorder();
            for loop_ in loops.into_iter().rev() {
                let header = loop_.get_header();
                let phis: Vec<InstId> = cx
                    .function
                    .block(header)
                    .iter()
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
    done: &mut IdSet<InstId>,
) {
    // Instructions processed for CurrIV.
    let mut simplified: IdSet<InstId> = IdSet::new();
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
    simplified: &mut IdSet<InstId>,
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

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L703
/// `IndVarSimplify::simplifyAndExtend` over every loop of `function`, the
/// innermost first, with the IV widening (`WidenIV`) interleaved with the
/// simplification; whether anything changed. Instructions left dead are
/// `dead`'s.
///
/// Not translated of `SimplifyIndvar::simplifyUsers` (DEVIATE 17):
/// `replaceIVUserWithLoopInvariant`, `foldIVUser`,
/// `eliminateOverflowIntrinsic`, `eliminateSaturatingIntrinsic` (MIR lacks
/// them, D9), `eliminateTrunc`, `eliminateSDiv`, `simplifyIVRemainder`,
/// `eliminateIdentitySCEV`, `strengthenBinaryOp`, `replaceFloatIVWithIntegerIV`
/// and `makeIVComparisonInvariant`.
pub fn simplified_and_extended(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let headers: Vec<BlockId> = {
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        headers
    };
    // `Rewriter.disableCanonicalMode()`.
    let mut rewriter = llrm_analysis::scev::expander::Expander::new("indvars");
    rewriter.set_canonical_mode(false);
    let mut dead: Vec<InstId> = Vec::new();
    let mut e = Edit { context, layout, function, declared, outer, se, env, rewriter: &mut rewriter, dead: &mut dead };
    let has_guards = {
        let cx = e.env.cx(&*e.context, &*e.function);
        e.se.has_guards(&cx)
    };
    let mut changed = false;
    for header in headers {
        // `IndVarSimplify::run`: stay out of trouble without LoopSimplify
        // form.
        let simple = {
            let cx = e.env.cx(&*e.context, &*e.function);
            cx.loops
                .get_loop_for(header)
                .is_some_and(
                    |found| found.get_loop_preheader(cx.function).is_some()
                        && found.get_loop_latch(cx.function).is_some()
                        && found.has_dedicated_exits(cx.function),
                )
        };
        if simple {
            changed |= simplify_and_extend(&mut e, header, has_guards);
        }
    }
    changed
}

/// `simplifyAndExtend(L, Rewriter, LI)`.
fn simplify_and_extend(
    e: &mut Edit,
    header: BlockId,
    has_guards: bool,
) -> bool {
    let mut wide_ivs: Vec<WideIvInfo> = Vec::new();
    let mut loop_phis: Vec<InstId> = e
        .function
        .block(header)
        .iter()
        .take_while(|inst| e.function.instruction(*inst).opcode == Opcode::Phi)
        .collect();

    // Each round of simplification iterates through the SimplifyIVUsers
    // worklist for all current phis, then determines whether any IVs can be
    // widened. Widening adds new phis to LoopPhis, inducing another round of
    // simplification on the wide IVs.
    let mut changed = false;
    while !loop_phis.is_empty() {
        // Evaluate as many IV expressions as possible before widening any IVs.
        // This forces SCEV to set no-wrap flags before evaluating sign/zero
        // extension. The first time SCEV attempts to normalize sign/zero
        // extension, the result becomes final. So for the most predictable
        // results, we delay evaluation of sign/zero extend evaluation until
        // needed, and avoid running other SCEV based analysis prior to
        // simplifyAndExtend.
        while let Some(curr_iv) = loop_phis.pop() {
            // Information about sign/zero extensions of CurrIV.
            let mut info = WideIvInfo { narrow_iv: curr_iv, widest_native_type: None, is_signed: false };
            changed |= simplify_users_of_iv(e, header, curr_iv, &mut info);
            if info.widest_native_type.is_some() {
                wide_ivs.push(info);
            }
        }

        while let Some(info) = wide_ivs.pop() {
            let mut widener = WidenIv::new(e, &info, has_guards, true);
            if let Some(wide_phi) = widener.create_wide_iv(e) {
                changed = true;
                loop_phis.push(wide_phi);
            }
        }
    }
    changed
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L923
/// `simplifyUsersOfIV` with the visitor `IndVarSimplifyVisitor`: the users of
/// `curr_iv` simplified; whether any was.
fn simplify_users_of_iv(
    e: &mut Edit,
    header: BlockId,
    curr_iv: InstId,
    info: &mut WideIvInfo,
) -> bool {
    let Some(result) = e.function.instruction(curr_iv).result else { return false };
    {
        let cx = e.env.cx(&*e.context, &*e.function);
        if !e.se.is_value_scevable(&cx, Operand::Value(result)) {
            return false;
        }
    }
    let mut changed = false;
    // Instructions processed by SimplifyIndvar for CurrIV.
    let mut simplified: IdSet<InstId> = IdSet::new();
    // Use-def pairs if IV users waiting to be processed for CurrIV.
    let mut simple_iv_users: Vec<(InstId, InstId)> = Vec::new();

    // Push users of the current LoopPhi. In rare cases, pushIVUsers may be
    // called multiple times for the same LoopPhi. This is the proper thing to
    // do for loop header phis that use each other.
    push_iv_users_of(e, header, curr_iv, &mut simplified, &mut simple_iv_users);

    while let Some((use_inst, iv_operand)) = simple_iv_users.pop() {
        // If a user of the IndVar is trivially dead, we prefer just to mark it
        // dead rather than try to do some complex analysis or transformation
        // (such as widening) basing on it.
        if crate::local::is_instruction_trivially_dead(
            e.context,
            e.outer.callees(),
            &e.outer.globals,
            e.function,
            use_inst,
        ) {
            e.dead.push(use_inst);
            continue;
        }

        // Bypass back edges to avoid extra work.
        if use_inst == curr_iv {
            continue;
        }

        if eliminate_iv_user(e, header, use_inst, iv_operand, &mut changed) {
            push_iv_users_of(e, header, iv_operand, &mut simplified, &mut simple_iv_users);
            continue;
        }

        if let Opcode::Cast(_) = e.function.instruction(use_inst).opcode {
            visit_iv_cast(e, use_inst, info);
            continue;
        }
        if is_simple_iv_user_of(e, header, use_inst) {
            push_iv_users_of(e, header, use_inst, &mut simplified, &mut simple_iv_users);
        }
    }
    changed
}

/// `pushIVUsers` over the edit's function.
fn push_iv_users_of(
    e: &Edit,
    header: BlockId,
    def: InstId,
    simplified: &mut IdSet<InstId>,
    simple_iv_users: &mut Vec<(InstId, InstId)>,
) {
    let cx = e.env.cx(&*e.context, &*e.function);
    let found = cx.loops.get_loop_for(header).expect("the loop");
    push_iv_users(&cx, &found, def, simplified, simple_iv_users);
}

/// `isSimpleIVUser` over the edit's function.
fn is_simple_iv_user_of(
    e: &mut Edit,
    header: BlockId,
    inst: InstId,
) -> bool {
    let cx = e.env.cx(&*e.context, &*e.function);
    let found = cx.loops.get_loop_for(header).expect("the loop");
    is_simple_iv_user(e.se, &cx, &found, inst)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L861
/// `eliminateIVUser` for the comparison: eliminate an operation that consumes
/// a simple IV and has no observable side-effect given the range of IV values.
/// (The other eliminations are DEVIATE 17.)
fn eliminate_iv_user(
    e: &mut Edit,
    header: BlockId,
    use_inst: InstId,
    iv_operand: InstId,
    changed: &mut bool,
) -> bool {
    if let Opcode::ICmp(_) = e.function.instruction(use_inst).opcode {
        eliminate_iv_comparison_edit(e, header, use_inst, iv_operand, changed);
        return true;
    }
    false
}

/// `eliminateIVComparison`: replaces the comparison by what ScalarEvolution
/// decides, or gives it the `samesign` flag.
fn eliminate_iv_comparison_edit(
    e: &mut Edit,
    _header: BlockId,
    icmp: InstId,
    iv_operand: InstId,
    changed: &mut bool,
) {
    let (decided, same_sign) = {
        let cx = e.env.cx(&*e.context, &*e.function);
        let instruction = cx.function.instruction(icmp);
        let Opcode::ICmp(original_pred) = instruction.opcode else { unreachable!("a comparison") };
        let mut pred = original_pred;
        let Some(iv_result) = cx.function.instruction(iv_operand).result else { return };
        let iv_value = Operand::Value(iv_result);
        let mut iv_oper_idx = 0;
        if iv_value != instruction.operands[0] {
            // Swapped
            iv_oper_idx = 1;
            pred = pred.swapped();
        }
        let (iv_op, other) = (instruction.operands[iv_oper_idx], instruction.operands[1 - iv_oper_idx]);
        if !e.se.is_value_scevable(&cx, iv_op) || !e.se.is_value_scevable(&cx, other) {
            return;
        }
        // Get the SCEVs for the ICmp operands (in the specific context of the
        // current loop)
        let icmp_loop = cx.function.parent(icmp).and_then(|block| cx.loops.get_loop_for(block)).map(|l| l.get_header());
        let s = e.se.get_scev_at_scope_of_value(&cx, iv_op, icmp_loop).scev;
        let x = e.se.get_scev_at_scope_of_value(&cx, other, icmp_loop).scev;

        // If the condition is always true or always false in the given
        // context, replace it with a constant value.
        let Some(result) = instruction.result else { return };
        let users: Vec<InstId> = cx.function.users(result).iter().map(|used| used.user).collect();
        let decided = common_dominator(&cx, &users).and_then(|ctx| e.se.evaluate_predicate_at(&cx, pred, s, x, ctx));
        let flags = instruction.flags;
        let signed =
            matches!(
                original_pred,
                IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
            );
        let unsigned =
            matches!(
                original_pred,
                IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
            );
        let same_sign = decided.is_none()
            && (signed || (unsigned && !flags.contains(llrm_mir::opcode::Flags::SAMESIGN)))
            && e.se.have_same_sign(&cx, s, x);
        (decided.map(|value| (result, value)), same_sign.then_some(original_pred))
    };
    if let Some((result, value)) = decided {
        {
            let cx = e.env.cx(&*e.context, &*e.function);
            e.se.forget_value(&cx, Operand::Value(result));
        }
        let constant = counting::constant(e.context, &BigInt::from(u8::from(value)), 1);
        e.function.replace_all_uses_with(result, constant);
        e.dead.push(icmp);
        *changed = true;
        return;
    }
    // Set the samesign flag on the compare if legal, and canonicalize to the
    // unsigned variant (for signed compares) hoping that it will open the doors
    // for other optimizations.
    if let Some(original) = same_sign {
        let unsigned = match original {
            IntPredicate::Sgt => IntPredicate::Ugt,
            IntPredicate::Sge => IntPredicate::Uge,
            IntPredicate::Slt => IntPredicate::Ult,
            IntPredicate::Sle => IntPredicate::Ule,
            same => same,
        };
        e.function.set_icmp_predicate(icmp, unsigned);
        let mut flags = e.function.instruction(icmp).flags;
        flags.insert(llrm_mir::opcode::Flags::SAMESIGN);
        e.function.set_flags(icmp, flags);
        *changed = true;
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L618
/// `visitIVCast`: update the information about the induction variable that is
/// extended by this sign or zero extend operation. This is used to determine
/// the final width of the IV before actually widening it.
fn visit_iv_cast(
    e: &mut Edit,
    cast: InstId,
    wi: &mut WideIvInfo,
) {
    let instruction = e.function.instruction(cast);
    let is_signed = match instruction.opcode {
        Opcode::Cast(CastOp::SExt) => true,
        Opcode::Cast(CastOp::ZExt) => false,
        _ => return,
    };
    let (ty, operand) = (instruction.ty, instruction.operands[0]);
    let Some(width) = e.context.types.int_bits(ty) else { return };
    if !e.layout.legal_integer(width) {
        return;
    }

    // Check that `Cast` actually extends the induction variable (we rely on
    // this later). This takes care of cases where `Cast` is extending a
    // truncation of the narrow induction variable, and thus can end up being
    // narrower than the "narrow" induction variable.
    let narrow_ty = e.function.instruction(wi.narrow_iv).ty;
    let narrow_iv_width = e.context.types.int_bits(narrow_ty).expect("an integer counter");
    if narrow_iv_width >= width {
        return;
    }

    // Cast is either an sext or zext up to this point. We should not widen an
    // indvar if arithmetics on the wider indvar are more expensive than those
    // on the narrower indvar. We check only the cost of ADD because at least an
    // ADD is required to increment the induction variable. We could compute
    // more comprehensively the cost of all instructions on the induction
    // variable when necessary.
    // DEVIATE 18: a target that states no cost model (the 16-bit one) gets no
    // widening past the pointer's index width: its arithmetic on a wider type
    // costs more, and `n32` states `i32` legal there.
    if e.outer.target().tti().is_none() && width > e.layout.pointer(0).index_bits {
        return;
    }
    if let Some(tti) = e.outer.target().tti() {
        use llrm_mir::tti::{ArithOp, CostKind, OperandInfo, Ty as TtiTy};
        let source = match operand {
            Operand::Value(value) => e.function.value(value).ty,
            Operand::Constant(constant) => e.context.get(constant).ty,
            Operand::Block(_) => unreachable!("a block is not a value"),
        };
        let source_bits = e.context.types.int_bits(source).unwrap_or(width);
        let wide = tti.arithmetic_cost(
            ArithOp::Add,
            TtiTy::Int(width),
            CostKind::RecipThroughput,
            OperandInfo::ANY,
            OperandInfo::ANY,
        );
        let narrow = tti.arithmetic_cost(
            ArithOp::Add,
            TtiTy::Int(source_bits),
            CostKind::RecipThroughput,
            OperandInfo::ANY,
            OperandInfo::ANY,
        );
        if wide.value > narrow.value {
            return;
        }
    }

    let ty = llrm_analysis::scev::expr::Ty::Int(width);
    match wi.widest_native_type {
        Some(Ty::Int(held)) if width <= held => {
            // We extend the IV to satisfy the sign of its user(s), or 'signed'
            // if there are multiple users with both sign- and zero
            // extensions, in order not to introduce nondeterministic behaviour
            // based on the unspecified order of a PHI nodes' users-iterator.
            wi.is_signed |= is_signed;
        }
        _ => {
            wi.widest_native_type = Some(ty);
            wi.is_signed = is_signed;
        }
    }
}

#[cfg(test)]
#[path = "simplifyindvar_tests.rs"]
mod tests;

/// Forces the switch in this thread (a test of the new path).
#[cfg(test)]
pub(crate) fn tests_forced(on: bool) {
    tests::FORCED.with(|one| one.set(Some(on)));
}
