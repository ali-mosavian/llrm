//! InstCombine's `visitMul`, `visitUDiv`, `visitSDiv`, `visitURem`, `visitSRem`
//! and the helpers they call, from
//! [`InstCombineMulDivRem.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp)
//! at d1106deb71cc.
//!
//! [`visit_mul`] (and likewise `visit_udiv`, `visit_sdiv`, `visit_urem`,
//! `visit_srem`) is
//! [`visitMul`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L193):
//! the same steps in the same order, each a function named for the C++ one
//! (or for the block of the `visit*` it is) with its own permalink. A step that
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
//! DEVIATE 2 (temporary): `foldBinOpSelectBinOp` and
//! `foldBinOpOfSelectAndCastOfSelectCondition` (select family) are left out,
//! each a `// DEVIATE 2:` comment at its place in the order.
//!
//! DEVIATE 3 (temporary): the folds that build an intrinsic call (`abs`,
//! `umin`, `umax`, `cttz`) are left out until the Builder can declare an
//! intrinsic (`combine_build` DEVIATE 2); each is a `// DEVIATE 3:` comment at
//! its place. `takeLog2` cannot look through `umin`/`umax` for the same reason.
//!
//! DEVIATE 4: a constant that `ConstantExpr::getNeg` or `getShl` would leave
//! as an expression is not built (MIR holds no binary constant expressions):
//! the fold is skipped where the constants do not fold to a number.
//!
//! DEVIATE 6 (temporary): `Negator::Negate` is not translated; it is taken to
//! fail, so the two folds that use it are skipped and the step after each
//! (the `zext`/`sext` multiply by `-1 << C`) is tried where C++ would stop.
//!
//! DEVIATE 7 (temporary): `SimplifyDemandedInstructionBits` is not translated
//! (no demanded-bits simplifier in `combine`); its steps in
//! `commonIDivTransforms` and `commonIRemTransforms` are left out.
//!
//! DEVIATE 8: `simplifyICmpInst` in `visitURem` is asked without the context
//! instruction, dominator tree and assumptions (no query that takes them):
//! it can only find less.
//!
//! # Used by
//! `combine::visit`, once the mul, div and rem slots are flipped.
//!
//! # How it works
//! Each `visit_*` returns the first step that changes the multiply. The steps
//! rewrite it to a cheaper form (`x * 2^k` to a shift, `x * -1` to a negation,
//! a multiply by a boolean to a `select`), or replace it by an existing value.
//! A rewrite that needs new operands builds them with the `Combiner`'s
//! builder, which folds through `simplify*` first. If nothing rewrites it,
//! known bits and ranges may add a multiply's `nsw` and `nuw`. A divide by a
//! power of two becomes a shift, one by a constant is folded into its
//! operands, and `srem`/`urem` into a mask or the other remainder.
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
use llrm_mir::valuetracking::guarantee::{
    is_guaranteed_not_to_be_poison, is_guaranteed_not_to_be_undef, is_guaranteed_to_transfer_execution_to_successor,
};
use llrm_mir::valuetracking::nonzero;
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};
use llrm_mir::valuetracking::power2::is_known_to_be_a_power_of_two;
use llrm_mir::valuetracking::range::{
    OverflowResult, SelectOperand, compute_overflow_for_signed_mul, compute_overflow_for_unsigned_mul,
    is_known_negation,
};
use llrm_mir::valuetracking::{Query, known};

use crate::combine::{Combiner, Visit};
use crate::combine_distribute::fold_using_distributive_laws;
use crate::combine_shared::{has_one_use, query, replace_operand, simplify_associative_or_commutative};

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

/// Arithmetic on `w`-bit two's-complement numbers held in a `u128` (the
/// `APInt` operations the div and rem folds use).
pub(crate) mod number {
    use llrm_mir::context::{mask, signed};

    /// `multiplyOverflows(C1, C2, Product, IsSigned)`: the product of two
    /// `w`-bit numbers, none if it does not fit.
    pub fn mul_no_overflow(
        c1: u128,
        c2: u128,
        w: u32,
        is_signed: bool,
    ) -> Option<u128> {
        if !is_signed {
            return c1.checked_mul(c2).filter(|&p| p <= mask(w));
        }
        let p = signed(c1, w).checked_mul(signed(c2, w))?;
        let fits = w >= 128 || (-(1i128 << (w - 1))..(1i128 << (w - 1))).contains(&p);
        fits.then_some(p as u128 & mask(w))
    }

    /// `isMultiple(C1, C2, Quotient, IsSigned)`: `C1 / C2` if `C1` is a
    /// multiple of `C2`.
    pub fn is_multiple(
        c1: u128,
        c2: u128,
        w: u32,
        is_signed: bool,
    ) -> Option<u128> {
        // Bail if we will divide by zero.
        if c2 == 0 {
            return None;
        }
        if !is_signed {
            return (c1 % c2 == 0).then(|| c1 / c2);
        }
        // Bail if we would divide INT_MIN by -1.
        if c1 == 1u128 << (w - 1) && c2 == mask(w) {
            return None;
        }
        let (a, b) = (signed(c1, w), signed(c2, w));
        (a.wrapping_rem(b) == 0).then(|| a.wrapping_div(b) as u128 & mask(w))
    }

    /// `APIntOps::GreatestCommonDivisor(A, B, IsSigned)`.
    pub fn greatest_common_divisor(
        a: u128,
        b: u128,
        w: u32,
        is_signed: bool,
    ) -> u128 {
        let abs = |v: u128| if is_signed && signed(v, w) < 0 { v.wrapping_neg() & mask(w) } else { v };
        let (mut a, mut b) = (abs(a), abs(b));
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    }

    /// `APInt::getSignificantBits`.
    pub fn significant_bits(
        bits: u128,
        w: u32,
    ) -> u32 {
        let s = signed(bits, w);
        let redundant = if s < 0 { (!s).leading_zeros() } else { s.leading_zeros() };
        128 - redundant + 1
    }

    /// `-C` of a `w`-bit number.
    pub fn neg_bits(
        bits: u128,
        w: u32,
    ) -> u128 {
        bits.wrapping_neg() & mask(w)
    }

    /// `m_NegatedPower2`: `-(2^k)`.
    pub fn is_negated_power2(
        bits: u128,
        w: u32,
    ) -> bool {
        bits >> (w - 1) & 1 == 1 && (bits.wrapping_neg() & mask(w)).is_power_of_two()
    }
}

use number::{greatest_common_divisor, is_multiple, is_negated_power2, mul_no_overflow, neg_bits, significant_bits};

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

/// The `nsw` and `nuw` of `flags`.
fn wrap_of(flags: Flags) -> Flags {
    wrap(flags.contains(Flags::NUW), flags.contains(Flags::NSW))
}

/// The wrap flag a divide's operand must carry: `nsw` signed, `nuw` unsigned.
fn no_wrap_flag(is_signed: bool) -> Flags {
    if is_signed { Flags::NSW } else { Flags::NUW }
}

