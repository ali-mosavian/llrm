//! InstCombine's `visitShl`, `visitLShr` and `visitAShr` and the helpers they
//! call, from
//! [`InstCombineShifts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp)
//! at d1106deb71cc.
//!
//! [`visit_shl`], [`visit_lshr`] and [`visit_ashr`] are the C++ functions: the
//! same steps in the same order, each helper a function named for the C++ one
//! with its own permalink. A step that returns `nullptr` there returns `None`
//! here, one that returns an instruction returns `Some(Visit)` (a new
//! instruction replaces the visited one; `&I` is `Visit::Changed`).
//!
//! MIRROR: the order of the steps and each condition.
//!
//! DEVIATE 1: `foldVectorBinop` is not translated: MIR has no vector
//! operations.
//!
//! DEVIATE 4: constants are `u128` bits with a width, so a mask built in twice
//! the width (`dropRedundantMaskingOfLeftShiftInput`) is not built for a type
//! wider than 64 bits, and a sum of shift amounts at or past twice the width
//! (which LLVM folds to a poison mask) is left alone; an `undef` or `poison`
//! constant amount is no constant sum, where LLVM replaces the undefs.
//!
//! DEVIATE 5: the folds that match `scmp` and `ucmp` are translated, but MIR
//! has neither intrinsic yet, so no test reaches them.
//!
//! # Used by
//! `combine::visit`, once the shift family's slot is flipped.
//!
//! # How it works
//! `visit_*` first replaces the shift by a value it simplifies to, then tries
//! the transforms the three shifts share (`commonShiftTransforms`: a constant
//! amount pulled through the operand, two shifts the same way joined, an
//! amount reduced), then the transforms of its own direction. A rewrite that
//! needs new operands builds them with the `Combiner`'s builder, which folds
//! through `simplify*` first.
//!
//! # Example
//! `(x >> 3) << 3` clears the low bits:
//!
//! ```text
//! %a = lshr i32 %x, 3
//! %r = shl i32 %a, 3    ; %r = and i32 %x, -8
//! ```
//!
//! # Runs
//! Not a pass: called by the combine driver once its slot is flipped.

use llrm_mir::context::{mask, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::{simplify_binop_at, simplify_instruction_in};
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::signbits::compute_num_sign_bits;
use llrm_mir::valuetracking::{Query, known, pattern, power2};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{has_one_use, query, replace_operand, should_change_type};

/// `ShiftSemantics`: what a shifted-back value must still equal.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Semantics {
    Lossy,
    Unsigned,
    Signed,
}

/// The low `n` bits of a `w`-bit value set (`APInt::getLowBitsSet`).
fn low_bits(
    w: u32,
    n: u32,
) -> u128 {
    if n == 0 {
        0
    } else if n >= w {
        mask(w)
    } else {
        (1u128 << n) - 1
    }
}

/// The high `n` bits of a `w`-bit value set (`APInt::getHighBitsSet`).
fn high_bits(
    w: u32,
    n: u32,
) -> u128 {
    mask(w) & !low_bits(w, w - n.min(w))
}

/// `APInt::shl` of a `w`-bit `x`; a shift past the width is 0.
fn shl_w(
    x: u128,
    n: u32,
    w: u32,
) -> u128 {
    if n >= w { 0 } else { (x << n) & mask(w) }
}

/// `APInt::lshr` of a `w`-bit `x`; a shift past the width is 0.
fn lshr_w(
    x: u128,
    n: u32,
    w: u32,
) -> u128 {
    if n >= w { 0 } else { x >> n }
}

/// `APInt::ashr` of a `w`-bit `x`.
fn ashr_w(
    x: u128,
    n: u32,
    w: u32,
) -> u128 {
    (signed(x, w) >> n.min(w - 1)) as u128 & mask(w)
}

/// `APInt::countr_zero`.
fn countr_zero(
    x: u128,
    w: u32,
) -> u32 {
    if x == 0 { w } else { x.trailing_zeros() }
}

/// `APInt::countl_zero`.
fn countl_zero(
    x: u128,
    w: u32,
) -> u32 {
    if x == 0 { w } else { x.leading_zeros() - (128 - w) }
}

/// `APInt::countl_one`.
fn countl_one(
    x: u128,
    w: u32,
) -> u32 {
    countl_zero(!x & mask(w), w)
}

/// The value the instruction `inst` made.
fn result_of(
    c: &Combiner,
    inst: InstId,
) -> Operand {
    Operand::Value(c.function.instruction(inst).result.expect("a value"))
}

/// The instruction that defines `v`, when `v` is one.
fn defining(
    c: &Combiner,
    v: Operand,
) -> Option<InstId> {
    query(c, None).instruction_of(v).map(|(inst, _)| inst)
}

/// The visited shift: its opcode, operands and flags.
fn shift_parts(
    c: &Combiner,
    inst: InstId,
) -> (BinaryOp, Operand, Operand, Flags) {
    let instruction = c.function.instruction(inst);
    let Opcode::Binary(op) = instruction.opcode else { unreachable!("a shift") };
    (op, instruction.operands[0], instruction.operands[1], instruction.flags)
}

/// `m_Shift(m_Value(a), m_Value(b))`: the instruction, its opcode, operands
/// and flags.
fn shift_of(
    q: &Query,
    v: Operand,
) -> Option<(InstId, BinaryOp, Operand, Operand, Flags)> {
    let (inst, instruction) = q.instruction_of(v)?;
    let Opcode::Binary(op @ (BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)) = instruction.opcode else {
        return None;
    };
    Some((inst, op, instruction.operands[0], instruction.operands[1], instruction.flags))
}

fn is_shr(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::LShr | BinaryOp::AShr)
}

/// `m_Shr(m_Value(a), m_Value(b))`.
fn shr_of(
    q: &Query,
    v: Operand,
) -> Option<(InstId, BinaryOp, Operand, Operand, Flags)> {
    shift_of(q, v).filter(|(_, op, ..)| is_shr(*op))
}

/// `m_BinOp(op, a, b)`: the instruction, its operands and flags.
fn binop(
    q: &Query,
    v: Operand,
    op: BinaryOp,
) -> Option<(InstId, Operand, Operand, Flags)> {
    pattern::binary(q, v, op).map(|(inst, instruction, a, b)| (inst, a, b, instruction.flags))
}

/// `m_ZExtOrSelf(m_Value(x))`.
fn zext_or_self(
    q: &Query,
    v: Operand,
) -> Operand {
    pattern::cast(q, v, CastOp::ZExt).map_or(v, |(_, _, x)| x)
}

/// `m_APInt`: the value of an integer constant.
fn imm(
    q: &Query,
    v: Operand,
) -> Option<u128> {
    q.int_constant(v).map(|(bits, _)| bits)
}

/// `m_SpecificInt(n)`: the constant is `n` in its width.
fn is_specific_int(
    q: &Query,
    v: Operand,
    n: u128,
) -> bool {
    q.int_constant(v).is_some_and(|(bits, w)| bits == n & mask(w))
}

/// The width of the integer `v`.
fn width(
    q: &Query,
    v: Operand,
) -> u32 {
    q.scalar_bits(v)
}

/// The flags that are set, from `(condition, flag)` pairs.
fn flags_of(pairs: &[(bool, Flags)]) -> Flags {
    let mut flags = Flags::default();
    for &(set, flag) in pairs {
        if set {
            flags.insert(flag);
        }
    }
    flags
}

/// `flags` with `flag` set.
fn with_flag(
    mut flags: Flags,
    flag: Flags,
) -> Flags {
    flags.insert(flag);
    flags
}

/// `simplifyShlInst`, `simplifyLShrInst` or `simplifyAShrInst` of the visited
/// shift: what it equals, replaced.
fn simplified(
    c: &Combiner,
    inst: InstId,
) -> Option<Visit> {
    let env = c.env;
    let found = simplify_instruction_in(c.context, env.globals, env.layout, Some(&env.analyses), c.function, inst)?;
    (Some(found) != c.function.instruction(inst).result.map(Operand::Value)).then_some(Visit::Replace(found))
}

/// A new cast of `v` to `ty` replaces `at`: `new ZExtInst(v, Ty)`.
fn replace_with_cast(
    c: &mut Combiner,
    at: InstId,
    op: CastOp,
    v: Operand,
    ty: TypeId,
) -> Visit {
    c.replace_with_new(at, Opcode::Cast(op), ty, vec![v], Flags::default())
}

/// `canTryToConstantAddTwoShiftAmounts`
/// ([:25](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L25)):
/// whether the sum of two shift amounts, which may be looked at past a `zext`,
/// still fits the narrower amount's type.
fn can_try_to_constant_add_two_shift_amounts(
    q: &Query,
    sh0: Operand,
    amt0: Operand,
    sh1: Operand,
    amt1: Operand,
) -> bool {
    if q.type_of(amt0) != q.type_of(amt1) {
        return false;
    }
    let maximal_total = u128::from(width(q, sh0) - 1) + u128::from(width(q, sh1) - 1);
    mask(width(q, amt0)) >= maximal_total
}

/// `reassociateShiftAmtsOfTwoSameDirectionShifts`
/// ([:58](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L58)):
/// `(x shift Q) shift K` as `x shift (Q+K)` when the sum is a constant below
/// the width. With `analyze_for_sign_bit_extraction`, the base value of two
/// right shifts that sum to the width less one. The value is a new
/// instruction, built before `sh0`.
pub fn reassociate_shift_amts_of_two_same_direction_shifts(
    c: &mut Combiner,
    sh0: InstId,
    analyze_for_sign_bit_extraction: bool,
) -> Option<Operand> {
    struct Plan {
        op: BinaryOp,
        x: Operand,
        amt: u128,
        amt_ty: TypeId,
        flags: Flags,
        trunc_to: Option<TypeId>,
    }
    let plan = {
        let q = query(c, Some(sh0));
        let sh0_value = result_of(c, sh0);
        let (_, op0, sh0_op0, raw_amt0, flags0) = shift_of(&q, sh0_value)?;
        q.instruction_of(sh0_op0)?;
        let amt0 = zext_or_self(&q, raw_amt0);
        // A truncation between the two shifts is looked through.
        let (sh1, trunc) = match pattern::cast(&q, sh0_op0, CastOp::Trunc) {
            Some((_, _, inner)) if q.instruction_of(inner).is_some() => (inner, Some(sh0_op0)),
            _ => (sh0_op0, None),
        };
        let (_, op1, x, raw_amt1, flags1) = shift_of(&q, sh1)?;
        let amt1 = zext_or_self(&q, raw_amt1);
        if !can_try_to_constant_add_two_shift_amounts(&q, sh0_value, amt0, sh1, amt1) {
            return None;
        }
        let had_two_right_shifts = is_shr(op0) && is_shr(op1);
        if analyze_for_sign_bit_extraction && !had_two_right_shifts {
            return None;
        }
        let identical = op0 == op1;
        if !identical && !analyze_for_sign_bit_extraction {
            return None;
        }
        // The truncation needs an extra instruction: one operand must be
        // one-use.
        if trunc.is_some()
            && !analyze_for_sign_bit_extraction
            && !(has_one_use(c, c.function.instruction(sh0).operands[0])
                || has_one_use(c, c.function.instruction(sh0).operands[1]))
        {
            return None;
        }
        let env = c.env;
        let sum = simplify_binop_at(
            c.context,
            env.globals,
            env.layout,
            Some(&env.analyses),
            c.function,
            sh0,
            BinaryOp::Add,
            amt0,
            amt1,
        )?;
        let new_amt = imm(&q, sum)?;
        let new_amt_width = width(&q, sum);
        let x_width = width(&q, x);
        if new_amt >= u128::from(x_width) {
            return None;
        }
        if had_two_right_shifts && (trunc.is_some() || analyze_for_sign_bit_extraction) {
            if new_amt != u128::from(x_width - 1) {
                return None;
            }
            if analyze_for_sign_bit_extraction {
                return Some(x);
            }
        }
        let x_ty = q.type_of(x)?;
        if q.type_of(sum) != Some(x_ty) && new_amt_width > x_width {
            return None;
        }
        let no_trunc = trunc.is_none();
        let flags = if !no_trunc {
            Flags::default()
        } else if op0 == BinaryOp::Shl {
            flags_of(&[
                (flags0.contains(Flags::NUW) && flags1.contains(Flags::NUW), Flags::NUW),
                (flags0.contains(Flags::NSW) && flags1.contains(Flags::NSW), Flags::NSW),
            ])
        } else {
            flags_of(&[(flags0.contains(Flags::EXACT) && flags1.contains(Flags::EXACT), Flags::EXACT)])
        };
        Plan { op: op0, x, amt: new_amt, amt_ty: x_ty, flags, trunc_to: trunc.and_then(|_| q.type_of(sh0_value)) }
    };
    let amount = c.int(plan.amt_ty, plan.amt);
    let ty = c.type_of(plan.x);
    let shifted = c.insert_before(sh0, Opcode::Binary(plan.op), ty, vec![plan.x, amount], plan.flags);
    let shifted = result_of(c, shifted);
    Some(match plan.trunc_to {
        Some(to) => {
            let new = c.insert_before(sh0, Opcode::Cast(CastOp::Trunc), to, vec![shifted], Flags::default());
            result_of(c, new)
        }
        None => shifted,
    })
}

