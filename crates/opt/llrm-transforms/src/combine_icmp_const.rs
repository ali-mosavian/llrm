//! `icmp (binop ...), C`: the folds of a compare of a binary operator with a
//! constant, so far the `and` family.
//!
//! Translation, from
//! [`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)
//! at d1106deb71cc, of
//! [`foldICmpInstWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3644),
//! [`foldICmpBinOpWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L4124),
//! [`foldICmpAndConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1951),
//! [`foldICmpAndConstConst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1807)
//! and
//! [`foldICmpAndShift`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1720).
//!
//! MIRROR: the order of the folds and each condition.
//!
//! OWED, each at its place as a `// OWED:` comment: the other opcodes of
//! `foldICmpBinOpWithConstant` (`foldICmpXorConstant`, `foldICmpOrConstant`,
//! `foldICmpShlConstant`, `foldICmpShrConstant`,
//! `foldICmpSRemConstant`, `foldICmpUDivConstant`, `foldICmpDivConstant`,
//! ),
//! the `or`-of-select case of `foldICmpBinOpEqualityWithConstant`, the
//! truth-table fold, and the select, truncate, intrinsic and `extractvalue`
//! arms of `foldICmpInstWithConstant`.
//!
//! DEVIATE 1: the load from an indexed global fold
//! (`foldCmpLoadFromIndexedGlobal`) and the floating-point class folds on a
//! bitcast (`is.fpclass`) are not translated.
//!
//! # How it works
//! The compare's left operand is an `and` of a value with a constant `C2`,
//! compared with a constant `C1`. The `and` goes where the compare says what
//! it keeps: `(X & signbit) != 0` is `X < 0`, `(X & -2^n) == 0` is `X u< 2^n`,
//! `(X & 1) != 0` is `trunc X`, a shift under the mask moves into the
//! constants, and a mask of a narrowed value widens.
//!
//! # Used by
//! `combine_compares::visit_icmp`.
//!
//! # Example
//! A sign test of a mask is a compare of the value:
//!
//! ```text
//! %m = and i16 %x, -32768
//! %r = icmp ne i16 %m, 0          ; %r = icmp slt i16 %x, 0
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::fp::is_sign_bit_check;
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query};

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

/// `new TruncInst(x, i1)`.
fn new_trunc_to_bool(
    c: &mut Combiner,
    inst: InstId,
    x: Operand,
) -> Visit {
    let i1 = c.context.types.int(1);
    c.replace_with_new(inst, Opcode::Cast(CastOp::Trunc), i1, vec![x], Flags::default())
}

/// `APInt::isNegatedPowerOf2`.
fn is_negated_power_of_two(
    value: u128,
    width: u32,
) -> bool {
    value.wrapping_neg() & mask(width) != 0 && (value.wrapping_neg() & mask(width)).is_power_of_two()
}

fn is_negative(
    value: u128,
    width: u32,
) -> bool {
    value >> (width - 1) & 1 == 1
}

fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

fn is_equality(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
}

/// `m_c_And(m_Neg(X), m_Deferred(X))` of the operands of an `and`: `X`.
fn neg_and_self(
    q: &Query,
    a: Operand,
    b: Operand,
) -> Option<Operand> {
    let neg_of = |v: Operand| {
        let (_, _, zero, x) = pattern::binary(q, v, BinaryOp::Sub)?;
        (q.int_constant(zero)?.0 == 0).then_some(x)
    };
    [(a, b), (b, a)].into_iter().find_map(|(neg, other)| neg_of(neg).filter(|&x| x == other))
}

/// `foldICmpInstWithConstant`.
pub fn fold_icmp_inst_with_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (value, width) = query(c, None).int_constant(op1)?;
    // foldICmpBinOpWithConstant, the `and`, `mul` and `add` cases.
    // OWED: the other opcodes, foldICmpBinOpEqualityWithConstant and the
    // truth-table fold; the select, intrinsic and extractvalue arms, and the
    // rest of the trunc arm.
    let trunc = {
        let q = query(c, None);
        pattern::cast(&q, op0, CastOp::Trunc).map(|(_, trunc, x)| (trunc.flags, x))
    };
    if let Some((flags, x)) = trunc {
        return fold_icmp_trunc_constant(c, inst, pred, (flags, x), value, width);
    }
    let and = {
        let q = query(c, None);
        pattern::binary(&q, op0, BinaryOp::And).map(|(_, _, x, y)| (x, y))
    };
    if let Some(operands) = and {
        return fold_icmp_and_constant(c, inst, pred, op0, operands, value, width);
    }
    let mul = {
        let q = query(c, None);
        pattern::binary(&q, op0, BinaryOp::Mul).map(|(_, mul, x, y)| (mul.flags, x, y))
    };
    if let Some((flags, x, y)) = mul {
        return fold_icmp_mul_constant(c, inst, pred, flags, (x, y), value, width);
    }
    let sub = {
        let q = query(c, None);
        pattern::binary(&q, op0, BinaryOp::Sub).map(|(_, sub, x, y)| (sub.flags, x, y))
    };
    if let Some((flags, x, y)) = sub
        && let Some(visit) = fold_icmp_sub_constant(c, inst, pred, op0, flags, (x, y), value, width)
    {
        return Some(visit);
    }
    let add = {
        let q = query(c, None);
        pattern::binary(&q, op0, BinaryOp::Add).map(|(_, add, x, y)| (add.flags, x, y))
    };
    if let Some((flags, x, y)) = add
        && let Some(visit) = fold_icmp_add_constant(c, inst, pred, op0, flags, (x, y), value, width)
    {
        return Some(visit);
    }
    fold_icmp_bin_op_equality_with_constant(c, inst, pred, op0, value, width)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3046