/// A new cast of `v` to the visited instruction's type, as what `visit*`
/// returns.
fn replace_with_cast(
    c: &mut Combiner,
    at: InstId,
    op: CastOp,
    v: Operand,
) -> Visit {
    let ty = c.function.instruction(at).ty;
    c.replace_with_new(at, Opcode::Cast(op), ty, vec![v], Flags::default())
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
        return Some(replace_with_cast(c, inst, CastOp::ZExt, and));
    }
    // (sext bool X) * (zext bool Y) --> sext (and X, Y)
    // (zext bool X) * (sext bool Y) --> sext (and X, Y)
    let mixed = both(c, CastOp::SExt, CastOp::ZExt).or_else(|| both(c, CastOp::ZExt, CastOp::SExt));
    if let Some((x, y)) = mixed
        && (has_one_use(c, op0) || has_one_use(c, op1))
    {
        let and = c.create_binop(inst, BinaryOp::And, x, y, Flags::default());
        return Some(replace_with_cast(c, inst, CastOp::SExt, and));
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

    // log2(umin(X, Y)) -> umin(log2(X), log2(Y))
    // log2(umax(X, Y)) -> umax(log2(X), log2(Y))
    if has_one_use(c, op) {
        for base in ["umin", "umax"] {
            let args = {
                let q = query(c, None);
                pattern::intrinsic_call(&q, op, &format!("llvm.{base}")).map(|(_, args)| args.to_vec())
            };
            // Use AssumeNonZero as false here. Otherwise we can hit case where
            // log2(umax(X, Y)) != umax(log2(X), log2(Y)) (because overflow).
            if let Some([x, y]) = args.as_deref()
                && let Some(log_x) = take_log2(c, at, *x, depth, false, do_fold)
                && let Some(log_y) = take_log2(c, at, *y, depth, false, do_fold)
            {
                return Some(if do_fold { c.create_intrinsic(at, base, ty, &[log_x, log_y]) } else { op });
            }
        }
    }

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
    let phi = c.fold_binop_with_phi_operands(inst);
    if !matches!(phi, Visit::None) {
        return phi;
    }
    if let Some(v) = fold_using_distributive_laws(c, inst) {
        return Visit::Replace(v);
    }
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
    let folded = c.fold_bin_op_into_select_or_phi(inst);
    if !matches!(folded, Visit::None) {
        return folded;
    }
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
    // abs(X) * abs(Y) -> abs(X * Y)
    if flags.contains(Flags::NSW) && has_one_use(c, op0) && has_one_use(c, op1) {
        let abs_of = |c: &Combiner, v: Operand| {
            let q = query(c, None);
            let (_, call) = pattern::intrinsic(&q, v, IntrinsicId::Abs)?;
            let args = pattern::call_arguments(call);
            pattern::one(&q, args[1]).then_some(args[0])
        };
        if let (Some(x), Some(y)) = (abs_of(c, op0), abs_of(c, op1)) {
            let product = c.create_binop(inst, BinaryOp::Mul, x, y, wrap(false, true));
            let ty = c.type_of(product);
            let i1 = c.context.types.int(1);
            let poison = c.int(i1, 1);
            return Visit::Replace(c.create_intrinsic(inst, "abs", ty, &[product, poison]));
        }
    }

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
    // ((ashr X, 31) | 1) * X --> abs(X)
    // X * ((ashr X, 31) | 1) --> abs(X)
    let sign_or_one = |c: &Combiner, v: Operand| {
        let q = query(c, None);
        let bits = width(c, v);
        let (l, r, _) = binop(&q, v, BinaryOp::Or)?;
        [(l, r), (r, l)]
            .into_iter()
            .find_map(
                |(shifted, one)| {
                    if !pattern::one(&q, one) {
                        return None;
                    }
                    let (x, amount, _) = binop(&q, shifted, BinaryOp::AShr)?;
                    pattern::apint(&q, amount).is_some_and(|(value, _)| value == (bits - 1) as u128).then_some(x)
                },
            )
    };
    let abs_of = [(op0, op1), (op1, op0)].into_iter().find_map(|(or, x)| (sign_or_one(c, or) == Some(x)).then_some(x));
    if let Some(x) = abs_of {
        let i1 = c.context.types.int(1);
        let poison = c.int(i1, u128::from(has_nsw));
        let ty = c.type_of(x);
        return Visit::Replace(c.create_intrinsic(inst, "abs", ty, &[x, poison]));
    }
    if let Some(narrowed) = c.narrow_math_if_no_overflow(inst) {
        return narrowed;
    }
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

// ---- div and rem ----

/// The `exact` flag, or none.
fn exact_if(exact: bool) -> Flags {
    if exact { Flags::EXACT } else { Flags::default() }
}

/// `m_c_Mul(m_Specific(x), m_Value(Y))`: `Y` and the flags of the multiply.
fn mul_by(
    q: &Query,
    v: Operand,
    x: Operand,
) -> Option<(Operand, Flags)> {
    let (a, b, flags) = binop(q, v, BinaryOp::Mul)?;
    if a == x {
        Some((b, flags))
    } else if b == x {
        Some((a, flags))
    } else {
        None
    }
}

/// [`simplifyValueKnownNonZero`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L47):
/// `v` is used where it is known non-zero; the operand to use instead, built
/// before `at`.
fn simplify_value_known_non_zero(
    c: &mut Combiner,
    v: Operand,
    at: InstId,
    cxt: InstId,
) -> Option<Operand> {
    // If V has multiple uses, then we would have to do more analysis to
    // determine if this is safe.
    if !has_one_use(c, v) {
        return None;
    }
    // ((1 << A) >>u B) --> (1 << (A-B)). Because V cannot be zero, we know
    // that B is less than A.
    let one_shl = {
        let q = query(c, Some(cxt));
        binop(&q, v, BinaryOp::LShr).and_then(|(shl, b, _)| {
            let (one, a, _) = binop(&q, shl, BinaryOp::Shl)?;
            (has_one_use(c, shl) && pattern::one(&q, one)).then_some((one, a, b))
        })
    };
    if let Some((one, a, b)) = one_shl {
        let a = c.create_sub(at, a, b);
        return Some(c.create_shl(at, one, a));
    }

    // (PowerOfTwo >>u B) --> isExact since shifting out the result would make
    // it inexact. Similarly for <<.
    let i = inst_def(c, v)?;
    let Opcode::Binary(op @ (BinaryOp::Shl | BinaryOp::LShr)) = c.function.instruction(i).opcode else { return None };
    let operand = c.function.instruction(i).operands[0];
    if !is_power_of_two_at(c, cxt, operand, false) {
        return None;
    }
    let mut made_change = false;
    // We know that this is an exact/nuw shift and that the input is a non-zero
    // context as well.
    if let Some(v2) = simplify_value_known_non_zero(c, operand, i, cxt) {
        replace_operand(c, i, 0, v2);
        made_change = true;
    }
    let flags = c.function.instruction(i).flags;
    let wanted = if op == BinaryOp::LShr { Flags::EXACT } else { Flags::NUW };
    if !flags.contains(wanted) {
        let mut new = flags;
        new.insert(wanted);
        c.function.set_flags(i, new);
        made_change = true;
    }
    made_change.then_some(v)
}

/// `isKnownToBeAPowerOfTwo(v, OrZero, &I)`.
fn is_power_of_two_at(
    c: &Combiner,
    inst: InstId,
    v: Operand,
    or_zero: bool,
) -> bool {
    is_known_to_be_a_power_of_two(&query(c, Some(inst)), v, or_zero, 0)
}

/// [`simplifyDivRemOfSelectWithZeroOp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L1141):
/// `div/rem X, (Cond ? 0 : Y)` is `div/rem X, Y`.
fn simplify_div_rem_of_select_with_zero_op(
    c: &mut Combiner,
    inst: InstId,
) -> bool {
    let divisor = c.function.instruction(inst).operands[1];
    let found = {
        let q = query(c, Some(inst));
        let Some((select, _, cond, on_true, on_false)) = pattern::select(&q, divisor) else { return false };
        // div/rem X, (Cond ? 0 : Y) -> div/rem X, Y; and (Cond ? Y : 0).
        if pattern::zero(&q, on_true) {
            (select, cond, on_false, false)
        } else if pattern::zero(&q, on_false) {
            (select, cond, on_true, true)
        } else {
            return false;
        }
    };
    let (select, cond, non_null, cond_is_true) = found;
    // Change the div/rem to use 'Y' instead of the select.
    replace_operand(c, inst, 1, non_null);

    // The select, or the condition of the select, may have multiple uses.
    // Based on our knowledge that the operand must be non-zero, propagate the
    // known value for the select into other uses of it, and a known value of
    // the condition into its other users.
    let select_value = c.function.instruction(select).result.expect("a value");
    if c.function.users(select_value).is_empty() && has_one_use(c, cond) {
        return true;
    }
    let cond_ty = c.type_of(cond);
    let known_cond = c.int(cond_ty, cond_is_true as u128);
    let (mut si, mut select_cond) = (Some(Operand::Value(select_value)), Some(cond));
    // Scan the current block backward, looking for other uses of SI.
    let mut at = inst;
    while let Some(prev) = c.function.prev_in_block(at) {
        at = prev;
        // An instruction we can't assume will return: information from below
        // it cannot be propagated above it.
        if !is_guaranteed_to_transfer_execution_to_successor(&query(c, None), at) {
            break;
        }
        // Replace uses of the select or its condition with the known values.
        for index in 0..c.function.instruction(at).operands.len() {
            let op = c.function.instruction(at).operands[index];
            if Some(op) == si {
                replace_operand(c, at, index, non_null);
                c.worklist.push(at);
            } else if Some(op) == select_cond {
                replace_operand(c, at, index, known_cond);
                c.worklist.push(at);
            }
        }
        // If we past the instruction, quit looking for it.
        let result = c.function.instruction(at).result.map(Operand::Value);
        if at == select {
            si = None;
        }
        if result.is_some() && result == select_cond {
            select_cond = None;
        }
        // If we ran out of things to eliminate, break out of the loop.
        if select_cond.is_none() && si.is_none() {
            break;
        }
    }
    true
}

/// [`foldIDivShl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L1237):
/// a common factor of dividend and divisor disguised as a left shift.
fn fold_idiv_shl(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Operand> {
    let Opcode::Binary(op) = c.function.instruction(inst).opcode else { return None };
    let is_signed = op == BinaryOp::SDiv;
    let (op0, op1, flags) = parts(c, inst);
    let exact = flags.contains(Flags::EXACT);
    let ty = c.type_of(op0);

    // With appropriate no-wrap constraints, remove a common factor in the
    // dividend and divisor that is disguised as a left-shifted value.
    let mul_by_shl = {
        let q = query(c, Some(inst));
        binop(&q, op1, BinaryOp::Shl).and_then(|(x, z, shl_flags)| {
            let (y, mul_flags) = mul_by(&q, op0, x)?;
            Some((z, y, mul_flags, shl_flags))
        })
    };
    if let Some((z, y, mul_flags, shl_flags)) = mul_by_shl {
        // Both operands must have the matching no-wrap for this kind of
        // division.
        let has_nuw = mul_flags.contains(Flags::NUW) && shl_flags.contains(Flags::NUW);
        let has_nsw = mul_flags.contains(Flags::NSW) && shl_flags.contains(Flags::NSW);
        // (X * Y) u/ (X << Z) --> Y u>> Z
        if !is_signed && has_nuw {
            return Some(c.create_binop(inst, BinaryOp::LShr, y, z, exact_if(exact)));
        }
        // (X * Y) s/ (X << Z) --> Y s/ (1 << Z)
        if is_signed && has_nsw && (has_one_use(c, op0) || has_one_use(c, op1)) {
            let one = c.int(ty, 1);
            let shl = c.create_shl(inst, one, z);
            return Some(c.create_binop(inst, BinaryOp::SDiv, y, shl, exact_if(exact)));
        }
    }

    // With appropriate no-wrap constraints, remove a common factor in the
    // dividend and divisor that is disguised as a left-shift amount.
    let shl_pair = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Shl).and_then(|(x, z, f0)| {
            let (y, z1, f1) = binop(&q, op1, BinaryOp::Shl)?;
            (z1 == z).then_some((x, y, f0, f1))
        })
    };
    if let Some((x, y, f0, f1)) = shl_pair {
        let (nuw0, nsw0, nuw1, nsw1) =
            (f0.contains(Flags::NUW), f0.contains(Flags::NSW), f1.contains(Flags::NUW), f1.contains(Flags::NSW));
        // For unsigned div, we need 'nuw' on both shifts or 'nsw' on both
        // shifts + 'nuw' on the dividend.
        if !is_signed && ((nuw0 && nuw1) || (nuw0 && nsw0 && nsw1)) {
            return Some(c.create_binop(inst, BinaryOp::UDiv, x, y, exact_if(exact)));
        }
        // For signed div, we need 'nsw' on both shifts + 'nuw' on the divisor.
        if is_signed && nsw0 && nsw1 && nuw1 {
            return Some(c.create_binop(inst, BinaryOp::SDiv, x, y, exact_if(exact)));
        }
    }

    // If X << Y and X << Z does not overflow, then:
    // (X << Y) / (X << Z) -> (1 << Y) / (1 << Z) -> 1 << Y >> Z
    let same_base = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Shl).and_then(|(x, y, f0)| {
            let (x1, z, f1) = binop(&q, op1, BinaryOp::Shl)?;
            (x1 == x).then_some((x, y, z, f0, f1))
        })
    };
    if let Some((x, y, z, f0, f1)) = same_base {
        let no_wrap = if is_signed {
            f0.contains(Flags::NSW) && f1.contains(Flags::NSW)
        } else {
            f0.contains(Flags::NUW) && f1.contains(Flags::NUW)
        };
        if no_wrap {
            let one = c.int(c.type_of(x), 1);
            // Only preserve the nsw flag if dividend has nsw or divisor has
            // nsw and operator is sdiv.
            let nsw =
                if is_signed { f0.contains(Flags::NUW) || f1.contains(Flags::NUW) } else { f0.contains(Flags::NSW) };
            let dividend = c.create_binop(inst, BinaryOp::Shl, one, y, wrap(true, nsw));
            return Some(c.create_binop(inst, BinaryOp::LShr, dividend, z, exact_if(exact)));
        }
    }
    None
}