/// `m_c_And(P, m_Value(X))`: `v` is an `and` where `a` or `b` is a mask `P`
/// shape yields the shift amount of; that amount and the other operand.
fn c_and_with_mask(
    q: &Query,
    v: Operand,
    shape: impl Fn(Operand) -> Option<Operand>,
) -> Option<(Operand, Operand)> {
    let (_, a, b, _) = binop(q, v, BinaryOp::And)?;
    shape(a).map(|amount| (amount, b)).or_else(|| shape(b).map(|amount| (amount, a)))
}

/// `dropRedundantMaskingOfLeftShiftInput`
/// ([:188](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L188)):
/// a mask under a left shift that the shift's own amount clears the same bits
/// of (`(x & ((1 << m) - 1)) << s` with `m + s u>= width`) is dropped.
fn drop_redundant_masking_of_left_shift_input(
    c: &mut Combiner,
    outer: InstId,
) -> Option<Visit> {
    struct Plan {
        x: Operand,
        narrow_ty: TypeId,
        had_trunc: bool,
        new_mask: Option<u128>,
        amount: Operand,
    }
    let plan = {
        let q = query(c, Some(outer));
        let outer_value = result_of(c, outer);
        let (_, _, masked0, raw_amt, _) = shift_of(&q, outer_value)?;
        let shift_sh_amt = zext_or_self(&q, raw_amt);
        // A truncation under the shift must be one-use.
        let mut masked = masked0;
        if let Some((_, _, inner)) = pattern::cast(&q, masked0, CastOp::Trunc) {
            if !has_one_use(c, masked0) {
                return None;
            }
            masked = inner;
        }
        let narrow_ty = q.type_of(outer_value)?;
        let widest_ty = q.type_of(masked)?;
        let had_trunc = widest_ty != narrow_ty;
        let narrow_w = width(&q, outer_value);
        let wide_w = width(&q, masked);
        // DEVIATE 4: no mask in twice a width past 64 bits.
        if wide_w * 2 > 128 {
            return None;
        }
        let ext_w = wide_w * 2;
        let all_ones = |v: Operand| pattern::all_ones(&q, v);
        // ((1 << m) - 1)
        let mask_a = |v: Operand| {
            let (_, shl, rhs, _) = binop(&q, v, BinaryOp::Add)?;
            let (_, one, amount, _) = binop(&q, shl, BinaryOp::Shl)?;
            (pattern::one(&q, one) && all_ones(rhs)).then_some(amount)
        };
        // (~(-1 << m))
        let mask_b = |v: Operand| {
            let (_, ones, amount, _) = binop(&q, pattern::not(&q, v)?, BinaryOp::Shl)?;
            all_ones(ones).then_some(amount)
        };
        // (-1 l>> m)
        let mask_c = |v: Operand| {
            let (_, ones, amount, _) = binop(&q, v, BinaryOp::LShr)?;
            all_ones(ones).then_some(amount)
        };
        // ((-1 << m) l>> m)
        let mask_d = |v: Operand| {
            let (_, shl, amount, _) = binop(&q, v, BinaryOp::LShr)?;
            let (_, ones, again, _) = binop(&q, shl, BinaryOp::Shl)?;
            (all_ones(ones) && again == amount).then_some(amount)
        };
        let new_mask;
        let x;
        if let Some((mask_sh_amt, found)) = c_and_with_mask(&q, masked, |v| mask_a(v).or_else(|| mask_b(v))) {
            x = found;
            let mask_sh_amt = zext_or_self(&q, mask_sh_amt);
            if !can_try_to_constant_add_two_shift_amounts(&q, outer_value, shift_sh_amt, masked, mask_sh_amt) {
                return None;
            }
            let env = c.env;
            let sum = simplify_binop_at(
                c.context,
                env.globals,
                env.layout,
                Some(&env.analyses),
                c.function,
                outer,
                BinaryOp::Add,
                mask_sh_amt,
                shift_sh_amt,
            )?;
            let sum = imm(&q, sum)?;
            // `~(-1 << sum)` in twice the width; a sum past it is a poison
            // mask.
            if sum >= u128::from(ext_w) {
                return None;
            }
            let inverted = shl_w(mask(ext_w), sum as u32, ext_w);
            new_mask = !inverted & mask(ext_w);
        } else if let Some((mask_sh_amt, found)) = c_and_with_mask(&q, masked, |v| mask_c(v).or_else(|| mask_d(v)))
            .or_else(|| {
                let (_, _, shl, amount, _) = shr_of(&q, masked)?;
                let (_, x, shl_amount, _) = binop(&q, shl, BinaryOp::Shl)?;
                (shl_amount == amount).then_some((amount, x))
            })
        {
            x = found;
            let mask_sh_amt = zext_or_self(&q, mask_sh_amt);
            if !can_try_to_constant_add_two_shift_amounts(&q, outer_value, shift_sh_amt, masked, mask_sh_amt) {
                return None;
            }
            let env = c.env;
            let diff = simplify_binop_at(
                c.context,
                env.globals,
                env.layout,
                Some(&env.analyses),
                c.function,
                outer,
                BinaryOp::Sub,
                shift_sh_amt,
                mask_sh_amt,
            )?;
            let diff = imm(&q, diff)?;
            let amount_w = width(&q, shift_sh_amt);
            let to_clear = u128::from(wide_w).wrapping_sub(diff) & mask(amount_w);
            if to_clear >= u128::from(ext_w) {
                return None;
            }
            new_mask = lshr_w(mask(ext_w), to_clear as u32, ext_w);
        } else {
            return None;
        }
        let new_mask = new_mask & mask(narrow_w);
        let need_mask = new_mask != mask(narrow_w);
        if need_mask && (!has_one_use(c, masked) || binop(&q, masked, BinaryOp::AShr).is_some()) {
            return None;
        }
        Plan {
            x,
            narrow_ty,
            had_trunc,
            new_mask: need_mask.then_some(new_mask),
            amount: c.function.instruction(outer).operands[1],
        }
    };
    let x = if plan.had_trunc {
        c.create_cast(outer, CastOp::Trunc, plan.x, plan.narrow_ty, Flags::default())
    } else {
        plan.x
    };
    let Some(new_mask) = plan.new_mask else {
        return Some(c.replace_with_binop(outer, BinaryOp::Shl, x, plan.amount, Flags::default()));
    };
    let ty = c.type_of(x);
    let shifted = c.insert_before(outer, Opcode::Binary(BinaryOp::Shl), ty, vec![x, plan.amount], Flags::default());
    let shifted = result_of(c, shifted);
    let mask = c.int(plan.narrow_ty, new_mask);
    Some(c.replace_with_binop(outer, BinaryOp::And, shifted, mask, Flags::default()))
}

/// `foldShiftOfShiftedBinOp`
/// ([:353](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L353)):
/// `shift (binop (shift X, C0), Y), C1` as `binop (shift X, C0+C1), (shift Y,
/// C1)`.
fn fold_shift_of_shifted_bin_op(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (shift_op, op0, op1, _) = shift_parts(c, inst);
    let plan = {
        let q = query(c, Some(inst));
        let (_, bin) = q.instruction_of(op0)?;
        let Opcode::Binary(bin_op) = bin.opcode else { return None };
        let bitwise = matches!(bin_op, BinaryOp::And | BinaryOp::Or | BinaryOp::Xor);
        if !(bitwise || matches!(bin_op, BinaryOp::Add | BinaryOp::Sub)) || !has_one_use(c, op0) {
            return None;
        }
        let c1 = imm(&q, op1)?;
        // The add and sub transform works with shl only.
        if matches!(bin_op, BinaryOp::Add | BinaryOp::Sub) && shift_op != BinaryOp::Shl {
            return None;
        }
        let w = width(&q, op0);
        let (lhs, rhs) = (bin.operands[0], bin.operands[1]);
        // A shift by a constant whose sum with `c1` is below the width.
        let first_shift = |v: Operand, other: Operand| {
            let (_, x, c0, _) = binop(&q, v, shift_op)?;
            let c0 = imm(&q, c0)?;
            (has_one_use(c, v) || pattern::imm_constant(&q, other)).then_some(())?;
            ((c0 + c1) & mask(w) < u128::from(w)).then_some((x, c0))
        };
        // Sub is not commutative: a first shift in operand 1 keeps its place.
        let (x, c0, y, first_is_op1) = if let Some((x, c0)) = first_shift(lhs, rhs) {
            (x, c0, rhs, false)
        } else if let Some((x, c0)) = first_shift(rhs, lhs) {
            (x, c0, lhs, bin_op == BinaryOp::Sub)
        } else {
            return None;
        };
        (bin_op, x, c0, y, first_is_op1, c1, w)
    };
    let (bin_op, x, c0, y, first_is_op1, c1, w) = plan;
    let ty = c.type_of(op0);
    let sum = c.int(ty, (c0 + c1) & mask(w));
    let new_shift1 = c.create_binop(inst, shift_op, x, sum, Flags::default());
    let new_shift2 = c.create_binop(inst, shift_op, y, op1, Flags::default());
    let (a, b) = if first_is_op1 { (new_shift2, new_shift1) } else { (new_shift1, new_shift2) };
    Some(c.replace_with_binop(inst, bin_op, a, b, Flags::default()))
}

