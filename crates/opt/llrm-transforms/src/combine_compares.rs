//! InstCombine's `visitICmpInst` and the helpers it calls, from
//! [`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)
//! at d1106deb71cc.
//!
//! [`visit_icmp`] is
//! [`visitICmpInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L7941):
//! the same steps in the same order, each a function named for the C++ one with
//! its own permalink. A step that returns `nullptr` there returns `None` here,
//! one that returns an instruction returns `Some(Visit)`.
//!
//! MIRROR: the order of the steps and each condition of the steps below.
//!
//! This is the first slice of the family. A step not translated yet stays in
//! its place as a `// OWED:` comment naming the C++ function, so the order
//! stays LLVM's. The owed steps, with their exits:
//! `foldICmpTruncWithTruncOrExt`, `canonicalizeICmpBool`,
//! `canonicalizeICmpPredicate` (needs `canFreelyInvertAllUsersOf`,
//! uselist-port's shared helper), `foldICmpWithConstant`,
//! `foldICmpWithDominatingICmp`, `foldICmpUsingBoolRange`,
//! `foldIsMultipleOfAPowerOfTwo`, `foldICmpBinOp`, `foldICmpInstWithConstant`,
//! `foldSignBitTest`, `foldOpIntoPhi`, `foldICmpInstWithConstantNotInt`,
//! `foldICmpCommutative`, `foldCmpSelectOfConstants` and the select pair fold,
//! the zero folds `foldIRemByPowerOfTwoToBitTest` and `stripNullTest`, and the
//! `dyn_castNegVal` select fold.
//!
//! DEVIATE 1 (permanent): `samesign` is not a flag of MIR's `icmp`; the folds
//! that read or set it are left out (the unsigned-to-`samesign` step of
//! `foldICmpUsingKnownBits`; the signed-to-unsigned step is made, which is what
//! sets the flag in LLVM).
//!
//! DEVIATE 2 (temporary): `foldICmpUsingKnownBits` computes the known bits of
//! the operands, where LLVM first runs `SimplifyDemandedBits` on them
//! (InstCombineSimplifyDemanded.cpp, not translated): the same facts or fewer.
//!
//! # Used by
//! `combine::visit`, once the compares slot is flipped.
//!
//! # How it works
//! `visit_icmp` orders the operands (the more complex first), asks InstSimplify,
//! and then rewrites the compare to a canonical or cheaper one: a non-strict
//! predicate against a constant becomes the strict one (`x <= 5` is `x < 6`),
//! a compare the known bits decide or reduce (`A <u B` where `max(A) == min(B)`
//! is `A != B`) is rewritten.
//!
//! # Example
//! `x u<= 5` is `x u< 6`:
//!
//! ```text
//! %r = icmp ule i16 %x, 5   ; %r = icmp ult i16 %x, 6
//! ```

use llrm_mir::context::{mask, signed};
use llrm_mir::knownbits::KnownBits;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::simplify_instruction_in;
use llrm_mir::valuetracking::range::{self, SelectOperand, SelectPatternFlavor};
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{get_complexity, query};

/// `ICmpInst::isSigned`.
fn is_signed(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle)
}

/// `ICmpInst::getUnsignedPredicate`.
fn unsigned_predicate(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Sgt => IntPredicate::Ugt,
        IntPredicate::Sge => IntPredicate::Uge,
        IntPredicate::Slt => IntPredicate::Ult,
        IntPredicate::Sle => IntPredicate::Ule,
        other => other,
    }
}

/// `InstCombiner::isCanonicalPredicate`.
fn is_canonical_predicate(pred: IntPredicate) -> bool {
    !matches!(
        pred,
        IntPredicate::Ne | IntPredicate::Ule | IntPredicate::Sle | IntPredicate::Uge | IntPredicate::Sge
    )
}

/// The parts of the compare `inst`: predicate and both operands.
fn parts(
    c: &Combiner,
    inst: InstId,
) -> (IntPredicate, Operand, Operand) {
    let instruction = c.function.instruction(inst);
    let Opcode::ICmp(pred) = instruction.opcode else { unreachable!("an icmp") };
    (pred, instruction.operands[0], instruction.operands[1])
}