/// `foldICmpSubConstant`: `icmp (sub X, Y), C`. The `urem` fold and the
/// multi-use `sub` folds that `X - Y == 0` is guarded for are all here.
fn fold_icmp_sub_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    sub: Operand,
    flags: Flags,
    (x, y): (Operand, Operand),
    value: u128,
    width: u32,
) -> Option<Visit> {
    let ty = c.type_of(sub);
    let m = mask(width);
    let equality = is_equality(pred);

    // (X - (X urem D)) is D*(X/D), a multiple of D, so it is u> C exactly when
    // X u>= D (for C u< D), and u< C exactly when X u< D (for 0 u< C u<=
    // D).
    let d = {
        let q = query(c, None);
        pattern::binary(&q, y, BinaryOp::URem)
            .filter(|&(_, _, deferred, _)| deferred == x)
            .and_then(|(_, _, _, d)| q.int_constant(d))
            .filter(|&(d, _)| d != 0)
    };
    if let Some((d, _)) = d {
        if pred == IntPredicate::Ugt && value < d {
            let bound = c.int(ty, d - 1);
            return Some(new_icmp(c, inst, IntPredicate::Ugt, x, bound));
        }
        if pred == IntPredicate::Ult && value != 0 && value <= d {
            let bound = c.int(ty, d);
            return Some(new_icmp(c, inst, IntPredicate::Ult, x, bound));
        }
    }

    // (SubC - Y) == C) --> Y == (SubC - C)
    let sub_c = query(c, None).int_constant(x).map(|(v, _)| v);
    if equality && let Some(sub_c) = sub_c {
        let bound = c.int(ty, sub_c.wrapping_sub(value) & m);
        return Some(new_icmp(c, inst, pred, y, bound));
    }

    // (icmp P (sub nuw|nsw C2, Y), C) -> (icmp swap(P) Y, C2-C)
    let signed = is_signed(pred);
    let (has_nsw, has_nuw) = (flags.contains(Flags::NSW), flags.contains(Flags::NUW));
    if let Some(c2) = sub_c
        && ((!signed && !equality && has_nuw) || (signed && has_nsw))
    {
        let overflows = if signed {
            let (a, b) = (llrm_mir::context::signed(c2, width), llrm_mir::context::signed(value, width));
            let (low, high) = (
                llrm_mir::context::signed(1u128 << (width - 1), width),
                llrm_mir::context::signed((1u128 << (width - 1)) - 1, width),
            );
            a.checked_sub(b).is_none_or(|r| r < low || r > high)
        } else {
            c2 < value
        };
        if !overflows {
            let bound = c.int(ty, c2.wrapping_sub(value) & m);
            return Some(new_icmp(c, inst, pred.swapped(), y, bound));
        }
    }

    // X - Y == 0 --> X == Y. Not when the sub feeds a phi: that guards a loop
    // test's codegen.
    let feeds_phi = match sub {
        Operand::Value(result) => {
            c.function.users(result).iter().any(|u| c.function.instruction(u.user).opcode == Opcode::Phi)
        }
        _ => false,
    };
    if equality && value == 0 && !feeds_phi {
        return Some(new_icmp(c, inst, pred, x, y));
    }

    // The following transforms are only worth it if the only user of the
    // subtract is the icmp.
    if !has_one_use(c, sub) {
        return None;
    }
    if has_nsw {
        let (all_ones, one) = (value == m, value == 1);
        match pred {
            IntPredicate::Sgt if all_ones => return Some(new_icmp(c, inst, IntPredicate::Sge, x, y)),
            IntPredicate::Sgt if value == 0 => return Some(new_icmp(c, inst, IntPredicate::Sgt, x, y)),
            IntPredicate::Slt if value == 0 => return Some(new_icmp(c, inst, IntPredicate::Slt, x, y)),
            IntPredicate::Slt if one => return Some(new_icmp(c, inst, IntPredicate::Sle, x, y)),
            _ => {}
        }
    }
    let c2 = sub_c?;
    // C2 - Y <u C -> (Y | (C - 1)) == C2  iff (C2 & (C - 1)) == C - 1 and C is
    // a power of 2
    if pred == IntPredicate::Ult && value.is_power_of_two() && c2 & (value - 1) == value - 1 {
        let low = c.int(ty, value - 1);
        let or = c.create_binop(inst, BinaryOp::Or, y, low, Flags::default());
        return Some(new_icmp(c, inst, IntPredicate::Eq, or, x));
    }
    // C2 - Y >u C -> (Y | C) != C2  iff C2 & C == C and C + 1 is a power of 2
    if pred == IntPredicate::Ugt
        && value.wrapping_add(1) & m != 0
        && (value.wrapping_add(1) & m).is_power_of_two()
        && c2 & value == value
    {
        let cv = c.int(ty, value);
        let or = c.create_binop(inst, BinaryOp::Or, y, cv, Flags::default());
        return Some(new_icmp(c, inst, IntPredicate::Ne, or, x));
    }
    // Canonicalize any remaining sub to add as: (C2 - Y) > C --> (Y + ~C2) < ~C
    let not_c2 = c.int(ty, !c2 & m);
    let mut keep = Flags::default();
    if has_nuw {
        keep = keep.union(Flags::NUW);
    }
    if has_nsw {
        keep = keep.union(Flags::NSW);
    }
    let add = c.create_binop(inst, BinaryOp::Add, y, not_c2, keep);
    let bound = c.int(ty, !value & m);
    Some(new_icmp(c, inst, pred.swapped(), add, bound))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3736
/// `foldICmpBinOpEqualityWithConstant`: `icmp eq/ne (binop X, Y), C`, the
/// `srem`, `add`, `xor`, `or` (the all-ones mask), and `udiv`/`sdiv` cases.
///
/// OWED: the `or`-of-select case.
fn fold_icmp_bin_op_equality_with_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    bo: Operand,
    value: u128,
    width: u32,
) -> Option<Visit> {
    if !is_equality(pred) {
        return None;
    }
    let m = mask(width);
    let ty = c.type_of(bo);
    let (op, bo_inst, x, y) = {
        let q = query(c, None);
        let (_, instruction) = q.instruction_of(bo)?;
        let Opcode::Binary(op) = instruction.opcode else { return None };
        (op, instruction.flags, instruction.operands[0], instruction.operands[1])
    };
    let y_const = query(c, None).int_constant(y).map(|(v, _)| v);
    match op {
        // If we have a signed (X % (2^c)) == 0, turn it into an unsigned one.
        BinaryOp::SRem if value == 0 && has_one_use(c, bo) => {
            let (d, _) = query(c, None).int_constant(y)?;
            let signed_d = llrm_mir::context::signed(d, width);
            if signed_d > 1 && d.is_power_of_two() {
                let rem = c.create_binop(inst, BinaryOp::URem, x, y, Flags::default());
                let zero = c.int(ty, 0);
                return Some(new_icmp(c, inst, pred, rem, zero));
            }
            None
        }
        BinaryOp::Add => {
            // (A + C2) == C --> A == (C - C2)
            if let Some(c2) = y_const {
                if has_one_use(c, bo) {
                    let bound = c.int(ty, value.wrapping_sub(c2) & m);
                    return Some(new_icmp(c, inst, pred, x, bound));
                }
                return None;
            }
            // Replace ((add A, B) != 0) with (A != -B) if A or B is efficiently
            // invertible, or if the add has just this one use.
            if value == 0 {
                if has_one_use(c, bo) {
                    // (add nuw A, B) != 0 -> (or A, B) != 0
                    if bo_inst.contains(Flags::NUW) {
                        let or = c.create_binop(inst, BinaryOp::Or, x, y, Flags::default());
                        let zero = c.int(ty, 0);
                        return Some(new_icmp(c, inst, pred, or, zero));
                    }
                    let zero = c.int(ty, 0);
                    let neg = c.create_binop(inst, BinaryOp::Sub, zero, y, Flags::default());
                    return Some(new_icmp(c, inst, pred, x, neg));
                }
            }
            None
        }
        BinaryOp::Xor => {
            // For the xor case, we can xor two constants together, eliminating
            // the explicit xor.
            if let Some(boc) = y_const {
                let bound = c.int(ty, (value ^ boc) & m);
                return Some(new_icmp(c, inst, pred, x, bound));
            }
            // Replace ((xor A, B) != 0) with (A != B)
            (value == 0).then(|| new_icmp(c, inst, pred, x, y))
        }
        BinaryOp::Or => {
            // Replace (X | C) == -1 with (X & ~C) == ~C. This removes the -1
            // constant.
            let boc = y_const?;
            if has_one_use(c, bo) && value == m {
                let not_boc = c.int(ty, !boc & m);
                let and = c.create_binop(inst, BinaryOp::And, x, not_boc, Flags::default());
                return Some(new_icmp(c, inst, pred, and, not_boc));
            }
            None
        }
        BinaryOp::UDiv | BinaryOp::SDiv => {
            if bo_inst.contains(Flags::EXACT) {
                // div exact X, Y eq/ne 0 -> X eq/ne 0; eq/ne 1 -> X eq/ne Y
                if value == 0 {
                    let zero = c.int(ty, 0);
                    return Some(new_icmp(c, inst, pred, x, zero));
                }
                if value == 1 {
                    return Some(new_icmp(c, inst, pred, x, y));
                }
            }
            if op == BinaryOp::UDiv && value == 0 {
                // (icmp eq/ne (udiv A, B), 0) -> (icmp ugt/ule B, A)
                let new_pred = if pred == IntPredicate::Ne { IntPredicate::Ule } else { IntPredicate::Ugt };
                return Some(new_icmp(c, inst, new_pred, y, x));
            }
            None
        }
        _ => None,
    }
}