/// [`commonIDivRemTransforms`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L1318):
/// the transforms common to the integer divides and remainders.
fn common_idiv_rem_transforms(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    // DEVIATE 1: a constant vector divisor with a zero or undef element.
    let phi = c.fold_binop_with_phi_operands(inst);
    if !matches!(phi, Visit::None) {
        return Some(phi);
    }

    // The RHS is known non-zero.
    let divisor = c.function.instruction(inst).operands[1];
    if let Some(v) = simplify_value_known_non_zero(c, divisor, inst, inst) {
        replace_operand(c, inst, 1, v);
        return Some(Visit::Changed);
    }
    // Handle cases involving: div/rem X, (select Cond, Y, Z)
    if simplify_div_rem_of_select_with_zero_op(c, inst) {
        return Some(Visit::Changed);
    }
    // If the divisor is a select-of-constants, try to constant fold all div
    // ops: C div/rem (select Cond, TrueC, FalseC) --> select Cond, (C div/rem
    // TrueC), (C div/rem FalseC)
    let (op0, op1, _) = parts(c, inst);
    if imm_constant(c, op0)
        && let Some(select) = inst_def(c, op1).filter(|&def| c.function.instruction(def).opcode == Opcode::Select)
        && c.function.instruction(select).operands[1..].iter().all(|&arm| imm_constant(c, arm))
    {
        let found = c.fold_op_into_select(inst, select, true, false);
        if !matches!(found, Visit::None) {
            return Some(found);
        }
    }
    None
}

/// The `X / C1` of `op0` when it divides with `op` by a constant: `X` and
/// `C1`.
fn div_by_constant(
    c: &Combiner,
    v: Operand,
    op: BinaryOp,
) -> Option<(Operand, u128)> {
    let q = query(c, None);
    let (x, d, _) = binop(&q, v, op)?;
    Some((x, q.int_constant(d)?.0))
}