/// `InstCombinerImpl::commonShiftTransforms`
/// ([:410](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L410)).
fn common_shift_transforms(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let found = c.fold_binop_with_phi_operands(inst);
    if !matches!(found, Visit::None) {
        return Some(found);
    }
    let (op, op0, op1, flags) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let w = c.context.types.int_bits(ty).expect("an integer");

    // If the shift amount is a one-use `sext`, demote it to `zext`.
    let sext = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op1, CastOp::SExt).filter(|_| has_one_use(c, op1)).map(|(_, _, y)| y)
    };
    if let Some(y) = sext {
        let new_ext = c.create_cast(inst, CastOp::ZExt, y, ty, Flags::default());
        return Some(c.replace_with_binop(inst, op, op0, new_ext, Flags::default()));
    }

    // See if we can fold away this shift.
    let found = c.simplify_demanded_instruction_bits(inst);
    if !matches!(found, Visit::None) {
        return Some(found);
    }
    // Try to fold a constant and a select into the select's arguments.
    if matches!(op0, Operand::Constant(_))
        && let Some(select) = c.definition_of(op1).filter(|&def| c.function.instruction(def).opcode == Opcode::Select)
    {
        let found = c.fold_op_into_select(inst, select, false, false);
        if !matches!(found, Visit::None) {
            return Some(found);
        }
    }

    if pattern::imm_constant(&query(c, Some(inst)), op1)
        && let Some(res) = fold_shift_by_constant(c, op0, op1, inst)
    {
        return Some(res);
    }

    if let Some(new_shift) = reassociate_shift_amts_of_two_same_direction_shifts(c, inst, false) {
        return Some(Visit::Replace(new_shift));
    }

    // Pre-shift a constant shifted by a variable amount with a constant
    // offset: C shift (A add nuw C1) --> (C shift C1) shift A
    let pre_shift = {
        let q = query(c, Some(inst));
        match (op0, pattern::add_like(&q, op1, Flags::NUW)) {
            (Operand::Constant(_), Some((a, c1))) if matches!(c1, Operand::Constant(_)) => Some((a, c1)),
            _ => None,
        }
    };
    if let Some((a, c1)) = pre_shift {
        let new_c = c.create_binop(inst, op, op0, c1, Flags::default());
        let new_flags = if op == BinaryOp::Shl {
            flags_of(&[(flags.contains(Flags::NSW), Flags::NSW), (flags.contains(Flags::NUW), Flags::NUW)])
        } else {
            flags_of(&[(flags.contains(Flags::EXACT), Flags::EXACT)])
        };
        return Some(c.replace_with_binop(inst, op, new_c, a, new_flags));
    }

    let ac = imm(&query(c, Some(inst)), op0);
    if let Some(ac) = ac {
        // C << (X - AddC) --> (C >> AddC) << X
        // C >> (X - AddC) --> (C << AddC) >> X
        let pre_shift = {
            let q = query(c, Some(inst));
            binop(&q, op1, BinaryOp::Add).and_then(|(_, a, addc, _)| {
                let addc = imm(&q, addc)?;
                let negated = addc.wrapping_neg() & mask(w);
                (addc >> (w - 1) & 1 == 1 && negated < u128::from(w)).then_some((a, negated as u32))
            })
        };
        if let Some((a, pos_offset)) = pre_shift {
            let suitable = match op {
                BinaryOp::Shl => {
                    (flags.contains(Flags::NSW) || flags.contains(Flags::NUW))
                        && ac == shl_w(lshr_w(ac, pos_offset, w), pos_offset, w)
                }
                BinaryOp::LShr => flags.contains(Flags::EXACT) && ac == lshr_w(shl_w(ac, pos_offset, w), pos_offset, w),
                _ => flags.contains(Flags::EXACT) && ac == ashr_w(shl_w(ac, pos_offset, w), pos_offset, w),
            };
            if suitable {
                let bits = if op == BinaryOp::Shl { lshr_w(ac, pos_offset, w) } else { shl_w(ac, pos_offset, w) };
                let new_c = c.int(ty, bits);
                let new_flags = if op == BinaryOp::Shl {
                    flags_of(&[(flags.contains(Flags::NUW), Flags::NUW)])
                } else {
                    Flags::EXACT
                };
                return Some(c.replace_with_binop(inst, op, new_c, a, new_flags));
            }
        }

        // C1 << (C2 - X) -> (C1 << C2) >> X
        // C1 >> (C2 - X) -> (C1 >> C2) << X
        // X must be u<= C2 (checked by NUWSub). Also (X ^ C2) when it is C2 -
        // X.
        let c2_x = {
            let q = query(c, Some(inst));
            let by_sub = binop(&q, op1, BinaryOp::Sub).and_then(|(_, lhs, x, f)| {
                let c2 = u64::try_from(imm(&q, lhs)?).ok()?;
                f.contains(Flags::NUW).then_some((c2, x))
            });
            by_sub.or_else(|| {
                let (_, x, c2, _) = binop(&q, op1, BinaryOp::Xor)?;
                let c2 = u64::try_from(imm(&q, c2)?).ok()?;
                let zero = known::compute_known_bits(&q, x, 0).zero;
                ((u128::from(c2) | zero) & mask(w) == mask(w)).then_some((c2, x))
            })
        };
        if let Some((c2, x)) = c2_x {
            let c2 = c2.min(u64::from(w)) as u32;
            if op == BinaryOp::Shl {
                if countl_zero(ac, w) >= c2 {
                    let new_c = c.int(ty, shl_w(ac, c2, w));
                    return Some(c.replace_with_binop(inst, BinaryOp::LShr, new_c, x, Flags::EXACT));
                }
                if countl_one(ac, w) > c2 {
                    let new_c = c.int(ty, shl_w(ac, c2, w));
                    return Some(c.replace_with_binop(inst, BinaryOp::AShr, new_c, x, Flags::EXACT));
                }
            } else if countr_zero(ac, w) >= c2 {
                if ac >> (w - 1) & 1 == 0 {
                    let new_c = c.int(ty, lshr_w(ac, c2, w));
                    let new_flags = flags_of(&[(true, Flags::NUW), (true, Flags::NSW)]);
                    return Some(c.replace_with_binop(inst, BinaryOp::Shl, new_c, x, new_flags));
                }
                if op == BinaryOp::LShr {
                    let new_c = c.int(ty, lshr_w(ac, c2, w));
                    return Some(c.replace_with_binop(inst, BinaryOp::Shl, new_c, x, Flags::NUW));
                }
                let new_c = c.int(ty, ashr_w(ac, c2, w));
                return Some(c.replace_with_binop(inst, BinaryOp::Shl, new_c, x, Flags::NSW));
            }
        }
    }

    // X shift (A srem C) -> X shift (A and (C - 1)) iff C is a power of 2.
    // Shifts by negative values (which could occur if A were negative) are
    // undefined.
    let srem = {
        let q = query(c, Some(inst));
        binop(&q, op1, BinaryOp::SRem)
            .filter(|_| has_one_use(c, op1))
            .and_then(|(_, a, power, _)| Some((a, pattern::power2_constant(&q, power)?)))
    };
    if let Some((a, power)) = srem {
        let mask_c = c.int(ty, power - 1);
        let rem = c.create_binop(inst, BinaryOp::And, a, mask_c, Flags::default());
        replace_operand(c, inst, 1, rem);
        return Some(Visit::Changed);
    }

    if let Some(logic) = fold_shift_of_shifted_bin_op(c, inst) {
        return Some(logic);
    }

    let or_all_ones = {
        let q = query(c, Some(inst));
        binop(&q, op1, BinaryOp::Or).is_some_and(|(_, _, rhs, _)| is_specific_int(&q, rhs, u128::from(w - 1)))
    };
    if or_all_ones {
        let amount = c.int(ty, u128::from(w - 1));
        replace_operand(c, inst, 1, amount);
        return Some(Visit::Changed);
    }

    // (lshr|ashr (scmp|ucmp X, Y), N-1) --> (zext|sext (icmp slt|ult X, Y))
    let cmp_intrinsic = {
        let q = query(c, Some(inst));
        (is_shr(op) && has_one_use(c, op0) && is_specific_int(&q, op1, u128::from(w - 1)))
            .then(|| {
                [("llvm.scmp", IntPredicate::Slt), ("llvm.ucmp", IntPredicate::Ult)]
                    .into_iter()
                    .find_map(
                        |(name, pred)| pattern::intrinsic_call(&q, op0, name).map(|(_, args)| (pred, args[0], args[1])),
                    )
            })
            .flatten()
    };
    if let Some((pred, a, b)) = cmp_intrinsic {
        let cmp = c.create_icmp(inst, pred, a, b);
        let ext = if op == BinaryOp::LShr { CastOp::ZExt } else { CastOp::SExt };
        return Some(replace_with_cast(c, inst, ext, cmp, ty));
    }

    None
}

/// `canEvaluateShiftedShift`
/// ([:575](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L575)):
/// whether `outer (inner x, C1), C2` with logical shifts and constant amounts
/// simplifies.
fn can_evaluate_shifted_shift(
    c: &Combiner,
    outer_amount: u32,
    is_outer_shl: bool,
    semantics: Semantics,
    inner: InstId,
    cxt: InstId,
) -> bool {
    let q = query(c, Some(cxt));
    let instruction = c.function.instruction(inner);
    let Opcode::Binary(op) = instruction.opcode else { return false };
    // Constant scalar shifts only.
    let Some(inner_amount) = imm(&q, instruction.operands[1]) else { return false };
    let is_inner_shl = op == BinaryOp::Shl;
    if !is_outer_shl && semantics == Semantics::Signed {
        return is_inner_shl && instruction.flags.contains(Flags::NSW) && inner_amount == u128::from(outer_amount);
    }
    // Two logical shifts in the same direction.
    if is_inner_shl == is_outer_shl {
        return semantics == Semantics::Lossy;
    }
    // Equal amounts in opposite directions become an `and`.
    if inner_amount == u128::from(outer_amount) {
        return true;
    }
    // A bigger first shift folds only where the masked-off bits are zero.
    let w = width(&q, Operand::Value(instruction.result.expect("a shift")));
    if inner_amount > u128::from(outer_amount) && inner_amount < u128::from(w) {
        let inner_amount = inner_amount as u32;
        let mask_shift = if is_inner_shl { w - inner_amount } else { inner_amount - outer_amount };
        let bits = shl_w(low_bits(w, outer_amount), mask_shift, w);
        if known::masked_value_is_zero(&q, instruction.operands[0], bits, 0) {
            return true;
        }
    }
    false
}

/// `InstCombinerImpl::canEvaluateShifted`
/// ([:633](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L633)):
/// whether `v` can be computed shifted by `num_bits`, the shift then
/// disappearing.
pub fn can_evaluate_shifted(
    c: &Combiner,
    v: Operand,
    num_bits: u32,
    is_left_shift: bool,
    semantics: Semantics,
    cxt: InstId,
) -> bool {
    let q = query(c, Some(cxt));
    // Immediates shift left always; right only where no bit is lost.
    if pattern::imm_constant(&q, v) {
        if semantics == Semantics::Lossy {
            return true;
        }
        return match q.int_constant(v) {
            Some((bits, w)) if !is_left_shift => countr_zero(bits, w) >= num_bits,
            _ => false,
        };
    }
    let Some((inst, instruction)) = q.instruction_of(v) else { return false };
    // Mutating something with several uses would duplicate it.
    if !has_one_use(c, v) {
        return false;
    }
    let operands = instruction.operands.clone();
    let evaluable = |operand: Operand| can_evaluate_shifted(c, operand, num_bits, is_left_shift, semantics, inst);
    match instruction.opcode {
        // Bitwise operators can be evaluated shifted, whatever the shift.
        Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) => {
            evaluable(operands[0]) && evaluable(operands[1])
        }
        Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr) => {
            can_evaluate_shifted_shift(c, num_bits, is_left_shift, semantics, inst, cxt)
        }
        Opcode::Select => evaluable(operands[1]) && evaluable(operands[2]),
        // A phi whose every input can change; only one-use instructions are
        // looked at, so no cycle is.
        Opcode::Phi => pattern::phi_incoming(instruction).all(|(incoming, _)| evaluable(incoming)),
        // (shr (mul X, -(1 << C)), C) -> (and (neg X), C')
        Opcode::Binary(BinaryOp::Mul) => {
            !is_left_shift
                && semantics == Semantics::Unsigned
                && q.int_constant(operands[1]).is_some_and(|(bits, w)| {
                    let negated = bits.wrapping_neg() & mask(w);
                    bits != 0 && negated.is_power_of_two() && countr_zero(bits, w) == num_bits
                })
        }
        Opcode::Binary(BinaryOp::Add) => {
            if is_left_shift {
                return semantics == Semantics::Lossy && evaluable(operands[0]) && evaluable(operands[1]);
            }
            if semantics == Semantics::Lossy {
                return false;
            }
            let wrap_required = (semantics == Semantics::Signed && instruction.flags.contains(Flags::NSW))
                || (semantics == Semantics::Unsigned && instruction.flags.contains(Flags::NUW));
            wrap_required && evaluable(operands[0]) && evaluable(operands[1])
        }
        _ => false,
    }
}

/// `foldShiftedShift`
/// ([:724](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L724)):
/// `outer (inner x, C1), C2`, with `inner` changed in place.
fn fold_shifted_shift(
    c: &mut Combiner,
    at: InstId,
    inner: InstId,
    outer_amount: u32,
    is_outer_shl: bool,
    semantics: Semantics,
) -> Operand {
    let instruction = c.function.instruction(inner);
    let Opcode::Binary(op) = instruction.opcode else { unreachable!("a shift") };
    let (x, flags) = (instruction.operands[0], instruction.flags);
    let is_inner_shl = op == BinaryOp::Shl;
    let ty = c.type_of(x);
    let w = c.context.types.int_bits(ty).expect("an integer");
    let inner_amount = imm(&query(c, Some(at)), instruction.operands[1]).expect("a constant amount") as u32;

    // Change the shift amount and clear the poison flags.
    let new_inner_shift = |c: &mut Combiner, amount: u32| {
        let amount = c.int(ty, u128::from(amount));
        c.function.set_operand(inner, 1, amount);
        let cleared =
            if is_inner_shl { flags.without(Flags::NUW).without(Flags::NSW) } else { flags.without(Flags::EXACT) };
        c.function.set_flags(inner, cleared);
        result_of(c, inner)
    };

    // Two logical shifts in the same direction.
    if is_inner_shl == is_outer_shl {
        // An oversized composite shift gives 0 to unsigned shifts.
        if inner_amount + outer_amount >= w {
            return c.int(ty, 0);
        }
        return new_inner_shift(c, inner_amount + outer_amount);
    }

    // Equal amounts in opposite directions become an `and`.
    if inner_amount == outer_amount {
        if !is_outer_shl && semantics == Semantics::Signed {
            return x;
        }
        if !is_outer_shl && semantics == Semantics::Unsigned && is_inner_shl && flags.contains(Flags::NUW) {
            return x;
        }
        let bits = if is_inner_shl { low_bits(w, w - outer_amount) } else { high_bits(w, w - outer_amount) };
        let mask_c = c.int(ty, bits);
        let and = c.create_binop(at, BinaryOp::And, x, mask_c, Flags::default());
        if let Some(and_inst) = defining(c, and) {
            c.function.move_to(and_inst, Position::Before(inner)).expect("a placed instruction");
        }
        return and;
    }

    // `canEvaluateShiftedShift` has made sure the masked-off bits are unused:
    // lshr (shl X, C1), C2 --> shl X, C1 - C2
    new_inner_shift(c, inner_amount - outer_amount)
}

