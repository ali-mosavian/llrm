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
//! `replaceIVUserWithLoopInvariant`, the `samesign` canonicalisation,
//! `eliminateSDiv`, `simplifyIVRemainder`, `strengthenBinaryOp`,
//! `eliminateTrunc` and the float-induction replacement.
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
//! Step 0 of [`crate::indvars`], with the widening.
//!
//! # Needs
//! `llrm_analysis::scev` (`ScevSession`): `evaluatePredicateAt`,
//! `getSCEVAtScope`, trip counts and guards.
//!
//! # Keeps
//! Blocks and edges are as they were: only the uses of compares change.
//!
//! # See also
//! [`crate::decide`]; LLVM SimplifyIndVar
//! (`simplifyUsers`, `eliminateIVComparison`).

use llrm_analysis::num::Num as BigInt;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expr::Kind;
use llrm_analysis::scev::expr::Ty;
use llrm_analysis::scev::expr::Use;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_mir::passes::Outer;

use crate::counting;
use crate::widen::{Edit, WideIvInfo, WidenIv};

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
/// `simplifyLoopIVs(L, ...)` (`SimplifyIndVar.cpp:2020`) for the loop headed by
/// `header`: the users of each of its header phis simplified, without widening;
/// whether any was. What the unroller runs after a peel, where the residual
/// loop's steps and exit test get the no-wrap flags SCEV proves.
pub fn simplified_loop_ivs(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    outer: &Outer,
    header: BlockId,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let mut rewriter = llrm_analysis::scev::expander::Expander::new("indvars");
    rewriter.set_canonical_mode(false);
    let mut dead: Vec<InstId> = Vec::new();
    let mut e = Edit { context, layout, function, declared, outer, se, env, rewriter: &mut rewriter, dead: &mut dead };
    let phis: Vec<InstId> =
        e.function.iter_block(header).take_while(|inst| e.function.instruction(*inst).opcode == Opcode::Phi).collect();
    let mut changed = false;
    for phi in phis {
        // `simplifyLoopIVs` passes no visitor: a cast of the IV is looked
        // through.
        changed |= simplify_users_of_iv(&mut e, header, phi, None);
    }
    changed
}

/// `IndVarSimplify::simplifyAndExtend` over every loop of `function`, the
/// innermost first, with the IV widening (`WidenIV`) interleaved with the
/// simplification; whether anything changed. Instructions left dead are
/// `dead`'s.
///
/// Not translated of `SimplifyIndvar::simplifyUsers` (DEVIATE 17):
/// `replaceIVUserWithLoopInvariant`, `replaceFloatIVWithIntegerIV`, and the
/// overflow and saturating intrinsic eliminations (MIR lacks them, D9).
pub fn simplified_and_extended(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    simplified_and_extended_in(&mut session, context, layout, function, declared, outer)
}