/// [`commonIDivTransforms`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L1358):
/// the transforms common to `udiv` and `sdiv`.
fn common_idiv_transforms(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    if let Some(res) = common_idiv_rem_transforms(c, inst) {
        return Some(res);
    }
    let Opcode::Binary(op) = c.function.instruction(inst).opcode else { return None };
    let (op0, op1, flags) = parts(c, inst);
    let is_signed = op == BinaryOp::SDiv;
    let exact = flags.contains(Flags::EXACT);
    let ty = c.type_of(op0);
    let w = width(c, op0);
    let new_div = |c: &mut Combiner, x: Operand, d: u128| {
        let d = c.int(ty, d);
        c.replace_with_binop(inst, op, x, d, exact_if(exact))
    };

    if let Some((c2, _)) = apint(c, op1) {
        // (X / C1) / C2  -> X / (C1*C2)
        if let Some((x, c1)) = div_by_constant(c, op0, op)
            && let Some(product) = mul_no_overflow(c1, c2, w, is_signed)
        {
            let d = c.int(ty, product);
            return Some(c.replace_with_binop(inst, op, x, d, Flags::default()));
        }

        // (X * C1) / C2 with the multiply not wrapping.
        let mul = {
            let q = query(c, Some(inst));
            let wrap_flag = no_wrap_flag(is_signed);
            binop(&q, op0, BinaryOp::Mul)
                .filter(|&(.., f)| f.contains(wrap_flag))
                .and_then(|(x, k, f)| q.int_constant(k).map(|(k, _)| (x, k, f)))
        };
        if let Some((x, c1, mul_flags)) = mul {
            // (X * C1) / C2 -> X / (C2 / C1) if C2 is a multiple of C1.
            if let Some(quotient) = is_multiple(c2, c1, w, is_signed) {
                return Some(new_div(c, x, quotient));
            }
            // (X * C1) / C2 -> X * (C1 / C2) if C1 is a multiple of C2.
            if let Some(quotient) = is_multiple(c1, c2, w, is_signed) {
                let q = c.int(ty, quotient);
                let nuw = !is_signed && mul_flags.contains(Flags::NUW);
                return Some(c.replace_with_binop(
                    inst,
                    BinaryOp::Mul,
                    x,
                    q,
                    wrap(nuw, mul_flags.contains(Flags::NSW)),
                ));
            }
            // (X * C1) / C2 -> (X * (C1/D)) / (C2/D) if D = gcd(C1, C2) > 1.
            if has_one_use(c, op0) {
                let gcd = greatest_common_divisor(c1, c2, w, is_signed);
                if gcd > 1 {
                    let split = |v: u128| {
                        if is_signed { signed(v, w).wrapping_div(signed(gcd, w)) as u128 & mask(w) } else { v / gcd }
                    };
                    let (new_c1, new_c2) = (c.int(ty, split(c1)), split(c2));
                    let new_mul = c.create_binop(inst, BinaryOp::Mul, x, new_c1, wrap_of(mul_flags));
                    return Some(new_div(c, new_mul, new_c2));
                }
            }
        }

        // (X << C1) / C2 with the shift not wrapping and C1 in range.
        let limit = (if is_signed { w - 1 } else { w }) as u128;
        let shl = {
            let q = query(c, Some(inst));
            let wrap_flag = no_wrap_flag(is_signed);
            binop(&q, op0, BinaryOp::Shl)
                .filter(|&(.., f)| f.contains(wrap_flag))
                .and_then(|(x, k, f)| q.int_constant(k).map(|(k, _)| (x, k, f)))
                .filter(|&(_, k, _)| k < limit)
        };
        if let Some((x, c1, shl_flags)) = shl {
            let c1_shifted = 1u128 << c1;
            // (X << C1) / C2 -> X / (C2 >> C1) if C2 is a multiple of 1 << C1.
            if let Some(quotient) = is_multiple(c2, c1_shifted, w, is_signed) {
                return Some(new_div(c, x, quotient));
            }
            // (X << C1) / C2 -> X * ((1 << C1) / C2) if 1 << C1 is a multiple
            // of C2.
            if let Some(quotient) = is_multiple(c1_shifted, c2, w, is_signed) {
                let q = c.int(ty, quotient);
                let nuw = !is_signed && shl_flags.contains(Flags::NUW);
                return Some(c.replace_with_binop(
                    inst,
                    BinaryOp::Mul,
                    x,
                    q,
                    wrap(nuw, shl_flags.contains(Flags::NSW)),
                ));
            }
            // (X << C1) / C2 -> (X << (C1 - K)) / (C2 / (1 << K)) where K =
            // min(C1, countr_zero(C2)), the shared power of 2.
            if has_one_use(c, op0) {
                let k = (c2.trailing_zeros().min(w) as u128).min(c1) as u32;
                if k > 0 {
                    let new_c2 = if is_signed { signed(c2, w) >> k } else { (c2 >> k) as i128 };
                    let new_c2 = new_c2 as u128 & mask(w);
                    let amount = c.int(ty, c1 - k as u128);
                    let new_shift = c.create_binop(inst, BinaryOp::Shl, x, amount, wrap_of(shl_flags));
                    return Some(new_div(c, new_shift, new_c2));
                }
            }
        }

        // Distribute div over add to eliminate a matching div/mul pair:
        // ((X * C2) + C1) / C2 --> X + C1/C2
        let add_of_mul = {
            let q = query(c, Some(inst));
            let wrap_flag = no_wrap_flag(is_signed);
            pattern::add_like(&q, op0, wrap_flag).and_then(|(m, k)| {
                let (x, factor, f) = binop(&q, m, BinaryOp::Mul)?;
                let (k, _) = q.int_constant(k)?;
                (f.contains(wrap_flag) && q.int_constant(factor)?.0 == c2).then_some((x, k))
            })
        };
        if let Some((x, c1)) = add_of_mul {
            if is_signed && let Some(quotient) = is_multiple(c1, c2, w, true) {
                let q = c.int(ty, quotient);
                return Some(c.replace_with_binop(inst, BinaryOp::Add, x, q, wrap(false, true)));
            }
            if !is_signed && c2 != 0 {
                let q = c.int(ty, c1 / c2);
                return Some(c.replace_with_binop(inst, BinaryOp::Add, x, q, wrap(true, false)));
            }
        }

        if c2 != 0 {
            let folded = c.fold_bin_op_into_select_or_phi(inst);
            if !matches!(folded, Visit::None) {
                return Some(folded);
            }
        }
    }

    if pattern::one(&query(c, Some(inst)), op0) {
        if is_signed {
            // 1 / 0 --> undef ; 1 / 1 --> 1 ; 1 / -1 --> -1 ; 1 / anything
            // else --> 0, as (Op1 + 1) u< 3 ? Op1 : 0. Op1 must be frozen
            // because we are increasing its number of uses.
            let f1 = freeze_unless_guaranteed(c, inst, op1);
            let inc = c.create_add(inst, f1, op0);
            let three = c.int(ty, 3);
            let cmp = c.create_icmp(inst, IntPredicate::Ult, inc, three);
            let zero = c.int(ty, 0);
            return Some(replace_with_select(c, inst, cmp, f1, zero));
        }
        // If Op1 is 0 then it's undefined behaviour. If Op1 is 1 then the
        // result is one, otherwise it's zero.
        let cmp = c.create_icmp(inst, IntPredicate::Eq, op1, op0);
        return Some(replace_with_cast(c, inst, CastOp::ZExt, cmp));
    }

    // DEVIATE 7: SimplifyDemandedInstructionBits.

    // (X - (X rem Y)) / Y -> X / Y; usually originates as ((X / Y) * Y) / Y
    let rem_op = if is_signed { BinaryOp::SRem } else { BinaryOp::URem };
    let sub = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Sub)
            .filter(|&(x, z, _)| binop(&q, z, rem_op).is_some_and(|(rx, ry, _)| rx == x && ry == op1))
    };
    if let Some((x, ..)) = sub {
        return Some(c.replace_with_binop(inst, op, x, op1, Flags::default()));
    }

    // (X << Y) / X -> 1 << Y
    let wrap_flag = no_wrap_flag(is_signed);
    let shl_of_divisor = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Shl).filter(|&(x, _, f)| x == op1 && f.contains(wrap_flag))
    };
    if let Some((_, y, _)) = shl_of_divisor {
        let one = c.int(ty, 1);
        return Some(c.replace_with_binop(inst, BinaryOp::Shl, one, y, wrap(!is_signed, is_signed)));
    }

    // X / (X * Y) -> 1 / Y if the multiplication does not overflow.
    let mul_divisor = mul_by(&query(c, Some(inst)), op1, op0);
    if let Some((y, f)) = mul_divisor
        && f.contains(wrap_flag)
    {
        let one = c.int(ty, 1);
        replace_operand(c, inst, 0, one);
        replace_operand(c, inst, 1, y);
        return Some(Visit::Changed);
    }

    // (X << Z) / (X * Y) -> (1 << Z) / Y
    if !is_signed && has_one_use(c, op1) {
        let found = {
            let q = query(c, Some(inst));
            binop(&q, op0, BinaryOp::Shl)
                .filter(|&(.., f)| f.contains(Flags::NUW))
                .and_then(|(x, z, _)| mul_by(&q, op1, x).map(|(y, f)| (z, y, f)))
        };
        if let Some((z, y, f)) = found
            && f.contains(Flags::NUW)
        {
            let one = c.int(ty, 1);
            let shl = c.create_binop(inst, BinaryOp::Shl, one, z, wrap(true, false));
            return Some(c.replace_with_binop(inst, BinaryOp::UDiv, shl, y, exact_if(exact)));
        }
    }

    if let Some(r) = fold_idiv_shl(c, inst) {
        return Some(Visit::Replace(r));
    }

    // With the appropriate no-wrap constraint, remove a multiply by the
    // divisor after peeking through another divide:
    // ((Op1 * X) / Y) / Op1 --> X / Y
    let through_div = {
        let q = query(c, Some(inst));
        binop(&q, op0, op).and_then(|(m, y, inner)| {
            let (x, mul_flags) = mul_by(&q, m, op1)?;
            Some((x, y, inner, mul_flags))
        })
    };
    if let Some((x, y, inner, mul_flags)) = through_div {
        let ok = mul_flags.contains(wrap_flag);
        if ok {
            // Exact propagates only if both of the original divides are exact.
            let both_exact = exact && inner.contains(Flags::EXACT);
            return Some(c.replace_with_binop(inst, op, x, y, exact_if(both_exact)));
        }
    }

    // X / (select Cond, 1, Y) --> select Cond, X, (X / Y)
    // X / (select Cond, Y, 1) --> select Cond, (X / Y), X
    // Division by 1 is a no-op, so we sink the division into the non-1 arm.
    // For sdiv, limit Y to constant to avoid signed overflow concern.
    if has_one_use(c, op1) {
        let select = {
            let q = query(c, Some(inst));
            pattern::select(&q, op1).map(|(_, _, cond, t, f)| (cond, t, f))
        };
        if let Some((cond, t, f)) = select {
            let safe = |c: &Combiner, v: Operand| {
                let q = query(c, Some(inst));
                if is_signed {
                    q.int_constant(v).is_some_and(|(bits, w)| bits != 0 && bits != mask(w))
                } else {
                    nonzero::is_known_non_zero(&q, v, 0) && is_guaranteed_not_to_be_poison(&q, v, 0)
                }
            };
            let one_first = pattern::one(&query(c, Some(inst)), t) && safe(c, f);
            let one_second = !one_first && pattern::one(&query(c, Some(inst)), f) && safe(c, t);
            if one_first || one_second {
                let div_y = if one_first { f } else { t };
                let div = c.create_binop(inst, op, op0, div_y, exact_if(exact));
                let (a, b) = if one_first { (op0, div) } else { (div, op0) };
                return Some(replace_with_select(c, inst, cond, a, b));
            }
        }
    }

    // (X * Y) / (X * Z) --> Y / Z (and commuted variants)
    let mul0 = binop(&query(c, Some(inst)), op0, BinaryOp::Mul);
    if let Some((x, y, f0)) = mul0 {
        let (ob0_nsw, ob0_nuw) = (f0.contains(Flags::NSW), f0.contains(Flags::NUW));
        let create_div_or_null = |c: &Combiner, a: Operand, b: Operand| -> Option<(Operand, Operand)> {
            let q = query(c, Some(inst));
            let f1 = binop(&q, op1, BinaryOp::Mul)?.2;
            if is_signed && ob0_nsw && f1.contains(Flags::NSW) && q.int_constant(b).is_some_and(|(k, w)| k != mask(w)) {
                return Some((a, b));
            }
            if !is_signed && ob0_nuw {
                if f1.contains(Flags::NUW) {
                    return Some((a, b));
                }
                if let (Some((c1, _)), Some((c2, _))) = (q.int_constant(a), q.int_constant(b))
                    && c2 <= c1
                {
                    return Some((a, b));
                }
            }
            None
        };
        let by_x = mul_by(&query(c, Some(inst)), op1, x).and_then(|(z, _)| create_div_or_null(c, y, z));
        if let Some((a, b)) = by_x {
            return Some(c.replace_with_binop(inst, op, a, b, Flags::default()));
        }
        let by_y = mul_by(&query(c, Some(inst)), op1, y).and_then(|(z, _)| create_div_or_null(c, x, z));
        if let Some((a, b)) = by_y {
            return Some(c.replace_with_binop(inst, op, a, b, Flags::default()));
        }
    }
    None
}

