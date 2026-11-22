//! The vector costs of `AArch64TTIImpl` beyond memory and integer arithmetic:
//! casts, compares and selects, and the scalarization overhead a replicated
//! access or an expanded select pays. Each is the pin's (d1106deb71cc) for the
//! fixed NEON types the port prices (power-of-two lane counts, one register or
//! whole registers: DEVIATE VT1); a type or a case the pin prices some other
//! way is `None`, not a guess.

use llrm_mir::intrinsics::ReduceKind;
use llrm_mir::opcode::FloatPredicate;
use llrm_mir::tti::{
    ArithOp, CastKind, CmpSel, Cost, CostKind, ExtOperand, ExtUser, LaneOp, Predicate, Ty, VectorInstrContext, VectorTy,
};

use crate::tti::{vector_arithmetic_cost, vector_pieces};

/// `ST->getVectorInsertExtractBaseCost()`, 2 on both cpus.
pub(crate) const INSERT_EXTRACT_BASE_COST: i64 = 2;

const fn v(
    lane: Ty,
    count: u32,
) -> VectorTy {
    VectorTy { lane, count }
}

/// `ConversionTbl` of `AArch64TTIImpl::getCastInstrCost`
/// ([L4036](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L4036)):
/// the fixed-vector rows, destination before source, without the half-precision
/// ones (no `Ty` for them).
const CONVERSIONS: &[(CastKind, VectorTy, VectorTy, i64)] = &[
    (CastKind::Trunc, v(Ty::Int(8), 2), v(Ty::Int(64), 2), 1), // xtn
    (CastKind::Trunc, v(Ty::Int(16), 2), v(Ty::Int(64), 2), 1), // xtn
    (CastKind::Trunc, v(Ty::Int(32), 2), v(Ty::Int(64), 2), 1), // xtn
    (CastKind::Trunc, v(Ty::Int(8), 4), v(Ty::Int(32), 4), 1), // xtn
    (CastKind::Trunc, v(Ty::Int(8), 4), v(Ty::Int(64), 4), 3), // 2 xtn + 1 uzp1
    (CastKind::Trunc, v(Ty::Int(16), 4), v(Ty::Int(32), 4), 1), // xtn
    // 1 uzp1 + 1 xtn
    (CastKind::Trunc, v(Ty::Int(16), 4), v(Ty::Int(64), 4), 2),
    (CastKind::Trunc, v(Ty::Int(32), 4), v(Ty::Int(64), 4), 1), // 1 uzp1
    (CastKind::Trunc, v(Ty::Int(8), 8), v(Ty::Int(16), 8), 1),  // 1 xtn
    // 1 uzp1 + 1 xtn
    (CastKind::Trunc, v(Ty::Int(8), 8), v(Ty::Int(32), 8), 2),
    // 3 x uzp1 + xtn
    (CastKind::Trunc, v(Ty::Int(8), 8), v(Ty::Int(64), 8), 4),
    (CastKind::Trunc, v(Ty::Int(16), 8), v(Ty::Int(32), 8), 1), // 1 uzp1
    (CastKind::Trunc, v(Ty::Int(16), 8), v(Ty::Int(64), 8), 3), // 3 x uzp1
    (CastKind::Trunc, v(Ty::Int(32), 8), v(Ty::Int(64), 8), 2), // 2 x uzp1
    (CastKind::Trunc, v(Ty::Int(8), 16), v(Ty::Int(16), 16), 1), // uzp1
    // (2 + 1) x uzp1
    (CastKind::Trunc, v(Ty::Int(8), 16), v(Ty::Int(32), 16), 3),
    // (4 + 2 + 1) x uzp1
    (CastKind::Trunc, v(Ty::Int(8), 16), v(Ty::Int(64), 16), 7),
    (CastKind::Trunc, v(Ty::Int(16), 16), v(Ty::Int(32), 16), 2), // 2 x uzp1
    // (4 + 2) x uzp1
    (CastKind::Trunc, v(Ty::Int(16), 16), v(Ty::Int(64), 16), 6),
    (CastKind::Trunc, v(Ty::Int(32), 16), v(Ty::Int(64), 16), 4), // 4 x uzp1
    (CastKind::SExt, v(Ty::Int(64), 4), v(Ty::Int(16), 4), 3),
    (CastKind::ZExt, v(Ty::Int(64), 4), v(Ty::Int(16), 4), 3),
    (CastKind::SExt, v(Ty::Int(64), 4), v(Ty::Int(32), 4), 2),
    (CastKind::ZExt, v(Ty::Int(64), 4), v(Ty::Int(32), 4), 2),
    (CastKind::SExt, v(Ty::Int(32), 8), v(Ty::Int(8), 8), 3),
    (CastKind::ZExt, v(Ty::Int(32), 8), v(Ty::Int(8), 8), 3),
    (CastKind::SExt, v(Ty::Int(32), 8), v(Ty::Int(16), 8), 2),
    (CastKind::ZExt, v(Ty::Int(32), 8), v(Ty::Int(16), 8), 2),
    (CastKind::SExt, v(Ty::Int(64), 8), v(Ty::Int(8), 8), 7),
    (CastKind::ZExt, v(Ty::Int(64), 8), v(Ty::Int(8), 8), 7),
    (CastKind::SExt, v(Ty::Int(64), 8), v(Ty::Int(16), 8), 6),
    (CastKind::ZExt, v(Ty::Int(64), 8), v(Ty::Int(16), 8), 6),
    (CastKind::SExt, v(Ty::Int(16), 16), v(Ty::Int(8), 16), 2),
    (CastKind::ZExt, v(Ty::Int(16), 16), v(Ty::Int(8), 16), 2),
    (CastKind::SExt, v(Ty::Int(32), 16), v(Ty::Int(8), 16), 6),
    (CastKind::ZExt, v(Ty::Int(32), 16), v(Ty::Int(8), 16), 6),
    (CastKind::FPExt, v(Ty::Double, 2), v(Ty::Float, 2), 1), // fcvtl
    (CastKind::FPExt, v(Ty::Double, 4), v(Ty::Float, 4), 2), // fcvtl+fcvtl2
    (CastKind::FPTrunc, v(Ty::Float, 2), v(Ty::Double, 2), 1), // fcvtn
    (CastKind::FPTrunc, v(Ty::Float, 4), v(Ty::Double, 4), 2), // fcvtn+fcvtn2
    (CastKind::SIToFP, v(Ty::Float, 2), v(Ty::Int(32), 2), 1),
    (CastKind::SIToFP, v(Ty::Float, 4), v(Ty::Int(32), 4), 1),
    (CastKind::SIToFP, v(Ty::Double, 2), v(Ty::Int(64), 2), 1),
    (CastKind::UIToFP, v(Ty::Float, 2), v(Ty::Int(32), 2), 1),
    (CastKind::UIToFP, v(Ty::Float, 4), v(Ty::Int(32), 4), 1),
    (CastKind::UIToFP, v(Ty::Double, 2), v(Ty::Int(64), 2), 1),
    (CastKind::SIToFP, v(Ty::Float, 2), v(Ty::Int(8), 2), 3),
    (CastKind::SIToFP, v(Ty::Float, 2), v(Ty::Int(16), 2), 3),
    (CastKind::UIToFP, v(Ty::Float, 2), v(Ty::Int(8), 2), 3),
    (CastKind::UIToFP, v(Ty::Float, 2), v(Ty::Int(16), 2), 3),
    (CastKind::SIToFP, v(Ty::Float, 4), v(Ty::Int(8), 4), 4),
    (CastKind::SIToFP, v(Ty::Float, 4), v(Ty::Int(16), 4), 2),
    (CastKind::UIToFP, v(Ty::Float, 4), v(Ty::Int(8), 4), 3),
    (CastKind::UIToFP, v(Ty::Float, 4), v(Ty::Int(16), 4), 2),
    (CastKind::SIToFP, v(Ty::Float, 8), v(Ty::Int(8), 8), 10),
    (CastKind::SIToFP, v(Ty::Float, 8), v(Ty::Int(16), 8), 4),
    (CastKind::UIToFP, v(Ty::Float, 8), v(Ty::Int(8), 8), 10),
    (CastKind::UIToFP, v(Ty::Float, 8), v(Ty::Int(16), 8), 4),
    (CastKind::SIToFP, v(Ty::Float, 16), v(Ty::Int(8), 16), 21),
    (CastKind::UIToFP, v(Ty::Float, 16), v(Ty::Int(8), 16), 21),
    (CastKind::SIToFP, v(Ty::Double, 2), v(Ty::Int(8), 2), 4),
    (CastKind::SIToFP, v(Ty::Double, 2), v(Ty::Int(16), 2), 4),
    (CastKind::SIToFP, v(Ty::Double, 2), v(Ty::Int(32), 2), 2),
    (CastKind::UIToFP, v(Ty::Double, 2), v(Ty::Int(8), 2), 4),
    (CastKind::UIToFP, v(Ty::Double, 2), v(Ty::Int(16), 2), 4),
    (CastKind::UIToFP, v(Ty::Double, 2), v(Ty::Int(32), 2), 2),
    (CastKind::SIToFP, v(Ty::Double, 4), v(Ty::Int(32), 4), 4),
    (CastKind::UIToFP, v(Ty::Double, 4), v(Ty::Int(32), 4), 4),
    (CastKind::FPToSI, v(Ty::Int(32), 2), v(Ty::Float, 2), 1),
    (CastKind::FPToSI, v(Ty::Int(32), 4), v(Ty::Float, 4), 1),
    (CastKind::FPToSI, v(Ty::Int(64), 2), v(Ty::Double, 2), 1),
    (CastKind::FPToUI, v(Ty::Int(32), 2), v(Ty::Float, 2), 1),
    (CastKind::FPToUI, v(Ty::Int(32), 4), v(Ty::Float, 4), 1),
    (CastKind::FPToUI, v(Ty::Int(64), 2), v(Ty::Double, 2), 1),
    (CastKind::FPToSI, v(Ty::Int(64), 2), v(Ty::Float, 2), 2),
    (CastKind::FPToSI, v(Ty::Int(16), 2), v(Ty::Float, 2), 1),
    (CastKind::FPToSI, v(Ty::Int(8), 2), v(Ty::Float, 2), 1),
    (CastKind::FPToUI, v(Ty::Int(64), 2), v(Ty::Float, 2), 2),
    (CastKind::FPToUI, v(Ty::Int(16), 2), v(Ty::Float, 2), 1),
    (CastKind::FPToUI, v(Ty::Int(8), 2), v(Ty::Float, 2), 1),
    (CastKind::FPToSI, v(Ty::Int(16), 4), v(Ty::Float, 4), 2),
    (CastKind::FPToSI, v(Ty::Int(8), 4), v(Ty::Float, 4), 2),
    (CastKind::FPToUI, v(Ty::Int(16), 4), v(Ty::Float, 4), 2),
    (CastKind::FPToUI, v(Ty::Int(8), 4), v(Ty::Float, 4), 2),
    (CastKind::FPToSI, v(Ty::Int(32), 2), v(Ty::Double, 2), 2),
    (CastKind::FPToSI, v(Ty::Int(16), 2), v(Ty::Double, 2), 2),
    (CastKind::FPToSI, v(Ty::Int(8), 2), v(Ty::Double, 2), 2),
    (CastKind::FPToUI, v(Ty::Int(32), 2), v(Ty::Double, 2), 2),
    (CastKind::FPToUI, v(Ty::Int(16), 2), v(Ty::Double, 2), 2),
    (CastKind::FPToUI, v(Ty::Int(8), 2), v(Ty::Double, 2), 2),
];

