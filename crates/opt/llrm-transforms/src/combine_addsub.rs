//! InstCombine's `visitAdd` and the helpers it calls, from
//! [`InstCombineAddSub.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp)
//! at d1106deb71cc.
//!
//! [`visit_add`] is
//! [`visitAdd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAddSub.cpp#L1604):
//! the same steps in the same order, each a function named for the C++ one
//! with its own permalink. A step that returns `nullptr` there returns `None`
//! here, one that returns an instruction returns `Some(Visit)` (a new
//! instruction replaces the visited one; `&I` is `Visit::Changed`).
//!
//! MIRROR: the order of the steps and each condition.
//!
//! DEVIATE 1: `foldVectorBinop` is not translated: MIR has no vector
//! operations.
//!
//! DEVIATE 2 (temporary, `uselist-port`): steps that call a helper another
//! family owns are left out and named where they stood:
//! Each is a `// DEVIATE 2:` comment at its place in the order.
//! `foldBinopWithPhiOperands`, `foldBinOpIntoSelectOrPhi` and
//! `foldBinOpSelectBinOp` are the phi and select families'.
//!
//! DEVIATE 3 (temporary): the folds that build an intrinsic call (`usub.sat`,
//! `uadd.sat`, `umax`, `umin`, `scmp`, `ctpop`...) are left out until the
//! Builder can declare an intrinsic (`combine_build` DEVIATE 2); each is a
//! `// DEVIATE 3:` comment at its place.
//!
//! DEVIATE 4: a constant that `ConstantExpr::getAdd` would leave as an
//! expression is not built (MIR holds no binary constant expressions): the
//! fold is skipped where the constants do not fold to a number.
//!
//! # Used by
//! `combine::visit`, once the add family's slot is flipped.
//!
//! # How it works
//! `visit_add` returns the first step that changes the add. The steps rewrite
//! it to a cheaper form (`x + x` to `x << 1`, `-a + b` to `b - a`, constants
//! combined through a `sub` or a `zext`), or replace it by an existing value.
//! A rewrite that needs new operands builds them with the `Combiner`'s
//! builder, which folds through `simplify*` first.
//!
//! # Example
//! `(x + 1) + ~y` is `x - y`:
//!
//! ```text
//! %a = add i32 %x, 1
//! %n = xor i32 %y, -1
//! %r = add i32 %a, %n   ; %r = sub i32 %x, %y
//! ```