/// [`simplified_and_extended`] over `session`, whose expressions are valid for
/// `function`.
pub fn simplified_and_extended_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    outer: &Outer,
) -> bool {
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let headers: Vec<BlockId> = {
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        crate::loopfilter::retain(&mut headers);
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
            cx.loops.get_loop_for(header).is_some_and(|found| found.is_loop_simplify_form(cx.function))
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
    let mut loop_phis: Vec<InstId> =
        e.function.iter_block(header).take_while(|inst| e.function.instruction(*inst).opcode == Opcode::Phi).collect();

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
            changed |= simplify_users_of_iv(e, header, curr_iv, Some(&mut info));
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
    mut info: Option<&mut WideIvInfo>,
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

        // `foldIVUser`: fold an IV operand into its use, while it folds.
        let mut iv_operand = Some(iv_operand);
        while let Some(current) = iv_operand {
            let Some(folded) = fold_iv_user(e, use_inst, current, &mut changed) else { break };
            iv_operand = crate::widen::defining(e.function, folded);
        }
        let Some(iv_operand) = iv_operand else { continue };

        if eliminate_iv_user(e, header, use_inst, iv_operand, &mut changed) {
            push_iv_users_of(e, header, iv_operand, &mut simplified, &mut simple_iv_users);
            continue;
        }

        if matches!(e.function.instruction(use_inst).opcode, Opcode::Binary(_))
            && strengthen_binary_op(e, use_inst, iv_operand)
        {
            // re-queue uses of the now modified binary operator and fall
            // through to the checks that remain.
            changed = true;
            push_iv_users_of(e, header, iv_operand, &mut simplified, &mut simple_iv_users);
        }

        if let Opcode::Cast(_) = e.function.instruction(use_inst).opcode
            && let Some(info) = info.as_deref_mut()
        {
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
/// `eliminateIVUser`: eliminate an operation that consumes a simple IV and has
/// no observable side-effect given the range of IV values. (The overflow and
/// saturating intrinsics are MIR's absent ones, D9.)
fn eliminate_iv_user(
    e: &mut Edit,
    header: BlockId,
    use_inst: InstId,
    iv_operand: InstId,
    changed: &mut bool,
) -> bool {
    match e.function.instruction(use_inst).opcode {
        Opcode::ICmp(_) => {
            eliminate_iv_comparison_edit(e, header, use_inst, iv_operand, changed);
            return true;
        }
        Opcode::Binary(op @ (BinaryOp::SRem | BinaryOp::URem)) => {
            simplify_iv_remainder(e, header, use_inst, iv_operand, op == BinaryOp::SRem, changed);
            return true;
        }
        Opcode::Binary(BinaryOp::SDiv) => return eliminate_sdiv(e, header, use_inst, changed),
        Opcode::Cast(CastOp::Trunc) => {
            if eliminate_trunc(e, header, use_inst, changed) {
                return true;
            }
        }
        _ => {}
    }
    eliminate_identity_scev(e, use_inst, iv_operand, changed)
}

/// `eliminateIVComparison`: replaces the comparison by what ScalarEvolution
/// decides, or gives it the `samesign` flag.
fn eliminate_iv_comparison_edit(
    e: &mut Edit,
    header: BlockId,
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
    if make_iv_comparison_invariant(e, header, icmp, iv_operand) {
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

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L204
/// `makeIVComparisonInvariant`: a comparison of an IV that is the same on every
/// iteration but the first (or the last) is rewritten to compare loop-invariant
/// values, computed in the preheader. Whether it was.
fn make_iv_comparison_invariant(
    e: &mut Edit,
    header: BlockId,
    icmp: InstId,
    iv_operand: InstId,
) -> bool {
    use llrm_analysis::scev::expand_cost::MachineCosts;
    let (invariant, preheader_term) = {
        let cx = e.env.cx(&*e.context, &*e.function);
        let Some(l) = cx.loops.get_loop_for(header) else { return false };
        let Some(preheader) = l.get_loop_preheader(cx.function) else { return false };
        let instruction = cx.function.instruction(icmp);
        let Opcode::ICmp(mut pred) = instruction.opcode else { return false };
        let Some(iv_result) = cx.function.instruction(iv_operand).result else { return false };
        let mut iv_oper_idx = 0;
        if Operand::Value(iv_result) != instruction.operands[0] {
            // Swapped
            iv_oper_idx = 1;
            pred = pred.swapped();
        }
        // The SCEVs of the compare's operands, in the context of the current
        // loop.
        let icmp_loop =
            cx.function.parent(icmp).and_then(|block| cx.loops.get_loop_for(block)).map(|one| one.get_header());
        let s = e.se.get_scev_at_scope_of_value(&cx, instruction.operands[iv_oper_idx], icmp_loop).scev;
        let x = e.se.get_scev_at_scope_of_value(&cx, instruction.operands[1 - iv_oper_idx], icmp_loop).scev;
        let Some(found) = e.se.get_loop_invariant_predicate(&cx, pred, s, x, header, Some(icmp)) else {
            return false;
        };
        // Do not generate something ridiculous.
        let Some(term) = cx.function.terminator(preheader) else { return false };
        let tti = MachineCosts { tti: e.outer.target().tti(), minimum_size: false };
        let expander = llrm_analysis::scev::expander::Expander::new("indvars");
        if expander.is_high_cost_expansion(&cx, e.se, &[found.lhs, found.rhs], header, 2 * 4, &tti, term)
            || !expander.is_safe_to_expand_at(&cx, e.se, found.lhs, term)
            || !expander.is_safe_to_expand_at(&cx, e.se, found.rhs, term)
        {
            return false;
        }
        (found, term)
    };
    e.rewriter.set_insert_point(preheader_term);
    let scev_ty = { e.se.type_of(invariant.lhs) };
    let new_lhs =
        e.rewriter.expand_code_for(&e.env, e.context, e.declared, e.function, e.se, invariant.lhs, Some(scev_ty));
    let new_rhs =
        e.rewriter.expand_code_for(&e.env, e.context, e.declared, e.function, e.se, invariant.rhs, Some(scev_ty));
    e.function.set_icmp_predicate(icmp, invariant.pred);
    e.function.set_operand(icmp, 0, new_lhs);
    e.function.set_operand(icmp, 1, new_rhs);
    true
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L145
/// `foldIVUser`: fold an IV operand into its use. This removes increments of
/// an aligned IV when used by a instruction that ignores the low bits. The
/// operand of `iv_operand` for this induction variable if it can be folded (in
/// case more folding opportunities have been exposed), else none.
fn fold_iv_user(
    e: &mut Edit,
    use_inst: InstId,
    iv_operand: InstId,
    changed: &mut bool,
) -> Option<Operand> {
    use crate::widen::{defining, operand_type, result_of, value_op};
    let instruction = e.function.instruction(use_inst);
    let (opcode, use_operands, use_ty, exact) = match instruction.opcode {
        Opcode::Binary(op @ (BinaryOp::UDiv | BinaryOp::LShr)) => (
            op,
            instruction.operands.clone(),
            instruction.ty,
            instruction.flags.contains(llrm_mir::opcode::Flags::EXACT),
        ),
        _ => return None,
    };
    // We're only interested in the case where we know something about the
    // numerator and have a constant denominator.
    const OPER_IDX: usize = 0;
    if value_op(e, iv_operand) != use_operands[OPER_IDX] {
        return None;
    }
    let Operand::Constant(divisor) = use_operands[1] else { return None };
    let llrm_mir::context::ConstantKind::Int(mut d) = e.context.get(divisor).kind else { return None };

    // Attempt to fold a binary operator with constant operand.
    // e.g. ((I + 1) >> 2) => I >> 2
    let iv = e.function.instruction(iv_operand);
    if !matches!(iv.opcode, Opcode::Binary(_)) || !matches!(iv.operands[1], Operand::Constant(_)) {
        return None;
    }
    // IVSrc must be the (SCEVable) IV, since the other operand is const.
    let iv_src = iv.operands[0];

    if opcode == BinaryOp::LShr {
        // Get a constant for the divisor. See createSCEV.
        let bit_width = e.context.types.int_bits(use_ty)?;
        if d >= u128::from(bit_width) {
            return None;
        }
        d = 1u128 << d;
    }
    let bits = e.context.types.int_bits(use_ty)?;
    let (folded, must_drop_exact) = {
        let cx = e.env.cx(&*e.context, &*e.function);
        let lhs = e.se.get_scev(&cx, iv_src);
        let rhs = e.se.get_constant(llrm_analysis::scev::ap::Ap::new(bits, d));
        let folded = e.se.get_udiv_expr(&cx, Use::of(lhs), Use::of(rhs));
        // We might have 'exact' flag set at this point which will no longer be
        // correct after we make the replacement.
        let mut must_drop_exact = false;
        if exact {
            let product =
                e.se.get_mul_expr(&cx, vec![Use::of(folded), Use::of(rhs)], llrm_analysis::scev::Flags::default(), 0)
                    .scev;
            must_drop_exact = lhs != product;
        }
        // We have something that might fold it's operand. Compare SCEVs.
        let use_op = value_op(e, use_inst);
        if !e.se.is_value_scevable(&cx, use_op) {
            return None;
        }
        // Bypass the operand if SCEV can prove it has no effect.
        if e.se.get_scev(&cx, use_op) != folded {
            return None;
        }
        (folded, must_drop_exact)
    };
    let _ = folded;

    e.function.set_operand(use_inst, OPER_IDX, iv_src);
    if must_drop_exact {
        crate::local::drop_poison_generating_flags(e.context, e.function, use_inst);
    }
    *changed = true;
    if e.function.users(result_of(e.function, iv_operand)).is_empty() {
        e.dead.push(iv_operand);
    }
    let _ = (defining, operand_type);
    Some(iv_src)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L632
/// `eliminateIdentitySCEV`: eliminate any operation that SCEV can prove is an
/// identity function.
fn eliminate_identity_scev(
    e: &mut Edit,
    use_inst: InstId,
    iv_operand: InstId,
    changed: &mut bool,
) -> bool {
    use crate::widen::{result_of, value_op};
    let Some(use_result) = e.function.instruction(use_inst).result else { return false };
    let use_op = Operand::Value(use_result);
    let iv_op = value_op(e, iv_operand);
    let (use_ty, iv_ty) = (e.function.instruction(use_inst).ty, e.function.instruction(iv_operand).ty);
    {
        let cx = e.env.cx(&*e.context, &*e.function);
        if !e.se.is_value_scevable(&cx, use_op) || use_ty != iv_ty {
            return false;
        }
        let use_scev = e.se.get_scev(&cx, use_op);
        if use_scev != e.se.get_scev(&cx, iv_op) {
            return false;
        }
        // getSCEV(X) == getSCEV(Y) does not guarantee that X and Y are related
        // in the dominator tree, even if X is an operand to Y. For instance,
        // `%M = phi (%X, %iv)` where `%X = add %iv, 0` is on one arm only:
        // `%M.replaceAllUsesWith(%X)` would be incorrect.
        if cx.function.instruction(use_inst).opcode == Opcode::Phi
            && !cx.tree.instruction_dominates(cx.function, iv_operand, use_inst)
        {
            return false;
        }
        if !crate::exitvalues::replacement_preserves_lcssa_form(&cx, use_inst, iv_op) {
            return false;
        }
        // Make sure the operand is not more poisonous than the instruction.
        // (DEVIATE 19: `canReuseInstruction` is not translated, so an operand
        // that does not imply the poison of the use is not reused.)
        let query = cx.query(None);
        if !llrm_mir::valuetracking::guarantee::implies_poison(&query, iv_op, use_op) {
            return false;
        }
        e.se.forget_value(&cx, use_op);
    }
    e.function.replace_all_uses_with(result_of(e.function, use_inst), iv_op);
    *changed = true;
    e.dead.push(use_inst);
    true
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L303
/// `eliminateSDiv`: replace `sdiv` by `udiv` if both of the operands are
/// non-negative.
fn eliminate_sdiv(
    e: &mut Edit,
    _header: BlockId,
    sdiv: InstId,
    changed: &mut bool,
) -> bool {
    use crate::widen::{result_of, value_op};
    let operands = e.function.instruction(sdiv).operands.clone();
    let (ty, flags) = (e.function.instruction(sdiv).ty, e.function.instruction(sdiv).flags);
    let non_negative = {
        let cx = e.env.cx(&*e.context, &*e.function);
        // Get the SCEVs for the ICmp operands.
        let n = e.se.get_scev(&cx, operands[0]);
        let d = e.se.get_scev(&cx, operands[1]);
        // Simplify unnecessary loops away.
        let l = cx.function.parent(sdiv).and_then(|block| cx.loops.get_loop_for(block)).map(|one| one.get_header());
        let n = e.se.get_scev_at_scope(&cx, n, l);
        let d = e.se.get_scev_at_scope(&cx, d, l);
        e.se.is_known_non_negative(&cx, n) && e.se.is_known_non_negative(&cx, d)
    };
    // Replace sdiv by udiv if both of the operands are non-negative
    if !non_negative {
        return false;
    }
    let mut udiv_flags = llrm_mir::opcode::Flags::default();
    if flags.contains(llrm_mir::opcode::Flags::EXACT) {
        udiv_flags.insert(llrm_mir::opcode::Flags::EXACT);
    }
    let udiv = e.function.create_instruction(Opcode::Binary(BinaryOp::UDiv), ty, operands, udiv_flags, None);
    e.function.insert(udiv, llrm_mir::edit::Position::Before(sdiv)).expect("a placed instruction");
    let udiv_op = value_op(e, udiv);
    e.function.replace_all_uses_with(result_of(e.function, sdiv), udiv_op);
    *changed = true;
    e.dead.push(sdiv);
    true
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L385
/// `simplifyIVRemainder`: eliminate useless remainder operations operating on
/// an induction variable or replace `srem` by `urem`.
fn simplify_iv_remainder(
    e: &mut Edit,
    _header: BlockId,
    rem: InstId,
    iv_operand: InstId,
    is_signed: bool,
    changed: &mut bool,
) {
    use crate::widen::{result_of, value_op};
    let operands = e.function.instruction(rem).operands.clone();
    let (n_value, d_value) = (operands[0], operands[1]);
    // We're only interested in the case where we know something about the
    // numerator, unless it is a srem, because we want to replace srem by urem
    // in general.
    let used_as_numerator = value_op(e, iv_operand) == n_value;
    if !used_as_numerator && !is_signed {
        return;
    }
    // 0: leave, 1: the numerator, 2: the numerator or zero, 3: srem to urem.
    let action = {
        let cx = e.env.cx(&*e.context, &*e.function);
        let n = e.se.get_scev(&cx, n_value);
        // Simplify unnecessary loops away.
        let rem_loop =
            cx.function.parent(rem).and_then(|block| cx.loops.get_loop_for(block)).map(|one| one.get_header());
        let n = e.se.get_scev_at_scope(&cx, n, rem_loop);
        let is_numerator_non_negative = !is_signed || e.se.is_known_non_negative(&cx, n);
        // Do not proceed if the Numerator may be negative
        if !is_numerator_non_negative {
            return;
        }
        let d = e.se.get_scev(&cx, d_value);
        let d = e.se.get_scev_at_scope(&cx, d, rem_loop);

        let mut action = 0;
        if used_as_numerator {
            let lt = if is_signed { IntPredicate::Slt } else { IntPredicate::Ult };
            if e.se.is_known_predicate_scev(&cx, lt, n, d) {
                action = 1;
            } else {
                let ty = e.se.type_of(n);
                let one = e.se.get_one(&cx, ty);
                let n_less_one = e.se.get_minus_scev(&cx, Use::of(n), Use::of(one), 0, 0);
                if e.se.is_known_predicate_scev(&cx, lt, n_less_one, d) {
                    action = 2;
                }
            }
        }
        // Try to replace SRem with URem, if both N and D are known
        // non-negative. Since we had already check N, we only need to check D
        // now
        if action == 0 && is_signed && e.se.is_known_non_negative(&cx, d) {
            action = 3;
        }
        action
    };
    let result = result_of(e.function, rem);
    let ty = e.function.instruction(rem).ty;
    match action {
        // i % n  -->  i  if i is in [0,n).
        1 => {
            e.function.replace_all_uses_with(result, n_value);
        }
        // (i+1) % n  -->  (i+1)==n?0:(i+1)  if i is in [0,n).
        2 => {
            let bit = e.context.types.int(1);
            let icmp = e
                .function
                .create_instruction(
                    Opcode::ICmp(IntPredicate::Eq),
                    bit,
                    vec![n_value, d_value],
                    llrm_mir::opcode::Flags::default(),
                    None,
                );
            e.function.insert(icmp, llrm_mir::edit::Position::Before(rem)).expect("a placed instruction");
            let zero = Operand::Constant(e.context.int(ty, 0));
            let icmp_op = value_op(e, icmp);
            let select = e
                .function
                .create_instruction(
                    Opcode::Select,
                    ty,
                    vec![icmp_op, zero, n_value],
                    llrm_mir::opcode::Flags::default(),
                    Some("iv.rem"),
                );
            e.function.insert(select, llrm_mir::edit::Position::Before(rem)).expect("a placed instruction");
            let select_op = value_op(e, select);
            e.function.replace_all_uses_with(result, select_op);
        }
        // i %s n -> i %u n if i >= 0 and n >= 0
        3 => {
            let urem = e
                .function
                .create_instruction(
                    Opcode::Binary(BinaryOp::URem),
                    ty,
                    vec![n_value, d_value],
                    llrm_mir::opcode::Flags::default(),
                    None,
                );
            e.function.insert(urem, llrm_mir::edit::Position::Before(rem)).expect("a placed instruction");
            let urem_op = value_op(e, urem);
            e.function.replace_all_uses_with(result, urem_op);
        }
        _ => return,
    }
    *changed = true;
    e.dead.push(rem);
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L525
/// `eliminateTrunc`: replace the comparisons of a `trunc` of an IV with
/// comparisons of the IV when the truncation collapses (`iv ==
/// sext(trunc(iv))` or `zext`).
fn eliminate_trunc(
    e: &mut Edit,
    header: BlockId,
    trunc: InstId,
    changed: &mut bool,
) -> bool {
    use llrm_mir::opcode::Flags as IrFlags;

    use crate::widen::{build_cast, operand_type, result_of, value_op};
    // It is always legal to replace
    //   icmp <pred> i32 trunc(iv), n
    // with
    //   icmp <pred> i64 sext(trunc(iv)), sext(n), if pred is signed predicate.
    // Or with
    //   icmp <pred> i64 zext(trunc(iv)), zext(n), if pred is unsigned
    // predicate. Or with either of these if pred is an equality predicate.
    //
    // If we can prove that iv == sext(trunc(iv)) or iv == zext(trunc(iv)) for
    // every comparison which uses trunc, it means that we can replace each of
    // them with comparison of iv against sext/zext(n). We no longer need trunc
    // after that.
    let iv = e.function.instruction(trunc).operands[0];
    let iv_ty = operand_type(e, iv);
    let trunc_op = value_op(e, trunc);
    let (does_sext_collapse, does_zext_collapse) = {
        let cx = e.env.cx(&*e.context, &*e.function);
        if !e.se.is_value_scevable(&cx, iv) {
            return false;
        }
        let iv_scev = e.se.get_scev(&cx, iv);
        let ti_scev = e.se.get_scev(&cx, trunc_op);
        let ty = e.se.type_of(iv_scev);
        // Check if iv == zext(trunc(iv)) and if iv == sext(trunc(iv)). If so,
        // we can get rid of trunc
        (
            iv_scev == e.se.get_sign_extend_expr(&cx, Use::of(ti_scev), ty, 0),
            iv_scev == e.se.get_zero_extend_expr(&cx, Use::of(ti_scev), ty, 0),
        )
    };
    // If neither sext nor zext does collapse, it is not profitable to do any
    // transform. Bail.
    if !does_sext_collapse && !does_zext_collapse {
        return false;
    }

    // Collect users of the trunc that look like comparisons against
    // invariants. Bail if we find something different.
    let mut icmp_users: Vec<InstId> = Vec::new();
    let users: Vec<InstId> = e.function.users(result_of(e.function, trunc)).iter().map(|used| used.user).collect();
    {
        let cx = e.env.cx(&*e.context, &*e.function);
        let found = cx.loops.get_loop_for(header).expect("the loop");
        for user in users {
            // We don't care about users in unreachable blocks.
            if cx.function.parent(user).is_some_and(|block| !cx.tree.is_reachable_from_entry(block)) {
                continue;
            }
            let instruction = cx.function.instruction(user);
            let Opcode::ICmp(pred) = instruction.opcode else { return false };
            let (op0, op1) = (instruction.operands[0], instruction.operands[1]);
            if !(op0 == trunc_op && found.is_loop_invariant(cx.function, op1))
                && !(op1 == trunc_op && found.is_loop_invariant(cx.function, op0))
            {
                return false;
            }
            let signed = matches!(
                pred,
                IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
            );
            let unsigned =
                matches!(
                    pred,
                    IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
                );
            // If we cannot get rid of trunc, bail.
            if signed && !does_sext_collapse {
                return false;
            }
            if unsigned && !does_zext_collapse {
                return false;
            }
            // For equality, either signed or unsigned works.
            icmp_users.push(user);
        }
    }

    // Replace all comparisons against trunc with comparisons against IV.
    for ici in icmp_users {
        let (pred, operands) = {
            let instruction = e.function.instruction(ici);
            let Opcode::ICmp(pred) = instruction.opcode else { unreachable!("a compare") };
            (pred, instruction.operands.clone())
        };
        let (is_swapped, op1) = {
            let cx = e.env.cx(&*e.context, &*e.function);
            let found = cx.loops.get_loop_for(header).expect("the loop");
            let swapped = found.is_loop_invariant(cx.function, operands[0]);
            (swapped, if swapped { operands[0] } else { operands[1] })
        };
        // For signed/unsigned predicate, replace the old comparison with
        // comparison of immediate IV against sext/zext of the invariant
        // argument. If we can use either sext or zext (i.e. we are dealing
        // with equality predicate), then prefer zext as a more canonical form.
        let mut pred = if is_swapped { pred.swapped() } else { pred };
        let unsigned_pred =
            matches!(
                pred,
                IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
            );
        let can_use_zext = unsigned_pred
            || (does_zext_collapse && {
                if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
                    true
                } else {
                    // Otherwise we can only use zext when comparing two
                    // non-negative or two negative values. But in practice, we
                    // will never pass DoesZExtCollapse check for a negative
                    // value, because zext(trunc(x)) is non-negative. So it
                    // only make sense to check for non-negativity here.
                    let cx = e.env.cx(&*e.context, &*e.function);
                    let s1 = e.se.get_scev(&cx, operands[0]);
                    let s2 = e.se.get_scev(&cx, operands[1]);
                    e.se.is_known_non_negative(&cx, s1) && e.se.is_known_non_negative(&cx, s2)
                }
            });
        // `Builder.CreateZExt/SExt(Op1, IVTy)`, hoisted into the preheader by
        // `makeLoopInvariant`: Op1 is loop invariant and an extension is safe
        // to speculate.
        let preheader = {
            let cx = e.env.cx(&*e.context, &*e.function);
            cx.loops.get_loop_for(header).and_then(|found| found.get_loop_preheader(cx.function))
        };
        let at = preheader.and_then(|block| e.function.terminator(block)).unwrap_or(ici);
        let ext = if can_use_zext {
            pred = match pred {
                IntPredicate::Sgt => IntPredicate::Ugt,
                IntPredicate::Sge => IntPredicate::Uge,
                IntPredicate::Slt => IntPredicate::Ult,
                IntPredicate::Sle => IntPredicate::Ule,
                same => same,
            };
            build_cast(e, CastOp::ZExt, op1, iv_ty, IrFlags::default(), at)
        } else {
            build_cast(e, CastOp::SExt, op1, iv_ty, IrFlags::default(), at)
        };
        let bit = e.context.types.int(1);
        let new_cmp = e.function.create_instruction(Opcode::ICmp(pred), bit, vec![iv, ext], IrFlags::default(), None);
        e.function.insert(new_cmp, llrm_mir::edit::Position::Before(ici)).expect("a placed instruction");
        let new_cmp_op = value_op(e, new_cmp);
        e.function.replace_all_uses_with(result_of(e.function, ici), new_cmp_op);
        e.dead.push(ici);
    }

    // Trunc no longer needed.
    let trunc_ty = e.function.instruction(trunc).ty;
    let poison = Operand::Constant(
        e.context.constant(llrm_mir::context::Constant { ty: trunc_ty, kind: llrm_mir::context::ConstantKind::Poison }),
    );
    e.function.replace_all_uses_with(result_of(e.function, trunc), poison);
    e.dead.push(trunc);
    *changed = true;
    true
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L778
/// `strengthenBinaryOp`: annotate an overflowing operation with nsw / nuw if
/// it provably does not overflow; a `shl` whose result an IV-ranged right
/// shift reads is `exact`.
fn strengthen_binary_op(
    e: &mut Edit,
    bo: InstId,
    iv_operand: InstId,
) -> bool {
    let Opcode::Binary(op) = e.function.instruction(bo).opcode else { return false };
    (matches!(
        op,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl
    )
        && strengthen_overflowing_operation(e, bo))
        || (op == BinaryOp::Shl && strengthen_right_shift(e, bo, iv_operand))
}

/// `strengthenOverflowingOperation`: annotate BO with nsw / nuw if it provably
/// does not signed-overflow / unsigned-overflow. Returns true if anything
/// changed, false otherwise.
fn strengthen_overflowing_operation(
    e: &mut Edit,
    bo: InstId,
) -> bool {
    use llrm_mir::opcode::Flags as IrFlags;
    let flags = {
        let cx = e.env.cx(&*e.context, &*e.function);
        e.se.get_strengthened_no_wrap_flags_from_bin_op(&cx, bo)
    };
    let Some((nuw, nsw)) = flags else { return false };
    let mut held = e.function.instruction(bo).flags;
    if nuw {
        held.insert(IrFlags::NUW);
    }
    if nsw {
        held.insert(IrFlags::NSW);
    }
    e.function.set_flags(bo, held);
    // The getStrengthenedNoWrapFlagsFromBinOp() check inferred additional
    // nowrap flags on addrecs while performing zero/sign extensions. We
    // could call forgetValue() here to make sure those flags also propagate
    // to any other SCEV expressions based on the addrec. However, this can
    // have pathological compile-time impact, see
    // https://bugs.llvm.org/show_bug.cgi?id=50384.
    true
}

/// `strengthenRightShift`: annotate the Shr in (X << IVOperand) >> C as exact
/// using the information from the IV's range. Returns true if anything
/// changed, false otherwise.
fn strengthen_right_shift(
    e: &mut Edit,
    bo: InstId,
    iv_operand: InstId,
) -> bool {
    use llrm_mir::opcode::Flags as IrFlags;

    use crate::widen::{result_of, value_op};
    let iv_op = value_op(e, iv_operand);
    // `m_Shl(m_Value(), m_Specific(IVOperand))`.
    if e.function.instruction(bo).operands[1] != iv_op {
        return false;
    }
    let iv_range = {
        let cx = e.env.cx(&*e.context, &*e.function);
        let s = e.se.get_scev(&cx, iv_op);
        e.se.get_unsigned_range(&cx, s)
    };
    let mut changed = false;
    let bo_op = value_op(e, bo);
    let users: Vec<InstId> = e.function.users(result_of(e.function, bo)).iter().map(|used| used.user).collect();
    for user in users {
        let instruction = e.function.instruction(user);
        if !matches!(
            instruction.opcode,
            Opcode::Binary(BinaryOp::AShr | BinaryOp::LShr)
        )
            || instruction.operands[0] != bo_op
        {
            continue;
        }
        let Operand::Constant(constant) = instruction.operands[1] else { continue };
        let llrm_mir::context::ConstantKind::Int(c) = e.context.get(constant).kind else { continue };
        if !instruction.flags.contains(IrFlags::EXACT) && iv_range.get_unsigned_min() >= c {
            let mut flags = instruction.flags;
            flags.insert(IrFlags::EXACT);
            e.function.set_flags(user, flags);
            changed = true;
        }
    }
    changed
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
    // DEVIATE 18: no widening past the pointer's index width (the 16-bit
    // target's): its arithmetic on a wider type costs more, and `n32` states
    // `i32` legal there, so the X86 model prices the add alike.
    if width > e.layout.pointer(0).index_bits {
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
