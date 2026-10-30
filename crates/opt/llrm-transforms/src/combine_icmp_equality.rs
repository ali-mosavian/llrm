//! InstCombine's `foldICmpEquality`, from
//! [`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L6222)
//! at d1106deb71cc.
//!
//! [`fold_icmp_equality`] is the pin's function: the same folds in the same
//! order, each arm commented with the C++ it translates. An arm that matched
//! and did not return falls through, as there. `visit_icmp` calls it where
//! `visitICmpInst` does.
//!
//! MIRROR: the order of the arms and the conditions of each.
//!
//! DEVIATE: no vectors. `foldICmpEqualityWithOffset` simplifies without the
//! instruction as context (no dominating conditions or assumptions: the same
//! folds or fewer), and the select it builds does not copy the old select's
//! metadata.
//!
//! # Example
//! `(b & 255) == zext a` is `a == trunc b`:
//!
//! ```text
//! %m = and i16 %b, 255   ; %t = trunc i16 %b to i8
//! %z = zext i8 %a to i16 ; %r = icmp eq i8 %a, %t
//! %r = icmp eq i16 %m, %z
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate};
use llrm_mir::valuetracking::pattern::{self, imm_constant, low_bit_mask};
use llrm_mir::valuetracking::power2::is_known_to_be_a_power_of_two;

use crate::combine::{Combiner, Visit};
use crate::combine_compares::new_icmp;
use crate::combine_shared::{has_one_use, query};

/// The number and width of a constant integer (`m_APInt`).
fn apint(
    c: &Combiner,
    v: Operand,
) -> Option<(u128, u32)> {
    query(c, None).int_constant(v)
}

/// `m_BinOp(op, m_Value(a), m_Value(b))`.
fn binary(
    c: &Combiner,
    v: Operand,
    op: BinaryOp,
    cxt: InstId,
) -> Option<(Operand, Operand)> {
    pattern::binary(&query(c, Some(cxt)), v, op).map(|(_, _, a, b)| (a, b))
}

/// `m_Cast(m_Value(x))` of one opcode.
fn cast(
    c: &Combiner,
    v: Operand,
    op: CastOp,
    cxt: InstId,
) -> Option<Operand> {
    pattern::cast(&query(c, Some(cxt)), v, op).map(|(_, _, x)| x)
}

/// `m_OneUse(m_BinOp(op, a, b))`.
fn one_use_binary(
    c: &Combiner,
    v: Operand,
    op: BinaryOp,
    cxt: InstId,
) -> Option<(Operand, Operand)> {
    if has_one_use(c, v) { binary(c, v, op, cxt) } else { None }
}

/// `APInt::isNegatedPowerOf2`.
fn is_negated_power_of_two(
    bits: u128,
    width: u32,
) -> bool {
    bits >> (width - 1) & 1 == 1 && (bits.wrapping_neg() & mask(width)).is_power_of_two()
}

/// `m_ImmConstant`.
fn is_imm(
    c: &Combiner,
    v: Operand,
) -> bool {
    imm_constant(&query(c, None), v)
}