use llrm_mir::context::{mask, signed};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::range::{
    OverflowResult, compute_overflow_for_signed_add, compute_overflow_for_signed_sub,
};
use llrm_mir::valuetracking::{Query, known, nonzero, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_add_idioms::fold_add_rest;
use crate::combine_distribute::fold_using_distributive_laws;
use crate::combine_shared::{
    binop, commutative, fold_constants, has_one_use, inst_def, parts, query, simplify_associative_or_commutative,
    simplify_visited, specific_int, width, wrap_of,
};
use crate::combine_shift_binop::fold_binop_shift_with_shift;

/// `InstCombiner::AddOne` and `SubOne` of a constant.
pub fn add_to(
    c: &Combiner,
    v: Operand,
    delta: i128,
) -> Option<Operand> {
    let one = c.int(c.type_of(v), delta as u128);
    fold_constants(c, BinaryOp::Add, v, one)
}

/// `willNotOverflowSignedAdd(l, r, I)`: never overflows, asked at `cxt`.
fn will_not_overflow_signed_add(
    c: &Combiner,
    cxt: InstId,
    l: Operand,
    r: Operand,
) -> bool {
    compute_overflow_for_signed_add(&query(c, Some(cxt)), l, r) == OverflowResult::NeverOverflows
}

/// `willNotOverflowSignedSub(l, r, I)`.
pub fn will_not_overflow_signed_sub(
    c: &Combiner,
    cxt: InstId,
    l: Operand,
    r: Operand,
) -> bool {
    compute_overflow_for_signed_sub(&query(c, Some(cxt)), l, r) == OverflowResult::NeverOverflows
}

/// `m_SExtLike`: `sext x`, or `zext nneg x`.
fn sext_like(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    if let Some((_, _, x)) = pattern::cast(q, v, CastOp::SExt) {
        return Some(x);
    }
    let (_, instruction, x) = pattern::cast(q, v, CastOp::ZExt)?;
    instruction.flags.contains(Flags::NNEG).then_some(x)
}

/// `foldBoxMultiply` (:1525): `((XLow * YHigh) + (YLow * XHigh)) << HalfBits) +
/// (XLow * YLow)` is `X * Y`.
fn fold_box_multiply(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (lhs, rhs, _) = parts(c, inst);
    let bits = width(c, lhs);
    // Skip the odd bitwidth types.
    if bits & 1 == 1 {
        return None;
    }
    let half = bits >> 1;
    let half_mask = mask(half);
    let found = {
        let q = query(c, Some(inst));
        // ResLo = (CrossSum << HalfBits) + (YLo * XLo); the multiply has one
        // use so the number of multiplications does not grow.
        let shifted = |v: Operand| {
            binop(&q, v, BinaryOp::Shl)
                .filter(|&(_, amount, _)| specific_int(&q, amount, half as u128))
                .map(|(a, ..)| a)
        };
        let product = |v: Operand| if has_one_use(c, v) { binop(&q, v, BinaryOp::Mul) } else { None };
        let (cross_sum, y_lo, x_lo) = [(lhs, rhs), (rhs, lhs)]
            .into_iter()
            .find_map(
                |(a, b)| {
                    let cross_sum = shifted(a)?;
                    let (y_lo, x_lo, _) = product(b)?;
                    Some((cross_sum, y_lo, x_lo))
                },
            )?;
        // XLo = X & HalfMask, YLo = Y & HalfMask.
        let masked = |v: Operand| {
            let (x, m, _) = binop(&q, v, BinaryOp::And)?;
            specific_int(&q, m, half_mask).then_some(x)
        };
        let (x, y) = (masked(x_lo)?, masked(y_lo)?);
        // CrossSum = (X' * (Y >> Halfbits)) + (Y' * (X >> HalfBits)), X' being
        // X or XLo and Y' being Y or YLo.
        let high_times = |v: Operand, of: Operand, a: Operand, b: Operand| {
            commutative(&q, v, BinaryOp::Mul, |one| {
                binop(&q, one, BinaryOp::LShr)
                    .is_some_and(|(shifted, amount, _)| shifted == of && specific_int(&q, amount, half as u128))
            })
            .is_some_and(|(_, other)| other == a || other == b)
        };
        let (p, r, _) = binop(&q, cross_sum, BinaryOp::Add)?;
        let ok = (high_times(p, y, x, x_lo) && high_times(r, x, y, y_lo))
            || (high_times(r, y, x, x_lo) && high_times(p, x, y, y_lo));
        ok.then_some((x, y))
    };
    let (x, y) = found?;
    Some(c.replace_with_binop(inst, BinaryOp::Mul, x, y, Flags::default()))
}

/// `factorizeMathWithShlOps` (:1489): `add/sub (X << ShAmt), (Y << ShAmt)` is
/// `(add/sub X, Y) << ShAmt`. Called for `add` and `sub`.
pub fn factorize_math_with_shl_ops(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let Opcode::Binary(op) = c.function.instruction(inst).opcode else { return None };
    let (lhs, rhs, flags) = parts(c, inst);
    let found = {
        let q = query(c, Some(inst));
        let (x, shamt, f0) = binop(&q, lhs, BinaryOp::Shl)?;
        let (y, shamt1, f1) = binop(&q, rhs, BinaryOp::Shl)?;
        // Both are BinaryOperators with at least one used once.
        if shamt != shamt1 || !(has_one_use(c, lhs) || has_one_use(c, rhs)) {
            return None;
        }
        // No-wrap propagates only when all ops have no-wrap.
        let both = |flag: Flags| flags.contains(flag) && f0.contains(flag) && f1.contains(flag);
        (x, y, shamt, both(Flags::NSW), both(Flags::NUW))
    };
    let (x, y, shamt, nsw, nuw) = found;
    let mut wrap = Flags::default();
    if nsw {
        wrap.insert(Flags::NSW);
    }
    if nuw {
        wrap.insert(Flags::NUW);
    }
    // add/sub (X << ShAmt), (Y << ShAmt) --> (add/sub X, Y) << ShAmt
    let new_math = c.create_binop(inst, op, x, y, Flags::default());
    if let Operand::Value(value) = new_math
        && let llrm_mir::module::ValueDef::Instruction(made) = c.function.value(value).def
        && c.function.instruction(made).opcode == Opcode::Binary(op)
        && c.function.instruction(made).flags != wrap
        && c.function.instruction(made).operands == [x, y]
    {
        c.function.set_flags(made, wrap);
    }
    Some(c.replace_with_binop(inst, BinaryOp::Shl, new_math, shamt, wrap))
}

/// `combineAddSubWithShlAddSub` (:1291): `add A, (shl (neg B), Y)` is `sub A,
/// (shl B, Y)`.
fn combine_add_sub_with_shl_add_sub(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (lhs, rhs, _) = parts(c, inst);
    let (a, b, count) = {
        let q = query(c, Some(inst));
        // m_c_Add(m_OneUse(m_Shl(m_OneUse(m_Neg(m_Value(B))), m_Value(Cnt))),
        // m_Value(A))
        let shl_of_neg = |v: Operand| {
            if !has_one_use(c, v) {
                return None;
            }
            let (neg, count, _) = binop(&q, v, BinaryOp::Shl)?;
            if !has_one_use(c, neg) {
                return None;
            }
            let (_, b, _) = pattern::neg(&q, neg)?;
            Some((b, count))
        };
        let (b, count, a) =
            shl_of_neg(lhs).map(|(b, n)| (b, n, rhs)).or_else(|| shl_of_neg(rhs).map(|(b, n)| (b, n, lhs)))?;
        (a, b, count)
    };
    let new_shl = c.create_shl(inst, b, count);
    Some(c.replace_with_binop(inst, BinaryOp::Sub, a, new_shl, Flags::default()))
}

/// `foldNoWrapAdd` (:809): wrapping flags may allow combining constants
/// separated by an extend.
fn fold_no_wrap_add(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, _) = parts(c, inst);
    if !matches!(op1, Operand::Constant(_)) {
        return None;
    }
    let ty = c.type_of(inst_value(c, inst));
    // (zext (X +nuw C2)) + C1 --> zext (X + (C2 + trunc(C1)))
    let narrow = {
        let q = query(c, Some(inst));
        pattern::apint(&q, op1).and_then(|(c1, wide)| {
            let (_, _, source) = pattern::cast(&q, op0, CastOp::ZExt)?;
            let (x, c2_operand) = pattern::add_like(&q, source, Flags::NUW)?;
            let (c2, narrow_bits) = pattern::apint(&q, c2_operand)?;
            Some((c1, wide, x, c2, narrow_bits))
        })
    };
    if let Some((c1, wide, x, c2, narrow_bits)) = narrow {
        // C1->isNegative() && C1->sge(-C2->sext(C1->getBitWidth()))
        let negative = signed(c1, wide) < 0;
        let bound = -signed(c2, narrow_bits);
        if negative && signed(c1, wide) >= bound {
            let new_c = c2.wrapping_add(c1) & mask(narrow_bits);
            // If the smaller add will fold to zero, we don't need to check one
            // use.
            if new_c == 0 {
                return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![x], Flags::default()));
            }
            // Otherwise only do this if the existing zero extend will be
            // removed.
            if has_one_use(c, op0) {
                let nc = c.int(c.type_of(x), new_c);
                let sum = c.create_binop(inst, BinaryOp::Add, x, nc, Flags::NUW);
                return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![sum], Flags::default()));
            }
        }
    }
    // More general combining of constants in the wide type.
    // (sext (X +nsw NarrowC)) + C --> (sext X) + (sext(NarrowC) + C)
    // or (zext nneg (X +nsw NarrowC)) + C --> (sext X) + (sext(NarrowC) + C)
    let wide = |c: &Combiner, extend: bool| -> Option<(Operand, Operand)> {
        let q = query(c, Some(inst));
        if !has_one_use(c, op0) {
            return None;
        }
        let inner = if extend {
            pattern::cast(&q, op0, CastOp::ZExt).map(|(_, _, source)| source)?
        } else {
            sext_like(&q, op0)?
        };
        let (x, narrow_c) = pattern::add_like(&q, inner, if extend { Flags::NUW } else { Flags::NSW })?;
        matches!(narrow_c, Operand::Constant(_)).then_some((x, narrow_c))
    };
    for extend in [false, true] {
        if let Some((x, narrow_c)) = wide(c, extend) {
            let op = if extend { CastOp::ZExt } else { CastOp::SExt };
            let wide_c = c.create_cast(inst, op, narrow_c, ty, Flags::default());
            let new_c = c.create_add(inst, wide_c, op1);
            let wide_x = c.create_cast(inst, op, x, ty, Flags::default());
            return Some(c.replace_with_binop(inst, BinaryOp::Add, wide_x, new_c, Flags::default()));
        }
    }
    None
}