/// The number and width of a constant integer (`m_APInt`).
fn constant(
    c: &Combiner,
    operand: Operand,
) -> Option<(u128, u32)> {
    query(c, None).int_constant(operand)
}

/// `m_BinOp` of one opcode: its operands and flags.
fn binary_of(
    c: &Combiner,
    operand: Operand,
    wanted: BinaryOp,
) -> Option<(Operand, Operand, Flags)> {
    let (_, instruction) = query(c, None).instruction_of(operand)?;
    (matches!(instruction.opcode, Opcode::Binary(op) if op == wanted))
        .then(|| (instruction.operands[0], instruction.operands[1], instruction.flags))
}

/// A new compare in place of `inst`: `new ICmpInst(pred, a, b)`.
fn new_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
) -> Visit {
    let i1 = c.context.types.int(1);
    c.replace_with_new(inst, Opcode::ICmp(pred), i1, vec![a, b], Flags::default())
}

/// `visitICmpInst`.
pub fn visit_icmp(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let mut changed = false;
    let (pred, op0, op1) = parts(c, inst);
    // Orders the operands of the compare so that they are listed from most
    // complex to least complex. This puts constants before unary operators,
    // before binary operators.
    let (cplx0, cplx1) = {
        let q = query(c, Some(inst));
        (get_complexity(&q, op0), get_complexity(&q, op1))
    };
    let (mut pred, mut op0, mut op1) = (pred, op0, op1);
    if cplx0 < cplx1 {
        c.function.set_operands(inst, vec![op1, op0]);
        c.function.set_icmp_predicate(inst, pred.swapped());
        pred = pred.swapped();
        std::mem::swap(&mut op0, &mut op1);
        c.add_to_worklist(inst);
        changed = true;
    }
    {
        let env = c.env;
        if let Some(found) =
            simplify_instruction_in(c.context, env.globals, env.layout, Some(&env.analyses), c.function, inst)
            && Some(found) != c.function.instruction(inst).result.map(Operand::Value)
        {
            return Visit::Replace(found);
        }
    }
    // OWED: the `abs(val) != 0` select fold (dyn_castNegVal).
    // OWED: foldICmpTruncWithTruncOrExt.
    // OWED: canonicalizeICmpBool.
    if let Some(r) = canonicalize_cmp_with_constant(c, inst, pred, op0, op1) {
        return r;
    }
    // OWED: canonicalizeICmpPredicate (canFreelyInvertAllUsersOf).
    // OWED: foldICmpWithConstant, foldICmpWithDominatingICmp,
    // foldICmpUsingBoolRange.
    if let Some(r) = fold_icmp_using_known_bits(c, inst, pred, op0, op1) {
        return r;
    }
    // OWED: foldIsMultipleOfAPowerOfTwo.
    // Test if the ICmpInst instruction is used exclusively by a select as part
    // of a minimum or maximum operation. If so, refrain from doing any other
    // folding.
    if let Some(user) = only_user(c, inst)
        && c.function.instruction(user).opcode == Opcode::Select
    {
        let result = range::match_select_pattern(&query(c, None), Operand::Value(select_result(c, user)));
        if result.flavor != SelectPatternFlavor::None {
            return if changed { Visit::Changed } else { Visit::None };
        }
    }
    // Do this after checking for min/max to prevent infinite looping.
    if let Some(r) = fold_icmp_with_zero(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some(r) = fold_umax_of_not(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some((bits, width)) = constant(c, op1) {
        // For i32: x >u 2147483647 -> x <s 0  -> true if sign bit set
        if pred == IntPredicate::Ugt && bits == mask(width) >> 1 {
            let zero = c.int(c.type_of(op0), 0);
            return new_icmp(c, inst, IntPredicate::Slt, op0, zero);
        }
        // For i32: x <u 2147483648 -> x >s -1  -> true if sign bit clear
        if pred == IntPredicate::Ult && bits == 1u128 << (width - 1) {
            let ones = c.all_ones(c.type_of(op0));
            return new_icmp(c, inst, IntPredicate::Sgt, op0, ones);
        }
    }
    // OWED: foldICmpBinOp, foldICmpInstWithConstant, foldSignBitTest,
    // foldOpIntoPhi, foldICmpInstWithConstantNotInt, foldICmpCommutative,
    // matchSymmetricPair, foldCmpSelectOfConstants and the two-select fold.
    if changed { Visit::Changed } else { Visit::None }
}

/// The one user of `inst`'s result.
fn only_user(
    c: &Combiner,
    inst: InstId,
) -> Option<InstId> {
    let result = c.function.instruction(inst).result?;
    c.function.users(result).only().map(|one| one.user)
}

/// The value of the instruction `inst`.
fn select_result(
    c: &Combiner,
    inst: InstId,
) -> llrm_mir::module::ValueId {
    c.function.instruction(inst).result.expect("a select's value")
}

/// `canonicalizeCmpWithConstant`
/// ([:7424](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L7424)):
/// a non-canonical relational predicate against a constant, the constant
/// moved by one: `x <= 5` is `x < 6`.
fn canonicalize_cmp_with_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) || is_canonical_predicate(pred) {
        return None;
    }
    let (bits, width) = constant(c, op1)?;
    let ty = c.type_of(op1);
    let flipped = {
        let q = query(c, None);
        range::get_flipped_strictness_predicate_and_constant(&q, pred, false, SelectOperand::Int { ty, bits })
    }?;
    let (new_pred, _, SelectOperand::Int { bits: new_bits, .. }) = flipped else { return None };
    let _ = width;
    let new = c.int(ty, new_bits);
    Some(new_icmp(c, inst, new_pred, op0, new))
}