/// `AArch64TTIImpl::getExtendedReductionCost`
/// ([L6682](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L6682)):
/// an add reduction of an extension the target does natively (`uaddlv` of 8,
/// 16 or 32 bits to at most 32; `uaddlp` of 32 to 64) costs 2 for each register
/// but the first and 2 more. The rest is `BaseT`'s: `None`.
pub(crate) fn extended_reduction_cost(
    kind: ReduceKind,
    _unsigned: bool,
    result: Ty,
    vec: VectorTy,
    cost_kind: CostKind,
) -> Option<Cost> {
    if cost_kind != CostKind::RecipThroughput || kind != ReduceKind::Add {
        return None;
    }
    let (first, lane) = vector_pieces(vec)?;
    let result_bits = lane_bits(result)?;
    // `VecVT.getSizeInBits() >= 64`; the legal type is one register's lanes.
    if u64::from(lane) * u64::from(vec.count) < 64 {
        return None;
    }
    let native = match lane {
        8 | 16 => result_bits <= 32,
        32 => result_bits <= 64,
        _ => false,
    };
    native.then(|| Cost::of((first - 1) * 2 + 2))
}

/// `BasicTTIImplBase::getCastInstrCost`, a cast between vectors a legalization
/// splits (wider than a register): the cast of the halves twice, and the split
/// itself (1) unless both types split.
fn split_cast_cost(
    kind: CastKind,
    dst: VectorTy,
    src: VectorTy,
    cost_kind: CostKind,
) -> Option<Cost> {
    let (dst_pieces, _) = vector_pieces(dst)?;
    let (src_pieces, _) = vector_pieces(src)?;
    let (split_dst, split_src) = (dst_pieces > 1, src_pieces > 1);
    if (!split_dst && !split_src) || dst.count != src.count || dst.count < 2 {
        return None;
    }
    let half = |ty: VectorTy| VectorTy { lane: ty.lane, count: ty.count / 2 };
    let piece = cast_cost(kind, half(dst), half(src), cost_kind)?;
    let split = if split_dst && split_src { 0 } else { 1 };
    Some(Cost::of(split + 2 * piece.value))
}

