//! CodeGenPrepare's `optimizeCmp` steps after the overflow combines: the
//! power-of-two test of `ctpop` unfolded, an equality made the opposite of the
//! signed compare that dominates it, and compare operands swapped to match a
//! subtract
//! ([CodeGenPrepare.cpp#L8805](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8805)).
//!
//! # How it works
//! For each `icmp`, in the pin's order, the first that acts ends the visit.
//!
//! `unfoldPowerOf2Test`: `ctpop(x) == 1`, `ctpop(x) u< 2` and their inverses,
//! where `ctpop` is not fast (no POPCNT), become `(x & (x - 1)) == 0` when x
//! may be zero or is known non-zero, else `(x ^ (x - 1)) u> (x - 1)`.
//!
//! `foldICmpWithDominatingICmp`: only where the target does not lower an
//! equality with the signed compare beside it (PowerPC), so never on x86:
//! `a == b` in the false arm of `a < b` becomes `a > b` with its branches and
//! selects swapped, to share one compare.
//!
//! `swapICmpOperandsToExposeCSEOpportunities`: a compare of `a` and `b` with a
//! `sub b, a` among the users of `a` reads `b`, `a`, so the subtract and the
//! compare are one instruction.
//!
//! ```text
//!  %p = call i32 @llvm.ctpop.i32(i32 %x)         %m = add i32 %x, -1
//!  %c = icmp eq i32 %p, 1                  ==>   %a = and i32 %x, %m
//!                                                %c = icmp eq i32 %a, 0   ; x known non-zero
//! ```
//!
//! MIRROR: the three functions. DEVIATE CMP1: `isCtpopFast` is false for every
//! x86 CPU llrm builds for (none has POPCNT), not a query of the CPU. Exit:
//! the CPU-carrying `Lowering` (row 23, 2026-12-09).
//!
//! # Example
//! `test_ctpop_equal_one_of_a_nonzero_value_is_an_and_with_its_predecessor` in
//! `cmpopt_tests.rs` is the sketch above.
//!
//! # Runs
//! In `cgpaddr`'s run, after the compare sinks.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::lowering::Lowering;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::{Query, nonzero, pattern};

/// `Query` over `function` with no analyses.
fn query<'a>(
    context: &'a Context,
    globals: &'a [GlobalValue],
    layout: &'a DataLayout,
    function: &'a Function,
    cxt: InstId,
) -> Query<'a> {
    Query {
        context,
        globals,
        metadata: &[],
        function,
        layout,
        dt: None,
        assumptions: None,
        dom_conditions: None,
        cond_context: None,
        cxt: Some(cxt),
    }
}

fn int(
    context: &Context,
    ty: llrm_mir::types::TypeId,
    value: u128,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }))
}

/// `unfoldPowerOf2Test(cmp)`.
fn power_of_two_unfolded(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    lowering: &dyn Lowering,
    function: &mut Function,
    cmp: InstId,
) -> bool {
    let one = function.instruction(cmp).clone();
    let Opcode::ICmp(pred) = one.opcode else { return false };
    let &[count, bound] = &one.operands[..] else { return false };
    let Operand::Constant(bound_id) = bound else { return false };
    let ConstantKind::Int(c) = context.get(bound_id).kind else { return false };
    let (x, ctpop) = {
        let q = query(context, globals, layout, function, cmp);
        let Some((call, args)) = pattern::intrinsic_call(&q, count, "llvm.ctpop") else { return false };
        let &[x] = args else { return false };
        (x, call)
    };
    let strict = matches!(pred, IntPredicate::Eq | IntPredicate::Ne) && c == 1;
    let or_zero = (pred == IntPredicate::Ult && c == 2) || (pred == IntPredicate::Ugt && c == 1);
    if !strict && !or_zero {
        return false;
    }
    let Some(ty) = function.operand_type(context, x) else { return false };
    let Some(bits) = context.types.int_bits(ty) else { return false };
    // `isKnownNonZero(Cmp->getOperand(0))`: the count, not x.
    let nonzero_count = nonzero::is_known_non_zero(&query(context, globals, layout, function, cmp), count, 0);
    if lowering.is_ctpop_fast(bits) {
        // Only `ctpop(x) ==/!= 1` of a count known non-zero changes.
        if !strict || !nonzero_count {
            return false;
        }
        if pred == IntPredicate::Eq {
            function.set_operand(cmp, 1, int(context, ty, 2));
            function.set_icmp_predicate(cmp, IntPredicate::Ult);
        } else {
            function.set_icmp_predicate(cmp, IntPredicate::Ugt);
        }
        return true;
    }
    let all_ones = int(context, ty, llrm_mir::context::mask(bits));
    let zero = int(context, ty, 0);
    let i1 = context.types.int(1);
    let put = |function: &mut Function, opcode: Opcode, ty, operands: Vec<Operand>| -> Operand {
        let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
        function.insert(inst, Position::Before(cmp)).expect("a position before the compare");
        Operand::Value(function.instruction(inst).result.expect("a value"))
    };
    let new_cmp = if or_zero || (strict && nonzero_count) {
        // ctpop(x) u< 2 -> (x & (x - 1)) == 0, and its inverse.
        let sub = put(function, Opcode::Binary(BinaryOp::Add), ty, vec![x, all_ones]);
        let and = put(function, Opcode::Binary(BinaryOp::And), ty, vec![x, sub]);
        let new =
            if matches!(pred, IntPredicate::Ult | IntPredicate::Eq) { IntPredicate::Eq } else { IntPredicate::Ne };
        put(function, Opcode::ICmp(new), i1, vec![and, zero])
    } else {
        // ctpop(x) == 1 -> (x ^ (x - 1)) u> (x - 1), and its inverse.
        let sub = put(function, Opcode::Binary(BinaryOp::Add), ty, vec![x, all_ones]);
        let xor = put(function, Opcode::Binary(BinaryOp::Xor), ty, vec![x, sub]);
        let new = if pred == IntPredicate::Eq { IntPredicate::Ugt } else { IntPredicate::Ule };
        put(function, Opcode::ICmp(new), i1, vec![xor, sub])
    };
    let old = function.instruction(cmp).result.expect("a compare has a value");
    function.replace_all_uses_with(old, new_cmp);
    function.erase(cmp).expect("its uses were replaced");
    if function.instruction(ctpop).result.is_some_and(|result| function.users(result).is_empty()) {
        function.erase(ctpop).expect("a call nothing reads goes");
    }
    true
}

