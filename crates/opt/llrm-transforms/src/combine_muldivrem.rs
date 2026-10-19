//! InstCombine's `visitMul` and the helpers it calls, from
//! [`InstCombineMulDivRem.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp)
//! at d1106deb71cc.
//!
//! [`visit_mul`] is
//! [`visitMul`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L193):
//! the same steps in the same order, each a function named for the C++ one
//! (or for the block of `visitMul` it is) with its own permalink. A step that
//! returns `nullptr` there returns `None` here, one that returns an
//! instruction returns `Some(Visit)` (a new instruction replaces the visited
//! one; `&I` is `Visit::Changed`).
//!
//! MIRROR: the order of the steps and each condition. A `m_c_*` match commits
//! to the first operand order that matches, as in C++.
//!
//! DEVIATE 1: `foldVectorBinop` is not translated: MIR has no vector
//! operations, so no `ConstantVector` case either.
//!
//! DEVIATE 2 (temporary): steps that call a helper another family owns are
//! left out and named where they stood: `foldBinopWithPhiOperands` (phi
//! family), `foldUsingDistributiveLaws` (shared helpers),
//! `foldBinOpIntoSelectOrPhi`, `foldBinOpSelectBinOp` and
//! `foldBinOpOfSelectAndCastOfSelectCondition` (select family). Each is a
//! `// DEVIATE 2:` comment at its place in the order.
//!
//! DEVIATE 3 (temporary): the folds that build an intrinsic call (`abs`,
//! `umin`, `umax`) are left out until the Builder can declare an intrinsic
//! (`combine_build` DEVIATE 2); each is a `// DEVIATE 3:` comment at its
//! place. `takeLog2` cannot look through `umin`/`umax` for the same reason.
//!
//! DEVIATE 4: a constant that `ConstantExpr::getNeg` or `getShl` would leave
//! as an expression is not built (MIR holds no binary constant expressions):
//! the fold is skipped where the constants do not fold to a number.
//!
//! DEVIATE 5 (temporary): `narrowMathIfNoOverflow` (`InstructionCombining.cpp`,
//! shared with `visitAdd`) is left out until `combine_shared` has it.
//!
//! DEVIATE 6 (temporary): `Negator::Negate` is not translated; it is taken to
//! fail, so the two folds that use it are skipped and the step after each
//! (the `zext`/`sext` multiply by `-1 << C`) is tried where C++ would stop.
//!
//! # Used by
//! `combine::visit`, once the mul family's slot is flipped.
//!
//! # How it works
//! `visit_mul` returns the first step that changes the multiply. The steps
//! rewrite it to a cheaper form (`x * 2^k` to a shift, `x * -1` to a negation,
//! a multiply by a boolean to a `select`), or replace it by an existing value.
//! A rewrite that needs new operands builds them with the `Combiner`'s
//! builder, which folds through `simplify*` first. If nothing rewrites it,
//! known bits and ranges may add its `nsw` and `nuw`.
//!
//! # Example
//! `(zext i1 %b) * %y` is a select:
//!
//! ```text
//! %z = zext i1 %b to i32
//! %r = mul i32 %z, %y   ; %r = select i1 %b, i32 %y, i32 0
//! ```

use llrm_mir::constantfolding;
use llrm_mir::context::{mask, signed};
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::simplify_instruction_in;
use llrm_mir::valuetracking::MAX_ANALYSIS_RECURSION_DEPTH;
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef;
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};
use llrm_mir::valuetracking::range::{
    OverflowResult, compute_overflow_for_signed_mul, compute_overflow_for_unsigned_mul,
};
use llrm_mir::valuetracking::{Query, known};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query, simplify_associative_or_commutative};

/// `m_BinOp(op, a, b)`: the operands and flags of `v` if it is that operation.
fn binop(
    q: &Query,
    v: Operand,
    op: BinaryOp,
) -> Option<(Operand, Operand, Flags)> {
    pattern::binary(q, v, op).map(|(_, instruction, a, b)| (a, b, instruction.flags))
}

/// `m_c_BinOp(L, R)`: `matcher(first, second)` for the first order of `a`, `b`
/// in which it matches.
fn commuted<T>(
    a: Operand,
    b: Operand,
    matcher: impl Fn(Operand, Operand) -> Option<T>,
) -> Option<T> {
    matcher(a, b).or_else(|| matcher(b, a))
}

/// The visited instruction's operands and flags.
fn parts(
    c: &Combiner,
    inst: InstId,
) -> (Operand, Operand, Flags) {
    let instruction = c.function.instruction(inst);
    (instruction.operands[0], instruction.operands[1], instruction.flags)
}

/// The width of the integer `v` (a scalar: MIR has no vectors).
fn width(
    c: &Combiner,
    v: Operand,
) -> u32 {
    c.context.types.int_bits(c.type_of(v)).expect("an integer")
}

/// The number `v` is and its width (`m_APInt`).
fn apint(
    c: &Combiner,
    v: Operand,
) -> Option<(u128, u32)> {
    query(c, None).int_constant(v)
}

/// `m_ImmConstant`.
fn imm_constant(
    c: &Combiner,
    v: Operand,
) -> bool {
    pattern::imm_constant(&query(c, None), v)
}