/// `foldICmpEquality`.
pub fn fold_icmp_equality(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    if !matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
        return None;
    }
    let ty = c.type_of(op0);
    let width = c.context.types.int_bits(ty)?;

    if let Some((a, b)) = binary(c, op0, BinaryOp::Xor, inst) {
        // (A^B) == A  ->  B == 0
        if a == op1 || b == op1 {
            let other = if a == op1 { b } else { a };
            let zero = c.int(c.type_of(a), 0);
            return Some(new_icmp(c, inst, pred, other, zero));
        }
        if let Some((cc, d)) = binary(c, op1, BinaryOp::Xor, inst) {
            // A^c1 == C^c2 --> A == C^(c1^c2)
            if let (Some((c1, _)), Some((c2, _))) = (apint(c, b), apint(c, d))
                && has_one_use(c, op1)
            {
                let new = c.int(ty, c1 ^ c2);
                let xor = c.create_binop(inst, BinaryOp::Xor, cc, new, Flags::default());
                return Some(new_icmp(c, inst, pred, a, xor));
            }
            // A^B == A^D -> B == D
            if a == cc {
                return Some(new_icmp(c, inst, pred, b, d));
            }
            if a == d {
                return Some(new_icmp(c, inst, pred, b, cc));
            }
            if b == cc {
                return Some(new_icmp(c, inst, pred, a, d));
            }
            if b == d {
                return Some(new_icmp(c, inst, pred, a, cc));
            }
        }
    }

    // A == (A^B)  ->  B == 0
    if let Some((a, b)) = binary(c, op1, BinaryOp::Xor, inst)
        && (a == op0 || b == op0)
    {
        let other = if a == op0 { b } else { a };
        let zero = c.int(c.type_of(a), 0);
        return Some(new_icmp(c, inst, pred, other, zero));
    }

    // (X&Z) == (Y&Z) -> (X^Y) & Z == 0
    if let (Some((a, b)), Some((cc, d))) = (binary(c, op0, BinaryOp::And, inst), binary(c, op1, BinaryOp::And, inst)) {
        let found = if a == cc {
            Some((b, d, a))
        } else if a == d {
            Some((b, cc, a))
        } else if b == cc {
            Some((a, d, b))
        } else if b == d {
            Some((a, cc, b))
        } else {
            None
        };
        if let Some((x, y, z)) = found {
            // If X^Y is a negative power of two, then `icmp eq/ne (Z & NegP2),
            // 0` will fold to `icmp ult/uge Z, -NegP2` incurring no
            // additional instructions.
            let x_is_neg_p2 = match (apint(c, x), apint(c, y)) {
                (Some((c0, _)), Some((c1, _))) => is_negated_power_of_two((c0 ^ c1) & mask(width), width),
                _ => false,
            };
            // If either Op0/Op1 are both one use or X^Y will constant fold and
            // one of Op0/Op1 are one use, proceed.
            let use_count = i32::from(has_one_use(c, op0))
                + i32::from(has_one_use(c, op1))
                + i32::from(is_imm(c, x) && is_imm(c, y));
            if x_is_neg_p2 || use_count >= 2 {
                let xor = c.create_binop(inst, BinaryOp::Xor, x, y, Flags::default());
                let and = c.create_binop(inst, BinaryOp::And, xor, z, Flags::default());
                let zero = c.int(c.type_of(and), 0);
                return Some(new_icmp(c, inst, pred, and, zero));
            }
        }
    }

    // (X | C) == (Y | C) --> (X ^ Y) & ~C == 0
    if let (Some((x, k)), Some((y, k1))) =
        (one_use_binary(c, op0, BinaryOp::Or, inst), one_use_binary(c, op1, BinaryOp::Or, inst))
        && let Some((bits, _)) = apint(c, k)
        && k == k1
    {
        let xor = c.create_binop(inst, BinaryOp::Xor, x, y, Flags::default());
        let not = c.int(ty, !bits & mask(width));
        let and = c.create_binop(inst, BinaryOp::And, xor, not, Flags::default());
        let zero = c.int(c.type_of(and), 0);
        return Some(new_icmp(c, inst, pred, and, zero));
    }

    // (B & (Pow2C-1)) == zext A --> A == trunc B
    // (B & (Pow2C-1)) != zext A --> A != trunc B
    if let Some(a) = cast(c, op1, CastOp::ZExt, inst)
        && (has_one_use(c, op0) || has_one_use(c, op1))
        && let Some((b, k)) = binary(c, op0, BinaryOp::And, inst)
        && let Some((mask_bits, _)) = low_bit_mask(&query(c, Some(inst)), k)
        && Some(mask_bits.trailing_ones()) == c.context.types.int_bits(c.type_of(a))
    {
        let narrow = c.type_of(a);
        let trunc = c.create_cast(inst, CastOp::Trunc, b, narrow, Flags::default());
        return Some(new_icmp(c, inst, pred, a, trunc));
    }

    // (A >> C) == (B >> C) --> (A^B) u< (1 << C), for lshr and ashr pairs.
    for shift in [BinaryOp::LShr, BinaryOp::AShr] {
        let (Some((a, k0)), Some((b, k1))) = (one_use_binary(c, op0, shift, inst), one_use_binary(c, op1, shift, inst))
        else {
            continue;
        };
        let (Some((ap1, _)), Some((ap2, _))) = (apint(c, k0), apint(c, k1)) else { continue };
        if ap1 != ap2 {
            return None;
        }
        let sh_amt = ap1.min(u128::from(width));
        if sh_amt < u128::from(width) && sh_amt != 0 {
            let new_pred = if pred == IntPredicate::Ne { IntPredicate::Uge } else { IntPredicate::Ult };
            let xor = c.create_binop(inst, BinaryOp::Xor, a, b, Flags::default());
            let limit = c.int(c.type_of(a), 1u128 << sh_amt);
            return Some(new_icmp(c, inst, new_pred, xor, limit));
        }
        break;
    }

    // (A << C) == (B << C) --> ((A^B) & (~0U >> C)) == 0
    if let (Some((a, k0)), Some((b, k1))) =
        (one_use_binary(c, op0, BinaryOp::Shl, inst), one_use_binary(c, op1, BinaryOp::Shl, inst))
        && let Some((sh, _)) = apint(c, k0)
        && k0 == k1
        && sh < u128::from(width)
        && sh != 0
    {
        let xor = c.create_binop(inst, BinaryOp::Xor, a, b, Flags::default());
        let low = c.int(ty, mask(width - sh as u32));
        let and = c.create_binop(inst, BinaryOp::And, xor, low, Flags::default());
        let zero = c.int(c.type_of(and), 0);
        return Some(new_icmp(c, inst, pred, and, zero));
    }

    // icmp eq (trunc (lshr A, cst1)), cst --> icmp (and A, mask), cst
    if has_one_use(c, op0)
        && let Some(t) = cast(c, op0, CastOp::Trunc, inst)
        && let Some((a, k)) = one_use_binary(c, t, BinaryOp::LShr, inst)
        && let Some((sh, _)) = apint(c, k)
        && let Some((cst, _)) = apint(c, op1)
        // Only when A has multiple uses: it is most important when it exposes
        // other optimizations.
        && !has_one_use(c, a)
    {
        let a_ty = c.type_of(a);
        let a_size = c.context.types.int_bits(a_ty)?;
        if sh < u128::from(a_size) {
            let sh = sh as u32;
            let mask_v = (mask(width) << sh) & mask(a_size);
            let cmp_v = (cst << sh) & mask(a_size);
            let mask_c = c.int(a_ty, mask_v);
            let and = c.create_binop(inst, BinaryOp::And, a, mask_c, Flags::default());
            let cmp_c = c.int(a_ty, cmp_v);
            return Some(new_icmp(c, inst, pred, and, cmp_c));
        }
    }

    if let Some(r) = fold_icmp_intrinsic_with_intrinsic(c, inst, pred, op0, op1) {
        return Some(r);
    }

    // icmp eq (trunc (lshr A, BW), (ashr (trunc A), BW-1)): the top BW/2 + 1
    // bits are all the same, `icmp ult (add A, 2^(BW-1)), 2^BW`.
    if let Some((inner, amount)) = binary(c, op0, BinaryOp::AShr, inst)
        && let Some(a) = cast(c, inner, CastOp::Trunc, inst)
        && apint(c, amount).is_some_and(|(v, _)| v == u128::from(width) - 1)
        && let Some(t1) = cast(c, op1, CastOp::Trunc, inst)
        && let Some((a1, amount1)) = binary(c, t1, BinaryOp::LShr, inst)
        && a1 == a
        && apint(c, amount1).is_some_and(|(v, _)| v == u128::from(width))
        && c.context.types.int_bits(c.type_of(a)) == Some(width * 2)
        && (has_one_use(c, op0) || has_one_use(c, op1))
    {
        let a_ty = c.type_of(a);
        let half = 1u128 << (width - 1);
        let bias = c.int(a_ty, half);
        let add = c.create_binop(inst, BinaryOp::Add, a, bias, Flags::default());
        let limit = c.int(a_ty, half << 1);
        let new_pred = if pred == IntPredicate::Eq { IntPredicate::Ult } else { IntPredicate::Uge };
        return Some(new_icmp(c, inst, new_pred, add, limit));
    }

    // Canonicalize, assuming B_Pow2 != 0:
    // A & B_Pow2 != B_Pow2 -> A & B_Pow2 == 0
    // A & B_Pow2 == B_Pow2 -> A & B_Pow2 != 0
    for (and_side, other) in [(op0, op1), (op1, op0)] {
        let has_other =
            pattern::commutative_with(&query(c, Some(inst)), and_side, BinaryOp::And, |o| o == other).is_some();
        if has_other && is_known_to_be_a_power_of_two(&query(c, Some(inst)), other, false, 0) {
            let zero = c.int(c.type_of(and_side), 0);
            return Some(new_icmp(c, inst, pred.inverse(), and_side, zero));
        }
    }

    // icmp eq/ne X, OneUse(rotate-right(X)) -> icmp eq/ne X, rotate-left(X)
    for (a, rot) in [(op0, op1), (op1, op0)] {
        if !has_one_use(c, rot) {
            continue;
        }
        let found = pattern::intrinsic_call(&query(c, Some(inst)), rot, "llvm.fshr")
            .and_then(|(_, args)| <[Operand; 3]>::try_from(args).ok())
            .filter(|[x, y, _]| *x == a && *y == a);
        if let Some([_, _, amount]) = found {
            let rot_ty = c.type_of(op0);
            let fshl = c.create_intrinsic(inst, "fshl", rot_ty, &[a, a, amount]);
            return Some(new_icmp(c, inst, pred, a, fshl));
        }
    }

    // icmp eq/ne OneUse(A ^ Cst), B --> icmp eq/ne (A ^ B), Cst
    for (xor_side, b) in [(op0, op1), (op1, op0)] {
        if let Some((a, k)) = one_use_binary(c, xor_side, BinaryOp::Xor, inst)
            && is_imm(c, k)
            && !is_imm(c, b)
        {
            let xor = c.create_binop(inst, BinaryOp::Xor, a, b, Flags::default());
            return Some(new_icmp(c, inst, pred, xor, k));
        }
    }

    // (icmp eq/ne (and (add/sub/xor X, P2), P2), P2) -> (icmp eq/ne (and X,
    // P2), 0) (icmp eq/ne (and (add/sub/xor X, P2), P2), 0)  -> (icmp eq/ne
    // (and X, P2), P2)
    if let Some((a, b, is_zero)) = and_of_sum_with_power_of_two(c, inst, op0, op1)
        && is_known_to_be_a_power_of_two(&query(c, Some(inst)), a, true, 0)
    {
        let and = c.create_binop(inst, BinaryOp::And, b, a, Flags::default());
        let rhs = if is_zero { a } else { c.int(c.type_of(a), 0) };
        return Some(new_icmp(c, inst, pred, and, rhs));
    }

    fold_icmp_equality_with_offset(c, inst, pred, op0, op1)
}