/// `InstCombinerImpl::getShiftedValue`
/// ([:797](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L797)):
/// the computation `can_evaluate_shifted` approved, made. New instructions go
/// before `at`.
pub fn get_shifted_value(
    c: &mut Combiner,
    at: InstId,
    v: Operand,
    num_bits: u32,
    is_left_shift: bool,
    semantics: Semantics,
) -> Operand {
    // Constants shift.
    if matches!(v, Operand::Constant(_)) {
        let shift_op = if is_left_shift {
            BinaryOp::Shl
        } else if semantics == Semantics::Signed {
            BinaryOp::AShr
        } else {
            BinaryOp::LShr
        };
        let ty = c.type_of(v);
        let amount = c.int(ty, u128::from(num_bits));
        return c.create_binop(at, shift_op, v, amount, Flags::default());
    }
    let inst = defining(c, v).expect("an instruction");
    c.add_to_worklist(inst);
    let instruction = c.function.instruction(inst);
    let (opcode, operands, flags) = (instruction.opcode.clone(), instruction.operands.clone(), instruction.flags);
    let shift_operand = |c: &mut Combiner, index: usize| {
        let shifted = get_shifted_value(c, at, operands[index], num_bits, is_left_shift, semantics);
        c.function.set_operand(inst, index, shifted);
    };
    match opcode {
        Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) => {
            shift_operand(c, 0);
            shift_operand(c, 1);
            v
        }
        Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr) => {
            fold_shifted_shift(c, at, inst, num_bits, is_left_shift, semantics)
        }
        Opcode::Select => {
            shift_operand(c, 1);
            shift_operand(c, 2);
            v
        }
        Opcode::Phi => {
            for index in (0..operands.len()).step_by(2) {
                shift_operand(c, index);
            }
            v
        }
        Opcode::Binary(BinaryOp::Mul) => {
            let ty = c.type_of(v);
            let w = c.context.types.int_bits(ty).expect("an integer");
            let zero = c.int(ty, 0);
            let neg =
                c.insert_before(inst, Opcode::Binary(BinaryOp::Sub), ty, vec![zero, operands[0]], Flags::default());
            let neg = result_of(c, neg);
            let mask_c = c.int(ty, low_bits(w, w - num_bits));
            let and = c.insert_before(inst, Opcode::Binary(BinaryOp::And), ty, vec![neg, mask_c], Flags::default());
            result_of(c, and)
        }
        Opcode::Binary(BinaryOp::Add) => {
            if is_left_shift {
                c.function.set_flags(inst, flags.without(Flags::POISON_GENERATING));
            }
            shift_operand(c, 0);
            shift_operand(c, 1);
            v
        }
        _ => unreachable!("inconsistent with can_evaluate_shifted"),
    }
}

/// `canShiftBinOpWithConstantRHS`
/// ([:871](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L871)):
/// a bitwise operator or add with a constant right side may be pulled through
/// the shift.
fn can_shift_bin_op_with_constant_rhs(
    q: &Query,
    shift: BinaryOp,
    bo: Operand,
    bo_op: BinaryOp,
) -> bool {
    match bo_op {
        BinaryOp::Add => shift == BinaryOp::Shl,
        BinaryOp::Or | BinaryOp::And => true,
        // A `not` of a logical shift would become a plain `xor`.
        BinaryOp::Xor => !(matches!(shift, BinaryOp::Shl | BinaryOp::LShr) && pattern::not(q, bo).is_some()),
        _ => false,
    }
}

/// `InstCombinerImpl::FoldShiftByConstant`
/// ([:888](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L888)).
fn fold_shift_by_constant(
    c: &mut Combiner,
    op0: Operand,
    c1: Operand,
    inst: InstId,
) -> Option<Visit> {
    let (op, _, _, flags) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let w = c.context.types.int_bits(ty).expect("an integer");
    let is_left_shift = op == BinaryOp::Shl;

    // (C2 << X) << C1 --> (C2 << C1) << X
    // (C2 >> X) >> C1 --> (C2 >> C1) >> X
    let shifted_constant = {
        let q = query(c, Some(inst));
        binop(&q, op0, op).and_then(|(_, c2, x, f0)| pattern::imm_constant(&q, c2).then_some((c2, x, f0)))
    };
    if let Some((c2, x, f0)) = shifted_constant {
        let new_c = c.create_binop(inst, op, c2, c1, Flags::default());
        let new_flags = if is_left_shift {
            flags_of(&[
                (flags.contains(Flags::NUW) && f0.contains(Flags::NUW), Flags::NUW),
                (flags.contains(Flags::NSW) && f0.contains(Flags::NSW), Flags::NSW),
            ])
        } else {
            flags_of(&[(flags.contains(Flags::EXACT) && f0.contains(Flags::EXACT), Flags::EXACT)])
        };
        return Some(c.replace_with_binop(inst, op, new_c, x, new_flags));
    }

    // (X / +DivC) >> (Width - 1) --> ext (X <= -DivC)
    // (X / -DivC) >> (Width - 1) --> ext (X >= +DivC)
    let sdiv = {
        let q = query(c, Some(inst));
        if !is_left_shift && is_specific_int(&q, c1, u128::from(w - 1)) {
            binop(&q, op0, BinaryOp::SDiv).and_then(|(_, x, div, _)| {
                let div = imm(&q, div)?;
                (div != 0 && div != 1u128 << (w - 1)).then_some((x, div))
            })
        } else {
            None
        }
    };
    if let Some((x, div)) = sdiv {
        let neg_div = c.int(ty, div.wrapping_neg() & mask(w));
        let pred = if div >> (w - 1) & 1 == 1 { IntPredicate::Sge } else { IntPredicate::Sle };
        let cmp = c.create_icmp(inst, pred, x, neg_div);
        let ext = if op == BinaryOp::AShr { CastOp::SExt } else { CastOp::ZExt };
        return Some(replace_with_cast(c, inst, ext, cmp, ty));
    }

    let op1c = imm(&query(c, Some(inst)), c1)?;
    if op1c >= u128::from(w) {
        return None;
    }
    let amount = op1c as u32;

    // See if we can propagate this shift into the input: the trivial case of
    // lshr (shl x, c1), c2 and more complex ones.
    if op != BinaryOp::AShr {
        let semantics = if is_left_shift { Semantics::Lossy } else { Semantics::Unsigned };
        if can_evaluate_shifted(c, op0, amount, is_left_shift, semantics, inst) {
            let shifted = get_shifted_value(c, inst, op0, amount, is_left_shift, semantics);
            return Some(Visit::Replace(shifted));
        }
    }

    let found = c.fold_bin_op_into_select_or_phi(inst);
    if !matches!(found, Visit::None) {
        return Some(found);
    }

    if !has_one_use(c, op0) {
        return None;
    }

    // A bitwise operator with a constant right side, shifted only here, comes
    // out of the shift.
    let bo_rhs = {
        let q = query(c, Some(inst));
        q.instruction_of(op0).and_then(|(_, bo)| {
            let Opcode::Binary(bo_op) = bo.opcode else { return None };
            imm(&q, bo.operands[1])?;
            can_shift_bin_op_with_constant_rhs(&q, op, op0, bo_op).then_some((bo_op, bo.operands[0], bo.operands[1]))
        })
    };
    if let Some((bo_op, bo0, bo1)) = bo_rhs {
        let new_rhs = c.create_binop(inst, op, bo1, c1, Flags::default());
        let new_shift = c.create_binop(inst, op, bo0, c1, Flags::default());
        return Some(c.replace_with_binop(inst, bo_op, new_shift, new_rhs, Flags::default()));
    }

    // A select that conditionally runs a binary operator: pull the select and
    // the operator through the shift.
    //   shl (select C, (add X, C1), X), C2  -->
    //   Y = shl X, C2; select C, (add Y, C1 << C2), Y
    let true_arm = {
        let q = query(c, Some(inst));
        pattern::select(&q, op0).and_then(|(_, _, cond, t, f)| {
            let (_, tbo) = q.instruction_of(t)?;
            let Opcode::Binary(bo_op) = tbo.opcode else { return None };
            let ok = has_one_use(c, t)
                && !matches!(f, Operand::Constant(_))
                && tbo.operands[0] == f
                && imm(&q, tbo.operands[1]).is_some()
                && can_shift_bin_op_with_constant_rhs(&q, op, t, bo_op);
            ok.then_some((cond, bo_op, tbo.operands[1], f))
        })
    };
    if let Some((cond, bo_op, rhs, false_val)) = true_arm {
        let new_rhs = c.create_binop(inst, op, rhs, c1, Flags::default());
        let new_shift = c.create_binop(inst, op, false_val, c1, Flags::default());
        let new_op = c.create_binop(inst, bo_op, new_shift, new_rhs, Flags::default());
        return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![cond, new_op, new_shift], Flags::default()));
    }
    let false_arm = {
        let q = query(c, Some(inst));
        pattern::select(&q, op0).and_then(|(_, _, cond, t, f)| {
            let (_, fbo) = q.instruction_of(f)?;
            let Opcode::Binary(bo_op) = fbo.opcode else { return None };
            let ok = has_one_use(c, f)
                && !matches!(t, Operand::Constant(_))
                && fbo.operands[0] == t
                && imm(&q, fbo.operands[1]).is_some()
                && can_shift_bin_op_with_constant_rhs(&q, op, f, bo_op);
            ok.then_some((cond, bo_op, fbo.operands[1], t))
        })
    };
    if let Some((cond, bo_op, rhs, true_val)) = false_arm {
        let new_rhs = c.create_binop(inst, op, rhs, c1, Flags::default());
        let new_shift = c.create_binop(inst, op, true_val, c1, Flags::default());
        let new_op = c.create_binop(inst, bo_op, new_shift, new_rhs, Flags::default());
        return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![cond, new_shift, new_op], Flags::default()));
    }

    None
}

/// `InstCombinerImpl::foldLShrOverflowBit`
/// ([:1032](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L1032)):
/// `lshr (add (zext X), (zext Y)), K` as the carry of the narrow add.
fn fold_lshr_overflow_bit(
    c: &mut Combiner,
    inst: InstId,
) -> Option<Visit> {
    let (_, add, shift_amt, _) = shift_parts(c, inst);
    let ty = c.type_of(add);
    if c.context.types.int_bits(ty).expect("an integer") < 3 {
        return None;
    }
    let (x, y) = {
        let q = query(c, Some(inst));
        let amount = u32::try_from(imm(&q, shift_amt)?).ok()?;
        let (_, x, y, _) = binop(&q, add, BinaryOp::Add)?;
        let zext_of =
            |v: Operand| pattern::cast(&q, v, CastOp::ZExt).filter(|_| has_one_use(c, v)).map(|(_, _, narrow)| narrow);
        let (x, y) = (zext_of(x)?, zext_of(y)?);
        if amount == 1 || width(&q, x) != amount || width(&q, y) != amount {
            return None;
        }
        // `add` is read by the shift and by truncs to `amount` bits or fewer.
        let Operand::Value(add_value) = add else { return None };
        if !has_one_use(c, add) {
            for user in c.function.users(add_value) {
                if user.user == inst {
                    continue;
                }
                let read = c.function.instruction(user.user);
                let trunc_width = read.result.map(|result| width(&q, Operand::Value(result)));
                if read.opcode != Opcode::Cast(CastOp::Trunc) || trunc_width.is_none_or(|bits| bits > amount) {
                    return None;
                }
            }
        }
        (x, y)
    };
    // Insert at `add`, so the narrow add dominates its users.
    let add_inst = defining(c, add)?;
    let narrow_add = c.create_add(add_inst, x, y);
    let overflow = c.create_icmp(add_inst, IntPredicate::Ult, narrow_add, x);
    // The uses of the original add read a zext of the narrow add.
    if !has_one_use(c, add) {
        let wide = c.create_cast(add_inst, CastOp::ZExt, narrow_add, ty, Flags::default());
        c.replace_inst_with(add_inst, wide);
        c.erase(add_inst);
    }
    Some(replace_with_cast(c, inst, CastOp::ZExt, overflow, ty))
}

/// `setShiftFlags`
/// ([:1092](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L1092)):
/// `nuw` and `nsw` on a `shl`, `exact` on a right shift, from known bits.
fn set_shift_flags(
    c: &mut Combiner,
    inst: InstId,
) -> bool {
    let (op, op0, op1, flags) = shift_parts(c, inst);
    let new_flags = {
        let q = query(c, Some(inst));
        if op == BinaryOp::Shl {
            if flags.contains(Flags::NUW) && flags.contains(Flags::NSW) {
                return false;
            }
        } else {
            if flags.contains(Flags::EXACT) {
                return false;
            }
            // shr (shl X, Y), Y
            if binop(&q, op0, BinaryOp::Shl).is_some_and(|(_, _, amount, _)| amount == op1) {
                c.function.set_flags(inst, with_flag(flags, Flags::EXACT));
                return true;
            }
            // `exact` where the amount is cttz of the same operand.
            if pattern::intrinsic_call(&q, op1, "llvm.cttz").is_some_and(|(_, args)| args[0] == op0) {
                c.function.set_flags(inst, with_flag(flags, Flags::EXACT));
                return true;
            }
        }
        let known_count = known::compute_known_bits(&q, op1, 0);
        let w = known_count.width;
        // A shift by the width or more is poison: assume the count is below it.
        let max_count = known_count.max_value().min(u128::from(w - 1)) as u32;
        let known_amount = known::compute_known_bits(&q, op0, 0);
        if op == BinaryOp::Shl {
            let mut new = flags;
            if !flags.contains(Flags::NUW) && max_count <= known_amount.count_min_leading_zeros() {
                new.insert(Flags::NUW);
            }
            if !flags.contains(Flags::NSW)
                && (max_count < known_amount.count_min_sign_bits() || max_count < compute_num_sign_bits(&q, op0, 0))
            {
                new.insert(Flags::NSW);
            }
            new
        } else if max_count <= known_amount.count_min_trailing_zeros() {
            with_flag(flags, Flags::EXACT)
        } else {
            flags
        }
    };
    if new_flags == flags {
        return false;
    }
    c.function.set_flags(inst, new_flags);
    true
}