/// [`narrowUDivURem`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L1750):
/// a `udiv` or `urem` of zero-extended operands, sunk below the extend.
fn narrow_udiv_urem(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let Opcode::Binary(op) = c.function.instruction(inst).opcode else { return None };
    let (n, d, _) = parts(c, inst);
    let zext_of = |c: &Combiner, v: Operand| pattern::cast(&query(c, None), v, CastOp::ZExt).map(|(_, _, x)| x);
    let (zn, zd) = (zext_of(c, n), zext_of(c, d));
    if let (Some(x), Some(y)) = (zn, zd)
        && c.type_of(x) == c.type_of(y)
        && (has_one_use(c, n) || has_one_use(c, d))
    {
        // udiv (zext X), (zext Y) --> zext (udiv X, Y)
        let narrow = c.create_binop(inst, op, x, y, Flags::default());
        return Some(replace_with_cast(c, inst, CastOp::ZExt, narrow));
    }
    // The constant, if it is the same in the smaller type.
    let lossless = |c: &Combiner, k: Operand, narrow: u32| {
        let (bits, _) = apint(c, k)?;
        (bits <= mask(narrow)).then_some(bits)
    };
    if inst_def(c, n).is_some()
        && has_one_use(c, n)
        && let Some(x) = zn
        && matches!(d, Operand::Constant(_))
    {
        let xty = c.type_of(x);
        let narrow_c = lossless(c, d, width(c, x))?;
        // udiv (zext X), C --> zext (udiv X, C')
        let k = c.int(xty, narrow_c);
        let narrow = c.create_binop(inst, op, x, k, Flags::default());
        return Some(replace_with_cast(c, inst, CastOp::ZExt, narrow));
    }
    if inst_def(c, d).is_some()
        && has_one_use(c, d)
        && let Some(x) = zd
        && matches!(n, Operand::Constant(_))
    {
        let xty = c.type_of(x);
        let narrow_c = lossless(c, n, width(c, x))?;
        // udiv C, (zext X) --> zext (udiv C', X)
        let k = c.int(xty, narrow_c);
        let narrow = c.create_binop(inst, op, k, x, Flags::default());
        return Some(replace_with_cast(c, inst, CastOp::ZExt, narrow));
    }
    None
}