/// The value the instruction `inst` makes.
fn inst_value(
    c: &Combiner,
    inst: InstId,
) -> Operand {
    Operand::Value(c.function.instruction(inst).result.expect("a value"))
}

/// `foldAddWithConstant` (:856): an add with an immediate constant on the
/// right.
pub fn fold_add_with_constant(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, add_flags) = parts(c, inst);
    let ty = c.type_of(op1);
    let bits = width(c, op1);
    {
        let q = query(c, Some(inst));
        if !pattern::imm_constant(&q, op1) {
            return None;
        }
    }
    for found in [c.fold_bin_op_into_select_or_phi(inst), c.fold_bin_op_select_bin_op(inst)] {
        if !matches!(found, Visit::None) {
            return Some(found);
        }
    }

    // add (sub C1, X), C2 --> sub (add C1, C2), X
    let sub_of_constant = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Sub).filter(|&(c1, ..)| matches!(c1, Operand::Constant(_)))
    };
    if let Some((c1, x, _)) = sub_of_constant
        && let Some(sum) = fold_constants(c, BinaryOp::Add, c1, op1)
    {
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, sum, x, Flags::default()));
    }

    // add (sub X, Y), -1 --> add (not Y), X
    let minus_one = {
        let q = query(c, Some(inst));
        if has_one_use(c, op0) && pattern::all_ones(&q, op1) { binop(&q, op0, BinaryOp::Sub) } else { None }
    };
    if let Some((x, y, _)) = minus_one {
        let not_y = c.create_not(inst, y);
        return Some(c.replace_with_binop(inst, BinaryOp::Add, not_y, x, Flags::default()));
    }

    // zext(bool) + C -> bool ? C + 1 : C
    // sext(bool) + C -> bool ? C - 1 : C
    for (extend, delta) in [(CastOp::ZExt, 1), (CastOp::SExt, -1)] {
        let boolean = {
            let q = query(c, Some(inst));
            pattern::cast(&q, op0, extend).map(|(_, _, x)| x).filter(|&x| width(c, x) == 1)
        };
        if let Some(x) = boolean
            && let Some(changed) = add_to(c, op1, delta)
        {
            return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![x, changed, op1], Flags::default()));
        }
    }

    // ~X + C --> (C-1) - X
    let not_x = {
        let q = query(c, Some(inst));
        pattern::not(&q, op0)
    };
    if let Some(x) = not_x {
        // ~X + C has NSW and (C-1) won't overflow => (C-1)-X can have NSW
        let one = c.int(ty, 1);
        let will_not_sov = will_not_overflow_signed_sub(c, inst, op1, one);
        if let Some(diff) = fold_constants(c, BinaryOp::Sub, op1, one) {
            let mut flags = Flags::default();
            if add_flags.contains(Flags::NSW) && will_not_sov {
                flags.insert(Flags::NSW);
            }
            return Some(c.replace_with_binop(inst, BinaryOp::Sub, diff, x, flags));
        }
    }

    // (iN X s>> (N - 1)) + 1 --> zext (X > -1)
    let sign_shift = {
        let q = query(c, Some(inst));
        if has_one_use(c, op0) && pattern::one(&q, op1) {
            binop(&q, op0, BinaryOp::AShr).filter(|&(_, amount, _)| specific_int(&q, amount, (bits - 1) as u128))
        } else {
            None
        }
    };
    if let Some((x, _, _)) = sign_shift {
        let minus_one = c.all_ones(c.type_of(x));
        let not_neg = c.create_icmp(inst, IntPredicate::Sgt, x, minus_one);
        return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![not_neg], Flags::default()));
    }

    let (c_value, _) = {
        let q = query(c, Some(inst));
        pattern::apint(&q, op1)?
    };

    // (X | Op01C) + Op1C --> X + (Op01C + Op1C) iff the `or` is actually an
    // `add`
    let disjoint = {
        let q = query(c, Some(inst));
        pattern::disjoint_or(&q, op0).filter(|&(_, k)| pattern::imm_constant(&q, k))
    };
    if let Some((x, op01c)) = disjoint
        && let Some(sum) = fold_constants(c, BinaryOp::Add, op01c, op1)
    {
        let mut flags = Flags::default();
        if add_flags.contains(Flags::NSW) && will_not_overflow_signed_add(c, inst, op01c, op1) {
            flags.insert(Flags::NSW);
        }
        if add_flags.contains(Flags::NUW) {
            flags.insert(Flags::NUW);
        }
        return Some(c.replace_with_binop(inst, BinaryOp::Add, x, sum, flags));
    }

    // (X | C2) + C --> (X | C2) ^ C2 iff (C2 == -C)
    let or_const = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Or).and_then(|(_, k, _)| pattern::apint(&q, k)).map(|(c2, _)| c2)
    };
    if let Some(c2) = or_const
        && c2 == c_value.wrapping_neg() & mask(bits)
    {
        let k = c.int(ty, c2);
        return Some(c.replace_with_binop(inst, BinaryOp::Xor, op0, k, Flags::default()));
    }

    if c_value == 1u128 << (bits - 1) {
        // If wrapping is not allowed, then the addition must set the sign bit:
        // X + (signmask) --> X | signmask
        if add_flags.contains(Flags::NSW) || add_flags.contains(Flags::NUW) {
            return Some(c.replace_with_binop(inst, BinaryOp::Or, op0, op1, Flags::DISJOINT));
        }
        // If wrapping is allowed, then the addition flips the sign bit of LHS:
        // X + (signmask) --> X ^ signmask
        return Some(c.replace_with_binop(inst, BinaryOp::Xor, op0, op1, Flags::default()));
    }

    // Is this add the last step in a convoluted sext?
    // add(zext(xor i16 X, -32768), -32768) --> sext X
    let convoluted = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op0, CastOp::ZExt).and_then(|(_, _, inner)| {
            let (x, k, _) = binop(&q, inner, BinaryOp::Xor)?;
            let (c2, narrow) = pattern::apint(&q, k)?;
            // C2->isMinSignedValue() && C2->sext(BitWidth) == *C
            (c2 == 1u128 << (narrow - 1) && (signed(c2, narrow) as u128 & mask(bits)) == c_value).then_some(x)
        })
    };
    if let Some(x) = convoluted {
        return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::SExt), ty, vec![x], Flags::default()));
    }

    let xor_const = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Xor).and_then(|(x, k, _)| pattern::apint(&q, k).map(|(c2, _)| (x, c2)))
    };
    if let Some((x, c2)) = xor_const {
        // (X ^ signmask) + C --> (X + (signmask ^ C))
        if c2 == 1u128 << (bits - 1) {
            let k = c.int(ty, c2 ^ c_value);
            return Some(c.replace_with_binop(inst, BinaryOp::Add, x, k, Flags::default()));
        }
        // If X has no bits set other than an xor mask, xor is equivalent to
        // sub with no borrow between bits:
        // add (xor X, C2), C --> sub (C2 + C), X
        let known_zero = known::compute_known_bits(&query(c, Some(inst)), x, 0).zero;
        if (c2 | known_zero) & mask(bits) == mask(bits) {
            let k = c.int(ty, c2.wrapping_add(c_value));
            return Some(c.replace_with_binop(inst, BinaryOp::Sub, k, x, Flags::default()));
        }
        // Look for a math+logic pattern that corresponds to sext-in-register of
        // a value with cleared high bits. Convert that into a pair of shifts:
        // add (xor X, 0x80), 0xF..F80 --> (X << ShAmtC) >>s ShAmtC
        // add (xor X, 0xF..F80), 0x80 --> (X << ShAmtC) >>s ShAmtC
        if has_one_use(c, op0) && c2 == c_value.wrapping_neg() & mask(bits) {
            let shamt = if c_value.is_power_of_two() {
                bits - c_value.trailing_zeros() - 1
            } else if c2.is_power_of_two() {
                bits - c2.trailing_zeros() - 1
            } else {
                0
            };
            if shamt != 0 {
                let high = mask(bits) & !mask(bits - shamt);
                if known::masked_value_is_zero(&query(c, Some(inst)), x, high, 0) {
                    let amount = c.int(ty, shamt as u128);
                    let new_shl = c.create_shl(inst, x, amount);
                    return Some(c.replace_with_binop(inst, BinaryOp::AShr, new_shl, amount, Flags::default()));
                }
            }
        }
    }

    if c_value == 1 && has_one_use(c, op0) {
        // add (sext i1 X), 1 --> zext (not X)
        let boolean = {
            let q = query(c, Some(inst));
            pattern::cast(&q, op0, CastOp::SExt).map(|(_, _, x)| x).filter(|&x| width(c, x) == 1)
        };
        if let Some(x) = boolean {
            let not_x = c.create_not(inst, x);
            return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![not_x], Flags::default()));
        }
        // Shifts and add used to flip and mask off the low bit:
        // add (ashr (shl i32 X, 31), 31), 1 --> and (not X), 1
        let flip = {
            let q = query(c, Some(inst));
            binop(&q, op0, BinaryOp::AShr).and_then(|(shl, c3, _)| {
                let (x, c2, _) = binop(&q, shl, BinaryOp::Shl)?;
                let (a, _) = pattern::apint(&q, c2)?;
                let (b, _) = pattern::apint(&q, c3)?;
                (a == b && a == (bits - 1) as u128).then_some(x)
            })
        };
        if let Some(x) = flip {
            let not_x = c.create_not(inst, x);
            let one = c.int(ty, 1);
            return Some(c.replace_with_binop(inst, BinaryOp::And, not_x, one, Flags::default()));
        }
    }

    // umax(X, C) + -C --> usub.sat(X, C)
    let negated = c_value.wrapping_neg() & mask(bits);
    if has_one_use(c, op0) {
        let x = {
            let q = query(c, Some(inst));
            pattern::commutative_intrinsic_with(&q, op0, "llvm.umax", |a| {
                pattern::apint(&q, a).is_some_and(|(value, _)| value == negated)
            })
        };
        if let Some(x) = x {
            let k = c.int(ty, negated);
            return Some(Visit::Replace(c.create_intrinsic(inst, "usub.sat", ty, &[x, k])));
        }
    }
    // DEVIATE 3: uadd.sat(X, C) + -C --> umin(X, ~C)

    // Fold (add (zext (add X, -C)), C) -> (zext X) if X u>= C. Truncate C to
    // the narrow type to avoid mismatched width comparisons.
    let narrow = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op0, CastOp::ZExt).and_then(|(_, _, inner)| {
            let (x, k, _) = binop(&q, inner, BinaryOp::Add)?;
            let (inner_c, narrow_bits) = pattern::apint(&q, k)?;
            Some((x, inner_c, narrow_bits))
        })
    };
    if let Some((x, inner_c, narrow_bits)) = narrow
        && c_value & !mask(narrow_bits) == 0
    {
        let narrow_c = c_value & mask(narrow_bits);
        if inner_c == narrow_c.wrapping_neg() & mask(narrow_bits) {
            let q = query(c, Some(inst));
            let holds = if narrow_c == 1 {
                nonzero::is_known_non_zero(&q, x, 0)
            } else {
                known::compute_known_bits(&q, x, 0).min_value() >= narrow_c
            };
            if holds {
                return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![x], Flags::default()));
            }
        }
    }
    None
}