/// `ConstantFoldBinaryOpOperands` of two constants (DEVIATE 4: none where it
/// would stay an expression).
fn fold_constants(
    c: &Combiner,
    op: BinaryOp,
    a: Operand,
    b: Operand,
) -> Option<Operand> {
    let (Operand::Constant(x), Operand::Constant(y)) = (a, b) else { return None };
    let env = constantfolding::Env { globals: c.env.globals, layout: c.env.layout };
    constantfolding::constant_fold_binary_op_operands(c.context, env, op, x, y).map(Operand::Constant)
}

/// `ConstantExpr::getNeg` of a constant.
fn negate(
    c: &Combiner,
    v: Operand,
) -> Option<Operand> {
    let zero = c.int(c.type_of(v), 0);
    fold_constants(c, BinaryOp::Sub, zero, v)
}

/// `Constant::isNotMinSignedValue`: an integer other than `INT_MIN`.
fn is_not_min_signed(
    c: &Combiner,
    v: Operand,
) -> bool {
    apint(c, v).is_some_and(|(bits, w)| bits != 1u128 << (w - 1))
}

/// `m_NegatedPower2`: `-(2^k)`.
pub(crate) fn is_negated_power2(
    bits: u128,
    w: u32,
) -> bool {
    bits >> (w - 1) & 1 == 1 && (bits.wrapping_neg() & mask(w)).is_power_of_two()
}

/// `nuw` and `nsw` as flags.
fn wrap(
    nuw: bool,
    nsw: bool,
) -> Flags {
    let mut flags = Flags::default();
    if nuw {
        flags.insert(Flags::NUW);
    }
    if nsw {
        flags.insert(Flags::NSW);
    }
    flags
}

/// The instruction that defines the value `v`.
fn inst_def(
    c: &Combiner,
    v: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = v else { return None };
    match c.function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        _ => None,
    }
}

/// `dyn_cast<BinaryOperator>(v)->setHasNoUnsignedWrap()` and the like: `flags`
/// added to the binary operator `v` is, whether or not it is a new one.
fn add_flags_to(
    c: &mut Combiner,
    v: Operand,
    flags: Flags,
) {
    let Some(inst) = inst_def(c, v) else { return };
    if !matches!(c.function.instruction(inst).opcode, Opcode::Binary(_)) {
        return;
    }
    let old = c.function.instruction(inst).flags;
    let mut new = old;
    new.insert(flags);
    if new != old {
        c.function.set_flags(inst, new);
    }
}

/// `Builder.CreateFreeze(v)` unless `isGuaranteedNotToBeUndef(v)`.
pub(crate) fn freeze_unless_guaranteed(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
) -> Operand {
    if is_guaranteed_not_to_be_undef(&query(c, None), v, 0) {
        return v;
    }
    let ty = c.type_of(v);
    let new = c.insert_before(at, Opcode::Freeze, ty, vec![v], Flags::default());
    Operand::Value(c.function.instruction(new).result.expect("a value"))
}

/// `Builder.CreateNeg(v, "", HasNSW)`.
fn create_neg_flags(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    nsw: bool,
) -> Operand {
    let zero = c.int(c.type_of(v), 0);
    c.create_binop(at, BinaryOp::Sub, zero, v, wrap(false, nsw))
}

/// `BinaryOperator::CreateNeg(v)` (`CreateNSWNeg`) as what `visit*` returns.
pub(crate) fn replace_with_neg(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    nsw: bool,
) -> Visit {
    let zero = c.int(c.type_of(v), 0);
    c.replace_with_binop(at, BinaryOp::Sub, zero, v, wrap(false, nsw))
}

/// `createSelectInstWithUnknownProfile(cond, a, b)` as what `visit*` returns.
pub(crate) fn replace_with_select(
    c: &mut Combiner,
    at: InstId,
    cond: Operand,
    a: Operand,
    b: Operand,
) -> Visit {
    let ty = c.type_of(a);
    c.replace_with_new(at, Opcode::Select, ty, vec![cond, a, b], Flags::default())
}

/// `simplify*Inst(Op0, Op1, SQ.getWithInstruction(&I))`.
pub(crate) fn simplify_first(
    c: &Combiner,
    inst: InstId,
) -> Option<Operand> {
    let env = c.env;
    simplify_instruction_in(c.context, env.globals, env.layout, Some(&env.analyses), c.function, inst)
        .filter(|&found| Some(found) != c.function.instruction(inst).result.map(Operand::Value))
}

/// The `zext` or `sext` of a boolean `v` is: the opcode and the boolean.
fn bool_extend(
    c: &Combiner,
    v: Operand,
    op: CastOp,
) -> Option<Operand> {
    let q = query(c, None);
    let (_, _, x) = pattern::cast(&q, v, op)?;
    (width(c, x) == 1).then_some(x)
}