/// [`InstCombinerImpl::visitUDiv`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L1793).
pub fn visit_udiv(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifyUDivInst
    if let Some(found) = simplify_first(c, inst) {
        return Visit::Replace(found);
    }
    // DEVIATE 1: foldVectorBinop.
    // Handle the integer div common cases
    if let Some(common) = common_idiv_transforms(c, inst) {
        return common;
    }
    let (op0, op1, flags) = parts(c, inst);
    let ty = c.type_of(op0);
    let exact = flags.contains(Flags::EXACT);

    // (X lshr C1) udiv C2 --> X udiv (C2 << C1)
    let lshr = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::LShr).and_then(|(x, c1, f)| {
            let (c1, _) = q.int_constant(c1)?;
            let (c2, w) = q.int_constant(op1)?;
            Some((x, c1, c2, w, f))
        })
    };
    if let Some((x, c1, c2, w, lshr_flags)) = lshr {
        // C2->ushl_ov(*C1, Overflow)
        let overflow = c1 >= w as u128 || c1 > (c2.leading_zeros() - (128 - w)) as u128;
        if !overflow {
            let is_exact = exact && lshr_flags.contains(Flags::EXACT);
            let d = c.int(c.type_of(x), (c2 << c1) & mask(w));
            return c.replace_with_binop(inst, BinaryOp::UDiv, x, d, exact_if(is_exact));
        }
    }

    // (X udiv Y) udiv Z --> X udiv (Y * Z), if Y * Z does not overflow. This
    // is the variable-operand version of the (X / C1) / C2 fold in
    // commonIDivTransforms().
    let inner = {
        let q = query(c, Some(inst));
        if has_one_use(c, op0) { binop(&q, op0, BinaryOp::UDiv) } else { None }
    };
    if let Some((x, y, inner_flags)) = inner
        && compute_overflow_for_unsigned_mul(&query(c, Some(inst)), y, op1, false) == OverflowResult::NeverOverflows
    {
        let yz = c.create_binop(inst, BinaryOp::Mul, y, op1, wrap(true, false));
        // The result is exact only if both of the original divides are exact.
        let both_exact = exact && inner_flags.contains(Flags::EXACT);
        return c.replace_with_binop(inst, BinaryOp::UDiv, x, yz, exact_if(both_exact));
    }

    // Op0 / C where C is large (negative) --> zext (Op0 >= C). This also
    // handles non-constant values where the sign bit is known to be set.
    if known::is_known_negative(&query(c, Some(inst)), op1, 0) {
        let cmp = c.create_icmp(inst, IntPredicate::Uge, op0, op1);
        return replace_with_cast(c, inst, CastOp::ZExt, cmp);
    }
    // Op0 / (sext i1 X) --> zext (Op0 == -1) (if X is 0, the div is undefined)
    if bool_extend(c, op1, CastOp::SExt).is_some() {
        let ones = c.all_ones(ty);
        let cmp = c.create_icmp(inst, IntPredicate::Eq, op0, ones);
        return replace_with_cast(c, inst, CastOp::ZExt, cmp);
    }

    if let Some(narrow) = narrow_udiv_urem(c, inst) {
        return narrow;
    }

    // Look through a right-shift to find the common factor:
    // ((Op1 *nuw A) >> B) / Op1 --> A >> B
    let through_shift = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::LShr).and_then(|(m, b, f)| {
            let (a, mf) = mul_by(&q, m, op1)?;
            mf.contains(Flags::NUW).then_some((a, b, f))
        })
    };
    if let Some((a, b, shr_flags)) = through_shift {
        let is_exact = exact && shr_flags.contains(Flags::EXACT);
        return c.replace_with_binop(inst, BinaryOp::LShr, a, b, exact_if(is_exact));
    }

    // Op0 udiv Op1 -> Op0 lshr log2(Op1), if log2() folds away.
    if let Some(log2) = try_get_log2(c, inst, op1, true) {
        let shr = c.create_binop(inst, BinaryOp::LShr, op0, log2, exact_if(exact));
        return Visit::Replace(shr);
    }
    // Op0 udiv Op1 -> Op0 lshr cttz(Op1), if Op1 is a power of 2. This will
    // increase instruction count but it's okay since bitwise operations are
    // substantially faster than division.
    if is_power_of_two_at(c, inst, op1, true) {
        let i1 = c.context.types.int(1);
        let poison_on_zero = c.int(i1, 1);
        let cttz = c.create_intrinsic(inst, "cttz", ty, &[op1, poison_on_zero]);
        return Visit::Replace(c.create_binop(inst, BinaryOp::LShr, op0, cttz, exact_if(exact)));
    }
    Visit::None
}