/// `foldAddLikeCommutative` (:1342): the folds of `add LHS, RHS` that look at
/// `LHS` first. Called for both orders.
fn fold_add_like_commutative(
    c: &mut Combiner,
    inst: InstId,
    lhs: Operand,
    rhs: Operand,
    nsw: bool,
    nuw: bool,
) -> Option<Visit> {
    // (A - B) + (C - A) --> C - B
    let sub_pair = {
        let q = query(c, Some(inst));
        binop(&q, lhs, BinaryOp::Sub).and_then(|(a, b, left)| {
            let (cc, a1, right) = binop(&q, rhs, BinaryOp::Sub)?;
            (a1 == a).then_some((b, cc, left, right))
        })
    };
    if let Some((b, cc, left, right)) = sub_pair {
        let mut flags = Flags::default();
        if nsw && left.contains(Flags::NSW) && right.contains(Flags::NSW) {
            flags.insert(Flags::NSW);
        }
        if left.contains(Flags::NUW) && right.contains(Flags::NUW) {
            flags.insert(Flags::NUW);
        }
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, cc, b, flags));
    }

    // ((X s/ C1) << C2) + X => X s% -C1 where -C1 is 1 << C2
    let shifted_div = {
        let q = query(c, Some(inst));
        binop(&q, lhs, BinaryOp::Shl).and_then(|(div, c2, _)| {
            let (x, c1, _) = binop(&q, div, BinaryOp::SDiv)?;
            let (c1, w1) = pattern::apint(&q, c1)?;
            let (c2, w2) = pattern::apint(&q, c2)?;
            (x == rhs).then_some((c1, w1, c2, w2))
        })
    };
    if let Some((c1, w1, c2, w2)) = shifted_div {
        let minus_c1 = c1.wrapping_neg() & mask(w1);
        // MinusC1 == (One << *C2)
        if c2 < w2 as u128 && minus_c1 == (1u128 << c2) & mask(w2) {
            let new_rhs = c.int(c.type_of(rhs), minus_c1);
            return Some(c.replace_with_binop(inst, BinaryOp::SRem, rhs, new_rhs, Flags::default()));
        }
    }

    // (A + C) + (B & ~C) == A + (B | C)
    let or_pair = {
        let q = query(c, Some(inst));
        commutative(&q, lhs, BinaryOp::Add, |one| pattern::apint(&q, one).is_some()).and_then(|(k, a)| {
            let (c1, w) = pattern::apint(&q, k)?;
            let (_, b) = commutative(&q, rhs, BinaryOp::And, |one| specific_int(&q, one, !c1 & mask(w)))?;
            Some((a, b, c1))
        })
    };
    if let Some((a, b, c1)) = or_pair {
        // Replacing one add with {or, add}. Avoid growth if both sides are
        // shared.
        if !has_one_use(c, lhs) && !has_one_use(c, rhs) {
            return None;
        }
        let left_flags = c.function.instruction(inst_def(c, lhs)?).flags;
        let mut flags = Flags::default();
        if nsw && left_flags.contains(Flags::NSW) {
            flags.insert(Flags::NSW);
        }
        if nuw && left_flags.contains(Flags::NUW) {
            flags.insert(Flags::NUW);
        }
        let k = c.int(c.type_of(lhs), c1);
        let new_or = c.create_binop(inst, BinaryOp::Or, b, k, Flags::default());
        return Some(c.replace_with_binop(inst, BinaryOp::Add, a, new_or, flags));
    }
    None
}