/// [`foldMulSelectToNegate`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L104),
/// the integer arms: `mul (select C, 1, -1), X` is `select C, X, -X`. The
/// `fmul` arms are for floats.
fn fold_mul_select_to_negate(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Operand> {
    let (op0, op1, flags) = parts(c, inst);
    let any_wrap = flags.contains(Flags::NSW) || flags.contains(Flags::NUW);
    for ones_first in [true, false] {
        let found = {
            let q = query(c, Some(inst));
            commuted(op0, op1, |s, other| {
                if !has_one_use(c, s) {
                    return None;
                }
                let (_, _, cond, t, f) = pattern::select(&q, s)?;
                let ok = if ones_first {
                    pattern::one(&q, t) && pattern::all_ones(&q, f)
                } else {
                    pattern::all_ones(&q, t) && pattern::one(&q, f)
                };
                ok.then_some((cond, other))
            })
        };
        if let Some((cond, other)) = found {
            let neg = create_neg_flags(c, inst, other, any_wrap);
            return Some(if ones_first {
                c.create_select(inst, cond, other, neg)
            } else {
                c.create_select(inst, cond, neg, other)
            });
        }
    }
    None
}

/// [`foldMulShl1`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L146):
/// multiplications that contain a `(+/-1 << Z)` factor. Called twice to handle
/// commuted patterns.
fn fold_mul_shl1(
    c: &mut Combiner,
    inst: InstId,
    commute_operands: bool,
) -> Option<Operand> {
    let (mut x, mut y, flags) = parts(c, inst);
    if commute_operands {
        std::mem::swap(&mut x, &mut y);
    }
    let has_nsw = flags.contains(Flags::NSW);
    let has_nuw = flags.contains(Flags::NUW);

    // X * (1 << Z) --> X << Z
    let shl_one = {
        let q = query(c, Some(inst));
        binop(&q, y, BinaryOp::Shl).filter(|&(one, ..)| pattern::one(&q, one))
    };
    if let Some((_, z, shl_flags)) = shl_one {
        let propagate_nsw = has_nsw && shl_flags.contains(Flags::NSW);
        return Some(c.create_binop(inst, BinaryOp::Shl, x, z, wrap(has_nuw, propagate_nsw)));
    }

    // X * ((1 << Z) + 1) --> (X << Z) + X
    let shift_plus_one = {
        let q = query(c, Some(inst));
        if has_one_use(c, y) {
            binop(&q, y, BinaryOp::Add)
                .filter(|&(_, one, _)| pattern::one(&q, one))
                .and_then(
                    |(shift, ..)| {
                        // m_BinOp(Shift) then m_OneUse(m_Shl(m_One(),
                        // m_Value(Z))).
                        let (one, z, shift_flags) = binop(&q, shift, BinaryOp::Shl)?;
                        (has_one_use(c, shift) && pattern::one(&q, one)).then_some((z, shift_flags))
                    },
                )
        } else {
            None
        }
    };
    if let Some((z, shift_flags)) = shift_plus_one {
        let propagate_nsw = has_nsw && shift_flags.contains(Flags::NSW);
        // This increases uses of X, so it may require a freeze.
        let fr_x = freeze_unless_guaranteed(c, inst, x);
        let shl = c.create_binop(inst, BinaryOp::Shl, fr_x, z, wrap(has_nuw, propagate_nsw));
        return Some(c.create_binop(inst, BinaryOp::Add, shl, fr_x, wrap(has_nuw, propagate_nsw)));
    }

    // X * (~(-1 << Z)) --> (X << Z) - X
    let not_shl = {
        let q = query(c, Some(inst));
        if has_one_use(c, y) {
            pattern::not(&q, y).and_then(|shl| {
                let (ones, z, _) = binop(&q, shl, BinaryOp::Shl)?;
                (has_one_use(c, shl) && pattern::all_ones(&q, ones)).then_some(z)
            })
        } else {
            None
        }
    };
    if let Some(z) = not_shl {
        let fr_x = freeze_unless_guaranteed(c, inst, x);
        let shl = c.create_binop(inst, BinaryOp::Shl, fr_x, z, Flags::default());
        return Some(c.create_binop(inst, BinaryOp::Sub, shl, fr_x, Flags::default()));
    }
    None
}

/// `((X << C2) * C1)` is `X * (C1 << C2)`.
fn fold_mul_of_shl_by_constant(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, flags) = parts(c, inst);
    let (x, c2, shl_flags) = {
        let (x, c2, shl_flags) = binop(&query(c, Some(inst)), op0, BinaryOp::Shl)?;
        if !imm_constant(c, c2) || !imm_constant(c, op1) || apint(c, op1).is_none() {
            return None;
        }
        (x, c2, shl_flags)
    };
    let shl = fold_constants(c, BinaryOp::Shl, op1, c2)?;
    let nuw = flags.contains(Flags::NUW) && shl_flags.contains(Flags::NUW);
    let nsw = flags.contains(Flags::NSW) && shl_flags.contains(Flags::NSW) && is_not_min_signed(c, shl);
    Some(c.replace_with_binop(inst, BinaryOp::Mul, x, shl, wrap(nuw, nsw)))
}

/// `X * 2^C` is `X << C`.
fn fold_mul_by_power_of_two(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, flags) = parts(c, inst);
    let (bits, w) = apint(c, op1)?;
    if !bits.is_power_of_two() {
        return None;
    }
    let log2 = bits.trailing_zeros();
    let amount = c.int(c.type_of(op1), log2 as u128);
    let nsw = flags.contains(Flags::NSW) && log2 != w - 1;
    Some(c.replace_with_binop(inst, BinaryOp::Shl, op0, amount, wrap(flags.contains(Flags::NUW), nsw)))
}