/// `foldICmpTruncConstant`'s first arm: `icmp pred (trunc nsw X), C` is the
/// compare on `X` with `C` sign-extended; `trunc nuw` with an unsigned or
/// equality predicate, with `C` zero-extended, when `shouldChangeType` allows
/// computing in the wider type.
///
/// OWED: the `signum`, `shl` power-of-two and equality-mask arms.
fn fold_icmp_trunc_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    (flags, x): (Flags, Operand),
    value: u128,
    width: u32,
) -> Option<Visit> {
    let src = c.type_of(x);
    let src_bits = c.context.types.int_bits(src)?;
    if !crate::combine_shared::should_change_type(c.env.layout, width, src_bits) {
        return None;
    }
    let wide = if flags.contains(Flags::NSW) {
        llrm_mir::context::signed(value, width) as u128 & mask(src_bits)
    } else if !is_signed(pred) && flags.contains(Flags::NUW) {
        value
    } else {
        return None;
    };
    let bound = c.int(src, wide);
    Some(new_icmp(c, inst, pred, x, bound))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L2231
/// `foldICmpMulConstant`: `icmp (mul X, Y), C`.
#[allow(clippy::too_many_arguments)]
fn fold_icmp_mul_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    flags: Flags,
    (x, y): (Operand, Operand),
    value: u128,
    width: u32,
) -> Option<Visit> {
    let ty = c.type_of(x);
    let m = mask(width);
    let (nuw, nsw) = (flags.contains(Flags::NUW), flags.contains(Flags::NSW));
    let signed_pred = is_signed(pred);
    let strict = |p: IntPredicate| crate::constraintelim::strict_predicate(p);

    // A square against a constant is a compare of roots.
    if x == y && !signed_pred {
        let root = isqrt(value);
        let is_square = value == root * root;
        if is_equality(pred) && (nuw || (nsw && value == 0)) {
            if !is_square {
                let bool_ty = c.context.types.int(1);
                let constant = c.int(bool_ty, u128::from(pred == IntPredicate::Ne));
                return Some(Visit::Replace(constant));
            }
            let bound = c.int(ty, root);
            return Some(new_icmp(c, inst, pred, x, bound));
        }
        if nuw {
            if is_square {
                let bound = c.int(ty, root);
                return Some(new_icmp(c, inst, pred, x, bound));
            }
            let lt_or_le = matches!(pred, IntPredicate::Ult | IntPredicate::Ule);
            let bound = c.int(ty, if lt_or_le { root + 1 } else { root });
            return Some(new_icmp(c, inst, strict(pred), x, bound));
        }
    }

    let (mul_c, _) = query(c, None).int_constant(y)?;
    let mut pred = pred;

    // (X * +MulC) < 0 --> X < 0; (X * -MulC) < 0 --> X > 0
    if nsw && is_sign_test(&mut pred, value, width) {
        if is_negative(mul_c, width) {
            pred = pred.swapped();
        }
        let zero = c.int(ty, 0);
        return Some(new_icmp(c, inst, pred, x, zero));
    }
    if mul_c == 0 {
        return None;
    }

    if is_equality(pred) {
        let (sc, smul) = (llrm_mir::context::signed(value, width), llrm_mir::context::signed(mul_c, width));
        // (mul nsw X, MulC) eq/ne C --> X eq/ne C /s MulC
        if nsw && !(sc == llrm_mir::context::signed(1u128 << (width - 1), width) && smul == -1) && sc % smul == 0 {
            let bound = c.int(ty, (sc / smul) as u128 & m);
            return Some(new_icmp(c, inst, pred, x, bound));
        }
        // (mul nuw X, MulC) eq/ne C --> X eq/ne C /u MulC; (mul X, OddC) eq/ne
        // N * C --> X eq/ne N
        if value % mul_c == 0 && (mul_c & 1 == 1 || nuw) {
            let bound = c.int(ty, value / mul_c);
            return Some(new_icmp(c, inst, pred, x, bound));
        }
    }

    // (X * MulC) < C --> X < (C / MulC), given the matching no-overflow flag.
    let bound = if nsw && signed_pred {
        let (sc, smul) = (llrm_mir::context::signed(value, width), llrm_mir::context::signed(mul_c, width));
        // MININT / -1 --> overflow.
        if sc == llrm_mir::context::signed(1u128 << (width - 1), width) && smul == -1 {
            return None;
        }
        if smul < 0 {
            pred = pred.swapped();
        }
        // RoundingSDiv: UP (ceiling) for slt/sge, DOWN (floor) for sle/sgt.
        let (truncated, rest) = (sc / smul, sc % smul);
        let differs = (rest < 0) != (smul < 0);
        let chosen = if matches!(pred, IntPredicate::Slt | IntPredicate::Sge) {
            truncated + i128::from(rest != 0 && !differs)
        } else {
            truncated - i128::from(rest != 0 && differs)
        };
        chosen as u128 & m
    } else if nuw && !signed_pred && !is_equality(pred) {
        let (quotient, rest) = (value / mul_c, value % mul_c);
        if matches!(pred, IntPredicate::Ult | IntPredicate::Uge) { quotient + u128::from(rest != 0) } else { quotient }
    } else {
        return None;
    };
    let bound = c.int(ty, bound);
    Some(new_icmp(c, inst, pred, x, bound))
}