/// `getCastInstrCost` for fixed vectors, throughput: a row of the conversion
/// table, or a bitcast between types of the same registers
/// (`BasicTTIImpl::getCastInstrCost`: free).
pub(crate) fn cast_cost(
    kind: CastKind,
    dst: VectorTy,
    src: VectorTy,
    cost_kind: CostKind,
) -> Option<Cost> {
    if cost_kind == CostKind::Latency {
        return None;
    }
    if let Some(&(.., cost)) = CONVERSIONS.iter().find(|&&(one, to, from, _)| one == kind && to == dst && from == src) {
        return Some(Cost::of(cost));
    }
    // "INT_TO_FP of i64->f32 will scalarize, which is required to avoid
    // double-rounding issues": one scalar conversion (a legal operation: 1)
    // per lane, the lanes taken out of the source and put into the result
    // ([L4545](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L4545)).
    if matches!(kind, CastKind::SIToFP | CastKind::UIToFP)
        && dst.lane == Ty::Float
        && matches!(src.lane, Ty::Int(bits) if bits > 32)
    {
        let taken = scalarization_overhead(src, false, true, cost_kind)?;
        let put = scalarization_overhead(dst, true, false, cost_kind)?;
        return Some(Cost::of(i64::from(dst.count) + taken.value + put.value));
    }
    // `BasicTTIImplBase::getCastInstrCost`: a conversion between integer and
    // float lanes of one width, wider than the table's rows, is its halves'
    // (both types split, so the split is free).
    if matches!(
        kind,
        CastKind::SIToFP | CastKind::UIToFP | CastKind::FPToSI | CastKind::FPToUI
    )
        && vector_pieces(dst).zip(vector_pieces(src)).is_some_and(|((_, to), (_, from))| to == from)
    {
        return ext_cast_cost(kind, dst, src);
    }
    if matches!(kind, CastKind::SExt | CastKind::ZExt | CastKind::Trunc) {
        if let Some(cost) = split_cast_cost(kind, dst, src, cost_kind) {
            return Some(cost);
        }
    }
    if kind == CastKind::BitCast {
        let (to, from) = (vector_pieces(dst)?, vector_pieces(src)?);
        let bits = |ty: VectorTy, (_, lane): (i64, u32)| u64::from(lane) * u64::from(ty.count);
        if to.0 == from.0 && bits(dst, to) == bits(src, from) {
            return Some(Cost::FREE);
        }
    }
    None
}