/// `foldICmpIntrinsicWithIntrinsic`
/// ([:4040](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L4040)):
/// two calls of the same `bswap`, `bitreverse` or rotate compare their
/// operands; two rotates by different amounts become one rotate.
fn fold_icmp_intrinsic_with_intrinsic(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let (name0, name1, a0, a1) = {
        let q = query(c, Some(inst));
        let (call0, _) = q.instruction_of(op0)?;
        let (call1, _) = q.instruction_of(op1)?;
        (q.callee_name(call0)?.to_owned(), q.callee_name(call1)?.to_owned(), args_of(&q, op0)?, args_of(&q, op1)?)
    };
    if name0 != name1 {
        return None;
    }
    let base = |names: &str| names.strip_prefix("llvm.").map(|rest| rest.split('.').next().unwrap_or(rest).to_owned());
    match base(&name0)?.as_str() {
        // If both operands are byte-swapped or bit-reversed, just compare the
        // original values.
        "bswap" | "bitreverse" => Some(new_icmp(c, inst, pred, a0[0], a1[0])),
        "fshl" | "fshr" => {
            // If both operands are rotated by same amount, just compare the
            // original values.
            if a0[0] != a0[1] || a1[0] != a1[1] {
                return None;
            }
            if a0[2] == a1[2] {
                return Some(new_icmp(c, inst, pred, a0[0], a1[0]));
            }
            // rotate(X, AmtX) == rotate(Y, AmtY) -> rotate(X, AmtX - AmtY) == Y
            // when both rotates have one use, or one has and the amounts are
            // constants.
            let bw = c.context.types.int_bits(c.type_of(op0))?;
            let one_uses = i32::from(has_one_use(c, op0)) + i32::from(has_one_use(c, op1));
            if (one_uses == 2 || (one_uses == 1 && is_imm(c, a0[2]) && is_imm(c, a1[2]))) && bw.is_power_of_two() {
                let ty = c.type_of(op0);
                let sub = c.create_sub(inst, a0[2], a1[2]);
                let kind = base(&name0)?;
                let rotate = c.create_intrinsic(inst, &kind, ty, &[a0[0], a0[0], sub]);
                return Some(new_icmp(c, inst, pred, a1[0], rotate));
            }
            None
        }
        _ => None,
    }
}

