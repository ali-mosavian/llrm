//! The vector costs of `AArch64TTIImpl` beyond memory and integer arithmetic:
//! casts, compares and selects, and the scalarization overhead a replicated
//! access or an expanded select pays. Each is the pin's (d1106deb71cc) for the
//! fixed NEON types the port prices (power-of-two lane counts, one register or
//! whole registers: DEVIATE VT1); a type or a case the pin prices some other
//! way is `None`, not a guess.

use llrm_mir::intrinsics::ReduceKind;
use llrm_mir::opcode::FloatPredicate;
use llrm_mir::tti::{ArithOp, CastKind, CmpSel, Cost, CostKind, ExtOperand, ExtUser, Predicate, Ty, VectorTy};

use crate::tti::{vector_arithmetic_cost, vector_pieces};

/// `ST->getVectorInsertExtractBaseCost()`, 2 on both cpus.
const INSERT_EXTRACT_BASE_COST: i64 = 2;

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

/// `getCastInstrCost` for fixed vectors, throughput: a row of the conversion
/// table, or a bitcast between types of the same registers
/// (`BasicTTIImpl::getCastInstrCost`: free).
pub(crate) fn cast_cost(
    kind: CastKind,
    dst: VectorTy,
    src: VectorTy,
    cost_kind: CostKind,
) -> Option<Cost> {
    if cost_kind != CostKind::RecipThroughput {
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
    if matches!(kind, CastKind::SIToFP | CastKind::UIToFP | CastKind::FPToSI | CastKind::FPToUI)
        && vector_pieces(dst).zip(vector_pieces(src)).is_some_and(|((_, to), (_, from))| to == from)
    {
        return ext_cast_cost(kind, dst, src);
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
    let ((dst_registers, _), (src_registers, _)) = (vector_pieces(dst)?, vector_pieces(src)?);
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
    if cost_kind != CostKind::RecipThroughput {
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
    if cost_kind != CostKind::RecipThroughput {
        return None;
    }
    let (_, lane) = vector_pieces(ty)?;
    let moves = i64::from(insert) + i64::from(extract);
    let register_lanes = (128 / lane).min(ty.count);
    let priced = (0..ty.count).filter(|index| !ty.lane.is_float() || index % register_lanes != 0).count() as i64;
    Some(Cost::of(priced * moves * INSERT_EXTRACT_BASE_COST))
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