/// `InstCombinerImpl::visitAdd` (:1604).
pub fn visit_add(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifyAddInst
    if let Some(found) = simplify_visited(c, inst) {
        return found;
    }
    if simplify_associative_or_commutative(c, inst) {
        return Visit::Changed;
    }
    // DEVIATE 1: foldVectorBinop.
    let found = c.fold_binop_with_phi_operands(inst);
    if !matches!(found, Visit::None) {
        return found;
    }
    if let Some(v) = fold_using_distributive_laws(c, inst) {
        return Visit::Replace(v);
    }
    if let Some(r) = fold_box_multiply(c, inst) {
        return r;
    }
    if let Some(r) = factorize_math_with_shl_ops(c, inst) {
        return r;
    }
    if let Some(r) = fold_add_with_constant(c, inst) {
        return r;
    }
    if let Some(r) = fold_no_wrap_add(c, inst) {
        return r;
    }
    if let Some(r) = fold_binop_shift_with_shift(c, inst) {
        return r;
    }
    if let Some(r) = combine_add_sub_with_shl_add_sub(c, inst) {
        return r;
    }
    let (lhs, rhs, flags) = parts(c, inst);
    let (nsw, nuw) = (flags.contains(Flags::NSW), flags.contains(Flags::NUW));
    if let Some(r) = fold_add_like_commutative(c, inst, lhs, rhs, nsw, nuw) {
        return r;
    }
    if let Some(r) = fold_add_like_commutative(c, inst, rhs, lhs, nsw, nuw) {
        return r;
    }
    let ty = c.type_of(lhs);
    if width(c, lhs) == 1 {
        return c.replace_with_binop(inst, BinaryOp::Xor, lhs, rhs, Flags::default());
    }
    // X + X --> X << 1
    if lhs == rhs {
        let one = c.int(ty, 1);
        return c.replace_with_binop(inst, BinaryOp::Shl, lhs, one, wrap_of(flags));
    }
    fold_add_rest(c, inst)
}

#[cfg(test)]
#[path = "combine_addsub_tests.rs"]
mod tests;
