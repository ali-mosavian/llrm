//! InstSimplify of calls: `simplifyCall`, `simplifyIntrinsic`,
//! `simplifyUnaryIntrinsic` and `simplifyBinaryIntrinsic`, integer arms.
//!
//! A translation of LLVM's
//! [`InstructionSimplify.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp)
//! at d1106deb71cc: `simplifyCall`
//! ([:7653](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InstructionSimplify.cpp#L7653)),
//! `simplifyIntrinsic` (:7588 and :7340), `simplifyUnaryIntrinsic` (:6621),
//! `simplifyBinaryIntrinsic` (:6948), `foldMinMaxSharedOp` (:6742). A child of
//! `simplify.rs`, whose `Query` it shares. Each function keeps its C++ order of
//! checks.
//!
//! DEVIATE, permanent (MIR lacks the construct): vector arms (`vector_extract`,
//! `vector_insert`, `vector_splice*`, `get_active_lane_mask`, `vscale`, the
//! interleave identity; #2201), target intrinsics (`pdep`, `pext`, SVE
//! reductions, `load_relative`, `gc.relocate`), constrained FP intrinsics.
//!
//! OWED: the floating-point intrinsics (`fabs`, the idempotent rounding
//! functions, `exp`/`log` pairs, `powi`, `ldexp`, `copysign`, `minnum` and
//! kin, `fma`) read KnownFPClass and FP constants: #2246.
//!
//! Used by `simplify_instruction_with_operands`' call arm.

use super::*;
use crate::valuetracking::pattern::{IntrinsicId, intrinsic_id_of_name};

/// `Intrinsic::{smax,smin,umax,umin}`: `(signed, max)`.
pub(super) fn min_max_kind(id: IntrinsicId) -> Option<(bool, bool)> {
    match id {
        IntrinsicId::Smax => Some((true, true)),
        IntrinsicId::Smin => Some((true, false)),
        IntrinsicId::Umax => Some((false, true)),
        IntrinsicId::Umin => Some((false, false)),
        _ => None,
    }
}

/// `getInverseMinMaxIntrinsic`.
pub(super) fn inverse_min_max(id: IntrinsicId) -> IntrinsicId {
    match id {
        IntrinsicId::Smax => IntrinsicId::Smin,
        IntrinsicId::Smin => IntrinsicId::Smax,
        IntrinsicId::Umax => IntrinsicId::Umin,
        _ => IntrinsicId::Umax,
    }
}

/// `MinMaxIntrinsic::getSaturationPoint(IID, BitWidth)`.
pub(super) fn saturation_point(
    id: IntrinsicId,
    width: u32,
) -> u128 {
    match id {
        IntrinsicId::Smax => mask(width) >> 1,
        IntrinsicId::Smin => 1u128 << (width - 1),
        IntrinsicId::Umax => mask(width),
        _ => 0,
    }
}

/// `ICmpInst::getNonStrictPredicate(MinMaxIntrinsic::getPredicate(IID))`.
fn non_strict_predicate(id: IntrinsicId) -> IntPredicate {
    match id {
        IntrinsicId::Smax => IntPredicate::Sge,
        IntrinsicId::Smin => IntPredicate::Sle,
        IntrinsicId::Umax => IntPredicate::Uge,
        _ => IntPredicate::Ule,
    }
}

/// `MinMaxIntrinsic::getIdentity()`: the value that leaves the other operand.
pub(super) fn min_max_identity(
    id: IntrinsicId,
    width: Option<u32>,
) -> Option<u128> {
    min_max_kind(id)?;
    Some(saturation_point(inverse_min_max(id), width?))
}

/// `ICmpInst::compare(A, B, Pred)` of two integers of `width` bits.
fn compare(
    pred: IntPredicate,
    a: u128,
    b: u128,
    width: u32,
) -> bool {
    let (sa, sb) = (signed(a, width), signed(b, width));
    match pred {
        IntPredicate::Eq => a == b,
        IntPredicate::Ne => a != b,
        IntPredicate::Ugt => a > b,
        IntPredicate::Uge => a >= b,
        IntPredicate::Ult => a < b,
        IntPredicate::Ule => a <= b,
        IntPredicate::Sgt => sa > sb,
        IntPredicate::Sge => sa >= sb,
        IntPredicate::Slt => sa < sb,
        IntPredicate::Sle => sa <= sb,
    }
}