/// `mul (shr exact X, N), (2^N + 1)` is `add X, (shr exact X, N)`.
fn fold_mul_of_exact_shr(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, flags) = parts(c, inst);
    let bits = width(c, op0);
    if bits <= 2 {
        return None;
    }
    let (x, shift_c, mul_ap, is_ashr) = {
        let q = query(c, Some(inst));
        let (x, amount, f, is_ashr) = binop(&q, op0, BinaryOp::LShr)
            .map(|(x, a, f)| (x, a, f, false))
            .or_else(|| binop(&q, op0, BinaryOp::AShr).map(|(x, a, f)| (x, a, f, true)))?;
        if !f.contains(Flags::EXACT) {
            return None;
        }
        let (shift_c, _) = q.int_constant(amount)?;
        let (mul_ap, _) = q.int_constant(op1)?;
        (x, shift_c, mul_ap, is_ashr)
    };
    let minus_one = mul_ap.wrapping_sub(1) & mask(bits);
    if !minus_one.is_power_of_two() || shift_c != (127 - mul_ap.leading_zeros()) as u128 {
        return None;
    }
    let (has_nsw, has_nuw) = (flags.contains(Flags::NSW), flags.contains(Flags::NUW));
    let mut bin_op = op0;
    // mul nuw (ashr exact X, N) -> add nuw (X, lshr exact (X, N))
    if has_nuw && is_ashr && has_one_use(c, op0) {
        let amount = c.int(c.type_of(op0), shift_c);
        bin_op = c.create_binop(inst, BinaryOp::LShr, x, amount, Flags::EXACT);
    }
    let nsw = has_nsw && (has_nuw || !is_ashr || shift_c < (bits - 1) as u128);
    Some(c.replace_with_binop(inst, BinaryOp::Add, x, bin_op, wrap(has_nuw, nsw)))
}

/// `Op0->hasOneUse() && match(Op1, m_NegatedPower2())`: `X * (-1<<C)` as `(-X)
/// * (1<<C)`; DEVIATE 6 for the `Negator` half.
fn fold_mul_by_negated_power_of_two(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, _) = parts(c, inst);
    let (neg_pow2, w) = apint(c, op1)?;
    if !has_one_use(c, op0) || !is_negated_power2(neg_pow2, w) {
        return None;
    }
    // DEVIATE 6: Negator::Negate(/*IsNegation*/ true, HasNSW, Op0).

    // ({z/s}ext X) * (-1<<C) --> (zext (-X)) << C
    let x = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op0, CastOp::ZExt).or_else(|| pattern::cast(&q, op0, CastOp::SExt)).map(|(_, _, x)| x)?
    };
    let src_width = width(c, x);
    let shift_amt = neg_pow2.trailing_zeros();
    if shift_amt >= w - src_width {
        let ty = c.type_of(op0);
        let n = c.create_neg(inst, x);
        let z = c.create_cast(inst, CastOp::ZExt, n, ty, Flags::default());
        let amount = c.int(ty, shift_amt as u128);
        return Some(c.replace_with_binop(inst, BinaryOp::Shl, z, amount, Flags::default()));
    }
    None
}

/// `(shl X, C1) * (select cond, C2, C3)` is `X * (select cond, C2<<C1,
/// C3<<C1)`, and the same for a `mul`; commuted forms included.
fn fold_mul_of_select_of_constants(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, flags) = parts(c, inst);
    let found = {
        let q = query(c, Some(inst));
        // m_c_Mul(m_OneUse(m_Value(Other)), m_OneUse(m_Select(Cond, imm, imm)))
        let (other, cond, c2, c3) = commuted(op0, op1, |other, sel| {
            if !has_one_use(c, other) || !has_one_use(c, sel) {
                return None;
            }
            let (_, _, cond, c2, c3) = pattern::select(&q, sel)?;
            (pattern::imm_constant(&q, c2) && pattern::imm_constant(&q, c3)).then_some((other, cond, c2, c3))
        })?;
        let (op, x, c1, other_flags) = [BinaryOp::Mul, BinaryOp::Shl]
            .into_iter()
            .find_map(
                |op| {
                    let (x, c1, f) = binop(&q, other, op)?;
                    pattern::imm_constant(&q, c1).then_some((op, x, c1, f))
                },
            )?;
        (op, x, c1, other_flags, cond, c2, c3)
    };
    let (op, x, c1, other_flags, cond, c2, c3) = found;
    let new_tv = fold_constants(c, op, c2, c1)?;
    let new_fv = fold_constants(c, op, c3, c1)?;
    let new_sel = c.create_select(inst, cond, new_tv, new_fv);
    let nuw = flags.contains(Flags::NUW) && other_flags.contains(Flags::NUW);
    let nsw = flags.contains(Flags::NSW)
        && other_flags.contains(Flags::NSW)
        && is_not_min_signed(c, new_tv)
        && is_not_min_signed(c, new_fv);
    Some(c.replace_with_binop(inst, BinaryOp::Mul, x, new_sel, wrap(nuw, nsw)))
}