fn lane_bits(ty: Ty) -> Option<u32> {
    match ty {
        Ty::Int(bits) => Some(bits),
        _ => None,
    }
}

/// `isBinExtWideningInstruction`
/// ([L3771](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L3771)):
/// the vector type an `add`, `sub` or `mul` of two extensions is done in
/// (`smull`, `uaddl`, ...); none if it is not one, or is of at most 64 bits.
/// `Err`: a `mul` of a `zext` and a value whose known bits decide (`umull`
/// if they fit half the lane), which this port does not ask (DEVIATE ALU2).
fn bin_ext_widening(
    op: ArithOp,
    dst: VectorTy,
    args: [Option<ExtOperand>; 2],
    src_override: Option<Ty>,
) -> Result<Option<VectorTy>, ()> {
    if !matches!(op, ArithOp::Add | ArithOp::Sub | ArithOp::Mul) {
        return Ok(None);
    }
    let Some(dst_bits) = lane_bits(dst.lane).filter(|bits| [16, 32, 64].contains(bits)) else { return Ok(None) };
    let size = |one: ExtOperand| lane_bits(src_override.unwrap_or(one.src)).unwrap_or(u32::MAX);
    let max = match args {
        [Some(a), Some(b)] if a.kind == b.kind => size(a).max(size(b)),
        // "mul(sext, zext) will become smull(sext, zext) if the extends are
        // large enough."
        [Some(a), Some(b)] => {
            if size(a) >= dst_bits / 2 || size(b) >= dst_bits / 2 {
                return Ok(None);
            }
            dst_bits / 2
        }
        [Some(one), None] | [None, Some(one)] if op == ArithOp::Mul && one.kind == CastKind::ZExt => return Err(()),
        _ => return Ok(None),
    };
    if max * 2 > dst_bits || u64::from(max) * 2 * u64::from(dst.count) <= 64 {
        return Ok(None);
    }
    Ok(Some(VectorTy { lane: Ty::Int(max * 2), count: dst.count }))
}

/// `isSingleExtWideningInstruction` (L3698): an `add` of an extension, or a
/// `sub` of one as the second operand, with the lane twice the source's
/// (`saddw`, `usubw`).
fn single_ext_widening(
    op: ArithOp,
    dst: VectorTy,
    args: [Option<ExtOperand>; 2],
    src_override: Option<Ty>,
) -> bool {
    let Some(dst_bits) = lane_bits(dst.lane).filter(|bits| [16, 32, 64].contains(bits)) else { return false };
    let extended = match (op, args) {
        (ArithOp::Add | ArithOp::Sub, [_, Some(one)]) => one,
        (ArithOp::Add, [Some(one), None]) => one,
        _ => return false,
    };
    // The legalized types have as many elements: both are whole NEON
    // registers of the same count.
    lane_bits(src_override.unwrap_or(extended.src)) == Some(dst_bits / 2) && vector_pieces(dst).is_some()
}

/// `getArithmeticInstrCost` of a vector `add`, `sub` or `mul` whose operands
/// may be extensions: the pin's widening forms (L4973) before the plain cost.
pub(crate) fn arithmetic_cost_ext(
    op: ArithOp,
    ty: VectorTy,
    args: [Option<ExtOperand>; 2],
    plain: impl Fn(ArithOp, VectorTy) -> Option<Cost>,
) -> Result<Option<Cost>, ()> {
    let Some(narrow) = bin_ext_widening(op, ty, args, None)? else { return Ok(plain(op, ty)) };
    if narrow != ty {
        // The operation on the narrowest type, and a further extension.
        let (Some(done), Some(extend)) = (plain(op, narrow), ext_cast_cost(CastKind::ZExt, ty, narrow)) else {
            return Ok(None);
        };
        return Ok(Some(done + extend));
    }
    Ok(vector_pieces(ty).map(|(registers, _)| Cost::of(registers)))
}

/// An extension between fixed vectors, no user looked at: a row of the
/// conversion table, a legal one (one `shll` a register), or the split in
/// halves of `BasicTTIImplBase::getCastInstrCost` (L1379).
fn ext_cast_cost(
    kind: CastKind,
    dst: VectorTy,
    src: VectorTy,
) -> Option<Cost> {
    if let Some(&(.., cost)) = CONVERSIONS.iter().find(|&&(one, to, from, _)| one == kind && to == dst && from == src) {
        return Some(Cost::of(cost));
    }
    // A vector of `i1` (a mask) is promoted to bytes: one register to sixteen
    // lanes, a register more per sixteen after.
    let src_pieces = if src.lane == Ty::Int(1) { (i64::from((src.count / 16).max(1)), 1) } else { vector_pieces(src)? };
    let ((dst_registers, _), (src_registers, _)) = (vector_pieces(dst)?, src_pieces);
    if dst_registers == src_registers {
        return Some(Cost::of(src_registers));
    }
    // "If both types need to be split then the split is free."
    let split = i64::from(!(dst_registers > 1 && src_registers > 1));
    let half = |ty: VectorTy| VectorTy { lane: ty.lane, count: ty.count / 2 };
    if dst.count % 2 != 0 {
        return None;
    }
    Some(Cost::of(split) + ext_cast_cost(kind, half(dst), half(src))? * 2)
}