/// The arguments of the call `v`.
fn args_of(
    q: &llrm_mir::valuetracking::Query,
    v: Operand,
) -> Option<Vec<Operand>> {
    let (_, instruction) = q.instruction_of(v)?;
    matches!(instruction.opcode, llrm_mir::opcode::Opcode::Call(_))
        .then(|| instruction.operands[..instruction.operands.len() - 1].to_vec())
}

/// `OffsetOp`: `V op RHS` may simplify.
type OffsetOp = (BinaryOp, Operand);

/// `collectOffsetOp`: every pair (BinOp, RHS) that `BinOp V, RHS` may
/// simplify.
fn collect_offset_op(
    c: &Combiner,
    v: Operand,
    offsets: &mut Vec<OffsetOp>,
    allow_recursion: bool,
) {
    use llrm_mir::opcode::Opcode;
    let q = query(c, None);
    let Some((_, instruction)) = q.instruction_of(v) else { return };
    if !has_one_use(c, v) {
        return;
    }
    let ops = &instruction.operands;
    match instruction.opcode {
        Opcode::Binary(BinaryOp::Add) => {
            offsets.push((BinaryOp::Sub, ops[1]));
            offsets.push((BinaryOp::Sub, ops[0]));
        }
        Opcode::Binary(BinaryOp::Sub) => offsets.push((BinaryOp::Add, ops[1])),
        Opcode::Binary(BinaryOp::Xor) => {
            offsets.push((BinaryOp::Xor, ops[1]));
            offsets.push((BinaryOp::Xor, ops[0]));
        }
        Opcode::Binary(BinaryOp::Shl) => {
            if instruction.flags.contains(Flags::NSW) {
                offsets.push((BinaryOp::AShr, ops[1]));
            }
            if instruction.flags.contains(Flags::NUW) {
                offsets.push((BinaryOp::LShr, ops[1]));
            }
        }
        Opcode::Select if allow_recursion => {
            let arms = [ops[1], ops[2]];
            for arm in arms {
                collect_offset_op(c, arm, offsets, false);
            }
        }
        _ => {}
    }
}