/// `(X+C1)*MulC` is `X*MulC + C1*MulC`, and the same for `X|C1` when the `or`
/// is an `add`.
fn fold_add_like_times_constant(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, flags) = parts(c, inst);
    if !imm_constant(c, op1) || !has_one_use(c, op0) {
        return None;
    }
    let (x, c1) = {
        let q = query(c, Some(inst));
        pattern::add_like(&q, op0, Flags::default()).filter(|&(_, k)| pattern::imm_constant(&q, k))?
    };
    let def = inst_def(c, op0)?;
    let op0_nuw = c.function.instruction(def).opcode == Opcode::Binary(BinaryOp::Or)
        || c.function.instruction(def).flags.contains(Flags::NUW);
    // C1*MulC simplifies to a tidier constant.
    let new_c = c.create_binop(inst, BinaryOp::Mul, c1, op1, Flags::default());
    let new_mul = c.create_binop(inst, BinaryOp::Mul, x, op1, Flags::default());
    let nuw = flags.contains(Flags::NUW) && op0_nuw;
    if nuw {
        add_flags_to(c, new_mul, Flags::NUW);
    }
    Some(c.replace_with_binop(inst, BinaryOp::Add, new_mul, new_c, wrap(nuw, false)))
}

/// `(mul (div exact X, C0), C1)` is `(div exact X, C0 / C1)` iff `C0 % C1 ==
/// 0` and `X / (C0 / C1)` does not create UB.
fn fold_mul_of_exact_div(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, _) = parts(c, inst);
    if !has_one_use(c, op0) {
        return None;
    }
    let (c1, w) = apint(c, op1)?;
    let found = {
        let q = query(c, Some(inst));
        let exact_div = |op: BinaryOp| {
            let (x, d, f) = binop(&q, op0, op)?;
            let (d_bits, _) = q.int_constant(d)?;
            f.contains(Flags::EXACT).then_some((op, x, d, d_bits))
        };
        exact_div(BinaryOp::UDiv)
            .filter(|&(.., d_bits)| c1 != 0 && d_bits % c1 == 0)
            .or_else(
                || {
                    exact_div(BinaryOp::SDiv).filter(|&(.., d_bits)| {
                        // Rem.isZero() && !Quot.isAllOnes()
                        let (a, b) = (signed(d_bits, w), signed(c1, w));
                        b != 0 && a.wrapping_rem(b) == 0 && a.wrapping_div(b) as u128 & mask(w) != mask(w)
                    })
                },
            )
    };
    let (div_op, x, c0, _) = found?;
    let ty = c.type_of(op0);
    let new_divisor = c.create_binop(inst, div_op, c0, op1, Flags::default());
    Some(c.replace_with_new(inst, Opcode::Binary(div_op), ty, vec![x, new_divisor], Flags::EXACT))
}

/// `dyn_castNegVal`: `x` of `sub 0, x`, or the negation of an integer
/// constant.
fn dyn_cast_neg_val(
    c: &Combiner,
    v: Operand,
) -> Option<Operand> {
    if let Some((_, x, _)) = pattern::neg(&query(c, None), v) {
        return Some(x);
    }
    if apint(c, v).is_some() { negate(c, v) } else { None }
}

/// `(X / Y) * Y` is `X - (X % Y)`, `(X / Y) * -Y` is `(X % Y) - X`.
fn fold_div_times_divisor(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, _) = parts(c, inst);
    let as_div = |c: &Combiner, v: Operand| {
        let q = query(c, None);
        [BinaryOp::UDiv, BinaryOp::SDiv].into_iter().find_map(|op| binop(&q, v, op).map(|(x, d, f)| (op, x, d, f)))
    };
    let (y, div_value) = if as_div(c, op0).is_some() { (op1, op0) } else { (op0, op1) };
    let neg = dyn_cast_neg_val(c, y);
    let (div_op, x, div_op1, div_flags) = as_div(c, div_value)?;
    if !has_one_use(c, div_value) || (div_op1 != y && Some(div_op1) != neg) {
        return None;
    }
    // If the division is exact, X % Y is zero, so we end up with X or -X.
    if div_flags.contains(Flags::EXACT) {
        if div_op1 == y {
            return Some(Visit::Replace(x));
        }
        return Some(replace_with_neg(c, inst, x, false));
    }
    let rem_op = if div_op == BinaryOp::UDiv { BinaryOp::URem } else { BinaryOp::SRem };
    // X must be frozen because we are increasing its number of uses.
    let x_freeze = freeze_unless_guaranteed(c, inst, x);
    let rem = c.create_binop(inst, rem_op, x_freeze, div_op1, Flags::default());
    Some(if div_op1 == y {
        c.replace_with_binop(inst, BinaryOp::Sub, x_freeze, rem, Flags::default())
    } else {
        c.replace_with_binop(inst, BinaryOp::Sub, rem, x_freeze, Flags::default())
    })
}

