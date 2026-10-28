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
//! `foldICmpTruncWithTruncOrExt`,
//! `foldICmpWithConstant`, `foldICmpUsingBoolRange`,
//! `foldIsMultipleOfAPowerOfTwo`, `foldICmpBinOp`, `foldICmpInstWithConstant`,
//! `foldSignBitTest`, `foldICmpInstWithConstantNotInt`,
//! `foldICmpCommutative`, `foldCmpSelectOfConstants` and the select pair fold,
//! the zero folds `foldIRemByPowerOfTwoToBitTest` and `stripNullTest`, and the
//! `dyn_castNegVal` select fold.
//!
//! OPEN: two shapes the pin's first InstCombine folds and `visit_icmp` leaves,
//! the producing fold unidentified (not `foldICmpAndConstConst`,
//! `foldICmpWithLowBitMaskedVal`, InstSimplify's and-add or
//! `getDemandedBitsLHSMask`; a known-bits or range fold in the icmp visitor is
//! the lead). `(a & -4) == C` folds at C = 0 and signmin only
//! (`a s< -32764` at i16); `((b + M) & M) u< b` is `b != 0` for M = 3, 7.
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
//! `visit_icmp` orders the operands (the more complex first), asks
//! InstSimplify, and then rewrites the compare to a canonical or cheaper one: a
//! non-strict predicate against a constant becomes the strict one (`x <= 5` is
//! `x < 6`), a compare the known bits decide or reduce (`A <u B` where `max(A)
//! == min(B)` is `A != B`) is rewritten.
//!
//! # Example
//! `x u<= 5` is `x u< 6`:
//!
//! ```text
//! %r = icmp ule i16 %x, 5   ; %r = icmp ult i16 %x, 6
//! ```

use llrm_mir::context::{mask, signed};
use llrm_mir::knownbits::KnownBits;
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::{simplify_icmp_operands, simplify_instruction_in};
use llrm_mir::valuetracking::context::is_implied_condition_cmp;
use llrm_mir::valuetracking::nonzero::strip_and_accumulate_constant_offsets;
use llrm_mir::valuetracking::pattern::{CmpPredicate, Side};
use llrm_mir::valuetracking::range::{self, SelectOperand, SelectPatternFlavor};
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_icmp_binop::fold_icmp_bin_op;
use crate::combine_icmp_const::fold_icmp_inst_with_constant;
use crate::combine_shared::{get_complexity, has_one_use, query};