/// `Constant::getNullValue(Ty)` of an integer, a pointer or a struct.
fn null_value(
    q: &mut Query,
    ty: TypeId,
) -> Operand {
    use crate::types::Type;
    let kind = match q.context.types.get(ty) {
        Type::Int(_) => ConstantKind::Int(0),
        Type::Pointer(_) => ConstantKind::Null,
        _ => ConstantKind::Zero,
    };
    Operand::Constant(q.context.constant(Constant { ty, kind }))
}

/// `m_MaxOrMin(m_Value(X), m_Value(Y))`: a call of `smax`, `smin`, `umax` or
/// `umin`: its id and arguments.
pub(super) fn max_or_min(
    q: &Query,
    v: Operand,
) -> Option<(IntrinsicId, Operand, Operand)> {
    let tracking = q.tracking();
    let (inst, _) = tracking.instruction_of(v)?;
    let id = pattern::call_intrinsic(&tracking, inst)?;
    min_max_kind(id)?;
    let args = &q.function.instruction(inst).operands;
    let [x, y, _] = args[..] else { return None };
    Some((id, x, y))
}

/// `foldMinMaxSharedOp`: `min(max(X, Y), X)` and its kin.
fn fold_min_max_shared_op(
    q: &Query,
    id: IntrinsicId,
    op0: Operand,
    op1: Operand,
) -> Option<Operand> {
    let (id0, x, y) = max_or_min(q, op0)?;
    let shared =
        op1 == x || op1 == y || max_or_min(q, op1).is_some_and(|(_, a, b)| (a == x && b == y) || (a == y && b == x));
    if !shared {
        return None;
    }
    if id0 == id {
        return Some(op0);
    }
    if id0 == inverse_min_max(id) {
        return Some(op1);
    }
    None
}

/// `simplifyCall`.
pub(super) fn simplify_call(
    q: &mut Query,
    inst: InstId,
    operands: &[Operand],
) -> Option<Operand> {
    let instruction = q.function.instruction(inst);
    let Opcode::Call(info) = &instruction.opcode else { return None };
    // musttail calls can only be simplified if they are also DCEd.
    if info.tail == crate::opcode::Tail::MustTail {
        return None;
    }
    let ty = instruction.ty;
    let (&callee, args) = operands.split_last()?;
    // call undef -> poison; call null -> poison
    if let Operand::Constant(id) = callee
        && matches!(
            q.context.get(id).kind,
            ConstantKind::Undef | ConstantKind::Poison | ConstantKind::Null
        )
    {
        return Some(q.poison(ty));
    }
    let name = q.tracking().callee_name(inst)?.to_owned();
    if let Some(folded) = try_constant_fold_call(q, inst, &name, args, ty) {
        return Some(folded);
    }
    let id = intrinsic_id_of_name(&name)?;
    simplify_intrinsic(q, inst, id, args, ty)
}

/// `tryConstantFoldCall`.
fn try_constant_fold_call(
    q: &mut Query,
    inst: InstId,
    name: &str,
    args: &[Operand],
    ty: TypeId,
) -> Option<Operand> {
    let Opcode::Call(info) = &q.function.instruction(inst).opcode else { return None };
    let Operand::Constant(callee) = *q.function.instruction(inst).operands.last()? else { return None };
    let ConstantKind::Global(callee) = q.context.get(callee).kind else { return None };
    let function = q.globals.get(callee.0 as usize)?.function()?;
    if !crate::constantfolding::can_constant_fold_call_to(info, function, name) {
        return None;
    }
    let constants = args.iter().map(|&one| q.constant(one)).collect::<Option<Vec<_>>>()?;
    crate::constantfolding::constant_fold_intrinsic(q.context, name, &constants, ty, Some(q.function))
        .map(Operand::Constant)
}