/// `(zext bool X) * (zext bool Y)` and the other pairs of boolean extends.
fn fold_mul_of_bool_extends(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, _) = parts(c, inst);
    let ty = c.type_of(op0);
    let both = |c: &Combiner, a: CastOp, b: CastOp| {
        let q = query(c, None);
        let (_, _, x) = pattern::cast(&q, op0, a)?;
        let (_, _, y) = pattern::cast(&q, op1, b)?;
        (width(c, x) == 1 && c.type_of(x) == c.type_of(y)).then_some((x, y))
    };
    // (zext bool X) * (zext bool Y) --> zext (and X, Y)
    // (sext bool X) * (sext bool Y) --> zext (and X, Y)
    let same = both(c, CastOp::ZExt, CastOp::ZExt).or_else(|| both(c, CastOp::SExt, CastOp::SExt));
    if let Some((x, y)) = same
        && (has_one_use(c, op0) || has_one_use(c, op1) || x == y)
    {
        let and = c.create_binop(inst, BinaryOp::And, x, y, Flags::default());
        return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![and], Flags::default()));
    }
    // (sext bool X) * (zext bool Y) --> sext (and X, Y)
    // (zext bool X) * (sext bool Y) --> sext (and X, Y)
    let mixed = both(c, CastOp::SExt, CastOp::ZExt).or_else(|| both(c, CastOp::ZExt, CastOp::SExt));
    if let Some((x, y)) = mixed
        && (has_one_use(c, op0) || has_one_use(c, op1))
    {
        let and = c.create_binop(inst, BinaryOp::And, x, y, Flags::default());
        return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::SExt), ty, vec![and], Flags::default()));
    }
    None
}

/// The folds of a multiply by a boolean-like operand into a `select`.
fn fold_mul_to_select(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (op0, op1, flags) = parts(c, inst);
    let ty = c.type_of(op0);
    let zero = c.int(ty, 0);
    let bits = width(c, op0);

    // (zext bool X) * Y --> X ? Y : 0
    // Y * (zext bool X) --> X ? Y : 0
    if let Some(x) = bool_extend(c, op0, CastOp::ZExt) {
        return Some(replace_with_select(c, inst, x, op1, zero));
    }
    if let Some(x) = bool_extend(c, op1, CastOp::ZExt) {
        return Some(replace_with_select(c, inst, x, op0, zero));
    }

    // mul (sext X), Y -> select X, -Y, 0
    // mul Y, (sext X) -> select X, -Y, 0
    let sext = {
        let q = query(c, Some(inst));
        commuted(op0, op1, |s, y| {
            let (_, _, x) = pattern::cast(&q, s, CastOp::SExt).filter(|_| has_one_use(c, s))?;
            Some((x, y))
        })
    };
    if let Some((x, y)) = sext
        && width(c, x) == 1
    {
        let neg = create_neg_flags(c, inst, y, flags.contains(Flags::NSW));
        return Some(replace_with_select(c, inst, x, neg, zero));
    }

    if imm_constant(c, op1) {
        // (sext bool X) * C --> X ? -C : 0
        if let Some(x) = bool_extend(c, op0, CastOp::SExt)
            && let Some(neg_c) = negate(c, op1)
        {
            return Some(replace_with_select(c, inst, x, neg_c, zero));
        }

        // (ashr i32 X, 31) * C --> (X < 0) ? -C : 0
        let sign_splat = {
            let q = query(c, Some(inst));
            binop(&q, op0, BinaryOp::AShr)
                .filter(|_| has_one_use(c, op0))
                .and_then(|(x, amount, _)| q.int_constant(amount).map(|(a, _)| (x, a)))
        };
        if let Some((x, amount)) = sign_splat
            && amount == (bits - 1) as u128
            && let Some(neg_c) = negate(c, op1)
        {
            let is_neg = c.create_icmp(inst, IntPredicate::Slt, x, zero);
            return Some(replace_with_select(c, inst, is_neg, neg_c, zero));
        }
    }

    // (lshr X, 31) * Y --> (X < 0) ? Y : 0
    let lshr = {
        let q = query(c, Some(inst));
        commuted(op0, op1, |s, y| {
            let (x, amount, _) = binop(&q, s, BinaryOp::LShr)?;
            let (a, _) = q.int_constant(amount)?;
            Some((x, a, y))
        })
    };
    if let Some((x, amount, y)) = lshr
        && amount == (bits - 1) as u128
    {
        let is_neg = c.create_icmp(inst, IntPredicate::Slt, x, zero);
        return Some(replace_with_select(c, inst, is_neg, y, zero));
    }

    // (and X, 1) * Y --> (trunc X) ? Y : 0
    let and_one = {
        let q = query(c, Some(inst));
        commuted(op0, op1, |s, y| {
            let (x, one, _) = binop(&q, s, BinaryOp::And).filter(|_| has_one_use(c, s))?;
            pattern::one(&q, one).then_some((x, y))
        })
    };
    if let Some((x, y)) = and_one {
        let i1 = c.context.types.int(1);
        let tr = c.create_cast(inst, CastOp::Trunc, x, i1, Flags::default());
        return Some(replace_with_select(c, inst, tr, y, zero));
    }
    None
}