/// `isSignTest`: the compare is a signed one against zero, moving `pred` to
/// the form `X < 0` / `X > -1` takes.
fn is_sign_test(
    pred: &mut IntPredicate,
    value: u128,
    width: u32,
) -> bool {
    if !is_signed(*pred) {
        return false;
    }
    if value == 0 {
        return true;
    }
    if value == 1 {
        if *pred == IntPredicate::Slt {
            *pred = IntPredicate::Sle;
            return true;
        }
    } else if value == mask(width) && *pred == IntPredicate::Sgt {
        *pred = IntPredicate::Sge;
        return true;
    }
    false
}

/// `APInt::sqrtFloor`.
fn isqrt(value: u128) -> u128 {
    let (mut low, mut high) = (0u128, 1u128 << 64);
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        if mid.checked_mul(mid).is_some_and(|square| square <= value) { low = mid } else { high = mid - 1 }
    }
    low
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3231
/// `foldICmpAddConstant`: `icmp (add X, C2), C`.
fn fold_icmp_add_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    add: Operand,
    add_flags: Flags,
    (x, y): (Operand, Operand),
    value: u128,
    width: u32,
) -> Option<Visit> {
    let ty = c.type_of(add);
    let same_sign = c.function.instruction(inst).flags.contains(Flags::SAMESIGN);
    let m = mask(width);
    let (nuw, nsw) = (add_flags.contains(Flags::NUW), add_flags.contains(Flags::NSW));
    let smin = 1u128 << (width - 1);
    let smax = smin - 1;
    let wrap = |v: u128| v & m;

    // icmp ult (add nuw A, (lshr A, ShAmtC)), C --> icmp ult A, C
    // when C <= (1 << ShAmtC).
    if pred == IntPredicate::Ult && nuw {
        let found = {
            let q = query(c, Some(inst));
            [(x, y), (y, x)]
                .into_iter()
                .find_map(
                    |(a, shifted)| {
                        let (_, _, deferred, amount) = pattern::binary(&q, shifted, BinaryOp::LShr)?;
                        let (amount, _) = q.int_constant(amount)?;
                        (deferred == a && amount < u128::from(width) && value <= 1u128 << amount).then_some(a)
                    },
                )
        };
        if let Some(a) = found {
            let bound = c.int(c.type_of(a), value);
            return Some(new_icmp(c, inst, pred, a, bound));
        }
    }

    if is_equality(pred) {
        return None;
    }
    let (c2, _) = query(c, None).int_constant(y)?;

    // If the add does not wrap, we can always adjust the compare by
    // subtracting the constants. Equality comparisons are handled elsewhere.
    // SGE/SLE/UGE/ULE have been canonicalized to SGT/SLT/UGT/ULT.
    if nuw && matches!(pred, IntPredicate::Ugt | IntPredicate::Ult) && value >= c2 {
        // icmp Pred (add nuw X, C2), C --> icmp Pred X, (C - C2)
        let bound = c.int(ty, value - c2);
        return Some(new_icmp(c, inst, pred, x, bound));
    }

    let chosen = llrm_mir::valuetracking::pattern::CmpPredicate { pred, same_sign }.preferred_signed_predicate();
    if nsw && matches!(chosen, IntPredicate::Sgt | IntPredicate::Slt) {
        let (sc, sc2) = (llrm_mir::context::signed(value, width), llrm_mir::context::signed(c2, width));
        let (low, high) = (llrm_mir::context::signed(smin, width), llrm_mir::context::signed(smax, width));
        if let Some(new) = sc.checked_sub(sc2).filter(|new| (low..=high).contains(new)) {
            // icmp samesign ugt/ult (add nsw X, C2), C
            //   -> icmp sgt/slt X, (C - C2)
            let bound = c.int(ty, wrap(new as u128));
            return Some(new_icmp(c, inst, chosen, x, bound));
        }
    }

    let non_negative = |v: u128| !is_negative(v, width);
    if !is_signed(pred) && nsw && non_negative(value) && non_negative(wrap(value.wrapping_sub(c2))) {
        let range = {
            let q = query(c, Some(inst));
            llrm_mir::valuetracking::range::compute_constant_range(&q, x, true, 0)
        };
        let shifted = llrm_mir::constantrange::ConstantRange::single(width, c2);
        if range.add(&shifted).is_all_non_negative() {
            let signed_pred = match pred {
                IntPredicate::Ugt => IntPredicate::Sgt,
                IntPredicate::Uge => IntPredicate::Sge,
                IntPredicate::Ult => IntPredicate::Slt,
                _ => IntPredicate::Sle,
            };
            let bound = c.int(ty, wrap(value.wrapping_sub(c2)));
            return Some(new_icmp(c, inst, signed_pred, x, bound));
        }
    }

    let region = llrm_mir::constantrange::ConstantRange::make_exact_icmp_region(pred, width, value).subtract(c2);
    let (lower, upper) = (region.lower, region.upper);
    if is_signed(pred) {
        if lower == smin {
            let bound = c.int(ty, upper);
            return Some(new_icmp(c, inst, IntPredicate::Slt, x, bound));
        }
        if upper == smin {
            let bound = c.int(ty, lower);
            return Some(new_icmp(c, inst, IntPredicate::Sge, x, bound));
        }
    } else {
        if lower == 0 {
            let bound = c.int(ty, upper);
            return Some(new_icmp(c, inst, IntPredicate::Ult, x, bound));
        }
        if upper == 0 {
            let bound = c.int(ty, lower);
            return Some(new_icmp(c, inst, IntPredicate::Uge, x, bound));
        }
    }

    // This set of folds is intentionally placed after folds that use
    // no-wrapping flags because those folds are likely better for later
    // analysis/codegen.

    // Fold compare with offset to opposite sign compare if it eliminates
    // offset: (X + C2) >u C --> X <s -C2 (if C == C2 + SMAX)
    if pred == IntPredicate::Ugt && value == wrap(c2.wrapping_add(smax)) {
        let bound = c.int(ty, wrap(c2.wrapping_neg()));
        return Some(new_icmp(c, inst, IntPredicate::Slt, x, bound));
    }
    // (X + C2) <u C --> X >s ~C2 (if C == C2 + SMIN)
    if pred == IntPredicate::Ult && value == wrap(c2.wrapping_add(smin)) {
        let bound = c.int(ty, wrap(!c2));
        return Some(new_icmp(c, inst, IntPredicate::Sgt, x, bound));
    }
    // (X + C2) >s C --> X <u (SMAX - C) (if C == C2 - 1)
    if pred == IntPredicate::Sgt && value == wrap(c2.wrapping_sub(1)) {
        let bound = c.int(ty, wrap(smax.wrapping_sub(value)));
        return Some(new_icmp(c, inst, IntPredicate::Ult, x, bound));
    }
    // (X + C2) <s C --> X >u (C ^ SMAX) (if C == C2)
    if pred == IntPredicate::Slt && value == c2 {
        let bound = c.int(ty, value ^ smax);
        return Some(new_icmp(c, inst, IntPredicate::Ugt, x, bound));
    }
    // (X + -1) <u C --> X <=u C (if X is never null)
    if pred == IntPredicate::Ult && c2 == m {
        let q = query(c, Some(inst));
        if llrm_mir::valuetracking::nonzero::is_known_non_zero(&q, x, 0) {
            let bound = c.int(ty, value);
            return Some(new_icmp(c, inst, IntPredicate::Ule, x, bound));
        }
    }

    if !has_one_use(c, add) {
        return None;
    }

    // X+C <u C2 -> (X & -C2) == C
    //   iff C & (C2-1) == 0
    //       C2 is a power of 2
    if pred == IntPredicate::Ult && value.is_power_of_two() && c2 & (value - 1) == 0 {
        let neg_value = c.int(ty, wrap(value.wrapping_neg()));
        let masked = c.create_binop(inst, BinaryOp::And, x, neg_value, Flags::default());
        let bound = c.int(ty, wrap(c2.wrapping_neg()));
        return Some(new_icmp(c, inst, IntPredicate::Eq, masked, bound));
    }
    // X+C2 <u C -> (X & C) == 2C
    //   iff C == -(C2)
    //       C2 is a power of 2
    if pred == IntPredicate::Ult && c2.is_power_of_two() && value == wrap(c2.wrapping_neg()) {
        let cv = c.int(ty, value);
        let masked = c.create_binop(inst, BinaryOp::And, x, cv, Flags::default());
        let bound = c.int(ty, wrap(value.wrapping_mul(2)));
        return Some(new_icmp(c, inst, IntPredicate::Ne, masked, bound));
    }
    // X+C >u C2 -> (X & ~C2) != C
    //   iff C & C2 == 0
    //       C2+1 is a power of 2
    if pred == IntPredicate::Ugt && wrap(value.wrapping_add(1)).is_power_of_two() && c2 & value == 0 {
        let not_value = c.int(ty, wrap(!value));
        let masked = c.create_binop(inst, BinaryOp::And, x, not_value, Flags::default());
        let bound = c.int(ty, wrap(c2.wrapping_neg()));
        return Some(new_icmp(c, inst, IntPredicate::Ne, masked, bound));
    }
    // The range test idiom can use either ult or ugt. Arbitrarily
    // canonicalize to the ult form.
    // X+C2 >u C -> X+(C2-C-1) <u ~C
    if pred == IntPredicate::Ugt {
        let offset = c.int(ty, wrap(c2.wrapping_sub(value).wrapping_sub(1)));
        let shifted = c.create_binop(inst, BinaryOp::Add, x, offset, Flags::default());
        let bound = c.int(ty, wrap(!value));
        return Some(new_icmp(c, inst, IntPredicate::Ult, shifted, bound));
    }
    // OWED: zext(V) + C2 pred C -> V + C3 pred' C4 (needs
    // `ConstantRange::truncate` with the no-unsigned-wrap kind, and
    // `shouldChangeType`).
    None
}