/// `visitShl`'s folds of a shift by the constant `amount`.
fn shl_by_constant(
    c: &mut Combiner,
    inst: InstId,
    amount: u32,
) -> Option<Visit> {
    let (_, op0, op1, flags) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let w = c.context.types.int_bits(ty).expect("an integer");
    let nsw = flags.contains(Flags::NSW);
    let high_mask = |c: &Combiner| c.int(ty, high_bits(w, w - amount.min(w)));

    // shl (zext X), C --> zext (shl X, C), valid where X would have zeros
    // shifted out.
    let zext = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op0, CastOp::ZExt)
            .filter(|_| has_one_use(c, op0))
            .and_then(
                |(_, _, x)| {
                    let src_w = width(&q, x);
                    (amount < src_w && known::masked_value_is_zero(&q, x, high_bits(src_w, amount), 0)).then_some(x)
                },
            )
    };
    if let Some(x) = zext {
        let src_ty = c.type_of(x);
        let count = c.int(src_ty, u128::from(amount));
        let narrow = c.create_shl(inst, x, count);
        return Some(replace_with_cast(c, inst, CastOp::ZExt, narrow, ty));
    }

    // (X >> C) << C --> X & (-1 << C)
    let shr_same = {
        let q = query(c, Some(inst));
        shr_of(&q, op0).filter(|&(_, _, _, count, _)| count == op1).map(|(_, _, x, ..)| x)
    };
    if let Some(x) = shr_same {
        let mask_c = high_mask(c);
        return Some(c.replace_with_binop(inst, BinaryOp::And, x, mask_c, Flags::default()));
    }

    // (X >>?,exact C1) << C: the exact shift loses nothing.
    let shr_by_constant = {
        let q = query(c, Some(inst));
        shr_of(&q, op0).and_then(|(_, shr_op, x, count, shr_flags)| {
            let count = imm(&q, count).filter(|&count| count < u128::from(w))?;
            Some((shr_op, x, count as u32, shr_flags))
        })
    };
    if let Some((shr_op, x, shr_amount, shr_flags)) = shr_by_constant
        && shr_flags.contains(Flags::EXACT)
    {
        if shr_amount < amount {
            // If C1 < C: (X >>?,exact C1) << C --> X << (C - C1)
            let diff = c.int(ty, u128::from(amount - shr_amount));
            let new_flags = flags_of(&[
                (flags.contains(Flags::NUW) || (shr_amount != 0 && shr_op == BinaryOp::LShr && nsw), Flags::NUW),
                (nsw, Flags::NSW),
            ]);
            return Some(c.replace_with_binop(inst, BinaryOp::Shl, x, diff, new_flags));
        }
        if shr_amount > amount {
            // If C1 > C: (X >>?exact C1) << C --> X >>?exact (C1 - C)
            let diff = c.int(ty, u128::from(shr_amount - amount));
            return Some(c.replace_with_binop(inst, shr_op, x, diff, Flags::EXACT));
        }
    }
    if let Some((shr_op, x, shr_amount, shr_flags)) = shr_by_constant
        && has_one_use(c, op0)
    {
        if shr_amount < amount {
            // If C1 < C: (X >>? C1) << C --> (X << (C - C1)) & (-1 << C)
            let diff = c.int(ty, u128::from(amount - shr_amount));
            let new_flags = flags_of(&[
                (flags.contains(Flags::NUW) || (shr_amount != 0 && shr_op == BinaryOp::LShr && nsw), Flags::NUW),
                (nsw, Flags::NSW),
            ]);
            let new_shl = c.insert_before(inst, Opcode::Binary(BinaryOp::Shl), ty, vec![x, diff], new_flags);
            let new_shl = result_of(c, new_shl);
            let mask_c = high_mask(c);
            return Some(c.replace_with_binop(inst, BinaryOp::And, new_shl, mask_c, Flags::default()));
        }
        if shr_amount > amount {
            // If C1 > C: (X >>? C1) << C --> (X >>? (C1 - C)) & (-1 << C)
            let diff = c.int(ty, u128::from(shr_amount - amount));
            let new_shr = c.insert_before(
                inst,
                Opcode::Binary(shr_op),
                ty,
                vec![x, diff],
                flags_of(&[(shr_flags.contains(Flags::EXACT), Flags::EXACT)]),
            );
            let new_shr = result_of(c, new_shr);
            let mask_c = high_mask(c);
            return Some(c.replace_with_binop(inst, BinaryOp::And, new_shr, mask_c, Flags::default()));
        }
    }

    // Similar to above, through an intermediate trunc.
    let trunc_shr = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op0, CastOp::Trunc)
            .filter(|&(_, _, inner)| has_one_use(c, op0) && has_one_use(c, inner))
            .and_then(|(_, _, inner)| shr_of(&q, inner))
            .and_then(|(_, shr_op, x, count, _)| Some((shr_op, x, imm(&q, count)? as u32)))
    };
    if let Some((shr_op, x, shr_amount)) = trunc_shr {
        // The larger shift direction survives.
        let diff = shr_amount.abs_diff(amount);
        let x_ty = c.type_of(x);
        let diff_c = c.int(x_ty, u128::from(diff));
        let shift_op = if shr_amount > amount { shr_op } else { BinaryOp::Shl };
        let new_shift = c.create_binop(inst, shift_op, x, diff_c, Flags::default());
        let trunc = c.create_cast(inst, CastOp::Trunc, new_shift, ty, Flags::default());
        let mask_c = high_mask(c);
        return Some(c.replace_with_binop(inst, BinaryOp::And, trunc, mask_c, Flags::default()));
    }

    None
}

/// `visitShl`'s `(C1 - X) << C --> (C1 << C) - (X << C)`.
fn shl_of_sub_from_constant(
    c: &mut Combiner,
    inst: InstId,
    amount: u32,
) -> Option<Visit> {
    let (_, op0, op1, _) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let w = c.context.types.int_bits(ty).expect("an integer");
    let sub_from_constant = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Sub).filter(|_| has_one_use(c, op0)).and_then(|(_, c1, x, _)| Some((imm(&q, c1)?, x)))
    };
    if let Some((c1, x)) = sub_from_constant {
        let new_lhs = c.int(ty, shl_w(c1, amount, w));
        let new_shift = c.create_shl(inst, x, op1);
        return Some(c.replace_with_binop(inst, BinaryOp::Sub, new_lhs, new_shift, Flags::default()));
    }
    None
}

/// `visitShl`'s fold of `((X >> C) bop Y) << C` and its masked form.
fn shl_of_shifted_binop(
    c: &mut Combiner,
    inst: InstId,
    amount: u32,
) -> Option<Visit> {
    enum Found {
        /// `(X bop (Y << C)) & (-1 << C)`.
        Plain { x: Operand },
        /// `(X & (CC << C)) bop (Y << C)`.
        Masked { x: Operand, cc: u128 },
    }
    let (_, op0, op1, _) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let w = c.context.types.int_bits(ty).expect("an integer");
    let (bo_op, y, bo_flags, found) = {
        let q = query(c, Some(inst));
        let (_, bo) = q.instruction_of(op0).filter(|_| has_one_use(c, op0))?;
        let Opcode::Binary(bo_op) = bo.opcode else { return None };
        if !matches!(
            bo_op,
            BinaryOp::Add | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Sub
        ) {
            return None;
        }
        let (mut shr, mut y) = (bo.operands[0], bo.operands[1]);
        let shr_by_op1 = |v: Operand| shr_of(&q, v).filter(|&(_, _, _, count, _)| count == op1).map(|(_, _, x, ..)| x);
        // m_And(m_OneUse(m_Shr(X, op1)), m_APInt(CC))
        let masked_shr = |v: Operand| {
            let (_, lhs, cc, _) = binop(&q, v, BinaryOp::And)?;
            let x = shr_by_op1(lhs).filter(|_| has_one_use(c, lhs))?;
            Some((x, imm(&q, cc)?))
        };
        // Sub is not commutable: the shift right stays where it is.
        // (Y bop (X >> C)) << C --> ((X >> C) bop Y) << C
        if bo_op != BinaryOp::Sub && has_one_use(c, y) && (shr_by_op1(y).is_some() || masked_shr(y).is_some()) {
            std::mem::swap(&mut shr, &mut y);
        }
        let found = if let Some(x) = shr_by_op1(shr).filter(|_| has_one_use(c, shr)) {
            Found::Plain { x }
        } else if let Some((x, cc)) = masked_shr(shr).filter(|_| has_one_use(c, shr)) {
            Found::Masked { x, cc }
        } else {
            return None;
        };
        (bo_op, y, bo.flags, found)
    };
    let ys = c.create_shl(inst, y, op1);
    match found {
        Found::Plain { x } => {
            let b = c.create_binop(inst, bo_op, x, ys, Flags::default());
            let mask_c = c.int(ty, high_bits(w, w - amount.min(w)));
            Some(c.replace_with_binop(inst, BinaryOp::And, b, mask_c, Flags::default()))
        }
        Found::Masked { x, cc } => {
            let shifted_cc = c.int(ty, shl_w(cc, amount, w));
            let m = c.create_binop(inst, BinaryOp::And, x, shifted_cc, Flags::default());
            let disjoint = bo_op == BinaryOp::Or && bo_flags.contains(Flags::DISJOINT);
            Some(c.replace_with_binop(inst, bo_op, m, ys, flags_of(&[(disjoint, Flags::DISJOINT)])))
        }
    }
}

/// `InstCombinerImpl::visitShl`
/// ([:1151](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L1151)).
pub fn visit_shl(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if let Some(found) = simplified(c, inst) {
        return found;
    }
    // DEVIATE 1: foldVectorBinop.
    if let Some(r) = common_shift_transforms(c, inst) {
        return r;
    }
    if let Some(r) = drop_redundant_masking_of_left_shift_input(c, inst) {
        return r;
    }
    let (_, op0, op1, flags) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let w = c.context.types.int_bits(ty).expect("an integer");

    if let Some(amount) = imm(&query(c, Some(inst)), op1) {
        let amount = amount.min(u128::from(w)) as u32;
        if let Some(r) = shl_by_constant(c, inst, amount) {
            return r;
        }
        if let Some(r) = shl_of_shifted_binop(c, inst, amount) {
            return r;
        }
        if let Some(r) = shl_of_sub_from_constant(c, inst, amount) {
            return r;
        }
    }

    if set_shift_flags(c, inst) {
        return Visit::Changed;
    }

    // Transform (x >> y) << y to x & (-1 << y), for any right shift.
    let shr_x = {
        let q = query(c, Some(inst));
        shr_of(&q, op0).filter(|&(_, _, _, count, _)| count == op1 && has_one_use(c, op0)).map(|(_, _, x, ..)| x)
    };
    if let Some(x) = shr_x {
        let all_ones = c.all_ones(ty);
        let mask_c = c.create_shl(inst, all_ones, op1);
        return c.replace_with_binop(inst, BinaryOp::And, mask_c, x, Flags::default());
    }

    // Transform (-1 >> y) << y to -1 << y
    let ones_shr = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::LShr).is_some_and(|(_, ones, count, _)| pattern::all_ones(&q, ones) && count == op1)
    };
    if ones_shr {
        let all_ones = c.all_ones(ty);
        return c.replace_with_binop(inst, BinaryOp::Shl, all_ones, op1, Flags::default());
    }

    if pattern::imm_constant(&query(c, Some(inst)), op1) {
        // (X * C2) << C1 --> X * (C2 << C1)
        let mul = {
            let q = query(c, Some(inst));
            binop(&q, op0, BinaryOp::Mul).filter(|&(_, _, c2, _)| pattern::imm_constant(&q, c2))
        };
        if let Some((_, x, c2, _)) = mul {
            let shifted = c.create_shl(inst, c2, op1);
            return c.replace_with_binop(inst, BinaryOp::Mul, x, shifted, Flags::default());
        }
        // shl (zext i1 X), C1 --> select (X, 1 << C1, 0)
        let bool_zext = {
            let q = query(c, Some(inst));
            pattern::cast(&q, op0, CastOp::ZExt).filter(|&(_, _, x, ..)| width(&q, x) == 1).map(|(_, _, x)| x)
        };
        if let Some(x) = bool_zext {
            let one = c.int(ty, 1);
            let new_c = c.create_shl(inst, one, op1);
            let zero = c.int(ty, 0);
            return c.replace_with_new(inst, Opcode::Select, ty, vec![x, new_c, zero], Flags::default());
        }
    }

    if pattern::one(&query(c, Some(inst)), op0) {
        // (1 << (C - x)) -> ((1 << C) >> x) if C is bitwidth - 1
        let sub = {
            let q = query(c, Some(inst));
            binop(&q, op1, BinaryOp::Sub).filter(|&(_, c, _, _)| is_specific_int(&q, c, u128::from(w - 1)))
        };
        if let Some((_, _, x, _)) = sub {
            let sign_mask = c.int(ty, 1u128 << (w - 1));
            return c.replace_with_binop(inst, BinaryOp::LShr, sign_mask, x, Flags::default());
        }
        // 1 << (cttz X) --> -X & X
        let cttz = {
            let q = query(c, Some(inst));
            pattern::intrinsic_call(&q, op1, "llvm.cttz").filter(|_| has_one_use(c, op1)).map(|(_, args)| args[0])
        };
        if let Some(x) = cttz {
            let neg_x = c.create_neg(inst, x);
            return c.replace_with_binop(inst, BinaryOp::And, neg_x, x, Flags::default());
        }
    }

    // LHS << (cttz RHS) --> (RHS & -RHS) * LHS
    let cttz = {
        let q = query(c, Some(inst));
        pattern::intrinsic_call(&q, op1, "llvm.cttz").filter(|_| has_one_use(c, op1)).map(|(_, args)| args[0])
    };
    if let Some(x) = cttz {
        let neg_x = c.create_neg(inst, x);
        let low_bit = c.create_binop(inst, BinaryOp::And, neg_x, x, Flags::default());
        // nuw of the shl carries over.
        let new_flags = flags_of(&[(flags.contains(Flags::NUW), Flags::NUW)]);
        return c.replace_with_binop(inst, BinaryOp::Mul, low_bit, op0, new_flags);
    }

    Visit::None
}