/// [`takeLog2`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L1643):
/// the exact integer log2 of `op`. With `do_fold` the instructions are
/// created, else the answer is a placeholder (`op` itself, C++'s dummy
/// pointer). `None` on failure.
pub(crate) fn take_log2(
    c: &mut Combiner,
    at: InstId,
    op: Operand,
    mut depth: u32,
    assume_non_zero: bool,
    do_fold: bool,
) -> Option<Operand> {
    // log2(2^C) -> C
    if let Some((bits, _)) = apint(c, op)
        && bits.is_power_of_two()
    {
        return Some(if do_fold { c.int(c.type_of(op), bits.trailing_zeros() as u128) } else { op });
    }

    // The remaining tests are all recursive, so bail out if we hit the limit.
    let at_limit = depth == MAX_ANALYSIS_RECURSION_DEPTH;
    depth += 1;
    if at_limit {
        return None;
    }
    let ty = c.type_of(op);

    // log2(zext X) -> zext log2(X)
    let zext = pattern::cast(&query(c, None), op, CastOp::ZExt).map(|(_, _, x)| x);
    if let Some(x) = zext
        && let Some(log_x) = take_log2(c, at, x, depth, assume_non_zero, do_fold)
    {
        return Some(if do_fold { c.create_cast(at, CastOp::ZExt, log_x, ty, Flags::default()) } else { op });
    }

    // log2(trunc x) -> trunc log2(X)
    let trunc = pattern::cast(&query(c, None), op, CastOp::Trunc).map(|(_, i, x)| (x, i.flags));
    if let Some((x, flags)) = trunc {
        let nuw = flags.contains(Flags::NUW);
        if (assume_non_zero || nuw)
            && let Some(log_x) = take_log2(c, at, x, depth, assume_non_zero, do_fold)
        {
            return Some(if do_fold { c.create_cast(at, CastOp::Trunc, log_x, ty, wrap(nuw, false)) } else { op });
        }
    }

    // log2(X << Y) -> log2(X) + Y
    let shl = binop(&query(c, None), op, BinaryOp::Shl);
    if let Some((x, y, flags)) = shl
        && (assume_non_zero || flags.contains(Flags::NUW) || flags.contains(Flags::NSW))
        && let Some(log_x) = take_log2(c, at, x, depth, assume_non_zero, do_fold)
    {
        return Some(if do_fold { c.create_add(at, log_x, y) } else { op });
    }

    // log2(X >>u Y) -> log2(X) - Y
    let lshr = binop(&query(c, None), op, BinaryOp::LShr);
    if let Some((x, y, flags)) = lshr
        && (assume_non_zero || flags.contains(Flags::EXACT))
        && let Some(log_x) = take_log2(c, at, x, depth, assume_non_zero, do_fold)
    {
        return Some(if do_fold { c.create_sub(at, log_x, y) } else { op });
    }

    // log2(X & Y) -> either log2(X) or log2(Y). This requires `AssumeNonZero`
    // as `X & Y` may be zero when X != Y.
    let and = binop(&query(c, None), op, BinaryOp::And);
    if assume_non_zero && let Some((x, y, _)) = and {
        if let Some(log_x) = take_log2(c, at, x, depth, assume_non_zero, do_fold) {
            return Some(if do_fold { log_x } else { op });
        }
        if let Some(log_y) = take_log2(c, at, y, depth, assume_non_zero, do_fold) {
            return Some(if do_fold { log_y } else { op });
        }
    }

    // log2(Cond ? X : Y) -> Cond ? log2(X) : log2(Y)
    let select = pattern::select(&query(c, None), op).map(|(_, _, cond, x, y)| (cond, x, y));
    if let Some((cond, x, y)) = select
        && let Some(log_x) = take_log2(c, at, x, depth, assume_non_zero, do_fold)
        && let Some(log_y) = take_log2(c, at, y, depth, assume_non_zero, do_fold)
    {
        return Some(if do_fold { c.create_select(at, cond, log_x, log_y) } else { op });
    }

    // DEVIATE 3: log2(umin(X, Y)) -> umin(log2(X), log2(Y)), and umax.

    // log2(X + 1) IIF X[0,1] -> X
    if width(c, op) != 1
        && let Some((x, one, _)) = binop(&query(c, None), op, BinaryOp::Add)
        && pattern::one(&query(c, None), one)
        && let Some(def) = inst_def(c, op)
        && known::compute_known_bits(&query(c, Some(def)), x, 0).count_max_active_bits() == 1
    {
        return Some(if do_fold { x } else { op });
    }
    None
}

/// `tryGetLog2(Op, AssumeNonZero)`: `takeLog2` once to see whether it folds,
/// then to build.
pub(crate) fn try_get_log2(
    c: &mut Combiner,
    at: InstId,
    op: Operand,
    assume_non_zero: bool,
) -> Option<Operand> {
    take_log2(c, at, op, 0, assume_non_zero, false)?;
    take_log2(c, at, op, 0, assume_non_zero, true)
}