/// `getCastInstrCost` of a `sext` or `zext` with its users
/// (`GetUserAbsorbedCastCost`, L3893): free where every user absorbs it into
/// a widening instruction.
pub(crate) fn ext_cost(
    kind: CastKind,
    dst: VectorTy,
    src: VectorTy,
    users: &[ExtUser],
) -> Option<Cost> {
    if !users.is_empty() {
        let mut most = Cost::FREE;
        let mut all = true;
        for user in users {
            match absorbed_cost(kind, dst, src, user) {
                Ok(Some(cost)) => most = if cost.value > most.value { cost } else { most },
                Ok(None) => {
                    all = false;
                    break;
                }
                Err(()) => return None,
            }
        }
        if all {
            return Some(most);
        }
    }
    ext_cast_cost(kind, dst, src)
}

fn absorbed_cost(
    kind: CastKind,
    dst: VectorTy,
    src: VectorTy,
    user: &ExtUser,
) -> Result<Option<Cost>, ()> {
    let Some(op) = user.op else { return Ok(None) };
    if let Some(wide) = bin_ext_widening(op, dst, user.operands, None)? {
        // "The cost from Src->Src*2 needs to be added if required, the cost
        // from Src*2->ExtTy is free."
        let (wide_bits, src_bits) = (lane_bits(wide.lane).ok_or(())?, lane_bits(src.lane).ok_or(())?);
        if wide_bits > src_bits * 2 {
            let double = VectorTy { lane: Ty::Int(src_bits * 2), count: src.count };
            return ext_cast_cost(kind, double, src).map(Some).ok_or(());
        }
        return Ok(Some(Cost::FREE));
    }
    if single_ext_widening(op, dst, user.operands, None) {
        // "For adds only count the second operand as free if both operands are
        // extends but not the same operation."
        if op != ArithOp::Add || user.index == 1 || user.operands[1].is_some_and(|other| other.kind == kind) {
            return Ok(Some(Cost::FREE));
        }
    }
    Ok(None)
}

/// `getCmpSelInstrCost` of a fixed vector, throughput
/// ([L5363](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L5363)):
/// an `icmp` is one per register; an `fcmp` too, three for the predicates that
/// take two compares and an `or` (`one`, `ueq`, `ord`, `uno`); a select by a
/// vector of conditions follows [`select_cost`].
pub(crate) fn cmp_sel_cost(
    op: CmpSel,
    ty: VectorTy,
    predicate: Option<Predicate>,
    cost_kind: CostKind,
) -> Option<Cost> {
    if cost_kind == CostKind::Latency {
        return None;
    }
    let (registers, lane) = vector_pieces(ty)?;
    if !whole_registers(ty, lane) {
        return None;
    }
    match op {
        CmpSel::ICmp => matches!(ty.lane, Ty::Int(_)).then(|| Cost::of(registers)),
        CmpSel::FCmp => {
            let Some(Predicate::Float(predicate)) = predicate else { return None };
            if !ty.lane.is_float() || matches!(predicate, FloatPredicate::False | FloatPredicate::True) {
                return None;
            }
            let factor = if matches!(
                predicate,
                FloatPredicate::One | FloatPredicate::Ueq | FloatPredicate::Ord | FloatPredicate::Uno
            ) {
                3
            } else {
                1
            };
            Some(Cost::of(factor * registers))
        }
        CmpSel::Select => select_cost(ty, predicate, registers, lane),
    }
}

/// The types the pin prices a vector of at most one register as (and, wider,
/// the register it splits into): D registers of 64 bits, Q registers of 128,
/// and whole multiples of a Q register.
fn whole_registers(
    ty: VectorTy,
    lane: u32,
) -> bool {
    let bits = u64::from(lane) * u64::from(ty.count);
    bits == 64 || (bits >= 128 && bits % 128 == 0)
}