/// [`InstCombinerImpl::visitSDiv`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L1888).
pub fn visit_sdiv(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifySDivInst
    if let Some(found) = simplify_first(c, inst) {
        return Visit::Replace(found);
    }
    // DEVIATE 1: foldVectorBinop.
    // Handle the integer div common cases
    if let Some(common) = common_idiv_transforms(c, inst) {
        return common;
    }
    let (op0, op1, flags) = parts(c, inst);
    let ty = c.type_of(op0);
    let w = width(c, op0);
    let exact = flags.contains(Flags::EXACT);

    // sdiv Op0, -1 --> -Op0
    // sdiv Op0, (sext i1 X) --> -Op0 (because if X is 0, the op is undefined)
    if pattern::all_ones(&query(c, Some(inst)), op1) || bool_extend(c, op1, CastOp::SExt).is_some() {
        return replace_with_neg(c, inst, op0, true);
    }

    // X / INT_MIN --> X == INT_MIN
    if pattern::sign_mask(&query(c, Some(inst)), op1) {
        let cmp = c.create_icmp(inst, IntPredicate::Eq, op0, op1);
        return replace_with_cast(c, inst, CastOp::ZExt, cmp);
    }

    let op1_c = apint(c, op1);
    if exact {
        // sdiv exact X, 1<<C --> ashr exact X, C   iff  1<<C  is non-negative
        if let Some((bits, _)) = op1_c
            && bits.is_power_of_two()
            && bits >> (w - 1) == 0
        {
            let log2 = c.int(ty, bits.trailing_zeros() as u128);
            return c.replace_with_binop(inst, BinaryOp::AShr, op0, log2, Flags::EXACT);
        }

        // sdiv exact X, (1<<ShAmt) --> ashr exact X, ShAmt (if shl is
        // non-negative)
        let shl_one = {
            let q = query(c, Some(inst));
            binop(&q, op1, BinaryOp::Shl).filter(|&(one, _, f)| pattern::one(&q, one) && f.contains(Flags::NSW))
        };
        if let Some((_, sh_amt, _)) = shl_one {
            return c.replace_with_binop(inst, BinaryOp::AShr, op0, sh_amt, Flags::EXACT);
        }

        // sdiv exact X, -1<<C --> -(ashr exact X, C)
        if let Some((bits, _)) = op1_c
            && is_negated_power2(bits, w)
        {
            let log2 = c.int(ty, neg_bits(bits, w).trailing_zeros() as u128);
            let ashr = c.create_binop(inst, BinaryOp::AShr, op0, log2, Flags::EXACT);
            return replace_with_neg(c, inst, ashr, true);
        }
    }

    if let Some((bits, _)) = op1_c {
        // If the dividend is sign-extended and the constant divisor is small
        // enough to fit in the source type, shrink the division to the
        // narrower type: (sext X) sdiv C --> sext (X sdiv C)
        let src = {
            let q = query(c, Some(inst));
            pattern::cast(&q, op0, CastOp::SExt).map(|(_, _, x)| x).filter(|_| has_one_use(c, op0))
        };
        if let Some(src) = src
            && width(c, src) >= significant_bits(bits, w)
        {
            // The -1 divisor case is already handled above.
            let src_ty = c.type_of(src);
            let narrow_divisor = c.int(src_ty, bits);
            let narrow = c.create_binop(inst, BinaryOp::SDiv, src, narrow_divisor, Flags::default());
            return replace_with_cast(c, inst, CastOp::SExt, narrow);
        }

        // -X / C --> X / -C (if the negation doesn't overflow).
        let nsw_neg = {
            let q = query(c, Some(inst));
            pattern::neg(&q, op0).filter(|&(_, _, f)| f.contains(Flags::NSW))
        };
        if bits != 1u128 << (w - 1)
            && let Some((_, x, _)) = nsw_neg
        {
            let neg_c = c.int(ty, neg_bits(bits, w));
            return c.replace_with_binop(inst, BinaryOp::SDiv, x, neg_c, exact_if(exact));
        }
    }

    // -X / Y --> -(X / Y)
    let nsw_neg0 = {
        let q = query(c, Some(inst));
        pattern::neg(&q, op0).filter(|&(_, _, f)| f.contains(Flags::NSW) && has_one_use(c, op0))
    };
    if let Some((_, x, _)) = nsw_neg0 {
        let div = c.create_binop(inst, BinaryOp::SDiv, x, op1, exact_if(exact));
        return replace_with_neg(c, inst, div, true);
    }

    // abs(X) / X --> X > -1 ? 1 : -1
    // X / abs(X) --> X > -1 ? 1 : -1
    let abs_div = {
        let q = query(c, Some(inst));
        commuted(op0, op1, |a, x| {
            let (_, call) = pattern::intrinsic(&q, a, IntrinsicId::Abs).filter(|_| has_one_use(c, a))?;
            let args = pattern::call_arguments(call);
            (args.len() > 1 && args[0] == x && pattern::one(&q, args[1])).then_some(x)
        })
    };
    if let Some(x) = abs_div {
        let ones = c.all_ones(ty);
        let cond = c.create_icmp(inst, IntPredicate::Sgt, x, ones);
        let one = c.int(ty, 1);
        return replace_with_select(c, inst, cond, one, ones);
    }

    let known_dividend = known::compute_known_bits(&query(c, Some(inst)), op0, 0);
    if !exact
        && let Some((bits, _)) = op1_c
        && (bits.is_power_of_two() || is_negated_power2(bits, w))
        && known_dividend.count_min_trailing_zeros() >= bits.trailing_zeros()
    {
        c.function.set_flags(inst, flags_with(flags, Flags::EXACT));
        return Visit::Changed;
    }

    if known_dividend.is_non_negative() {
        // If both operands are unsigned, turn this into a udiv.
        if known::is_known_non_negative(&query(c, Some(inst)), op1, 0) {
            return c.replace_with_binop(inst, BinaryOp::UDiv, op0, op1, exact_if(exact));
        }

        if let Some((bits, _)) = op1_c
            && is_negated_power2(bits, w)
        {
            // X sdiv (-(1 << C)) -> -(X sdiv (1 << C)) -> -(X udiv (1 << C))
            // -> -(X u>> C)
            let log2 = c.int(ty, neg_bits(bits, w).trailing_zeros() as u128);
            let shr = c.create_binop(inst, BinaryOp::LShr, op0, log2, exact_if(exact));
            return replace_with_neg(c, inst, shr, false);
        }

        if is_power_of_two_at(c, inst, op1, true) {
            // X sdiv (1 << Y) -> X udiv (1 << Y) ( -> X u>> Y). Safe because
            // the only negative value (1 << Y) can take on is INT_MIN, and X
            // sdiv INT_MIN == X udiv INT_MIN == 0 if X doesn't have the sign
            // bit set.
            return c.replace_with_binop(inst, BinaryOp::UDiv, op0, op1, exact_if(exact));
        }
    }

    // -X / X --> X == INT_MIN ? 1 : -1
    if is_known_negation(&query(c, Some(inst)), SelectOperand::Value(op0), SelectOperand::Value(op1), false, true) {
        let min = c.int(ty, 1u128 << (w - 1));
        let cond = c.create_icmp(inst, IntPredicate::Eq, op0, min);
        let one = c.int(ty, 1);
        let ones = c.all_ones(ty);
        return replace_with_select(c, inst, cond, one, ones);
    }
    Visit::None
}

/// `flags` with `extra` added.
fn flags_with(
    flags: Flags,
    extra: Flags,
) -> Flags {
    let mut new = flags;
    new.insert(extra);
    new
}

/// [`simplifyIRemMulShl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L2398):
/// `(urem/srem (mul X, Y), (mul X, Z))` and the shift forms.
fn simplify_irem_mul_shl(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let Opcode::Binary(op) = c.function.instruction(inst).opcode else { return None };
    let is_srem = op == BinaryOp::SRem;
    let (op0, op1, _) = parts(c, inst);
    let ty = c.type_of(op0);
    let w = width(c, op0);

    // `m_Mul(V, m_APInt)` or `m_Shl(V, m_APInt)`: V, the number the operand
    // multiplies by, and whether `nsw` survives (not for a shift by BW - 1).
    // V is specific if given.
    let mul_or_shl_xc = |q: &Query, v: Operand, specific: Option<Operand>| {
        let (x, k, flags, is_shl) = binop(q, v, BinaryOp::Mul)
            .map(|(x, k, f)| (x, k, f, false))
            .filter(|&(x, ..)| specific.is_none_or(|s| s == x))
            .filter(|&(_, k, ..)| q.int_constant(k).is_some())
            .or_else(|| {
                binop(q, v, BinaryOp::Shl)
                    .map(|(x, k, f)| (x, k, f, true))
                    .filter(|&(x, ..)| specific.is_none_or(|s| s == x))
                    .filter(|&(_, k, ..)| q.int_constant(k).is_some())
            })?;
        let (k, _) = q.int_constant(k)?;
        Some(if is_shl {
            // C = 1 << Tmp; we cannot preserve NSW when shifting by BW - 1.
            (x, if k >= w as u128 { 0 } else { 1u128 << k }, flags, k < (w - 1) as u128)
        } else {
            (x, k, flags, true)
        })
    };
    // `m_Shl(m_APInt, V)`: the constant, V and the flags.
    let shl_cx = |q: &Query, v: Operand, specific: Option<Operand>| {
        let (k, x, flags) = binop(q, v, BinaryOp::Shl)?;
        let (k, _) = q.int_constant(k)?;
        specific.is_none_or(|s| s == x).then_some((k, x, flags))
    };

    let matched = {
        let q = query(c, Some(inst));
        if let Some((x, y, f0, p0)) = mul_or_shl_xc(&q, op0, None)
            && let Some((_, z, f1, p1)) = mul_or_shl_xc(&q, op1, Some(x))
        {
            Some((x, y, z, f0, p0, f1, p1, false))
        } else if let Some((y, x, f0)) = shl_cx(&q, op0, None)
            && let Some((z, _, f1)) = shl_cx(&q, op1, Some(x))
        {
            Some((x, y, z, f0, true, f1, true, true))
        } else {
            None
        }
    };
    let (x, y, z, f0, p0, f1, p1, shift_by_x) = matched?;
    // The `(x, k, ..)` of the first form carries the constant in `y`/`z`.
    let (bo0_nsw, bo0_nuw) = (p0 && f0.contains(Flags::NSW), f0.contains(Flags::NUW));
    let bo0_no_wrap = if is_srem { bo0_nsw } else { bo0_nuw };

    if z == 0 {
        return None;
    }
    let rem_yz = if is_srem { signed(y, w).wrapping_rem(signed(z, w)) as u128 & mask(w) } else { y % z };
    // (rem (mul nuw/nsw X, Y), (mul X, Z)) if (rem Y, Z) == 0 -> 0
    if rem_yz == 0 && bo0_no_wrap {
        let zero = c.int(ty, 0);
        return Some(Visit::Replace(zero));
    }

    // Either (RemSimplificationC << X) or (RemSimplificationC * X), depending
    // on whether we matched Op0/Op1 as (shl V, X) or (mul V, X) respectively.
    let create_mul_or_shift = |c: &mut Combiner, k: u128, flags: Flags| {
        let k = c.int(ty, k);
        if shift_by_x {
            c.replace_with_binop(inst, BinaryOp::Shl, k, x, flags)
        } else {
            c.replace_with_binop(inst, BinaryOp::Mul, x, k, flags)
        }
    };

    let (bo1_nsw, bo1_nuw) = (p1 && f1.contains(Flags::NSW), f1.contains(Flags::NUW));
    let bo1_no_wrap = if is_srem { bo1_nsw } else { bo1_nuw };
    // (rem (mul X, Y), (mul nuw/nsw X, Z)) if (rem Y, Z) == Y ->
    // (mul nuw/nsw X, Y)
    if rem_yz == y && bo1_no_wrap {
        // Copy any overflow flags from Op0.
        return Some(create_mul_or_shift(c, y, wrap(!is_srem || bo0_nuw, is_srem || bo0_nsw)));
    }

    // (rem (mul nuw/nsw X, Y), (mul {nsw} X, Z)) if Y >= Z ->
    // (mul {nuw} nsw X, (rem Y, Z))
    if y >= z && (if is_srem { bo0_nsw && bo1_nsw } else { bo0_nuw }) {
        return Some(create_mul_or_shift(c, rem_yz, wrap(bo0_nuw, true)));
    }
    None
}