/// `foldICmpAndConstant`: `icmp (and X, Y), C`.
fn fold_icmp_and_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    and: Operand,
    (x, y): (Operand, Operand),
    value: u128,
    width: u32,
) -> Option<Visit> {
    if let Some(found) = fold_icmp_and_const_const(c, inst, pred, and, (x, y), value, width) {
        return Some(found);
    }
    if let Some(true_if_neg) = is_sign_bit_check(pred, value, width) {
        let q = query(c, Some(inst));
        // ((X - 1) & ~X) <  0 --> X == 0
        // ((X - 1) & ~X) >= 0 --> X != 0
        let sub_one = pattern::binary(&q, x, BinaryOp::Add)
            .filter(|&(_, _, _, ones)| q.int_constant(ones).is_some_and(|(bits, w)| bits == mask(w)))
            .map(|(_, _, a, _)| a)
            .filter(|&a| pattern::not(&q, y) == Some(a));
        let min_signed_x = neg_and_self(&q, x, y);
        let ty = c.type_of(x);
        if let Some(a) = sub_one {
            let new_pred = if true_if_neg { IntPredicate::Eq } else { IntPredicate::Ne };
            let zero = c.int(ty, 0);
            return Some(new_icmp(c, inst, new_pred, a, zero));
        }
        // (X & -X) <  0 --> X == MinSignedC
        // (X & -X) > -1 --> X != MinSignedC
        if let Some(a) = min_signed_x {
            let new_pred = if true_if_neg { IntPredicate::Eq } else { IntPredicate::Ne };
            let min_signed = c.int(ty, 1u128 << (width - 1));
            return Some(new_icmp(c, inst, new_pred, a, min_signed));
        }
    }
    // OWED: foldCmpLoadFromIndexedGlobal (DEVIATE 1).
    if !is_equality(pred) {
        return None;
    }
    let one_use_and = has_one_use(c, and);
    let ty = c.type_of(and);
    // (X & -X) == 0 --> X == 0
    // (X & -X) != 0 --> X != 0
    // (X & -X) == 1 --> trunc X to i1
    // (X & -X) != 1 --> !(trunc X to i1)
    if width > 1 && (value == 0 || value == 1) {
        let matched = neg_and_self(&query(c, Some(inst)), x, y);
        if let Some(m) = matched {
            if value == 0 {
                let zero = c.int(ty, 0);
                return Some(new_icmp(c, inst, pred, m, zero));
            }
            if pred == IntPredicate::Eq {
                return Some(new_trunc_to_bool(c, inst, m));
            }
            // The original `and` must become dead.
            if one_use_and {
                let i1 = c.context.types.int(1);
                let trunc = c.create_cast(inst, CastOp::Trunc, m, i1, Flags::default());
                return Some(Visit::Replace(c.create_not(inst, trunc)));
            }
        }
    }
    // X & -C == -C -> X >  u ~C
    // X & -C != -C -> X <= u ~C
    //   iff C is a power of 2
    let cmp_rhs = c.function.instruction(inst).operands[1];
    if cmp_rhs == y && is_negated_power_of_two(value, width) {
        let new_pred = if pred == IntPredicate::Eq { IntPredicate::Ugt } else { IntPredicate::Ule };
        let minus_one = c.int(ty, value.wrapping_sub(1) & mask(width));
        return Some(new_icmp(c, inst, new_pred, x, minus_one));
    }
    // ((zext i1 X) & Y) == 0 --> !((trunc Y) & X)
    // ((zext i1 X) & Y) != 0 -->  ((trunc Y) & X)
    // ((zext i1 X) & Y) == 1 -->  ((trunc Y) & X)
    // ((zext i1 X) & Y) != 1 --> !((trunc Y) & X)
    if one_use_and && (value == 0 || value == 1) {
        let found = [(x, y), (y, x)]
            .into_iter()
            .find_map(
                |(first, other)| {
                    let q = query(c, Some(inst));
                    let (_, _, bit) = pattern::cast(&q, first, CastOp::ZExt)?;
                    (has_one_use(c, first) && q.scalar_bits(bit) == 1).then_some((bit, other))
                },
            );
        if let Some((bit, other)) = found {
            let i1 = c.context.types.int(1);
            let trunc_y = c.create_cast(inst, CastOp::Trunc, other, i1, Flags::default());
            if (value == 0) ^ (pred == IntPredicate::Ne) {
                let both = c.create_binop(inst, BinaryOp::And, trunc_y, bit, Flags::default());
                return Some(Visit::Replace(c.create_not(inst, both)));
            }
            return Some(c.replace_with_binop(inst, BinaryOp::And, trunc_y, bit, Flags::default()));
        }
    }
    // (icmp eq/ne (and (shl -1, X), Y), 0)
    //    -> (icmp eq/ne (lshr Y, X), 0)
    if value == 0 && one_use_and {
        let found = [(x, y), (y, x)]
            .into_iter()
            .find_map(
                |(first, other)| {
                    let q = query(c, Some(inst));
                    let (_, _, ones, amount) = pattern::binary(&q, first, BinaryOp::Shl)?;
                    let all_ones = q.int_constant(ones).is_some_and(|(bits, w)| bits == mask(w));
                    (has_one_use(c, first) && all_ones).then_some((amount, other))
                },
            );
        if let Some((amount, other)) = found {
            let lshr = c.create_binop(inst, BinaryOp::LShr, other, amount, Flags::default());
            let zero = c.int(c.type_of(lshr), 0);
            return Some(new_icmp(c, inst, pred, lshr, zero));
        }
    }
    // (icmp eq/ne (and (add A, Addend), Msk), C)
    //    -> (icmp eq/ne (and A, Msk), (and (sub C, Addend), Msk))
    if one_use_and {
        let found = {
            let q = query(c, Some(inst));
            (|| {
                let (a, addend) = {
                    let (_, _, a, addend) = pattern::binary(&q, x, BinaryOp::Add)?;
                    (a, q.int_constant(addend)?.0)
                };
                let (msk, _) = pattern::low_bit_mask(&q, y)?;
                (has_one_use(c, x) && value <= msk).then_some((a, addend, msk))
            })()
        };
        if let Some((a, addend, msk)) = found {
            let new_comperand = value.wrapping_sub(addend) & msk;
            let mask_value = c.int(c.type_of(a), msk);
            let mask_a = c.create_binop(inst, BinaryOp::And, a, mask_value, Flags::default());
            let comperand = c.int(c.type_of(mask_a), new_comperand);
            return Some(new_icmp(c, inst, pred, mask_a, comperand));
        }
    }
    None
}