/// A select of `ty` by a vector of `i1`: one `(F)CMxx` and `BFI` pair per
/// register where a compare of a type NEON compares feeds it
/// (`ValidMinMaxTys`); the `VectorSelectTbl` rows; else the select is
/// expanded (NEON has no `VSELECT`): one scalar select per lane and the inserts
/// that rebuild the vector (`BasicTTIImpl::getCmpSelInstrCost`).
fn select_cost(
    ty: VectorTy,
    predicate: Option<Predicate>,
    registers: i64,
    lane: u32,
) -> Option<Cost> {
    use FloatPredicate::{Oeq, Oge, Ogt, Ole, Olt, Une};
    let compare_and_insert = match predicate {
        Some(Predicate::Int(_)) => true,
        Some(Predicate::Float(Ole | Olt | Ogt | Oge | Oeq | Une)) => true,
        _ => false,
    };
    // A wider type is priced as its 128-bit register.
    let per_register = if u64::from(lane) * u64::from(ty.count) > 128 { 128 / lane } else { ty.count };
    let neon_compares = matches!(
        (ty.lane, per_register),
        (Ty::Int(8), 8 | 16)
            | (Ty::Int(16), 4 | 8)
            | (Ty::Int(32), 2 | 4)
            | (Ty::Int(64), 2)
            | (Ty::Float, 2 | 4)
            | (Ty::Double, 2)
    );
    if compare_and_insert && neon_compares {
        return Some(Cost::of(registers));
    }
    const AMORTIZATION_COST: i64 = 20;
    let table = match (ty.lane, ty.count) {
        (Ty::Float | Ty::Double, 2) | (Ty::Float, 4) => Some(2),
        (Ty::Int(16), 16) | (Ty::Int(32), 8) | (Ty::Int(32), 16) => Some(i64::from(ty.count)),
        (Ty::Int(64), 4 | 8 | 16) => Some(i64::from(ty.count) * AMORTIZATION_COST),
        _ => None,
    };
    if let Some(cost) = table {
        return Some(Cost::of(cost));
    }
    let inserts = scalarization_overhead(ty, true, false, CostKind::RecipThroughput)?;
    Some(Cost::of(i64::from(ty.count) + inserts.value))
}

/// `getScalarizationOverhead` of every lane
/// ([L4888](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L4888)):
/// an integer lane costs the base cost per insert and per extract; a
/// floating-point one is priced lane by lane (`getVectorInstrCostHelper`,
/// L4646), and lane 0 of a register is already in the vector.
pub(crate) fn scalarization_overhead(
    ty: VectorTy,
    insert: bool,
    extract: bool,
    cost_kind: CostKind,
) -> Option<Cost> {
    // An integer vector at code size costs 1 a lane (`VecInstCost`).
    let at_size = cost_kind == CostKind::CodeSize && !ty.lane.is_float();
    if cost_kind != CostKind::RecipThroughput && !at_size {
        return None;
    }
    let base = if at_size { 1 } else { INSERT_EXTRACT_BASE_COST };
    // A vector of pointers (the address of a scalarized store): every lane is
    // priced, lane 0 too (`getMemInstScalarizationCost` of the pin's opt:
    // 44 for 8 floats, 48 for 8 bytes at stride 8).
    let ty = if ty.lane == Ty::Pointer { VectorTy { lane: Ty::Int(64), ..ty } } else { ty };
    let (_, lane) = vector_pieces(ty)?;
    let moves = i64::from(insert) + i64::from(extract);
    let register_lanes = (128 / lane).min(ty.count);
    let priced = (0..ty.count).filter(|index| !ty.lane.is_float() || index % register_lanes != 0).count() as i64;
    Some(Cost::of(priced * moves * base))
}

/// `CostTblNoPairwise` of `AArch64TTIImpl::getArithmeticReductionCost`
/// ([L6598](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L6598)):
/// the horizontal `add` (`addv`, `addp`: 2) and the `or`, `xor` and `and`
/// sequences, by the legal type's lane bits and count.
fn no_pairwise(
    kind: ReduceKind,
    lane_bits: u32,
    count: u32,
) -> Option<i64> {
    let bitwise = |costs: [(u32, u32, i64); 7]| {
        costs.iter().find(|&&(bits, lanes, _)| bits == lane_bits && lanes == count).map(|&(.., cost)| cost)
    };
    match kind {
        ReduceKind::Add => matches!(
            (lane_bits, count),
            (8, 8 | 16) | (16, 4 | 8) | (32, 2 | 4) | (64, 2)
        ).then_some(2),
        // fmov + orr_lsr ..., ext + orr + the half-width row
        ReduceKind::Or | ReduceKind::Xor | ReduceKind::And => {
            bitwise([(8, 8, 5), (8, 16, 7), (16, 4, 4), (16, 8, 6), (32, 2, 3), (32, 4, 5), (64, 2, 3)])
        }
        _ => None,
    }
}

/// `getArithmeticReductionCost` and `getMinMaxReductionCost` for a fixed
/// vector, throughput, unordered
/// ([L6551](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L6551),
/// [L6499](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L6499)):
/// the table rows, a `faddp` per halving of a float add, and 2 for a min or
/// max plus 1 for each split. The rest is `BaseT`'s shuffle tree: not priced
/// (DEVIATE VT4: an ordered float reduction, a `mul` or `fmul`, a 64-bit min or
/// max, `fmax`).
pub(crate) fn reduction_cost(
    kind: ReduceKind,
    ty: VectorTy,
    ordered: bool,
    cost_kind: CostKind,
) -> Option<Cost> {
    if cost_kind != CostKind::RecipThroughput || ordered {
        return None;
    }
    let (first, lane_bits) = vector_pieces(ty)?;
    // The legal type: one register's lanes, or the whole vector where smaller.
    let legal = VectorTy { lane: ty.lane, count: ty.count.min(128 / lane_bits) };
    match kind {
        ReduceKind::Add => no_pairwise(kind, lane_bits, legal.count).map(|cost| Cost::of(first - 1 + cost)),
        ReduceKind::Or | ReduceKind::Xor | ReduceKind::And => {
            let cost = no_pairwise(kind, lane_bits, legal.count)?;
            let op = match kind {
                ReduceKind::Or => ArithOp::Or,
                ReduceKind::Xor => ArithOp::Xor,
                _ => ArithOp::And,
            };
            let split = if first == 1 { 0 } else { vector_arithmetic_cost(op, legal, cost_kind)?.value * (first - 1) };
            Some(Cost::of(cost + split))
        }
        ReduceKind::FAdd => {
            let count = legal.count;
            (ty.lane.is_float() && ty.count >= 2 && count >= 2 && count.is_power_of_two())
                .then(|| Cost::of(first - 1 + i64::from(count.ilog2())))
        }
        ReduceKind::SMax | ReduceKind::SMin | ReduceKind::UMax | ReduceKind::UMin => {
            // A `smax` of a legal type is one instruction a register; a `v2i64`
            // has none (priced by `BaseT`).
            (!ty.lane.is_float() && lane_bits < 64).then_some(Cost::of(first - 1 + 2))
        }
        ReduceKind::Mul | ReduceKind::FMul => None,
    }
}