/// [`commonIRemTransforms`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L2507):
/// the transforms common to `urem` and `srem`.
fn common_irem_transforms(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    if let Some(res) = common_idiv_rem_transforms(c, inst) {
        return Some(res);
    }
    let Opcode::Binary(op) = c.function.instruction(inst).opcode else { return None };
    let (op0, op1, _) = parts(c, inst);
    if matches!(op1, Operand::Constant(_))
        && let Some(def) = inst_def(c, op0)
    {
        match c.function.instruction(def).opcode {
            Opcode::Select => {
                let found = c.fold_op_into_select(inst, def, false, false);
                if !matches!(found, Visit::None) {
                    return Some(found);
                }
            }
            Opcode::Phi => {
                // foldOpIntoPhi will speculate instructions to the end of the
                // PHI's predecessor blocks, so do this only if we know the srem
                // or urem will not fault.
                if let Some((bits, w)) = apint(c, op1)
                    && bits != 0
                    && (op == BinaryOp::URem || bits != 1u128 << (w - 1))
                {
                    let folded = c.fold_op_into_phi(inst, def, false);
                    if folded != Visit::None {
                        return Some(folded);
                    }
                }
            }
            _ => {}
        }
        // DEVIATE 7: SimplifyDemandedInstructionBits.
    }
    simplify_irem_mul_shl(c, inst)
}

/// [`InstCombinerImpl::visitURem`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L2543).
pub fn visit_urem(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifyURemInst
    if let Some(found) = simplify_first(c, inst) {
        return Visit::Replace(found);
    }
    // DEVIATE 1: foldVectorBinop.
    if let Some(common) = common_irem_transforms(c, inst) {
        return common;
    }
    if let Some(narrow) = narrow_udiv_urem(c, inst) {
        return narrow;
    }

    // X urem Y -> X and Y-1, where Y is a power of 2,
    let (op0, op1, _) = parts(c, inst);
    let ty = c.type_of(op0);
    if is_power_of_two_at(c, inst, op1, true) {
        // This may increase instruction count, we don't enforce that Y is a
        // constant.
        let ones = c.all_ones(ty);
        let add = c.create_add(inst, op1, ones);
        return c.replace_with_binop(inst, BinaryOp::And, op0, add, Flags::default());
    }

    // 1 urem X -> zext(X != 1)
    if pattern::one(&query(c, Some(inst)), op0) {
        let one = c.int(ty, 1);
        let cmp = c.create_icmp(inst, IntPredicate::Ne, op1, one);
        return replace_with_cast(c, inst, CastOp::ZExt, cmp);
    }

    // Op0 urem C -> Op0 < C ? Op0 : Op0 - C, where C >= signbit. Op0 must be
    // frozen because we are increasing its number of uses.
    if apint(c, op1).is_some_and(|(bits, w)| bits >> (w - 1) == 1) {
        let f0 = freeze_unless_guaranteed(c, inst, op0);
        let cmp = c.create_icmp(inst, IntPredicate::Ult, f0, op1);
        let sub = c.create_sub(inst, f0, op1);
        return replace_with_select(c, inst, cmp, f0, sub);
    }

    // If the divisor is a sext of a boolean, then the divisor must be max
    // unsigned value (-1). Therefore, the remainder is Op0 unless Op0 is also
    // max unsigned value. In that case, the remainder is 0:
    // urem Op0, (sext i1 X) --> (Op0 == -1) ? 0 : Op0
    if bool_extend(c, op1, CastOp::SExt).is_some() {
        let frozen = freeze_unless_guaranteed(c, inst, op0);
        let ones = c.all_ones(ty);
        let cmp = c.create_icmp(inst, IntPredicate::Eq, frozen, ones);
        let zero = c.int(ty, 0);
        return replace_with_select(c, inst, cmp, zero, frozen);
    }

    // For "(X + 1) % Op1" and if (X u< Op1) => (X + 1) == Op1 ? 0 : X + 1 .
    let add_one = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Add).filter(|&(_, one, _)| pattern::one(&q, one))
    };
    if let Some((x, ..)) = add_one {
        // DEVIATE 8: simplifyICmpInst is asked without the context
        // instruction, so it can only find less.
        let val =
            llrm_mir::simplify::simplify_icmp_operands(c.context, c.env.layout, c.function, IntPredicate::Ult, x, op1);
        if val.is_some_and(|v| pattern::one(&query(c, Some(inst)), v)) {
            let frozen = freeze_unless_guaranteed(c, inst, op0);
            let cmp = c.create_icmp(inst, IntPredicate::Eq, frozen, op1);
            let zero = c.int(ty, 0);
            return replace_with_select(c, inst, cmp, zero, frozen);
        }
    }
    Visit::None
}

/// [`InstCombinerImpl::visitSRem`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L2617).
pub fn visit_srem(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    // simplifySRemInst
    if let Some(found) = simplify_first(c, inst) {
        return Visit::Replace(found);
    }
    // DEVIATE 1: foldVectorBinop.
    // Handle the integer rem common cases
    if let Some(common) = common_irem_transforms(c, inst) {
        return common;
    }
    let (op0, op1, _) = parts(c, inst);
    let ty = c.type_of(op0);
    let w = width(c, op0);

    // X % -Y -> X % Y
    if let Some((bits, _)) = apint(c, op1)
        && bits >> (w - 1) == 1
        && bits != 1u128 << (w - 1)
    {
        let pos = c.int(ty, neg_bits(bits, w));
        replace_operand(c, inst, 1, pos);
        return Visit::Changed;
    }

    // -X srem Y --> -(X srem Y)
    let nsw_neg = {
        let q = query(c, Some(inst));
        pattern::neg(&q, op0).filter(|&(_, _, f)| f.contains(Flags::NSW) && has_one_use(c, op0))
    };
    if let Some((_, x, _)) = nsw_neg {
        let rem = c.create_binop(inst, BinaryOp::SRem, x, op1, Flags::default());
        return replace_with_neg(c, inst, rem, true);
    }

    // If the sign bits of both operands are zero (i.e. we can prove they are
    // unsigned inputs), turn this into a urem.
    let sign = 1u128 << (w - 1);
    {
        let q = query(c, Some(inst));
        if !(known::masked_value_is_zero(&q, op1, sign, 0) && known::masked_value_is_zero(&q, op0, sign, 0)) {
            // DEVIATE 1: a constant vector divisor's negative elements.
            return Visit::None;
        }
    }
    // X srem Y -> X urem Y, iff X and Y don't have sign bit set
    c.replace_with_binop(inst, BinaryOp::URem, op0, op1, Flags::default())
}