/// [`InstCombinerImpl::visitMul`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L193).
pub fn visit_mul(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifyMulInst
    if let Some(found) = simplify_first(c, inst) {
        return Visit::Replace(found);
    }
    if simplify_associative_or_commutative(c, inst) {
        return Visit::Changed;
    }
    // DEVIATE 1: foldVectorBinop.
    // DEVIATE 2: foldBinopWithPhiOperands.
    // DEVIATE 2: foldUsingDistributiveLaws.
    let (op0, op1, flags) = parts(c, inst);
    let (has_nsw, has_nuw) = (flags.contains(Flags::NSW), flags.contains(Flags::NUW));

    // X * -1 --> 0 - X
    if pattern::all_ones(&query(c, Some(inst)), op1) {
        return replace_with_neg(c, inst, op0, has_nsw);
    }
    if let Some(r) = fold_mul_of_shl_by_constant(c, inst) {
        return r;
    }
    if let Some(r) = fold_mul_by_power_of_two(c, inst) {
        return r;
    }
    if let Some(r) = fold_mul_of_exact_shr(c, inst) {
        return r;
    }
    if let Some(r) = fold_mul_by_negated_power_of_two(c, inst) {
        return r;
    }
    // DEVIATE 2: foldBinOpIntoSelectOrPhi.
    // DEVIATE 2: foldBinOpSelectBinOp.
    if let Some(folded) = fold_mul_select_to_negate(c, inst) {
        return Visit::Replace(folded);
    }
    if let Some(r) = fold_mul_of_select_of_constants(c, inst) {
        return r;
    }
    if let Some(r) = fold_add_like_times_constant(c, inst) {
        return r;
    }

    // abs(X) * abs(X) -> X * X
    let abs_arg = |c: &Combiner, v: Operand| {
        let q = query(c, None);
        pattern::intrinsic(&q, v, IntrinsicId::Abs).map(|(_, i)| pattern::call_arguments(i).to_vec())
    };
    if op0 == op1
        && let Some(args) = abs_arg(c, op0)
    {
        return c.replace_with_binop(inst, BinaryOp::Mul, args[0], args[0], Flags::default());
    }
    // DEVIATE 3: abs(X) * abs(Y) -> abs(X * Y)

    // -X * C --> X * -C
    let neg0 = pattern::neg(&query(c, Some(inst)), op0);
    if let Some((_, x, _)) = neg0
        && matches!(op1, Operand::Constant(_))
        && let Some(neg_c) = negate(c, op1)
    {
        return c.replace_with_binop(inst, BinaryOp::Mul, x, neg_c, Flags::default());
    }

    // -X * -Y --> X * Y
    let neg1 = pattern::neg(&query(c, Some(inst)), op1);
    if let (Some((_, x, f0)), Some((_, y, f1))) = (neg0, neg1) {
        let nsw = has_nsw && f0.contains(Flags::NSW) && f1.contains(Flags::NSW);
        return c.replace_with_binop(inst, BinaryOp::Mul, x, y, wrap(false, nsw));
    }

    // -X * Y --> -(X * Y)
    // X * -Y --> -(X * Y)
    let neg_times = {
        let q = query(c, Some(inst));
        commuted(op0, op1, |n, y| {
            let (_, x, _) = pattern::neg(&q, n).filter(|_| has_one_use(c, n))?;
            Some((x, y))
        })
    };
    if let Some((x, y)) = neg_times {
        let mul = c.create_binop(inst, BinaryOp::Mul, x, y, Flags::default());
        return replace_with_neg(c, inst, mul, false);
    }

    // (-X * Y) * -X --> (X * Y) * X
    // (-X << Y) * -X --> (X << Y) * X
    // DEVIATE 6: Negator::Negate(false, /*IsNSW*/ false, Op0) when Op1 is -X.

    if let Some(r) = fold_mul_of_exact_div(c, inst) {
        return r;
    }
    if let Some(r) = fold_div_times_divisor(c, inst) {
        return r;
    }

    // Fold the following two scenarios:
    //   1) i1 mul -> i1 and.
    //   2) X * Y --> X & Y, iff X, Y can be only {0,1}.
    let and_one = |c: &Combiner, v: Operand| {
        let q = query(c, None);
        binop(&q, v, BinaryOp::And).is_some_and(|(_, one, _)| pattern::one(&q, one))
    };
    if width(c, op0) == 1 || (and_one(c, op0) && and_one(c, op1)) {
        return c.replace_with_binop(inst, BinaryOp::And, op0, op1, Flags::default());
    }

    if let Some(r) = fold_mul_shl1(c, inst, false) {
        return Visit::Replace(r);
    }
    if let Some(r) = fold_mul_shl1(c, inst, true) {
        return Visit::Replace(r);
    }
    if let Some(r) = fold_mul_of_bool_extends(c, inst) {
        return r;
    }
    if let Some(r) = fold_mul_to_select(c, inst) {
        return r;
    }
    // DEVIATE 3: ((ashr X, 31) | 1) * X --> abs(X)
    // DEVIATE 5: narrowMathIfNoOverflow.
    // DEVIATE 2: foldBinOpOfSelectAndCastOfSelectCondition.

    // (mul Op0 Op1): if Log2(Op0) folds away -> (shl Op1, Log2(Op0)), and if
    // Log2(Op1) folds away -> (shl Op0, Log2(Op1)).
    for (log_of, other) in [(op0, op1), (op1, op0)] {
        if let Some(res) = try_get_log2(c, inst, log_of, false) {
            // We can only propagate the nuw flag.
            return c.replace_with_binop(inst, BinaryOp::Shl, other, res, wrap(has_nuw, false));
        }
    }

    let mut now = flags;
    if !has_nsw && compute_overflow_for_signed_mul(&query(c, Some(inst)), op0, op1) == OverflowResult::NeverOverflows {
        now.insert(Flags::NSW);
    }
    if !has_nuw
        && compute_overflow_for_unsigned_mul(&query(c, Some(inst)), op0, op1, now.contains(Flags::NSW))
            == OverflowResult::NeverOverflows
    {
        now.insert(Flags::NUW);
    }
    if now == flags {
        return Visit::None;
    }
    c.function.set_flags(inst, now);
    Visit::Changed
}