/// `AArch64TTIImpl::getVectorInstrCost`
/// ([L4842](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L4842))
/// over `getVectorInstrCostHelper` (L4646), NEON only. A lane of a register
/// other than the first costs the base cost (1 for code size); lane 0 of a
/// floating-point vector is already in the register, an integer one needs a
/// move; an insert of a one-use load is a `ld1` lane, free where the cpu has a
/// fast one (`fast_ld1_single`).
///
/// DEVIATE VI1 (owner w12, exit 2026-11-30): an extract that fuses with a
/// scalar `fmul` (`ExtractCanFuseWithFmul`, L4772) is priced as any other
/// extract, since the hook is not given its users. Types the port does not
/// price (`vector_pieces`) are `None`.
pub(crate) fn vector_instr_cost(
    op: LaneOp,
    ty: VectorTy,
    index: Option<u32>,
    into_poison: bool,
    context: VectorInstrContext,
    cost_kind: CostKind,
    fast_ld1_single: bool,
) -> Option<Cost> {
    // A vector inserted into a poison one at lane 0 is a `dup`: free.
    if op == LaneOp::Insert && index == Some(0) && into_poison {
        return Some(Cost::of(0));
    }
    let (_, lane) = vector_pieces(ty)?;
    let base = if cost_kind == CostKind::CodeSize { 1 } else { INSERT_EXTRACT_BASE_COST };
    if ty.count == 1 {
        return Some(Cost::of(0));
    }
    if let Some(index) = index {
        // The type may be split: the lane is taken within its register.
        let index = index % (128 / lane).min(ty.count);
        if index == 0 && ty.lane.is_float() {
            return Some(Cost::of(0));
        }
        if context == VectorInstrContext::Load {
            let cost = if fast_ld1_single || cost_kind == CostKind::CodeSize { 0 } else { base + 1 };
            return Some(Cost::of(cost));
        }
    }
    Some(Cost::of(base))
}

/// `AArch64TTIImpl::getIntrinsicInstrCost` of `llvm.{s,u}{max,min}` of a fixed
/// vector
/// ([L710](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L710)):
/// one instruction a register of the types NEON has them for (8, 16 and 32-bit
/// lanes), two (`cmgt` and `bif`) for 64-bit lanes, and the promoted `v2i8`,
/// `v2i16` and `v4i8` a heavy 5 (signed) or 3. DEVIATE VT2 as the other
/// vector hooks: `TCK_Latency` is not priced; the base class's compare and
/// select price of the types not listed (a lane count of one) is not either.
pub(crate) fn min_max_cost(
    signed: bool,
    ty: VectorTy,
    cost_kind: CostKind,
) -> Option<Cost> {
    if cost_kind == CostKind::Latency {
        return None;
    }
    let (pieces, lane) = vector_pieces(ty)?;
    let bits = u64::from(lane) * u64::from(ty.count);
    if matches!((lane, ty.count), (8, 2) | (16, 2) | (8, 4)) {
        return Some(Cost::of(if signed { 5 } else { 3 }));
    }
    match lane {
        _ if ty.count < 2 || bits < 64 => None,
        64 => Some(Cost::of(pieces * 2)),
        _ => Some(Cost::of(pieces)),
    }
}

/// `isREVMask` (`AArch64PerfectShuffle.h`): lanes reversed within blocks of
/// `block_bits`.
fn is_rev_mask(
    mask: &[i32],
    lane_bits: u32,
    block_bits: u32,
) -> bool {
    let block = (block_bits / lane_bits) as usize;
    block > 1
        && mask.len() % block == 0
        && mask.iter().enumerate().all(|(i, &m)| m < 0 || m as usize == i - i % block + (block - 1 - i % block))
}