/// `OffsetResult`: a value, or a select to build.
enum OffsetResult {
    Invalid,
    Value(Operand),
    Select(Operand, Operand, Operand),
}

/// `ApplyOffsetImpl`: `V op RHS` simplified, when it introduces no poison.
fn apply_offset_impl(
    c: &Combiner,
    inst: InstId,
    v: Operand,
    op: BinaryOp,
    rhs: Operand,
) -> Option<Operand> {
    let q = query(c, Some(inst));
    match op {
        // V = shl nsw X, RHS => X = ashr V, RHS
        // V = shl nuw X, RHS => X = lshr V, RHS
        BinaryOp::AShr | BinaryOp::LShr => {
            let exact_shift = match (q.int_constant(v), q.int_constant(rhs)) {
                (Some((cv, width)), Some((crhs, _))) if crhs < u128::from(width) => {
                    let back = if op == BinaryOp::AShr {
                        (llrm_mir::context::signed(cv, width) >> crhs) as u128 & mask(width)
                    } else {
                        cv >> crhs
                    };
                    back << crhs & mask(width) == cv
                }
                _ => false,
            };
            let flag = if op == BinaryOp::AShr { Flags::NSW } else { Flags::NUW };
            let wrapped_shl = pattern::binary(&q, v, BinaryOp::Shl)
                .is_some_and(|(_, instruction, _, amount)| amount == rhs && instruction.flags.contains(flag));
            if !exact_shift && !wrapped_shl {
                return None;
            }
        }
        _ => {}
    }
    let simplified =
        llrm_mir::simplify::simplify_binop_operands(c.context, c.env.layout, c.function, op, Flags::default(), v, rhs)?;
    // Reject constant expressions as they don't simplify things.
    if matches!(simplified, Operand::Constant(_)) && !pattern::imm_constant(&q, simplified) {
        return None;
    }
    // Check if the transformation introduces poison.
    llrm_mir::valuetracking::guarantee::implies_poison(&q, rhs, v).then_some(simplified)
}