/// The bits `a pred b` compares: `ICmpInst::compare(KnownBits, KnownBits, Pred)`.
fn compare_known(
    pred: IntPredicate,
    a: &KnownBits,
    b: &KnownBits,
) -> Option<bool> {
    match pred {
        IntPredicate::Eq => KnownBits::eq(a, b),
        IntPredicate::Ne => KnownBits::ne(a, b),
        IntPredicate::Ugt => KnownBits::ugt(a, b),
        IntPredicate::Uge => KnownBits::uge(a, b),
        IntPredicate::Ult => KnownBits::ult(a, b),
        IntPredicate::Ule => KnownBits::ule(a, b),
        IntPredicate::Sgt => KnownBits::sgt(a, b),
        IntPredicate::Sge => KnownBits::sge(a, b),
        IntPredicate::Slt => KnownBits::slt(a, b),
        IntPredicate::Sle => KnownBits::sle(a, b),
    }
}

/// `APInt::ceilLogBase2`.
fn ceil_log2(
    value: u128,
    width: u32,
) -> u32 {
    let temp = value.wrapping_sub(1) & mask(width);
    128 - temp.leading_zeros()
}

/// `foldICmpUsingKnownBits`
/// ([:7106](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L7106)).
/// DEVIATE 2: no `SimplifyDemandedBits` first; DEVIATE 1: no `samesign`.
fn fold_icmp_using_known_bits(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let ty = c.type_of(op0);
    let width = c.context.types.int_bits(ty).filter(|&w| w <= 128)?;
    // Don't use dominating conditions when folding icmp using known bits. This
    // may convert signed into unsigned predicates in ways that other passes
    // (especially IndVarSimplify) may not be able to reliably undo.
    let (known0, known1) = {
        let q = Query { dom_conditions: None, ..query(c, Some(inst)) };
        (known::compute_known_bits(&q, op0, 0), known::compute_known_bits(&q, op1, 0))
    };
    let is_constant = |operand: Operand| matches!(operand, Operand::Constant(_));
    if !is_constant(op0) && known0.is_constant() {
        let value = c.int(ty, known0.constant_value());
        return Some(new_icmp(c, inst, pred, value, op1));
    }
    if !is_constant(op1) && known1.is_constant() {
        let value = c.int(ty, known1.constant_value());
        return Some(new_icmp(c, inst, pred, op0, value));
    }
    if let Some(res) = compare_known(pred, &known0, &known1) {
        let i1 = c.context.types.int(1);
        return Some(Visit::Replace(c.int(i1, u128::from(res))));
    }
    // Given the known and unknown bits, compute a range that the LHS could be
    // in. Compute the Min, Max and RHS values based on the known bits. For the
    // EQ and NE we use unsigned values.
    let (min0, max0, min1, max1) = if is_signed(pred) {
        (known0.signed_min_value(), known0.signed_max_value(), known1.signed_min_value(), known1.signed_max_value())
    } else {
        (known0.min_value(), known0.max_value(), known1.min_value(), known1.max_value())
    };
    let add = |a: u128, b: u128| a.wrapping_add(b) & mask(width);
    let sub = |a: u128, b: u128| a.wrapping_sub(b) & mask(width);
    // Don't break up a clamp pattern -- (min(max X, Y), Z) -- by replacing a
    // min/max canonical compare with some other compare. That could lead to
    // conflict with select canonicalization and infinite looping.
    let min_max_cmp = {
        let is_min_max = |operand: Operand| {
            let q = query(c, None);
            let tracking = q.instruction_of(operand).and_then(|(inst, _)| pattern::call_intrinsic(&q, inst));
            matches!(
                tracking,
                Some(
                    pattern::IntrinsicId::Smax
                        | pattern::IntrinsicId::Smin
                        | pattern::IntrinsicId::Umax
                        | pattern::IntrinsicId::Umin
                )
            )
        };
        match only_user(c, inst) {
            Some(user) if c.function.instruction(user).opcode == Opcode::Select => {
                let result = range::match_select_pattern(&query(c, None), Operand::Value(select_result(c, user)));
                matches!(
                    result.flavor,
                    SelectPatternFlavor::SMin
                        | SelectPatternFlavor::SMax
                        | SelectPatternFlavor::UMin
                        | SelectPatternFlavor::UMax
                ) && (is_min_max(op0) || is_min_max(op1))
            }
            _ => false,
        }
    };
    let cmp_c = constant(c, op1).map(|(bits, _)| bits);
    if !min_max_cmp {
        match pred {
            IntPredicate::Ult => {
                if min1 == max0 {
                    return Some(new_icmp(c, inst, IntPredicate::Ne, op0, op1));
                }
                if let Some(cmp_c) = cmp_c {
                    // A <u C -> A == C-1 if min(A)+1 == C
                    if cmp_c == add(min0, 1) {
                        let value = c.int(c.type_of(op1), sub(cmp_c, 1));
                        return Some(new_icmp(c, inst, IntPredicate::Eq, op0, value));
                    }
                    // X <u C --> X == 0, if the number of zero bits in the bottom of X
                    // exceeds the log2 of C.
                    if known0.count_min_trailing_zeros() >= ceil_log2(cmp_c, width) {
                        let zero = c.int(c.type_of(op1), 0);
                        return Some(new_icmp(c, inst, IntPredicate::Eq, op0, zero));
                    }
                }
            }
            IntPredicate::Ugt => {
                if max1 == min0 {
                    return Some(new_icmp(c, inst, IntPredicate::Ne, op0, op1));
                }
                if let Some(cmp_c) = cmp_c {
                    // A >u C -> A == C+1 if max(a)-1 == C
                    if cmp_c == sub(max0, 1) {
                        let value = c.int(c.type_of(op1), add(cmp_c, 1));
                        return Some(new_icmp(c, inst, IntPredicate::Eq, op0, value));
                    }
                    // X >u C --> X != 0, if the number of zero bits in the bottom of X
                    // exceeds the log2 of C.
                    if known0.count_min_trailing_zeros() >= 128 - cmp_c.leading_zeros() {
                        let zero = c.int(c.type_of(op1), 0);
                        return Some(new_icmp(c, inst, IntPredicate::Ne, op0, zero));
                    }
                }
            }
            IntPredicate::Slt => {
                if min1 == max0 {
                    return Some(new_icmp(c, inst, IntPredicate::Ne, op0, op1));
                }
                if let Some(cmp_c) = cmp_c
                    && cmp_c == add(min0, 1)
                {
                    let value = c.int(c.type_of(op1), sub(cmp_c, 1));
                    return Some(new_icmp(c, inst, IntPredicate::Eq, op0, value));
                }
            }
            IntPredicate::Sgt => {
                if max1 == min0 {
                    return Some(new_icmp(c, inst, IntPredicate::Ne, op0, op1));
                }
                if let Some(cmp_c) = cmp_c
                    && cmp_c == sub(max0, 1)
                {
                    let value = c.int(c.type_of(op1), add(cmp_c, 1));
                    return Some(new_icmp(c, inst, IntPredicate::Eq, op0, value));
                }
            }
            _ => {}
        }
    }
    // Based on the range information we know about the LHS, see if we can
    // simplify this comparison. For example, (x&4) < 8 is always true.
    match pred {
        IntPredicate::Eq | IntPredicate::Ne => {
            // If all bits are known zero except for one, then we know at most one bit
            // is set. If the comparison is against zero, then this is a check to see if
            // *that* bit is set.
            let op0_known_zero_inverted = !known0.zero & mask(width);
            if known1.is_zero() {
                // If the LHS is an AND with the same constant, look through it.
                let lhs = match binary_of(c, op0, BinaryOp::And) {
                    Some((x, m, _)) if constant(c, m).map(|(bits, _)| bits) == Some(op0_known_zero_inverted) => x,
                    _ => op0,
                };
                if let Some((c1_op, x, _)) = binary_of(c, lhs, BinaryOp::Shl)
                    && let Some((c1, _)) = constant(c, c1_op)
                    && c1.is_power_of_two()
                {
                    let log2_c1 = c1.trailing_zeros();
                    let c2 = op0_known_zero_inverted;
                    let c2_pow2 = ((c2 & !(c1 - 1)).wrapping_add(c1)) & mask(width);
                    if c2_pow2.is_power_of_two() {
                        // iff (C1 is pow2) & ((C2 & ~(C1-1)) + C1) is pow2):
                        // ((C1 << X) & C2) == 0 -> X >= (Log2(C2+C1) - Log2(C1))
                        // ((C1 << X) & C2) != 0 -> X  < (Log2(C2+C1) - Log2(C1))
                        let log2_c2 = c2_pow2.trailing_zeros();
                        let x_ty = c.type_of(x);
                        let cmp = c.int(x_ty, u128::from(log2_c2 - log2_c1));
                        let new_pred = if pred == IntPredicate::Eq { IntPredicate::Uge } else { IntPredicate::Ult };
                        return Some(new_icmp(c, inst, new_pred, x, cmp));
                    }
                }
            }
            // Op0 eq C_Pow2 -> Op0 ne 0 if Op0 is known to be C_Pow2 or zero.
            if known1.is_constant()
                && known1.constant_value().is_power_of_two()
                && (known0.zero | known1.zero, known0.one & known1.one) == (known0.zero, known0.one)
            {
                let zero = c.int(c.type_of(op1), 0);
                return Some(new_icmp(c, inst, pred.inverse(), op0, zero));
            }
        }
        IntPredicate::Sge => {
            if min1 == max0 {
                return Some(new_icmp(c, inst, IntPredicate::Eq, op0, op1));
            }
        }
        IntPredicate::Sle => {
            if max1 == min0 {
                return Some(new_icmp(c, inst, IntPredicate::Eq, op0, op1));
            }
        }
        IntPredicate::Uge => {
            if min1 == max0 {
                return Some(new_icmp(c, inst, IntPredicate::Eq, op0, op1));
            }
        }
        IntPredicate::Ule => {
            if max1 == min0 {
                return Some(new_icmp(c, inst, IntPredicate::Eq, op0, op1));
            }
        }
        _ => {}
    }
    // Turn a signed comparison into an unsigned one if both operands are known
    // to have the same sign.
    let sign = 1u128 << (width - 1);
    if is_signed(pred)
        && ((known0.zero & sign != 0 && known1.zero & sign != 0) || (known0.one & sign != 0 && known1.one & sign != 0))
    {
        c.function.set_icmp_predicate(inst, unsigned_predicate(pred));
        c.add_to_worklist(inst);
        return Some(Visit::Changed);
    }
    None
}