/// `m_NUWShl(m_Value(x), m_Specific(count))`: the shifted value.
fn nuw_shl_by(
    q: &Query,
    v: Operand,
    count: Operand,
) -> Option<Operand> {
    binop(q, v, BinaryOp::Shl).filter(|&(_, _, c, f)| c == count && f.contains(Flags::NUW)).map(|(_, x, ..)| x)
}

/// What a sign test says: the compare `x pred y` (`y` is zero where `None`).
type SignTest = (IntPredicate, Operand, Option<Operand>);

/// `or(x, -x)`: negative or not zero, so `x != 0`.
fn or_neg_test(
    q: &Query,
    v: Operand,
) -> Option<SignTest> {
    let (_, a, b, _) = binop(q, v, BinaryOp::Or)?;
    let neg_of = |neg: Operand, x: Operand| pattern::neg(q, neg).filter(|&(_, inner, _)| inner == x).map(|(_, x, _)| x);
    neg_of(a, b).or_else(|| neg_of(b, a)).map(|x| (IntPredicate::Ne, x, None))
}

/// `x -nsw y`: negative where `x < y`.
fn nsw_sub_test(
    q: &Query,
    v: Operand,
) -> Option<SignTest> {
    let (_, x, y, f) = binop(q, v, BinaryOp::Sub)?;
    f.contains(Flags::NSW).then_some((IntPredicate::Slt, x, Some(y)))
}

/// `(x - 1) & ~x`: negative only where `x == 0`.
fn lowest_clear_test(
    q: &Query,
    v: Operand,
) -> Option<SignTest> {
    let (_, a, b, _) = binop(q, v, BinaryOp::And)?;
    let minus_one =
        |v: Operand| binop(q, v, BinaryOp::Add).filter(|&(_, _, m, _)| pattern::all_ones(q, m)).map(|(_, x, ..)| x);
    let pair = |v: Operand, n: Operand| {
        let x = minus_one(v)?;
        (pattern::not(q, n)? == x).then_some(x)
    };
    pair(a, b).or_else(|| pair(b, a)).map(|x| (IntPredicate::Eq, x, None))
}

/// The tests `visitLShr` and `visitAShr` read a sign bit of as a compare.
const SIGN_TESTS: [fn(&Query, Operand) -> Option<SignTest>; 3] = [or_neg_test, nsw_sub_test, lowest_clear_test];

/// `visitLShr` and `visitAShr` of a one-use sign test by the width less one:
/// its compare, extended with `ext` (`zext` for lshr, `sext` for ashr).
fn fold_sign_test(
    c: &mut Combiner,
    inst: InstId,
    ext: CastOp,
) -> Option<Visit> {
    let (_, op0, _, _) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let (pred, x, y) = {
        let q = query(c, Some(inst));
        if !has_one_use(c, op0) {
            return None;
        }
        SIGN_TESTS.iter().find_map(|test| test(&q, op0))?
    };
    let y = y.unwrap_or_else(|| c.int(c.type_of(x), 0));
    let cmp = c.create_icmp(inst, pred, x, y);
    Some(replace_with_cast(c, inst, ext, cmp, ty))
}

/// `visitLShr`'s fold of a shift by the constant `amount`.
fn lshr_by_constant(
    c: &mut Combiner,
    inst: InstId,
    amount: u32,
) -> Option<Visit> {
    let (_, op0, op1, flags) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let w = c.context.types.int_bits(ty).expect("an integer");
    let exact = flags.contains(Flags::EXACT);
    let low_mask = |c: &Combiner| c.int(ty, low_bits(w, w - amount.min(w)));
    let layout = c.env.layout;

    // ctlz.i32(x)>>5 --> zext(x == 0), the same for cttz; ctpop.i32(x)>>5 -->
    // zext(x == -1)
    let count_intrinsic = {
        let q = query(c, Some(inst));
        (w.is_power_of_two() && w.ilog2() == amount)
            .then(|| {
                ["llvm.ctlz", "llvm.cttz", "llvm.ctpop"]
                    .into_iter()
                    .find_map(
                        |name| pattern::intrinsic_call(&q, op0, name).map(|(_, args)| (name == "llvm.ctpop", args[0])),
                    )
            })
            .flatten()
    };
    if let Some((is_pop, arg)) = count_intrinsic {
        let rhs = c.int(ty, if is_pop { u128::MAX } else { 0 });
        let cmp = c.create_icmp(inst, IntPredicate::Eq, arg, rhs);
        return Some(replace_with_cast(c, inst, CastOp::ZExt, cmp, ty));
    }

    let shl = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Shl).and_then(|(_, x, c1, f)| {
            let c1 = imm(&q, c1).filter(|&c1| c1 < u128::from(w))?;
            Some((x, c1 as u32, f))
        })
    };
    if let Some((x, shl_amount, shl_flags)) = shl {
        if shl_amount < amount {
            let diff = c.int(ty, u128::from(amount - shl_amount));
            if shl_flags.contains(Flags::NUW) {
                // (X <<nuw C1) >>u C --> X >>u (C - C1)
                return Some(c.replace_with_binop(inst, BinaryOp::LShr, x, diff, flags_of(&[(exact, Flags::EXACT)])));
            }
            if has_one_use(c, op0) {
                // (X << C1) >>u C --> (X >>u (C - C1)) & (-1 >> C)
                let new_lshr = c.create_binop(inst, BinaryOp::LShr, x, diff, flags_of(&[(exact, Flags::EXACT)]));
                let mask_c = low_mask(c);
                return Some(c.replace_with_binop(inst, BinaryOp::And, new_lshr, mask_c, Flags::default()));
            }
        } else if shl_amount > amount {
            let diff = c.int(ty, u128::from(shl_amount - amount));
            if shl_flags.contains(Flags::NUW) {
                // (X <<nuw C1) >>u C --> X <<nuw/nsw (C1 - C)
                let new_flags = flags_of(&[(true, Flags::NUW), (amount > 0, Flags::NSW)]);
                return Some(c.replace_with_binop(inst, BinaryOp::Shl, x, diff, new_flags));
            }
            if has_one_use(c, op0) {
                // (X << C1) >>u C --> X << (C1 - C) & (-1 >> C)
                let new_shl = c.create_shl(inst, x, diff);
                let mask_c = low_mask(c);
                return Some(c.replace_with_binop(inst, BinaryOp::And, new_shl, mask_c, Flags::default()));
            }
        } else {
            // (X << C) >>u C --> X & (-1 >>u C)
            let mask_c = low_mask(c);
            return Some(c.replace_with_binop(inst, BinaryOp::And, x, mask_c, Flags::default()));
        }
    }

    // ((X << C) + Y) >>u C --> (X + (Y >>u C)) & (-1 >>u C)
    let shl_add = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Add)
            .filter(|_| has_one_use(c, op0))
            .and_then(
                |(_, a, b, _)| {
                    let shl_by_op1 = |v: Operand| {
                        binop(&q, v, BinaryOp::Shl)
                            .filter(|&(_, _, count, _)| count == op1 && has_one_use(c, v))
                            .map(|(_, x, ..)| x)
                    };
                    shl_by_op1(a).map(|x| (x, b)).or_else(|| shl_by_op1(b).map(|x| (x, a)))
                },
            )
    };
    if let Some((x, y)) = shl_add {
        let new_lshr = c.create_binop(inst, BinaryOp::LShr, y, op1, Flags::default());
        let new_add = c.create_add(inst, new_lshr, x);
        let mask_c = low_mask(c);
        return Some(c.replace_with_binop(inst, BinaryOp::And, new_add, mask_c, Flags::default()));
    }

    // DEVIATE 2: LLVM asserts C < M for the fold below, as
    // SimplifyDemandedInstructionBits has already made a bigger shift zero.
    let src_width = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op0, CastOp::ZExt).map(|(_, _, x)| width(&q, x))
    };
    if src_width.is_some_and(|m| amount >= m) {
        return Some(Visit::Replace(c.int(ty, 0)));
    }
    let zext = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op0, CastOp::ZExt)
            .filter(|&(_, _, x)| has_one_use(c, op0) && should_change_type(layout, w, width(&q, x)))
            .map(|(_, _, x)| x)
    };
    if let Some(x) = zext {
        // lshr (zext iM X to iN), C --> zext (lshr X, C) to iN
        let src_ty = c.type_of(x);
        let count = c.int(src_ty, u128::from(amount));
        let new_lshr = c.create_binop(inst, BinaryOp::LShr, x, count, Flags::default());
        return Some(replace_with_cast(c, inst, CastOp::ZExt, new_lshr, ty));
    }

    let sext = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op0, CastOp::SExt).map(|(_, _, x)| (x, width(&q, x)))
    };
    if let Some((x, src_w)) = sext {
        // lshr (sext i1 X to iN), C --> select (X, -1 >> C, 0)
        if src_w == 1 {
            let new_c = low_mask(c);
            let zero = c.int(ty, 0);
            return Some(c.replace_with_new(inst, Opcode::Select, ty, vec![x, new_c, zero], Flags::default()));
        }
        if should_change_type(layout, w, src_w) && has_one_use(c, op0) {
            let src_ty = c.type_of(x);
            // lshr (sext iM X to iN), N-1 --> zext (lshr X, M-1) to iN
            if amount == w - 1 {
                let count = c.int(src_ty, u128::from(src_w - 1));
                let new_lshr = c.create_binop(inst, BinaryOp::LShr, x, count, Flags::default());
                return Some(replace_with_cast(c, inst, CastOp::ZExt, new_lshr, ty));
            }
            // lshr (sext iM X to iN), N-M --> zext (ashr X, min(N-M, M-1)) to
            // iN
            if amount == w - src_w {
                let count = c.int(src_ty, u128::from(amount.min(src_w - 1)));
                let ashr = c.create_binop(inst, BinaryOp::AShr, x, count, Flags::default());
                return Some(replace_with_cast(c, inst, CastOp::ZExt, ashr, ty));
            }
        }
    }

    if amount == w - 1 {
        // The sign bit of a sign test: a compare.
        if let Some(r) = fold_sign_test(c, inst, CastOp::ZExt) {
            return Some(r);
        }
        // Negative and odd: lshr i32 (srem X, 2), 31 --> and (X >> 31), X
        let srem_two = {
            let q = query(c, Some(inst));
            binop(&q, op0, BinaryOp::SRem)
                .filter(|&(_, _, two, _)| has_one_use(c, op0) && is_specific_int(&q, two, 2))
                .map(|(_, x, ..)| x)
        };
        if let Some(x) = srem_two {
            let count = c.int(ty, u128::from(amount));
            let sign_bit = c.create_binop(inst, BinaryOp::LShr, x, count, Flags::default());
            return Some(c.replace_with_binop(inst, BinaryOp::And, sign_bit, x, Flags::default()));
        }
    }

    // (trunc (X >>u C1)) >>u C
    let trunc_shr = {
        let q = query(c, Some(inst));
        pattern::cast(&q, op0, CastOp::Trunc)
            .filter(|_| has_one_use(c, op0))
            .and_then(
                |(_, _, src)| {
                    let (_, _, x, count, _) = shr_of(&q, src).filter(|&(_, op, ..)| op == BinaryOp::LShr)?;
                    Some((src, x, imm(&q, count)? as u32, width(&q, x)))
                },
            )
    };
    if let Some((trunc_src, x, c1, src_w)) = trunc_shr {
        let amt_sum = amount + c1;
        // If the combined shift fits the source width: (trunc (X >>u C1)) >>u C
        // --> and (trunc (X >>u (C1 + C)), MaskC). Where the first shift covers
        // the truncated bits the mask goes, and the use check is relaxed.
        if amt_sum < src_w && (has_one_use(c, trunc_src) || c1 >= src_w.saturating_sub(w)) {
            let x_ty = c.type_of(x);
            let count = c.int(x_ty, u128::from(amt_sum));
            let sum_shift = c.create_binop(inst, BinaryOp::LShr, x, count, Flags::default());
            let trunc = c.create_cast(inst, CastOp::Trunc, sum_shift, ty, Flags::default());
            let mask_c = c.int(ty, lshr_w(mask(w), amount, w));
            return Some(c.replace_with_binop(inst, BinaryOp::And, trunc, mask_c, Flags::default()));
        }
    }

    let nuw_mul = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Mul)
            .filter(|&(_, _, _, f)| f.contains(Flags::NUW))
            .and_then(|(_, x, mul_c, f)| Some((x, imm(&q, mul_c)?, f)))
    };
    if let Some((x, mul_c, mul_flags)) = nuw_mul {
        let minus_one_is_pow2 = mul_c >= 2 && (mul_c - 1).is_power_of_two();
        if w > 2 && minus_one_is_pow2 && 127 - mul_c.leading_zeros() == amount {
            // A "splat" mul replicates bits across each half of a value, so a
            // right shift simplifies back to X:
            // lshr i[2N] (mul nuw X, (2^N)+1), N --> X
            if amount * 2 == w {
                return Some(Visit::Replace(x));
            }
            // lshr (mul nuw (X, 2^N + 1)), N -> add nuw (X, lshr(X, N))
            if has_one_use(c, op0) {
                let count = c.int(ty, u128::from(amount));
                let shifted = c.create_binop(inst, BinaryOp::LShr, x, count, flags_of(&[(exact, Flags::EXACT)]));
                let new_flags = flags_of(&[(true, Flags::NUW), (mul_flags.contains(Flags::NSW), Flags::NSW)]);
                return Some(c.replace_with_binop(inst, BinaryOp::Add, x, shifted, new_flags));
            }
        }
        // If c is divisible by (1 << ShAmtC):
        // lshr (mul nuw x, MulC), ShAmtC -> mul nuw nsw x, (MulC >> ShAmtC)
        if has_one_use(c, op0) {
            let new_mul_c = lshr_w(mul_c, amount, w);
            if mul_c == shl_w(new_mul_c, amount, w) {
                let new_c = c.int(ty, new_mul_c);
                let new_flags = flags_of(&[(true, Flags::NUW), (true, Flags::NSW)]);
                return Some(c.replace_with_binop(inst, BinaryOp::Mul, x, new_c, new_flags));
            }
        }
    }

    // lshr (mul nsw (X, 2^N + 1)), N -> add nsw (X, lshr(X, N))
    let nsw_mul = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Mul)
            .filter(|&(_, _, _, f)| has_one_use(c, op0) && f.contains(Flags::NSW))
            .and_then(|(_, x, mul_c, _)| Some((x, imm(&q, mul_c)?)))
    };
    if let Some((x, mul_c)) = nsw_mul
        && w > 2
        && mul_c >= 2
        && (mul_c - 1).is_power_of_two()
        && 127 - mul_c.leading_zeros() == amount
    {
        let count = c.int(ty, u128::from(amount));
        let shifted = c.create_binop(inst, BinaryOp::LShr, x, count, flags_of(&[(exact, Flags::EXACT)]));
        return Some(c.replace_with_binop(inst, BinaryOp::Add, x, shifted, Flags::NSW));
    }

    // Try to narrow bswap. In the case where the shift amount equals the
    // bitwidth difference, the shift is eliminated.
    let narrow_swap = {
        let q = query(c, Some(inst));
        pattern::intrinsic_call(&q, op0, "llvm.bswap").and_then(|(_, args)| {
            let [extended] = *args else { return None };
            let (_, _, source) = pattern::cast(&q, extended, CastOp::ZExt)?;
            Some((extended, source))
        })
    };
    if let Some((extended, source)) = narrow_swap
        && has_one_use(c, op0)
        && has_one_use(c, extended)
    {
        let src_width = c.context.types.int_bits(c.type_of(source)).unwrap_or(0);
        if src_width != 0 && src_width % 16 == 0 {
            let width_diff = w - src_width;
            let narrow_ty = c.type_of(source);
            let narrow_swap = c.create_intrinsic(inst, "bswap", narrow_ty, &[source]);
            if amount >= width_diff {
                // (bswap (zext X)) >> C --> zext (bswap X >> C')
                let by = c.int(narrow_ty, u128::from(amount - width_diff));
                let shifted = c.create_binop(inst, BinaryOp::LShr, narrow_swap, by, Flags::default());
                return Some(c.replace_with_new(inst, Opcode::Cast(CastOp::ZExt), ty, vec![shifted], Flags::default()));
            }
            // (bswap (zext X)) >> C --> (zext (bswap X)) << C'
            let wide = c.create_cast(inst, CastOp::ZExt, narrow_swap, ty, Flags::default());
            let by = c.int(ty, u128::from(width_diff - amount));
            return Some(c.replace_with_binop(inst, BinaryOp::Shl, wide, by, Flags::default()));
        }
    }

    // Reduce add-carry of bools to logic:
    // ((zext BoolX) + (zext BoolY)) >> 1 --> zext (BoolX && BoolY)
    let bool_add = {
        let q = query(c, Some(inst));
        if amount == 1 {
            binop(&q, op0, BinaryOp::Add).and_then(|(_, x, y, _)| {
                let (_, _, bool_x) = pattern::cast(&q, x, CastOp::ZExt)?;
                let (_, _, bool_y) = pattern::cast(&q, y, CastOp::ZExt)?;
                (width(&q, bool_x) == 1
                    && width(&q, bool_y) == 1
                    && (has_one_use(c, x) || has_one_use(c, y) || has_one_use(c, op0)))
                .then_some((bool_x, bool_y))
            })
        } else {
            None
        }
    };
    if let Some((bool_x, bool_y)) = bool_add {
        let and = c.create_binop(inst, BinaryOp::And, bool_x, bool_y, Flags::default());
        return Some(replace_with_cast(c, inst, CastOp::ZExt, and, ty));
    }
    None
}