/// `foldICmpAndConstConst`: `icmp (and X, C2), C1`.
fn fold_icmp_and_const_const(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    and: Operand,
    (x, y): (Operand, Operand),
    c1: u128,
    width: u32,
) -> Option<Visit> {
    let is_ne = pred == IntPredicate::Ne;
    // icmp ne (and X, 1), 0 --> trunc X to i1
    if is_ne && c1 == 0 && query(c, None).int_constant(y) == Some((1, width)) {
        return Some(new_trunc_to_bool(c, inst, x));
    }
    let c2 = query(c, None).int_constant(y)?.0;
    let all = mask(width);
    // (and X, highmask) s> [0, ~highmask] --> X s> ~highmask
    if pred == IntPredicate::Sgt && c1 <= !c2 & all && is_negated_power_of_two(c2, width) {
        let bound = c.int(c.type_of(x), !c2 & all);
        return Some(new_icmp(c, inst, IntPredicate::Sgt, x, bound));
    }
    // (and X, highmask) s< [1, -highmask] --> X s< -highmask
    let sign = 1u128 << (width - 1);
    if pred == IntPredicate::Slt
        && c1 != sign
        && c1.wrapping_sub(1) & all <= !c2 & all
        && is_negated_power_of_two(c2, width)
        && c2 != sign
    {
        let bound = c.int(c.type_of(x), c2.wrapping_neg() & all);
        return Some(new_icmp(c, inst, IntPredicate::Slt, x, bound));
    }
    // Don't perform the following transforms if the AND has multiple uses.
    if !has_one_use(c, and) {
        return None;
    }
    if is_equality(pred) && c1 == 0 {
        // Replace (and X, (1 << size(X)-1) != 0) with X s< 0
        if c2 == sign {
            let zero = c.int(c.type_of(x), 0);
            let new_pred = if is_ne { IntPredicate::Slt } else { IntPredicate::Sge };
            return Some(new_icmp(c, inst, new_pred, x, zero));
        }
        let and_inst = query(c, None).instruction_of(and)?.0;
        let know = {
            let q = query(c, Some(and_inst));
            known::compute_known_bits(&q, x, 0)
        };
        // Set high zeros of C2 to allow matching negated power-of-2.
        let lead = know.count_min_leading_zeros().min(width);
        let high = if lead == 0 { 0 } else { all & !(all >> lead) };
        let new_c2 = c2 | high;
        // ((%x & C) == 0) --> %x u< (-C)  iff (-C) is power of two.
        if is_negated_power_of_two(new_c2, width) {
            let neg_c = c.int(c.type_of(and), new_c2.wrapping_neg() & all);
            let new_pred = if is_ne { IntPredicate::Uge } else { IntPredicate::Ult };
            return Some(new_icmp(c, inst, new_pred, x, neg_c));
        }
    }
    // icmp (and (trunc W), C2), C1 -> icmp (and W, C2'), C1', where the
    // constants have no sign bit or the compare is an equality.
    let wide = {
        let q = query(c, Some(inst));
        pattern::cast(&q, x, CastOp::Trunc).map(|(_, _, w)| w).filter(|_| has_one_use(c, x))
    };
    if let Some(w) = wide
        && (is_equality(pred) || (!is_negative(c1, width) && !is_negative(c2, width)))
    {
        let wide_ty = c.type_of(w);
        let zext_c1 = c.int(wide_ty, c1);
        let zext_c2 = c.int(wide_ty, c2);
        let new_and = c.create_binop(inst, BinaryOp::And, w, zext_c2, Flags::default());
        return Some(new_icmp(c, inst, pred, new_and, zext_c1));
    }
    if let Some(found) = fold_icmp_and_shift(c, inst, pred, and, (x, y), c1, c2, width) {
        return Some(found);
    }
    // (icmp pred (and (or (lshr A, B), A), 1), 0) -->
    // (icmp pred (and A, (or (shl 1, B), 1), 0))
    // iff pred isn't signed
    if !is_signed(pred) && c1 == 0 && has_one_use(c, x) && query(c, None).int_constant(y) == Some((1, width)) {
        let found = {
            let q = query(c, Some(inst));
            (|| {
                let (_, _, lshr, a) = pattern::binary(&q, x, BinaryOp::Or)?;
                let (_, _, shifted, b) = pattern::binary(&q, lshr, BinaryOp::LShr)?;
                (shifted == a).then_some((lshr, a, b))
            })()
        };
        if let Some((lshr, a, b)) = found {
            let removed =
                u32::from(has_one_use(c, and)) + u32::from(has_one_use(c, x)) + u32::from(has_one_use(c, lshr));
            // Compute A & ((1 << B) | 1)
            let imm = matches!(b, Operand::Constant(_));
            let required = if imm { 1 } else { 3 };
            if removed >= required {
                let one = y;
                let shl = c.create_binop(inst, BinaryOp::Shl, one, b, Flags::NUW);
                let new_or = c.create_binop(inst, BinaryOp::Or, shl, one, Flags::default());
                let new_and = c.create_binop(inst, BinaryOp::And, a, new_or, Flags::default());
                let rhs = c.function.instruction(inst).operands[1];
                return Some(new_icmp(c, inst, pred, new_and, rhs));
            }
        }
    }
    // OWED: the `is.fpclass` folds of an `and` of a bitcast (DEVIATE 1).
    None
}