/// `simplifyIntrinsic(CallBase)` and the `simplifyIntrinsic(IID, ...)` it
/// calls.
fn simplify_intrinsic(
    q: &mut Query,
    inst: InstId,
    id: IntrinsicId,
    args: &[Operand],
    ret: TypeId,
) -> Option<Operand> {
    if guarantee::intrinsic_propagates_poison(&q.tracking(), inst) && args.iter().any(|&one| q.is_poison(one)) {
        return Some(q.poison(ret));
    }
    if let Some(constants) = args.iter().map(|&one| q.constant(one)).collect::<Option<Vec<_>>>() {
        let name = q.tracking().callee_name(inst)?.to_owned();
        if let Some(folded) =
            crate::constantfolding::constant_fold_intrinsic(q.context, &name, &constants, ret, Some(q.function))
        {
            return Some(Operand::Constant(folded));
        }
    }
    match args {
        [op0] => simplify_unary_intrinsic(q, id, *op0),
        [op0, op1] => simplify_binary_intrinsic(q, id, ret, *op0, *op1),
        [op0, op1, op2] => simplify_ternary_intrinsic(q, id, ret, *op0, *op1, *op2),
        _ => None,
    }
}

/// `simplifyUnaryIntrinsic`, the integer arms.
fn simplify_unary_intrinsic(
    q: &mut Query,
    id: IntrinsicId,
    op0: Operand,
) -> Option<Operand> {
    match id {
        // bswap(bswap(x)) -> x; bitreverse(bitreverse(x)) -> x
        IntrinsicId::Bswap | IntrinsicId::Bitreverse => {
            let tracking = q.tracking();
            let (_, inner) = pattern::intrinsic_call(
                &tracking,
                op0,
                if id == IntrinsicId::Bswap { "llvm.bswap" } else { "llvm.bitreverse" },
            )?;
            inner.first().copied()
        }
        IntrinsicId::Ctpop => {
            // ctpop(X) -> 1 iff X is non-zero power of 2.
            let ty = q.ty(op0);
            if q.is_power_of_two(op0, false) {
                return Some(q.int(ty, 1));
            }
            // If everything but the lowest bit is zero, that bit is the
            // pop-count. Ex: ctpop(and X, 1) --> and X, 1
            let width = q.width(op0)?;
            if q.masked_value_is_zero(op0, mask(width) & !1) {
                return Some(op0);
            }
            None
        }
        _ => None,
    }
}