/// `foldICmpWithZero`
/// ([:1224](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1224)):
/// `icmp pred X, 0`. OWED: `foldIRemByPowerOfTwoToBitTest`, `stripNullTest`.
fn fold_icmp_with_zero(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if constant(c, op1).map(|(bits, _)| bits) != Some(0) {
        return None;
    }
    let equality = matches!(pred, IntPredicate::Eq | IntPredicate::Ne);
    // (icmp sgt smin(PosA, B) 0) -> (icmp sgt B 0)
    if pred == IntPredicate::Sgt {
        let min = {
            let q = query(c, Some(inst));
            let call = q.instruction_of(op0).and_then(|(call, _)| {
                (pattern::call_intrinsic(&q, call) == Some(pattern::IntrinsicId::Smin)).then(|| {
                    let operands = &c.function.instruction(call).operands;
                    (operands[0], operands[1])
                })
            });
            call.map(|(a, b)| (a, b, known::is_known_positive(&q, a, 0), known::is_known_positive(&q, b, 0)))
        };
        if let Some((a, b, a_positive, b_positive)) = min {
            if a_positive {
                return Some(new_icmp(c, inst, pred, b, op1));
            }
            if b_positive {
                return Some(new_icmp(c, inst, pred, a, op1));
            }
        }
    }
    if !equality {
        return None;
    }
    // Given: icmp eq/ne (urem %x, %y), 0. Iff %x has 0 or 1 bits set, and %y has
    // at least 2 bits set, omit 'urem': icmp eq/ne %x, 0
    if let Some((x, y, _)) = binary_of(c, op0, BinaryOp::URem) {
        let (xk, yk) = {
            let q = query(c, Some(inst));
            (known::compute_known_bits(&q, x, 0), known::compute_known_bits(&q, y, 0))
        };
        if xk.count_max_population() == 1 && yk.count_min_population() >= 2 {
            return Some(new_icmp(c, inst, pred, x, op1));
        }
    }
    // (icmp eq/ne (mul X Y)) -> (icmp eq/ne X/Y) if we know about whether X/Y are
    // odd/non-zero/there is no overflow.
    if let Some((x, y, flags)) = binary_of(c, op0, BinaryOp::Mul) {
        let q = query(c, Some(inst));
        let xk = known::compute_known_bits(&q, x, 0);
        // if X % 2 != 0: (icmp eq/ne Y)
        if xk.count_max_trailing_zeros() == 0 {
            return Some(new_icmp(c, inst, pred, y, op1));
        }
        let yk = known::compute_known_bits(&q, y, 0);
        // if Y % 2 != 0: (icmp eq/ne X)
        if yk.count_max_trailing_zeros() == 0 {
            return Some(new_icmp(c, inst, pred, x, op1));
        }
        if flags.contains(Flags::NUW) || flags.contains(Flags::NSW) {
            // if X non-zero and NoOverflow(X * Y): (icmp eq/ne Y)
            if xk.one != 0 || llrm_mir::valuetracking::nonzero::is_known_non_zero(&q, x, 0) {
                return Some(new_icmp(c, inst, pred, y, op1));
            }
            // if Y non-zero and NoOverflow(X * Y): (icmp eq/ne X)
            if yk.one != 0 || llrm_mir::valuetracking::nonzero::is_known_non_zero(&q, y, 0) {
                return Some(new_icmp(c, inst, pred, x, op1));
            }
        }
    }
    None
}