/// `foldICmpWithDominatingICmp(cmp)`.
fn dominating_compare_folded(
    lowering: &dyn Lowering,
    function: &mut Function,
    cmp: InstId,
) -> bool {
    if lowering.is_equality_cmp_folded_with_signed_cmp() {
        return false;
    }
    let one = function.instruction(cmp).clone();
    if one.opcode != Opcode::ICmp(IntPredicate::Eq) {
        return false;
    }
    let Some(result) = one.result else { return false };
    // Users other than a branch or the condition of a select would add IR.
    let users: Vec<InstId> = function.users(result).iter().map(|one| one.user).collect();
    let only_branches_and_selects = function
        .users(result)
        .iter()
        .all(
            |use_| {
                let user = function.instruction(use_.user);
                (user.opcode == Opcode::Br && user.operands.len() == 3 && use_.index == 0)
                    || (user.opcode == Opcode::Select && use_.index == 0)
            },
        );
    if !only_branches_and_selects {
        return false;
    }
    let Some(cmp_block) = function.parent(cmp) else { return false };
    let Some(dom_block) = function.sole_predecessor(cmp_block) else { return false };
    let Some(branch) = function.terminator(dom_block) else { return false };
    let &[dom_cond, Operand::Block(_), Operand::Block(false_block)] = &function.instruction(branch).operands[..] else {
        return false;
    };
    if function.instruction(branch).opcode != Opcode::Br || false_block != cmp_block {
        return false;
    }
    let Operand::Value(dom_value) = dom_cond else { return false };
    let ValueDef::Instruction(dom_cmp) = function.value(dom_value).def else { return false };
    let Opcode::ICmp(dom_pred) = function.instruction(dom_cmp).opcode else { return false };
    if function.instruction(dom_cmp).operands != one.operands
        || !matches!(dom_pred, IntPredicate::Sgt | IntPredicate::Slt)
    {
        return false;
    }
    for user in users {
        let mut operands = function.instruction(user).operands.clone();
        // A branch's successors, a select's values.
        operands.swap(1, 2);
        function.set_operands(user, operands);
    }
    function.set_icmp_predicate(cmp, dom_pred.swapped());
    true
}

/// `swapICmpOperandsToExposeCSEOpportunities(cmp)`.
fn operands_swapped(
    context: &Context,
    function: &mut Function,
    cmp: InstId,
) -> bool {
    let one = function.instruction(cmp).clone();
    let Opcode::ICmp(pred) = one.opcode else { return false };
    let &[op0, op1] = &one.operands[..] else { return false };
    let integer = function.operand_type(context, op0).is_some_and(|ty| context.types.int_bits(ty).is_some());
    if !integer || matches!(op0, Operand::Constant(_)) || matches!(op1, Operand::Constant(_)) || op0 == op1 {
        return false;
    }
    let Operand::Value(v0) = op0 else { return false };
    let mut good = 0i32;
    for (inspected, use_) in function.users(v0).iter().enumerate() {
        // Avoid walking many users.
        if inspected + 1 > 128 {
            return false;
        }
        let user = function.instruction(use_.user);
        if user.opcode != Opcode::Binary(BinaryOp::Sub) {
            continue;
        }
        match &user.operands[..] {
            [a, b] if *a == op1 && *b == op0 => good += 1,
            [a, b] if *a == op0 && *b == op1 => good -= 1,
            _ => {}
        }
    }
    if good > 0 {
        function.set_operands(cmp, vec![op1, op0]);
        function.set_icmp_predicate(cmp, pred.swapped());
        return true;
    }
    false
}

/// The steps of `optimizeCmp` after the overflow combines, for each `icmp`.
/// Whether any acted.
pub fn compares_optimized(
    context: &Context,
    globals: &[GlobalValue],
    layout: &DataLayout,
    lowering: &dyn Lowering,
    function: &mut Function,
) -> bool {
    let cmps: Vec<(BlockId, InstId)> =
        function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::ICmp(_))).collect();
    let mut changed = false;
    for (_, cmp) in cmps {
        if function.is_erased(cmp) {
            continue;
        }
        changed |= power_of_two_unfolded(context, globals, layout, lowering, function, cmp)
            || dominating_compare_folded(lowering, function, cmp)
            || operands_swapped(context, function, cmp);
    }
    changed
}

#[cfg(test)]
#[path = "cmpopt_tests.rs"]
mod tests;