/// `ApplyOffset`.
fn apply_offset(
    c: &Combiner,
    inst: InstId,
    v: Operand,
    op: BinaryOp,
    rhs: Operand,
) -> OffsetResult {
    use llrm_mir::opcode::Opcode;
    let select = {
        let q = query(c, Some(inst));
        q.instruction_of(v)
            .filter(|(_, i)| i.opcode == Opcode::Select)
            .map(|(_, i)| (i.operands[0], i.operands[1], i.operands[2]))
    };
    if let Some((cond, if_true, if_false)) = select {
        if !has_one_use(c, v) {
            return OffsetResult::Invalid;
        }
        let Some(t) = apply_offset_impl(c, inst, if_true, op, rhs) else { return OffsetResult::Invalid };
        let Some(f) = apply_offset_impl(c, inst, if_false, op, rhs) else { return OffsetResult::Invalid };
        return OffsetResult::Select(cond, t, f);
    }
    match apply_offset_impl(c, inst, v, op, rhs) {
        Some(found) => OffsetResult::Value(found),
        None => OffsetResult::Invalid,
    }
}

/// `foldICmpEqualityWithOffset`
/// ([:6140](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L6140)):
/// offset both sides of an equality to save instructions:
/// `icmp eq/ne X, Y -> icmp eq/ne X op Z, Y op Z`.
fn fold_icmp_equality_with_offset(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    let mut offsets = Vec::new();
    collect_offset_op(c, op0, &mut offsets, true);
    collect_offset_op(c, op1, &mut offsets, true);
    for (op, rhs) in offsets {
        let r0 = apply_offset(c, inst, op0, op, rhs);
        if matches!(r0, OffsetResult::Invalid) {
            continue;
        }
        let r1 = apply_offset(c, inst, op1, op, rhs);
        if matches!(r1, OffsetResult::Invalid) {
            continue;
        }
        let mut materialize = |result: OffsetResult| match result {
            OffsetResult::Value(v) => v,
            OffsetResult::Select(cond, t, f) => c.create_select(inst, cond, t, f),
            OffsetResult::Invalid => unreachable!("checked above"),
        };
        let lhs = materialize(r0);
        let rhs = materialize(r1);
        return Some(new_icmp(c, inst, pred, lhs, rhs));
    }
    None
}

/// `m_c_ICmp(m_OneUse(m_c_And(m_Value(A), Matcher)), m_Deferred(A))` or
/// `m_ICmp(m_OneUse(m_c_And(m_Value(A), Matcher)), m_Zero())`, `Matcher` being
/// `m_c_Add(m_Value(B), m_Deferred(A))`, `m_c_Xor(..)` or
/// `m_Sub(m_Value(B), m_Deferred(A))`: `(A, B, IsZero)`.
fn and_of_sum_with_power_of_two(
    c: &Combiner,
    inst: InstId,
    op0: Operand,
    op1: Operand,
) -> Option<(Operand, Operand, bool)> {
    let q = query(c, Some(inst));
    // The `and`'s first operand binds A, the other must be the matcher.
    let matched = |and: Operand| -> Vec<(Operand, Operand)> {
        let mut found = Vec::new();
        if !has_one_use(c, and) {
            return found;
        }
        let Some((_, _, x, y)) = pattern::binary(&q, and, BinaryOp::And) else { return found };
        for (a, m) in [(x, y), (y, x)] {
            let b = [BinaryOp::Add, BinaryOp::Xor]
                .into_iter()
                .find_map(
                    |op| {
                        let (_, _, p, r) = pattern::binary(&q, m, op)?;
                        if r == a {
                            Some(p)
                        } else if p == a {
                            Some(r)
                        } else {
                            None
                        }
                    },
                );
            let b = b.or_else(|| {
                let (_, _, p, r) = pattern::binary(&q, m, BinaryOp::Sub)?;
                (r == a).then_some(p)
            });
            if let Some(b) = b {
                found.push((a, b));
            }
        }
        found
    };
    // IsZero = false: the compare's other side is A itself, either order.
    for (and, other) in [(op0, op1), (op1, op0)] {
        if let Some(&(a, b)) = matched(and).iter().find(|(a, _)| *a == other) {
            return Some((a, b, false));
        }
    }
    // IsZero = true: `icmp (and ..), 0`.
    if pattern::zero(&q, op1)
        && let Some(&(a, b)) = matched(op0).first()
    {
        return Some((a, b, true));
    }
    None
}

#[cfg(test)]
#[path = "combine_icmp_equality_tests.rs"]
mod tests;