/// The two folds of a `umax` against its own complement in `visitICmpInst`
/// ([:8027](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L8027)):
/// `umax(x, C) u> ~x` and `umax(x, C) u< ~x`.
fn fold_umax_of_not(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if !matches!(pred, IntPredicate::Ugt | IntPredicate::Ult) {
        return None;
    }
    let (x, cc, width) = {
        let q = query(c, None);
        let (call, _) = q.instruction_of(op0)?;
        if pattern::call_intrinsic(&q, call) != Some(pattern::IntrinsicId::Umax) {
            return None;
        }
        let args = &c.function.instruction(call).operands;
        let (x, cst) = (args[0], args[1]);
        let (bits, width) = q.int_constant(cst)?;
        (x, bits, (bits, width).1)
    };
    if pattern::not(&query(c, None), op1) != Some(x) {
        return None;
    }
    let ty = c.type_of(x);
    let non_negative = signed(cc, width) >= 0;
    let (new_pred, new_c) = if pred == IntPredicate::Ugt {
        if non_negative { (IntPredicate::Slt, c.int(ty, 0)) } else { (IntPredicate::Ugt, c.int(ty, !cc & mask(width))) }
    } else if non_negative {
        (IntPredicate::Sgt, c.all_ones(ty))
    } else {
        (IntPredicate::Ult, c.int(ty, !cc & mask(width)))
    };
    Some(new_icmp(c, inst, new_pred, x, new_c))
}

#[cfg(test)]
#[path = "combine_compares_tests.rs"]
mod tests;