/// `InstCombinerImpl::visitLShr`
/// ([:1399](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L1399)).
pub fn visit_lshr(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if let Some(found) = simplified(c, inst) {
        return found;
    }
    // DEVIATE 1: foldVectorBinop.
    if let Some(r) = common_shift_transforms(c, inst) {
        return r;
    }
    let (_, op0, op1, flags) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let w = c.context.types.int_bits(ty).expect("an integer");
    let exact = flags.contains(Flags::EXACT);

    // lshr 1, X --> zext (X == 0)
    if pattern::one(&query(c, Some(inst)), op0) {
        let zero = c.int(c.type_of(op1), 0);
        let is_null = c.create_icmp(inst, IntPredicate::Eq, op1, zero);
        return replace_with_cast(c, inst, CastOp::ZExt, is_null, ty);
    }

    // (iN (~X) u>> (N - 1)) --> zext (X > -1)
    let not_x = {
        let q = query(c, Some(inst));
        pattern::not(&q, op0).filter(|_| has_one_use(c, op0) && is_specific_int(&q, op1, u128::from(w - 1)))
    };
    if let Some(x) = not_x {
        let ones = c.all_ones(c.type_of(x));
        let is_not_neg = c.create_icmp(inst, IntPredicate::Sgt, x, ones);
        return replace_with_cast(c, inst, CastOp::ZExt, is_not_neg, ty);
    }

    // ((X << nuw Z) sub nuw Y) >>u exact Z --> X sub nuw (Y >>u exact Z)
    let shl_sub = {
        let q = query(c, Some(inst));
        let nuw_shl_by_op1 = |v: Operand| nuw_shl_by(&q, v, op1);
        binop(&q, op0, BinaryOp::Sub)
            .filter(|&(_, _, _, f)| exact && has_one_use(c, op0) && f.contains(Flags::NUW))
            .and_then(|(_, lhs, y, f)| Some((nuw_shl_by_op1(lhs)?, y, f)))
    };
    if let Some((x, y, sub_flags)) = shl_sub {
        let new_lshr = c.create_binop(inst, BinaryOp::LShr, y, op1, Flags::EXACT);
        let new_flags = flags_of(&[(true, Flags::NUW), (sub_flags.contains(Flags::NSW), Flags::NSW)]);
        return c.replace_with_binop(inst, BinaryOp::Sub, x, new_lshr, new_flags);
    }

    // Fold (X + Y) / 2 --> (X & Y) iff (X u<= 1) && (Y u<= 1)
    let add_of_bits = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Add)
            .filter(|_| pattern::one(&q, op1))
            .filter(|&(_, x, y, _)| {
                known::compute_known_bits(&q, x, 0).count_max_active_bits() <= 1
                    && known::compute_known_bits(&q, y, 0).count_max_active_bits() <= 1
            })
            .map(|(_, x, y, _)| (x, y))
    };
    if let Some((x, y)) = add_of_bits {
        return c.replace_with_binop(inst, BinaryOp::And, x, y, Flags::default());
    }

    // (sub nuw X, (Y << nuw Z)) >>u exact Z --> (X >>u exact Z) sub nuw Y
    let sub_shl = {
        let q = query(c, Some(inst));
        let nuw_shl_by_op1 = |v: Operand| nuw_shl_by(&q, v, op1);
        binop(&q, op0, BinaryOp::Sub)
            .filter(|&(_, _, _, f)| exact && has_one_use(c, op0) && f.contains(Flags::NUW))
            .and_then(|(_, x, rhs, f)| Some((x, nuw_shl_by_op1(rhs)?, f)))
    };
    if let Some((x, y, sub_flags)) = sub_shl {
        let new_lshr = c.create_binop(inst, BinaryOp::LShr, x, op1, Flags::EXACT);
        let new_flags = flags_of(&[(true, Flags::NUW), (sub_flags.contains(Flags::NSW), Flags::NSW)]);
        return c.replace_with_binop(inst, BinaryOp::Sub, new_lshr, y, new_flags);
    }

    // If both the binop and the shift are nuw:
    // ((X << nuw Z) binop nuw Y) >>u Z --> X binop nuw (Y >>u Z)
    let nuw_binop = {
        let q = query(c, Some(inst));
        let nuw_shl_by_op1 = |v: Operand| nuw_shl_by(&q, v, op1);
        q.instruction_of(op0).filter(|_| has_one_use(c, op0)).and_then(|(_, bo)| {
            let Opcode::Binary(bo_op) = bo.opcode else { return None };
            let (lhs, rhs) = (bo.operands[0], bo.operands[1]);
            let (x, y) = nuw_shl_by_op1(lhs).map(|x| (x, rhs)).or_else(|| nuw_shl_by_op1(rhs).map(|x| (x, lhs)))?;
            if !matches!(
                bo_op,
                BinaryOp::Add | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
            ) {
                return None;
            }
            let overflowing = matches!(
                bo_op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl
            );
            (!overflowing || bo.flags.contains(Flags::NUW)).then_some((bo_op, x, y, bo.flags, overflowing))
        })
    };
    if let Some((bo_op, x, y, bo_flags, overflowing)) = nuw_binop {
        let new_lshr =
            c.create_binop(inst, BinaryOp::LShr, y, op1, flags_of(&[(exact && bo_op != BinaryOp::And, Flags::EXACT)]));
        let new_flags = if overflowing {
            flags_of(&[(true, Flags::NUW), (bo_flags.contains(Flags::NSW), Flags::NSW)])
        } else {
            flags_of(&[(bo_flags.contains(Flags::DISJOINT), Flags::DISJOINT)])
        };
        return c.replace_with_binop(inst, bo_op, new_lshr, x, new_flags);
    }

    if let Some(amount) = imm(&query(c, Some(inst)), op1)
        && let Some(r) = lshr_by_constant(c, inst, amount.min(u128::from(w)) as u32)
    {
        return r;
    }

    if set_shift_flags(c, inst) {
        return Visit::Changed;
    }

    // Transform (x << y) >> y to x & (-1 >> y)
    let shl_x = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Shl)
            .filter(|&(_, _, count, _)| count == op1 && has_one_use(c, op0))
            .map(|(_, x, ..)| x)
    };
    if let Some(x) = shl_x {
        let all_ones = c.all_ones(ty);
        let mask_c = c.create_binop(inst, BinaryOp::LShr, all_ones, op1, Flags::default());
        return c.replace_with_binop(inst, BinaryOp::And, mask_c, x, Flags::default());
    }

    // Transform (-1 << y) >> y to -1 >> y
    let ones_shl = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Shl).is_some_and(|(_, ones, count, _)| pattern::all_ones(&q, ones) && count == op1)
    };
    if ones_shl {
        let all_ones = c.all_ones(ty);
        return c.replace_with_binop(inst, BinaryOp::LShr, all_ones, op1, Flags::default());
    }

    if let Some(overflow) = fold_lshr_overflow_bit(c, inst) {
        return overflow;
    }

    // Transform ((pow2 << x) >> cttz(pow2 << y)) -> ((1 << x) >> y)
    let pow2_shifts = {
        let q = query(c, Some(inst));
        binop(&q, op0, BinaryOp::Shl).and_then(|(_, shl0_op0, shl0_op1, f0)| {
            let (_, args) = pattern::intrinsic_call(&q, op1, "llvm.cttz")?;
            let (_, shl0_again, shl1_op1, f1) = binop(&q, args[0], BinaryOp::Shl)?;
            (shl0_again == shl0_op0 && power2::is_known_to_be_a_power_of_two(&q, shl0_op0, true, 0))
                .then_some((shl0_op1, shl1_op1, f0, f1))
        })
    };
    if let Some((shl0_op1, shl1_op1, f0, f1)) = pow2_shifts {
        let has_nuw = f0.contains(Flags::NUW) && f1.contains(Flags::NUW);
        let has_nsw = f0.contains(Flags::NSW) && f1.contains(Flags::NSW);
        if has_nuw || has_nsw {
            let one = c.int(ty, 1);
            let new_flags = flags_of(&[(has_nuw, Flags::NUW), (has_nsw, Flags::NSW)]);
            let new_shl = c.create_binop(inst, BinaryOp::Shl, one, shl0_op1, new_flags);
            return c.replace_with_binop(inst, BinaryOp::LShr, new_shl, shl1_op1, Flags::default());
        }
    }
    Visit::None
}