/// `simplifyBinaryIntrinsic`, the integer arms.
fn simplify_binary_intrinsic(
    q: &mut Query,
    id: IntrinsicId,
    ret: TypeId,
    mut op0: Operand,
    mut op1: Operand,
) -> Option<Operand> {
    let width = q.context.types.int_bits(ret);
    match id {
        IntrinsicId::Abs => {
            let tracking = q.tracking();
            pattern::intrinsic_call(&tracking, op0, "llvm.abs").map(|_| op0)
        }
        IntrinsicId::Cttz => {
            // cttz(shl(1, X)) = X
            let (a, x, _) = q.binary_of(op0, BinaryOp::Shl).map(|(a, b, f)| (a, b, f))?;
            q.is_one(a).then_some(x)
        }
        IntrinsicId::Ctlz => {
            if let Some((a, x, _)) = q.binary_of(op0, BinaryOp::LShr)
                && is_negative(q, a)
            {
                // ctlz(lshr(-1, X)) = X
                return Some(x);
            }
            if let Some((a, ..)) = q.binary_of(op0, BinaryOp::AShr)
                && is_negative(q, a)
            {
                return Some(null_value(q, ret));
            }
            None
        }
        IntrinsicId::Ptrmask => {
            let ptr_ty = q.ty(op0);
            if q.is_undef(op0) || q.is_zero(op0) {
                return Some(null_value(q, ptr_ty));
            }
            // ptrmask(p, ptrtoint(p)) -> p
            if let Some((CastOp::PtrToInt, source, _)) = q.cast(op1)
                && source == op0
            {
                return Some(op0);
            }
            if q.is_all_ones(op1) || q.is_undef(op1) {
                return Some(op0);
            }
            if let Some((c, mask_width)) = q.number(op1)
                && pattern::imm_constant(&q.tracking(), op1)
            {
                // The bits of the pointer known to be zero do not matter.
                let irrelevant = q.known(op0).zero & mask(mask_width);
                if c | irrelevant == mask(mask_width) {
                    return Some(op0);
                }
            }
            None
        }
        IntrinsicId::Smax | IntrinsicId::Smin | IntrinsicId::Umax | IntrinsicId::Umin => {
            let width = width?;
            if op0 == op1 {
                return Some(op0);
            }
            // Canonicalize immediate constant as Op1.
            if pattern::imm_constant(&q.tracking(), op0) {
                std::mem::swap(&mut op0, &mut op1);
            }
            // Assume undef is the limit value.
            if q.is_undef(op1) {
                return Some(q.int(ret, saturation_point(id, width)));
            }
            let pred = non_strict_predicate(id);
            if let Some((c, _)) = q.number(op1) {
                // The limit value: the result is it. The other limit: the
                // other operand.
                if c == saturation_point(id, width) {
                    return Some(q.int(ret, c));
                }
                if c == saturation_point(inverse_min_max(id), width) {
                    return Some(op0);
                }
                // max (max X, C1), C2 --> max X, C1 if C1 >= C2 (min kin).
                if let Some((id0, m00, m01)) = max_or_min(q, op0)
                    && id0 == id
                    && let Some((inner, _)) = q.number(m00).or_else(|| q.number(m01))
                    && compare(pred, inner, c, width)
                {
                    return Some(op0);
                }
            }
            if let Some(v) = fold_min_max_shared_op(q, id, op0, op1) {
                return Some(v);
            }
            if let Some(v) = fold_min_max_shared_op(q, id, op1, op0) {
                return Some(v);
            }
            // min/max (X, Y) where X is already known <=/>= Y.
            let could_use_undef = std::mem::replace(&mut q.can_use_undef, false);
            let answer = if is_icmp_true(q, pred, op0, op1, RECURSION_LIMIT) {
                Some(op0)
            } else if is_icmp_true(q, pred, op1, op0, RECURSION_LIMIT) {
                Some(op1)
            } else {
                None
            };
            q.can_use_undef = could_use_undef;
            answer
        }
        IntrinsicId::Scmp | IntrinsicId::Ucmp => {
            if is_icmp_true(q, IntPredicate::Eq, op0, op1, RECURSION_LIMIT) {
                return Some(null_value(q, ret));
            }
            let (gt, lt) = if id == IntrinsicId::Scmp {
                (IntPredicate::Sgt, IntPredicate::Slt)
            } else {
                (IntPredicate::Ugt, IntPredicate::Ult)
            };
            if is_icmp_true(q, gt, op0, op1, RECURSION_LIMIT) {
                return Some(q.int(ret, 1));
            }
            if is_icmp_true(q, lt, op0, op1, RECURSION_LIMIT) {
                return Some(q.all_ones(ret));
            }
            None
        }
        IntrinsicId::UsubWithOverflow | IntrinsicId::SsubWithOverflow => {
            (op0 == op1 || q.is_undef(op0) || q.is_undef(op1)).then(|| null_value(q, ret))
        }
        IntrinsicId::UaddWithOverflow | IntrinsicId::SaddWithOverflow => {
            if q.is_undef(op0) || q.is_undef(op1) {
                let crate::types::Type::Struct { fields, .. } = q.context.types.get(ret).clone() else { return None };
                let sum = q.all_ones(fields[0]);
                let overflow = null_value(q, fields[1]);
                let (Operand::Constant(sum), Operand::Constant(overflow)) = (sum, overflow) else { return None };
                return Some(Operand::Constant(
                    q.context.constant(Constant { ty: ret, kind: ConstantKind::Aggregate(vec![sum, overflow]) }),
                ));
            }
            None
        }
        IntrinsicId::UmulWithOverflow | IntrinsicId::SmulWithOverflow => {
            (q.is_zero(op0) || q.is_zero(op1) || q.is_undef(op0) || q.is_undef(op1)).then(|| null_value(q, ret))
        }
        IntrinsicId::UaddSat | IntrinsicId::SaddSat => {
            if id == IntrinsicId::UaddSat && (q.is_all_ones(op0) || q.is_all_ones(op1)) {
                return Some(q.all_ones(ret));
            }
            if q.is_undef(op0) || q.is_undef(op1) {
                return Some(q.all_ones(ret));
            }
            if q.is_zero(op1) {
                return Some(op0);
            }
            if q.is_zero(op0) {
                return Some(op1);
            }
            None
        }
        IntrinsicId::UsubSat | IntrinsicId::SsubSat => {
            if id == IntrinsicId::UsubSat && (q.is_zero(op0) || q.is_all_ones(op1)) {
                return Some(q.null(ret));
            }
            if op0 == op1 || q.is_undef(op0) || q.is_undef(op1) {
                return Some(q.null(ret));
            }
            if q.is_zero(op1) {
                return Some(op0);
            }
            None
        }
        IntrinsicId::IsFpclass => {
            let (mask_bits, _) = q.number(op1)?;
            const ALL_FLAGS: u128 = 0x3ff;
            if mask_bits & ALL_FLAGS == ALL_FLAGS {
                return Some(q.int(ret, 1));
            }
            if mask_bits & ALL_FLAGS == 0 {
                return Some(q.int(ret, 0));
            }
            if q.is_undef(op0) {
                return Some(q.undef(ret));
            }
            None
        }
        _ => None,
    }
}