/// `foldICmpAndShift`: `icmp (and (sh X, Y), C2), C1`.
fn fold_icmp_and_shift(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    and: Operand,
    (shift, and_y): (Operand, Operand),
    c1: u128,
    c2: u128,
    width: u32,
) -> Option<Visit> {
    let (shift_op, shift_x, shift_y) = {
        let q = query(c, Some(inst));
        let (_, instruction) = q.instruction_of(shift)?;
        let Opcode::Binary(op @ (BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)) = instruction.opcode else {
            return None;
        };
        (op, instruction.operands[0], instruction.operands[1])
    };
    let all = mask(width);
    let is_shl = shift_op == BinaryOp::Shl;
    let c3 = query(c, None).int_constant(shift_y).map(|(bits, _)| bits);
    // If this is: (X >> C3) & C2 != C1 (where any shift and any compare could
    // exist), turn it into (X & (C2 << C3)) != (C1 << C3).
    if let Some(c3) = c3 {
        let sh = c3.min(u128::from(u32::MAX)) as u32;
        let wide = |v: u128| if sh >= width { 0 } else { v };
        let (new_and, new_cmp, shifted_out);
        let ashr = |v: u128| {
            let negative = is_negative(v, width);
            let s = sh.min(width - 1);
            let shifted = v >> s;
            if negative { (shifted | (all & !(all >> s))) & all } else { shifted }
        };
        match shift_op {
            BinaryOp::Shl => {
                // For a left shift, we can fold if the comparison is not
                // signed, or the mask and comparison values are not negative.
                if is_signed(pred) && (is_negative(c2, width) || is_negative(c1, width)) {
                    return None;
                }
                new_cmp = wide(c1 >> sh.min(127));
                new_and = wide(c2 >> sh.min(127));
                shifted_out = wide((new_cmp << sh.min(127)) & all) != c1;
            }
            BinaryOp::LShr => {
                new_cmp = wide((c1 << sh.min(127)) & all);
                new_and = wide((c2 << sh.min(127)) & all);
                shifted_out = wide(new_cmp >> sh.min(127)) != c1;
                if is_signed(pred) && (is_negative(new_and, width) || is_negative(new_cmp, width)) {
                    return None;
                }
            }
            _ => {
                new_cmp = wide((c1 << sh.min(127)) & all);
                new_and = wide((c2 << sh.min(127)) & all);
                shifted_out = ashr(new_cmp) != c1;
                if ashr(new_and) != c2 {
                    return None;
                }
            }
        }
        if shifted_out {
            // As a special case, the result is always true or false now.
            let i1 = c.context.types.int(1);
            if pred == IntPredicate::Eq {
                return Some(Visit::Replace(c.int(i1, 0)));
            }
            if pred == IntPredicate::Ne {
                return Some(Visit::Replace(c.int(i1, 1)));
            }
        } else {
            let ty = c.type_of(and);
            let and_const = c.int(ty, new_and);
            let masked = c.create_binop(inst, BinaryOp::And, shift_x, and_const, Flags::default());
            let cmp_const = c.int(ty, new_cmp);
            return Some(new_icmp(c, inst, pred, masked, cmp_const));
        }
    }
    // Turn ((X >> Y) & C2) == 0  into  (X & (C2 << Y)) == 0.
    let not_const_x = !matches!(shift_x, Operand::Constant(_));
    if has_one_use(c, shift)
        && c1 == 0
        && is_equality(pred)
        && shift_op != BinaryOp::AShr
        && ((!is_shl && c2 == 1) || not_const_x)
    {
        // Compute C2 << Y.
        let new_shift = if is_shl {
            c.create_binop(inst, BinaryOp::LShr, and_y, shift_y, Flags::default())
        } else {
            c.create_binop(inst, BinaryOp::Shl, and_y, shift_y, Flags::default())
        };
        // Compute X & (C2 << Y).
        let new_and = c.create_binop(inst, BinaryOp::And, shift_x, new_shift, Flags::default());
        let rhs = c.function.instruction(inst).operands[1];
        return Some(new_icmp(c, inst, pred, new_and, rhs));
    }
    None
}

#[cfg(test)]
#[path = "combine_icmp_const_tests.rs"]
mod tests;