/// `foldVariableSignZeroExtensionOfVariableHighBitExtract`
/// ([:1759](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L1759)):
/// a sign extension from a variable number of bits of a variable high-bit
/// extract.
fn fold_variable_sign_zero_extension_of_variable_high_bit_extract(
    c: &mut Combiner,
    old_ashr: InstId,
) -> Option<Visit> {
    let (_, op0, op1, _) = shift_parts(c, old_ashr);
    let old_ty = c.type_of(op0);
    struct Plan {
        maybe_trunc: Operand,
        had_trunc: bool,
        high_op: BinaryOp,
        x: Operand,
        skip: Operand,
        high_flags: Flags,
    }
    let plan = {
        let q = query(c, Some(old_ashr));
        let w = width(&q, op0);
        let splat = |v: Operand, bits: u32| is_specific_int(&q, v, u128::from(bits));
        // (Val << (bitwidth(Val)-Nbits)) a>> (bitwidth(Val)-Nbits)
        let (_, maybe_trunc, shl_amount, _) = binop(&q, op0, BinaryOp::Shl)?;
        q.instruction_of(maybe_trunc)?;
        let (_, c1, nbits_ext, _) = binop(&q, zext_or_self(&q, shl_amount), BinaryOp::Sub)?;
        let nbits = zext_or_self(&q, nbits_ext);
        let (_, c2, nbits_again, _) = binop(&q, zext_or_self(&q, op1), BinaryOp::Sub)?;
        if zext_or_self(&q, nbits_again) != nbits || !splat(c1, w) || !splat(c2, w) {
            return None;
        }
        // There may or may not be a truncation after the outer two shifts.
        let (high, had_trunc) = match pattern::cast(&q, maybe_trunc, CastOp::Trunc) {
            Some((_, _, inner)) if q.instruction_of(inner).is_some() => (inner, true),
            _ => (maybe_trunc, false),
        };
        // The innermost part must be a right shift extracting the high NBits.
        let (_, high_op, x, skip, high_flags) = shr_of(&q, high)?;
        let (_, c0, nbits_third, _) = binop(&q, zext_or_self(&q, skip), BinaryOp::Sub)?;
        if zext_or_self(&q, nbits_third) != nbits || !splat(c0, width(&q, high)) {
            return None;
        }
        Plan { maybe_trunc, had_trunc, high_op, x, skip, high_flags }
    };
    // The outermost shifts are redundant where the outermost and innermost are
    // the same shift; a truncation stays.
    if plan.high_op == BinaryOp::AShr {
        return Some(Visit::Replace(plan.maybe_trunc));
    }
    // A truncation needs one instruction to go away.
    if plan.had_trunc && !(has_one_use(c, op0) || has_one_use(c, op1)) {
        return None;
    }
    // Bypass the two innermost shifts; `exact` carries over.
    let flags = flags_of(&[(plan.high_flags.contains(Flags::EXACT), Flags::EXACT)]);
    if !plan.had_trunc {
        return Some(c.replace_with_binop(old_ashr, BinaryOp::AShr, plan.x, plan.skip, flags));
    }
    let x_ty = c.type_of(plan.x);
    let new_ashr = c.insert_before(old_ashr, Opcode::Binary(BinaryOp::AShr), x_ty, vec![plan.x, plan.skip], flags);
    let new_ashr = result_of(c, new_ashr);
    Some(replace_with_cast(c, old_ashr, CastOp::Trunc, new_ashr, old_ty))
}

/// `InstCombinerImpl::visitAShr`
/// ([:1833](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineShifts.cpp#L1833)).
pub fn visit_ashr(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if let Some(found) = simplified(c, inst) {
        return found;
    }
    // DEVIATE 1: foldVectorBinop.
    if let Some(r) = common_shift_transforms(c, inst) {
        return r;
    }
    let (_, op0, op1, flags) = shift_parts(c, inst);
    let ty = c.type_of(op0);
    let w = c.context.types.int_bits(ty).expect("an integer");
    let exact = flags.contains(Flags::EXACT);
    let layout = c.env.layout;

    let by_constant = imm(&query(c, Some(inst)), op1).filter(|&amount| amount < u128::from(w));
    if let Some(amount) = by_constant {
        let amount = amount as u32;

        // ashr (shl (zext X), C), C --> sext X, where the shift amount is the
        // difference in width of the destination and source types.
        let zext_shl = {
            let q = query(c, Some(inst));
            binop(&q, op0, BinaryOp::Shl)
                .filter(|&(_, _, count, _)| count == op1)
                .and_then(|(_, inner, ..)| pattern::cast(&q, inner, CastOp::ZExt))
                .map(|(_, _, x)| x)
                .filter(|&x| amount == w - width(&q, x))
        };
        if let Some(x) = zext_shl {
            return replace_with_cast(c, inst, CastOp::SExt, x, ty);
        }

        // We can't handle (X << C1) >>s C2: it shifts arbitrary bits in. But
        // (X <<nsw C1) >>s C2 only shifts in sign bits.
        let nsw_shl = {
            let q = query(c, Some(inst));
            binop(&q, op0, BinaryOp::Shl)
                .filter(|&(_, _, _, f)| f.contains(Flags::NSW))
                .and_then(|(_, x, count, _)| Some((x, imm(&q, count).filter(|&count| count < u128::from(w))? as u32)))
        };
        if let Some((x, shl_amount)) = nsw_shl {
            if shl_amount < amount {
                // (X <<nsw C1) >>s C2 --> X >>s (C2 - C1)
                let diff = c.int(ty, u128::from(amount - shl_amount));
                return c.replace_with_binop(inst, BinaryOp::AShr, x, diff, flags_of(&[(exact, Flags::EXACT)]));
            }
            if shl_amount > amount {
                // (X <<nsw C1) >>s C2 --> X <<nsw (C1 - C2)
                let diff = c.int(ty, u128::from(shl_amount - amount));
                return c.replace_with_binop(inst, BinaryOp::Shl, x, diff, Flags::NSW);
            }
        }

        let ashr_ashr = {
            let q = query(c, Some(inst));
            binop(&q, op0, BinaryOp::AShr)
                .and_then(|(_, x, count, _)| Some((x, imm(&q, count).filter(|&count| count < u128::from(w))? as u32)))
        };
        if let Some((x, inner)) = ashr_ashr {
            // Oversized arithmetic shifts replicate the sign bit.
            let amt_sum = (amount + inner).min(w - 1);
            let sum = c.int(ty, u128::from(amt_sum));
            return c.replace_with_binop(inst, BinaryOp::AShr, x, sum, Flags::default());
        }

        let sext = {
            let q = query(c, Some(inst));
            pattern::cast(&q, op0, CastOp::SExt)
                .filter(|&(_, _, x)| has_one_use(c, op0) && should_change_type(layout, w, width(&q, x)))
                .map(|(_, _, x)| (x, width(&q, x)))
        };
        if let Some((x, src_w)) = sext {
            // ashr (sext X), C --> sext (ashr X, C')
            let src_ty = c.type_of(x);
            let count = c.int(src_ty, u128::from(amount.min(src_w - 1)));
            let new_shift = c.create_binop(inst, BinaryOp::AShr, x, count, Flags::default());
            return replace_with_cast(c, inst, CastOp::SExt, new_shift, ty);
        }

        if amount == w - 1 {
            // The sign bit of a sign test: a compare.
            if let Some(r) = fold_sign_test(c, inst, CastOp::SExt) {
                return r;
            }
        }

        // ashr (mul nsw (X, 2^N + 1)), N -> add nsw (X, ashr(X, N))
        let nsw_mul = {
            let q = query(c, Some(inst));
            binop(&q, op0, BinaryOp::Mul)
                .filter(|&(_, _, _, f)| has_one_use(c, op0) && f.contains(Flags::NSW))
                .and_then(|(_, x, mul_c, f)| Some((x, imm(&q, mul_c)?, f)))
        };
        if let Some((x, mul_c, mul_flags)) = nsw_mul
            && w > 2
            && mul_c >= 2
            && (mul_c - 1).is_power_of_two()
            && 127 - mul_c.leading_zeros() == amount
            && amount < w - 1
        {
            let count = c.int(ty, u128::from(amount));
            let shifted = c.create_binop(inst, BinaryOp::AShr, x, count, flags_of(&[(exact, Flags::EXACT)]));
            let new_flags = flags_of(&[(true, Flags::NSW), (mul_flags.contains(Flags::NUW), Flags::NUW)]);
            return c.replace_with_binop(inst, BinaryOp::Add, x, shifted, new_flags);
        }
    }

    if set_shift_flags(c, inst) {
        return Visit::Changed;
    }

    // Prefer `-(x & 1)` over `(x << (bitwidth(x)-1)) a>> (bitwidth(x)-1)` to
    // splat the lowest bit.
    let splat_low_bit = {
        let q = query(c, Some(inst));
        if is_specific_int(&q, op1, u128::from(w - 1)) {
            binop(&q, op0, BinaryOp::Shl)
                .filter(|&(_, _, count, _)| has_one_use(c, op0) && is_specific_int(&q, count, u128::from(w - 1)))
                .map(|(_, x, ..)| x)
        } else {
            None
        }
    };
    if let Some(x) = splat_low_bit {
        let one = c.int(ty, 1);
        let masked = c.create_binop(inst, BinaryOp::And, x, one, Flags::default());
        let zero = c.int(ty, 0);
        return c.replace_with_binop(inst, BinaryOp::Sub, zero, masked, Flags::default());
    }

    if let Some(r) = fold_variable_sign_zero_extension_of_variable_high_bit_extract(c, inst) {
        return r;
    }

    // See if we can turn a signed shr into an unsigned shr.
    let sign_clear = {
        let q = query(c, Some(inst));
        known::masked_value_is_zero(&q, op0, 1u128 << (w - 1), 0)
    };
    if sign_clear {
        return c.replace_with_binop(inst, BinaryOp::LShr, op0, op1, flags_of(&[(exact, Flags::EXACT)]));
    }

    // ashr (xor %x, -1), %y --> xor (ashr %x, %y), -1; `exact` is dropped.
    let not_x = {
        let q = query(c, Some(inst));
        pattern::not(&q, op0).filter(|_| has_one_use(c, op0))
    };
    if let Some(x) = not_x {
        let new_ashr = c.create_binop(inst, BinaryOp::AShr, x, op1, Flags::default());
        let ones = c.all_ones(ty);
        return c.replace_with_binop(inst, BinaryOp::Xor, new_ashr, ones, Flags::default());
    }

    Visit::None
}

#[cfg(test)]
#[path = "combine_shifts_tests.rs"]
mod tests;