/// `ICmpInst::isSigned`.
fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
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
    if let Some(r) = canonicalize_icmp_bool(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some(r) = canonicalize_cmp_with_constant(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some(r) = canonicalize_icmp_predicate(c, inst, pred) {
        return r;
    }
    // OWED: foldICmpWithConstant, foldICmpUsingBoolRange.
    if let Some(r) = fold_icmp_with_dominating_icmp(c, inst, pred, op0, op1) {
        return r;
    }
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
    if let Some(r) = fold_icmp_bin_op(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some(r) = fold_icmp_inst_with_constant(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some(r) = fold_sign_bit_test(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some(r) = fold_icmp_with_ext_of_ext(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some(r) = fold_icmp_with_cast_op_of_constant(c, inst, pred, op0, op1) {
        return r;
    }
    for operand in [op0, op1] {
        if let Operand::Value(value) = operand
            && let ValueDef::Instruction(phi) = c.function.value(value).def
            && c.function.instruction(phi).opcode == Opcode::Phi
        {
            match c.fold_op_into_phi(inst, phi, false) {
                Visit::None => {}
                found => return found,
            }
        }
    }
    // foldICmpCommutative, its GEP step: both orders.
    if let Some(r) = fold_gep_icmp(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some(r) = fold_gep_icmp(c, inst, pred.swapped(), op1, op0) {
        return r;
    }
    if let Some(r) = fold_select_icmp(c, inst, pred, op0, op1) {
        return r;
    }
    if let Some(r) = fold_select_icmp(c, inst, pred.swapped(), op1, op0) {
        return r;
    }
    // OWED: foldICmpInstWithConstantNotInt, the rest of foldICmpCommutative,
    // matchSymmetricPair, foldCmpSelectOfConstants and the two-select fold.
    if changed { Visit::Changed } else { Visit::None }
}

/// `foldSelectICmp`
/// ([:4497](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L4497)):
/// `icmp (select c, a, b), rhs` is `select c, (icmp a, rhs), (icmp b, rhs)`
/// when an arm's compare simplifies, or the select's condition decides it,
/// and the select has no other user.
///
/// DEVIATE: the "global case" (`replacedSelectWithOperand`, a select with
/// several users) and the `ucmp`/`scmp` arms are not made.
fn fold_select_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    select_operand: Operand,
    rhs: Operand,
) -> Option<Visit> {
    let select = c.definition_of(select_operand).filter(|&def| c.function.instruction(def).opcode == Opcode::Select)?;
    let [condition, on_true, on_false] = c.function.instruction(select).operands[..] else { return None };
    let i1 = c.context.types.int(1);
    // Try to fold the comparison into the select arms, which will cause the
    // select to be converted into a logical and/or.
    let simplify_op = |c: &Combiner, op: Operand, select_cond_is_true: bool| -> Option<Operand> {
        if let Some(found) = simplify_icmp_operands(c.context, c.env.layout, c.function, pred, op, rhs) {
            return Some(found);
        }
        let q = query(c, Some(inst));
        let implied = is_implied_condition_cmp(
            &q,
            condition,
            CmpPredicate::new(pred),
            Side::of(&q, op),
            Side::of(&q, rhs),
            select_cond_is_true,
            0,
        )?;
        Some(c.int(i1, u128::from(implied)))
    };
    let on_true_cmp = simplify_op(c, on_true, true);
    let on_false_cmp = simplify_op(c, on_false, false);
    // We only want to perform this transformation if it will not lead to
    // additional code: both sides fold (the icmp becomes a select that
    // usually simplifies), or this is the select's only user.
    if !(on_true_cmp.is_some() && on_false_cmp.is_some())
        && !(on_true_cmp.or(on_false_cmp).is_some() && has_one_use(c, select_operand))
    {
        return None;
    }
    let on_true_cmp = on_true_cmp.unwrap_or_else(|| c.create_icmp(inst, pred, on_true, rhs));
    let on_false_cmp = on_false_cmp.unwrap_or_else(|| c.create_icmp(inst, pred, on_false, rhs));
    Some(c.replace_with_new(inst, Opcode::Select, i1, vec![condition, on_true_cmp, on_false_cmp], Flags::default()))
}

/// `foldGEPICmp`
/// ([:`foldGEPICmp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)):
/// a compare of two pointers off one base is a compare of the offsets, for an
/// equality or an unsigned predicate on GEPs with a no-wrap flag.
///
/// DEVIATE 1: pointer casts are not looked through (`stripPointerCasts` of the
/// right side, and the arm for two bases that differ only by casts). Exit:
/// `strip_pointer_casts_same_representation` in the query.
/// DEVIATE 2: the null arm and `transformToIndexedCompare` are not made.
fn fold_gep_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    gep_operand: Operand,
    rhs: Operand,
) -> Option<Visit> {
    use crate::combine_gep::GepFlags;
    // Signed compares of GEPs are not index compares: the final add of the
    // base can overflow.
    if pred.is_signed() {
        return None;
    }
    let lhs_gep = c.defining_gep(gep_operand).and_then(|def| c.gep_of(def))?;
    let equality = matches!(pred, IntPredicate::Eq | IntPredicate::Ne);
    // Unsigned predicates can be folded if the GEPs have any nowrap flag.
    let can_fold = |flags: GepFlags| equality || flags != GepFlags::default();
    // Without nuw the offsets can wrap the address space: compare them signed.
    let new_icmp_of = |c: &mut Combiner, flags: GepFlags, a: Operand, b: Operand| {
        if !flags.nuw {
            return new_icmp(c, inst, pred.signed(), a, b);
        }
        let i1 = c.context.types.int(1);
        let same_sign = if flags.nusw { Flags::SAMESIGN } else { Flags::default() };
        c.replace_with_new(inst, Opcode::ICmp(pred), i1, vec![a, b], same_sign)
    };

    let base = c.common_pointer_base(gep_operand, rhs);
    let index_ty = c.index_type(lhs_gep.ty);
    if base.ptr == Some(rhs) && can_fold(base.lhs_flags) && !c.is_expensive(&base) {
        // ((gep Ptr, OFFSET) cmp Ptr) ---> (OFFSET cmp 0)
        let offset = c.emit_gep_offsets(inst, &base.lhs_geps, base.lhs_flags, index_ty);
        let zero = c.int(c.type_of(offset), 0);
        return Some(new_icmp_of(c, base.lhs_flags, offset, zero));
    }

    let rhs_gep = c.defining_gep(rhs).and_then(|def| c.gep_of(def))?;
    let flags = GepFlags::of(lhs_gep.flags).and(GepFlags::of(rhs_gep.flags));
    let same_shape = lhs_gep.indices.len() == rhs_gep.indices.len() && lhs_gep.source == rhs_gep.source;

    if lhs_gep.ptr != rhs_gep.ptr {
        // If all indices are the same, just compare the base pointers.
        if same_shape
            && c.type_of(lhs_gep.ptr) == c.type_of(rhs_gep.ptr)
            && lhs_gep.indices == rhs_gep.indices
            && can_fold(flags)
        {
            let i1 = c.context.types.int(1);
            let operands = vec![lhs_gep.ptr, rhs_gep.ptr];
            return Some(c.replace_with_new(inst, Opcode::ICmp(pred), i1, operands, Flags::default()));
        }
    } else if same_shape {
        // If the GEPs only differ by one index, compare it.
        let mut differing = lhs_gep.indices.iter().zip(&rhs_gep.indices).filter(|(l, r)| l != r);
        let first = differing.next();
        let irreconcilable = first.is_some_and(|(l, r)| c.type_of(*l) != c.type_of(*r));
        match (first, differing.next()) {
            // The same GEP: no comparison is needed.
            (None, _) => {
                let i1 = c.context.types.int(1);
                return Some(Visit::Replace(c.int(i1, u128::from(pred.true_when_equal()))));
            }
            // Note that nowrap flags are always needed when comparing two
            // indices.
            (Some((&l, &r)), None) if !irreconcilable && flags != GepFlags::default() => {
                return Some(new_icmp_of(c, flags, l, r));
            }
            _ => {}
        }
    }

    if let Some(ptr) = base.ptr.filter(|_| !c.is_expensive(&base)) {
        // ((gep Ptr, OFFSET1) cmp (gep Ptr, OFFSET2)  --->  (OFFSET1 cmp
        // OFFSET2)
        let mut fold = can_fold(base.lhs_flags.and(base.rhs_flags));
        if !fold {
            // Without the flags, the offsets still compare when both cross the
            // base's alignment boundary the same number of times: either both
            // arguments wrap, or none does.
            let q = query(c, Some(inst));
            let width = c.env.layout.pointer(0).index_bits;
            let (l_base, l_offset) = strip_and_accumulate_constant_offsets(&q, gep_operand, width, true);
            let (r_base, r_offset) = strip_and_accumulate_constant_offsets(&q, rhs, width, true);
            if l_base == ptr && r_base == ptr {
                let alignment = i128::from(known::pointer_alignment(&q, ptr));
                let floor = |offset: u128| signed(offset, width).div_euclid(alignment);
                fold = floor(l_offset) == floor(r_offset);
            }
        }
        if fold {
            let l = c.emit_gep_offsets(inst, &base.lhs_geps, base.lhs_flags, index_ty);
            let r = c.emit_gep_offsets(inst, &base.rhs_geps, base.rhs_flags, index_ty);
            return Some(new_icmp_of(c, base.lhs_flags.and(base.rhs_flags), l, r));
        }
    }
    None
}

/// `foldSignBitTest`
/// ([:1193](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1193)):
/// `icmp eq/ne (trunc? (X >> (bits-1))), 0` tests the sign of `X`, as does the
/// base of two right shifts that sum to the width less one.
///
/// DEVIATE: no vectors.
fn fold_sign_bit_test(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    use llrm_mir::opcode::CastOp;
    if !matches!(pred, IntPredicate::Eq | IntPredicate::Ne) || constant(c, op1)?.0 != 0 {
        return None;
    }
    let val = c.definition_of(op0)?;
    let x = {
        let q = query(c, Some(inst));
        // m_TruncOrSelf(m_Shr(m_Value(X), m_Constant(C)))
        let shifted = pattern::cast(&q, op0, CastOp::Trunc).map_or(op0, |(_, _, inner)| inner);
        let shr = q.instruction_of(shifted).and_then(|(_, instruction)| match instruction.opcode {
            Opcode::Binary(BinaryOp::LShr | BinaryOp::AShr) => Some((instruction.operands[0], instruction.operands[1])),
            _ => None,
        });
        // `m_Constant(C)`: a shift by a value is the other arm's.
        shr.filter(|&(_, amount)| constant(c, amount).is_some())
    };
    let x = match x {
        Some((x, amount)) => {
            let bits = c.context.types.int_bits(c.type_of(x))?;
            let (shift, _) = constant(c, amount)?;
            if shift != u128::from(bits) - 1 {
                return None;
            }
            x
        }
        None => {
            if !matches!(c.function.instruction(val).opcode, Opcode::Binary(_)) {
                return None;
            }
            crate::combine_shifts::reassociate_shift_amts_of_two_same_direction_shifts(c, val, true)?
        }
    };
    let zero = c.int(c.type_of(x), 0);
    let sign = if pred == IntPredicate::Eq { IntPredicate::Sge } else { IntPredicate::Slt };
    Some(new_icmp(c, inst, sign, x, zero))
}

/// `foldICmpWithZextOrSext`'s extension-against-extension arm
/// ([:6724](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)):
/// `icmp pred (ext X), (ext Y)` is the compare on `X` and `Y`, the narrower
/// one extended to the wider's type when they differ (one extension must have
/// one use). Equality, and a signed compare of two `sext`, keep the predicate;
/// the other cases are unsigned. A `zext nneg` against a `sext` is a `sext`.
///
/// DEVIATE: no vectors.
fn fold_icmp_with_ext_of_ext(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    use llrm_mir::opcode::CastOp;
    let ext_of = |c: &Combiner, op: Operand| {
        [CastOp::ZExt, CastOp::SExt].into_iter().find_map(|kind| {
            pattern::cast(&query(c, Some(inst)), op, kind).map(|(_, instruction, x)| (kind, instruction.flags, x))
        })
    };
    let (kind0, flags0, mut x) = ext_of(c, op0)?;
    let (kind1, flags1, mut y) = ext_of(c, op1)?;
    let mut signed_ext = kind0 == CastOp::SExt;
    if kind0 != kind1 {
        let (x_ty, y_ty) = (c.type_of(x), c.type_of(y));
        if matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
            && c.context.types.int_bits(x_ty) == Some(1)
            && c.context.types.int_bits(y_ty) == Some(1)
        {
            let or = c.create_binop(inst, BinaryOp::Or, x, y, Flags::default());
            let zero = c.int(x_ty, 0);
            return Some(new_icmp(c, inst, pred, or, zero));
        }
        // A `zext nneg` reads as a `sext`; any other mismatch is not folded.
        let nneg = |kind: CastOp, flags: Flags| kind == CastOp::ZExt && flags.contains(Flags::NNEG);
        if nneg(kind0, flags0) || nneg(kind1, flags1) {
            signed_ext = true;
        } else {
            return None;
        }
    }
    let (x_ty, y_ty) = (c.type_of(x), c.type_of(y));
    if x_ty != y_ty {
        if !has_one_use(c, op0) && !has_one_use(c, op1) {
            return None;
        }
        let cast = if signed_ext { CastOp::SExt } else { CastOp::ZExt };
        let (x_bits, y_bits) = (c.context.types.int_bits(x_ty)?, c.context.types.int_bits(y_ty)?);
        if x_bits < y_bits {
            x = c.create_cast(inst, cast, x, y_ty, Flags::default());
        } else if y_bits < x_bits {
            y = c.create_cast(inst, cast, y, x_ty, Flags::default());
        } else {
            return None;
        }
    }
    let signed_pred = matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    );
    let narrowed = if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) || (signed_pred && signed_ext) {
        pred
    } else {
        match pred {
            IntPredicate::Sgt => IntPredicate::Ugt,
            IntPredicate::Sge => IntPredicate::Uge,
            IntPredicate::Slt => IntPredicate::Ult,
            IntPredicate::Sle => IntPredicate::Ule,
            other => other,
        }
    };
    Some(new_icmp(c, inst, narrowed, x, y))
}

/// `foldICmpWithCastOp`'s constant arm
/// ([:6777](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)):
/// `icmp pred (zext|sext X), C`, where `C` is an extension of a narrow
/// constant, is the compare on `X`: always for `eq` and `ne`; for a relational
/// predicate when it has the extension's signedness (`sext` with a signed one,
/// `zext` with an unsigned one), for `zext` with a signed one as the unsigned
/// predicate (the extended value and `C` are both non-negative), and for
/// `sext` with an unsigned one when `C` is non-negative in the narrow type.
///
/// DEVIATE: no vectors; the `trunc` arm is owed.
fn fold_icmp_with_cast_op_of_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    op1: Operand,
) -> Option<Visit> {
    use llrm_mir::opcode::CastOp;
    let (bits, width) = constant(c, op1)?;
    for kind in [CastOp::ZExt, CastOp::SExt] {
        let x = pattern::cast(&query(c, Some(inst)), op0, kind).map(|(_, _, x)| x);
        let Some(x) = x else { continue };
        let narrow = c.type_of(x);
        let Some(narrow_width) = c.context.types.int_bits(narrow) else { continue };
        let low = bits & mask(narrow_width);
        let back = if kind == CastOp::ZExt { low } else { (signed(low, narrow_width) as u128) & mask(width) };
        if back != bits {
            return None;
        }
        let signed_pred = matches!(
            pred,
            IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
        );
        let narrowed = if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) || (kind == CastOp::SExt) == signed_pred {
            pred
        } else if kind == CastOp::ZExt {
            match pred {
                IntPredicate::Sgt => IntPredicate::Ugt,
                IntPredicate::Sge => IntPredicate::Uge,
                IntPredicate::Slt => IntPredicate::Ult,
                _ => IntPredicate::Ule,
            }
        } else if kind == CastOp::SExt && low >> (narrow_width - 1) == 0 {
            // A negative `X` extends to a huge value, as it reads unsigned
            // in the narrow type: the compare against a non-negative constant
            // is the same.
            pred
        } else {
            return None;
        };
        let constant = c.int(narrow, low);
        return Some(new_icmp(c, inst, narrowed, x, constant));
    }
    None
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

/// `canonicalizeICmpBool`
/// ([:7513](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L7513)):
/// a compare of two `i1` is bitwise logic. Against a constant, InstSimplify
/// took 14 of the 20 cases; the rest are the `not` of the other operand.
fn canonicalize_icmp_bool(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    a: Operand,
    b: Operand,
) -> Option<Visit> {
    let i1 = c.context.types.int(1);
    if c.type_of(a) != i1 {
        return None;
    }
    let none = Flags::default();
    let ones = c.all_ones(i1);
    match constant(c, b).map(|(bits, _)| bits) {
        Some(0) => {
            return matches!(pred, IntPredicate::Eq | IntPredicate::Ule | IntPredicate::Sge)
                .then(|| c.replace_with_binop(inst, BinaryOp::Xor, a, ones, none));
        }
        Some(1) => {
            return matches!(pred, IntPredicate::Ne | IntPredicate::Ult | IntPredicate::Sgt)
                .then(|| c.replace_with_binop(inst, BinaryOp::Xor, a, ones, none));
        }
        _ => {}
    }
    Some(match pred {
        // icmp eq i1 A, B -> ~(A ^ B)
        IntPredicate::Eq => {
            let xor = c.create_binop(inst, BinaryOp::Xor, a, b, none);
            c.replace_with_binop(inst, BinaryOp::Xor, xor, ones, none)
        }
        // icmp ne i1 A, B -> A ^ B
        IntPredicate::Ne => c.replace_with_binop(inst, BinaryOp::Xor, a, b, none),
        // icmp ult i1 A, B -> ~A & B; ugt is ult swapped
        IntPredicate::Ult | IntPredicate::Ugt => {
            let (a, b) = if pred == IntPredicate::Ugt { (b, a) } else { (a, b) };
            let not = c.create_not(inst, a);
            c.replace_with_binop(inst, BinaryOp::And, not, b, none)
        }
        // icmp slt i1 A, B -> A & ~B; sgt is slt swapped
        IntPredicate::Slt | IntPredicate::Sgt => {
            let (a, b) = if pred == IntPredicate::Sgt { (b, a) } else { (a, b) };
            let not = c.create_not(inst, b);
            c.replace_with_binop(inst, BinaryOp::And, a, not, none)
        }
        // icmp ule i1 A, B -> ~A | B; uge is ule swapped
        IntPredicate::Ule | IntPredicate::Uge => {
            let (a, b) = if pred == IntPredicate::Uge { (b, a) } else { (a, b) };
            let not = c.create_not(inst, a);
            c.replace_with_binop(inst, BinaryOp::Or, not, b, none)
        }
        // icmp sle i1 A, B -> A | ~B; sge is sle swapped
        IntPredicate::Sle | IntPredicate::Sge => {
            let (a, b) = if pred == IntPredicate::Sge { (b, a) } else { (a, b) };
            let not = c.create_not(inst, b);
            c.replace_with_binop(inst, BinaryOp::Or, a, not, none)
        }
    })
}

/// `canonicalizeICmpPredicate`
/// ([:7448](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L7448)):
/// a compare with a non-canonical predicate (`ne`, `ule`, `sle`, `uge`, `sge`)
/// becomes the inverse one, if every user can take the inversion: a branch
/// swaps its successors, a select its values, a `not` is dropped
/// (`canFreelyInvertAllUsersOf`, `freelyInvertAllUsersOf`).
fn canonicalize_icmp_predicate(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
) -> Option<Visit> {
    if is_canonical_predicate(pred) {
        return None;
    }
    let result = c.function.instruction(inst).result?;
    let users: Vec<(InstId, u32)> = c.function.users(result).iter().map(|one| (one.user, one.index)).collect();
    for &(user, index) in &users {
        let one = c.function.instruction(user);
        let ok = match &one.opcode {
            // Only as the condition, and not a logical and/or: absorbing the
            // not would hide that pattern.
            Opcode::Select => {
                index == 0 && {
                    let q = query(c, None);
                    let value = Operand::Value(one.result?);
                    pattern::logical_and(&q, value).is_none() && pattern::logical_or(&q, value).is_none()
                }
            }
            Opcode::Br => one.operands.len() == 3,
            Opcode::Binary(BinaryOp::Xor) => pattern::not(&query(c, None), Operand::Value(one.result?)).is_some(),
            _ => false,
        };
        if !ok {
            return None;
        }
    }
    c.function.set_icmp_predicate(inst, pred.inverse());
    for (user, _) in users {
        match c.function.instruction(user).opcode {
            Opcode::Select | Opcode::Br => {
                let mut operands = c.function.instruction(user).operands.clone();
                operands.swap(1, 2);
                c.function.set_operands(user, operands);
                c.add_to_worklist(user);
            }
            _ => {
                c.replace_inst_with(user, Operand::Value(result));
                c.add_to_worklist(user);
            }
        }
    }
    Some(Visit::Changed)
}

/// `foldICmpWithDominatingICmp`
/// ([:1393](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1393)):
/// `icmp pred x, C` in a block a branch on `icmp domPred x, DomC` dominates:
/// the two ranges settle it, or leave one value, and it is an equality with it.
fn fold_icmp_with_dominating_icmp(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    x: Operand,
    y: Operand,
) -> Option<Visit> {
    use llrm_mir::constantrange::{ConstantRange, PreferredRangeType};
    // The simple implications were InstSimplify's; only a constant is asked.
    let (value, width) = constant(c, y)?;
    let cr = ConstantRange::make_exact_icmp_region(pred, width, value);
    let cmp_block = c.function.parent(inst)?;
    let branches: Vec<InstId> = c.env.analyses.dom_conditions()?.conditions_for(x).to_vec();
    for branch in branches {
        let &[Operand::Value(cond), Operand::Block(on_true), Operand::Block(on_false)] =
            &c.function.instruction(branch).operands[..]
        else {
            continue;
        };
        let llrm_mir::module::ValueDef::Instruction(dominating) = c.function.value(cond).def else { continue };
        let Opcode::ICmp(dom_pred) = c.function.instruction(dominating).opcode else { continue };
        let &[dom_x, dom_c] = &c.function.instruction(dominating).operands[..] else { continue };
        let Some((dom_value, _)) = constant(c, dom_c) else { continue };
        if dom_x != x {
            continue;
        }
        let Some(branch_block) = c.function.parent(branch) else { continue };
        let tree = c.env.analyses.dominator_tree();
        let on_edge = if tree.dominates_edge(c.function, branch_block, on_true, cmp_block) {
            dom_pred
        } else if tree.dominates_edge(c.function, branch_block, on_false, cmp_block) {
            dom_pred.inverse()
        } else {
            continue;
        };
        let dominating_cr = ConstantRange::make_exact_icmp_region(on_edge, width, dom_value);
        let intersection = dominating_cr.intersect_with(&cr, PreferredRangeType::Smallest);
        let difference = dominating_cr.difference(&cr);
        let i1 = c.context.types.int(1);
        if intersection.is_empty_set() {
            return Some(Visit::Replace(c.int(i1, 0)));
        }
        if difference.is_empty_set() {
            return Some(Visit::Replace(c.int(i1, 1)));
        }
        // Canonicalizing a sign bit comparison that a branch uses pessimizes
        // codegen: a branch on zero instead of a test and branch.
        let sign_bit = llrm_mir::valuetracking::fp::is_sign_bit_check(pred, value, width).is_some();
        let branch_use = {
            let result = c.function.instruction(inst).result?;
            c.function
                .users(result)
                .iter()
                .any(
                    |one| {
                        let user = c.function.instruction(one.user);
                        user.opcode == Opcode::Br && user.operands.len() == 3
                    },
                )
        };
        if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) || (sign_bit && branch_use) {
            continue;
        }
        // Avoid an infinite loop with min/max canonicalization.
        if let Some(user) = only_user(c, inst) {
            let q = query(c, None);
            if matches!(
                pattern::call_intrinsic(&q, user),
                Some(
                    pattern::IntrinsicId::Smax
                        | pattern::IntrinsicId::Smin
                        | pattern::IntrinsicId::Umax
                        | pattern::IntrinsicId::Umin
                )
            ) {
                continue;
            }
        }
        let ty = c.type_of(x);
        if let Some(only) = intersection.get_single_element() {
            let only = c.int(ty, only);
            return Some(new_icmp(c, inst, IntPredicate::Eq, x, only));
        }
        if let Some(missing) = difference.get_single_element() {
            let missing = c.int(ty, missing);
            return Some(new_icmp(c, inst, IntPredicate::Ne, x, missing));
        }
    }
    None
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

/// The bits `a pred b` compares: `ICmpInst::compare(KnownBits, KnownBits,
/// Pred)`.
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
                    // X <u C --> X == 0, if the number of zero bits in the
                    // bottom of X exceeds the log2 of C.
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
                    // X >u C --> X != 0, if the number of zero bits in the
                    // bottom of X exceeds the log2 of C.
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
            // If all bits are known zero except for one, then we know at most
            // one bit is set. If the comparison is against zero,
            // then this is a check to see if *that* bit is set.
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
                        // ((C1 << X) & C2) == 0 -> X >= (Log2(C2+C1) -
                        // Log2(C1)) ((C1 << X) & C2) !=
                        // 0 -> X  < (Log2(C2+C1) - Log2(C1))
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
            let call = q
                .instruction_of(op0)
                .and_then(
                    |(call, _)| (pattern::call_intrinsic(&q, call) == Some(pattern::IntrinsicId::Smin)).then(|| {
                        let operands = &c.function.instruction(call).operands;
                        (operands[0], operands[1])
                    }),
                );
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
    // Given: icmp eq/ne (urem %x, %y), 0. Iff %x has 0 or 1 bits set, and %y
    // has at least 2 bits set, omit 'urem': icmp eq/ne %x, 0
    if let Some((x, y, _)) = binary_of(c, op0, BinaryOp::URem) {
        let (xk, yk) = {
            let q = query(c, Some(inst));
            (known::compute_known_bits(&q, x, 0), known::compute_known_bits(&q, y, 0))
        };
        if xk.count_max_population() == 1 && yk.count_min_population() >= 2 {
            return Some(new_icmp(c, inst, pred, x, op1));
        }
    }
    // (icmp eq/ne (mul X Y)) -> (icmp eq/ne X/Y) if we know about whether X/Y
    // are odd/non-zero/there is no overflow.
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