/// `AArch64TTIImpl::getShuffleCost`
/// ([L6978](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L6978))
/// of a single-source shuffle of `ty` by `mask`, in the pin's order: the
/// kind the mask is (`improveShuffleKindFromMask`), identity free, the
/// one-instruction masks (a lane splat, `zip`, `trn`, `uzp`, `rev`) at one,
/// then the rows of `ShuffleTbl` (L7194), by register for a vector that
/// splits. A mask of the sub-register vectors, a 4-lane 16 or 32-bit mask
/// that is none of those (the perfect-shuffle table, L7160) and the
/// `BaseT` fall-through are not priced (DEVIATE SLP5, owner arm64, exit
/// 2026-11-24).
pub(crate) fn shuffle_cost(
    kind: llrm_mir::tti::ShuffleKind,
    ty: VectorTy,
    mask: &[i32],
    cost_kind: CostKind,
) -> Option<Cost> {
    use llrm_mir::tti::ShuffleKind as K;
    // An empty mask is the pin's "no mask given": the table row of the kind.
    if cost_kind != CostKind::RecipThroughput || (!mask.is_empty() && mask.len() != ty.count as usize) {
        return None;
    }
    let (pieces, lane) = vector_pieces(ty)?;
    let bits = u64::from(lane) * u64::from(ty.count);
    if !((bits == 64 && lane != 64) || bits % 128 == 0) {
        return None;
    }
    let per_register = (128 / lane).min(ty.count) as usize;
    // A mask over a vector that splits: each register's piece on its own, the
    // same piece once (L6992).
    if pieces > 1 && kind == K::PermuteSingleSrc {
        let mut seen: Vec<(Option<i32>, Vec<i32>)> = Vec::new();
        let mut total = 0;
        for chunk in mask.chunks(per_register) {
            let sources: Vec<i32> = chunk.iter().filter(|&&m| m >= 0).map(|&m| m / per_register as i32).collect();
            if sources.iter().any(|&source| Some(&source) != sources.first()) {
                return None;
            }
            let part: Vec<i32> = chunk.iter().map(|&m| if m < 0 { m } else { m % per_register as i32 }).collect();
            let key = (sources.first().copied(), part);
            if seen.contains(&key) {
                continue;
            }
            total +=
                shuffle_cost(kind, VectorTy { lane: ty.lane, count: per_register as u32 }, &key.1, cost_kind)?.value;
            seen.push(key);
        }
        return Some(Cost::of(total));
    }
    let defined: Vec<i32> = mask.iter().copied().filter(|&m| m >= 0).collect();
    if defined.iter().any(|&m| m >= ty.count as i32) {
        return None;
    }
    // `improveShuffleKindFromMask`.
    let splat = !defined.is_empty() && defined.iter().all(|&m| m == defined[0]);
    let kind = match kind {
        K::PermuteSingleSrc if is_rev_mask(mask, lane, bits as u32) && mask.len() > 1 => K::Reverse,
        K::PermuteSingleSrc if splat => K::Broadcast,
        other => other,
    };
    if !mask.is_empty()
        && matches!(kind, K::PermuteSingleSrc | K::PermuteTwoSrc)
        && mask.iter().enumerate().all(|(i, &m)| m < 0 || m as usize == i)
    {
        return Some(Cost::of(0));
    }
    let one_instruction = !mask.is_empty()
        && (splat
            || crate::dag::permute::matched(mask, mask.len()).is_some()
            || [16, 32, 64].iter().any(|&block| is_rev_mask(mask, lane, block)));
    if mask.len() == 4 && matches!(lane, 16 | 32) {
        // The perfect-shuffle table: the masks of one instruction are cost 1.
        return one_instruction.then(|| Cost::of(1));
    }
    if one_instruction && kind != K::Broadcast {
        return Some(Cost::of(1));
    }
    let register = VectorTy { lane: ty.lane, count: per_register as u32 };
    let (_, lane_bits) = vector_pieces(register)?;
    let row = match (kind, lane_bits, register.count) {
        (K::Broadcast, 8, 8 | 16) | (K::Broadcast, 16, 4 | 8) | (K::Broadcast, 32, 2 | 4) | (K::Broadcast, 64, 2) => 1,
        (K::PermuteSingleSrc, 32, 2) | (K::PermuteSingleSrc, 64, 2) => 1,
        (K::PermuteSingleSrc, 32, 4) | (K::PermuteSingleSrc, 16, 4) => 3,
        (K::PermuteSingleSrc, 16, 8) | (K::PermuteSingleSrc, 8, 8 | 16) => 8,
        (K::Reverse, 32, 2) | (K::Reverse, 64, 2) | (K::Reverse, 16, 4) | (K::Reverse, 8, 8) => 1,
        (K::Reverse, 32, 4) | (K::Reverse, 16, 8) | (K::Reverse, 8, 16) => 2,
        _ => return None,
    };
    Some(Cost::of(pieces * row))
}

/// `AArch64TTIImpl::getStoreMinimumVF`
/// ([L537](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.h#L537)):
/// a chain of 4 or more `i8` stores in a power-of-two count is cut to 4;
/// else `BasicTTIImpl::getStoreMinimumVF` halves the VF while a store of half
/// the lanes is a legal one (at most one register). DEVIATE VI2 (owner w12,
/// exit 2026-11-30): the pin's truncating-store test for an illegal half is
/// not made (no vector truncating store is priced).
pub(crate) fn store_minimum_vf(
    vf: u32,
    mem: Ty,
) -> u32 {
    if mem == Ty::Int(8) && vf.is_power_of_two() && vf >= 4 {
        return 4;
    }
    let legal = |lanes: u32| matches!(vector_pieces(v(mem, lanes)), Some((1, _)));
    let mut vf = vf;
    while vf > 2 && legal(vf / 2) {
        vf /= 2;
    }
    vf
}