/// `m_Negative`: a constant integer below zero.
fn is_negative(
    q: &Query,
    operand: Operand,
) -> bool {
    q.number(operand).is_some_and(|(bits, width)| signed(bits, width) < 0)
}

/// `simplifyIntrinsic`'s three-operand arms: `fshl`, `fshr`, `smul.fix`.
fn simplify_ternary_intrinsic(
    q: &mut Query,
    id: IntrinsicId,
    ret: TypeId,
    mut op0: Operand,
    mut op1: Operand,
    op2: Operand,
) -> Option<Operand> {
    match id {
        IntrinsicId::Fshl | IntrinsicId::Fshr => {
            let left = id == IntrinsicId::Fshl;
            let chosen = if left { op0 } else { op1 };
            if q.is_undef(op0) && q.is_undef(op1) {
                return Some(q.undef(ret));
            }
            // The shift amount is undef: the choice of operand is arbitrary.
            if q.is_undef(op2) {
                return Some(chosen);
            }
            if let Some((amount, width)) = q.number(op2) {
                let shift = amount % u128::from(width);
                if shift == 0 {
                    return Some(chosen);
                }
                // fshl (lshr X, C1), (shl X, C2), C1 -> X when C1 + C2 == width
                if let Some((x, c1)) = q.binary_of(op0, BinaryOp::LShr).and_then(|(x, c, _)| Some((x, q.number(c)?.0)))
                    && let Some((y, c2)) =
                        q.binary_of(op1, BinaryOp::Shl).and_then(|(x, c, _)| Some((x, q.number(c)?.0)))
                    && x == y
                    && (c1 + c2) & mask(width) == u128::from(width)
                    && shift == if left { c1 } else { c2 }
                {
                    return Some(x);
                }
            }
            if q.is_zero(op0) && q.is_zero(op1) {
                return Some(q.null(ret));
            }
            if q.is_all_ones(op0) && q.is_all_ones(op1) {
                return Some(q.all_ones(ret));
            }
            None
        }
        IntrinsicId::SmulFix | IntrinsicId::SmulFixSat => {
            // Canonicalize constant operand as Op1.
            if matches!(op0, Operand::Constant(_)) {
                std::mem::swap(&mut op0, &mut op1);
            }
            // X * 0 -> 0; X * undef -> 0
            if q.is_zero(op1) || q.is_undef(op1) {
                return Some(q.null(ret));
            }
            // X * (1 << Scale) -> X
            let width = q.context.types.int_bits(ret)?;
            let (scale, _) = q.number(op2)?;
            let scaled_one = 1u128.checked_shl(u32::try_from(scale).ok()?)?;
            if scale < u128::from(width) && scaled_one >> (width - 1) == 0 && q.number(op1) == Some((scaled_one, width))
            {
                return Some(op0);
            }
            None
        }
        _ => None,
    }
}
