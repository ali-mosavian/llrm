//! `LowerBUILD_VECTOR` and `LowerVECTOR_SHUFFLE` for the 128-bit integer
//! vectors
//! ([L17152](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L17152),
//! [L16124](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L16124)),
//! and the modified-immediate forms of `AArch64_AM`
//! ([AArch64AddressingModes.h
//! L486](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/MCTargetDesc/AArch64AddressingModes.h#L486)).
//!
//! DEVIATE VEC1: a `BUILD_VECTOR` that is a splat of a constant, the all-zeros
//! and all-ones vectors, a splat of one value, or a value in lane 0 alone is
//! lowered; a constant that is no splat is declined to the legalizer's pool
//! load; any other (the pin builds it with `INSERT_VECTOR_ELT`) is declined
//! and the block refused. A constant
//! with undefined lanes is tried only as the splat of its defined lanes
//! (`UndefBits` of `ConstantBuildVector` is not tried), and `FMOV`, the
//! `FNEG` forms and SVE are not made. A shuffle is lowered where it is a
//! splat of a scalar (`DUP`); `DUPLANE` and the other masks are declined.
//! Exit: the rest of both lowerings, with reverse (`REV64`) first.

use llrm_codegen::backend::selectiondag::SDValue;
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_dagisel::generated::aarch64::ids::isd as aarch64_isd;

/// `AArch64_AM::isAdvSIMDModImmType<n>`: whether the 64-bit pattern (two equal
/// 32-bit halves) is one a `MOVI`/`MVNI` form encodes.
mod modimm {
    fn halves_equal(imm: u64) -> bool {
        imm >> 32 == imm & 0xffff_ffff
    }
    pub fn type1(imm: u64) -> bool {
        halves_equal(imm) && imm & 0xffff_ff00_ffff_ff00 == 0
    }
    pub fn type2(imm: u64) -> bool {
        halves_equal(imm) && imm & 0xffff_00ff_ffff_00ff == 0
    }
    pub fn type3(imm: u64) -> bool {
        halves_equal(imm) && imm & 0xff00_ffff_ff00_ffff == 0
    }
    pub fn type4(imm: u64) -> bool {
        halves_equal(imm) && imm & 0x00ff_ffff_00ff_ffff == 0
    }
    pub fn type5(imm: u64) -> bool {
        halves_equal(imm) && (imm & 0x00ff_0000) >> 16 == imm & 0xff && imm & 0xff00_ff00_ff00_ff00 == 0
    }
    pub fn type6(imm: u64) -> bool {
        halves_equal(imm) && (imm & 0xff00_0000) >> 16 == imm & 0xff00 && imm & 0x00ff_00ff_00ff_00ff == 0
    }
    pub fn type7(imm: u64) -> bool {
        halves_equal(imm) && imm & 0xffff_00ff_ffff_00ff == 0x0000_00ff_0000_00ff
    }
    pub fn type8(imm: u64) -> bool {
        halves_equal(imm) && imm & 0xff00_ffff_ff00_ffff == 0x0000_ffff_0000_ffff
    }
    pub fn type9(imm: u64) -> bool {
        halves_equal(imm) && imm >> 48 == imm & 0xffff && imm >> 56 == imm & 0xff
    }
    pub fn type10(imm: u64) -> bool {
        imm.to_le_bytes().iter().all(|byte| *byte == 0 || *byte == 0xff)
    }
    /// `encodeAdvSIMDModImmType10`: a bit per byte.
    pub fn encode10(imm: u64) -> u64 {
        imm.to_le_bytes().iter().rev().fold(0, |code, byte| code << 1 | u64::from(*byte != 0))
    }
}

/// `AArch64_AM::encodeAdvSIMDModImmType10`.
pub(super) fn encode_mod_imm_type10(imm: u64) -> u64 {
    modimm::encode10(imm)
}

/// The lane `operand` is, if it is a constant, at the lane's width.
fn lane_constant(
    dag: &SelectionDAG,
    operand: SDValue,
    lane_bits: u32,
) -> Option<u64> {
    // A float lane is its bits (`BuildVectorSDNode::isConstantSplat`).
    let bits = dag.constant(operand).or_else(|| dag.fp_constant(operand))?;
    Some((bits & ((1u128 << lane_bits) - 1)) as u64)
}

/// `MOVI`/`MVNI`/`NVCAST` of a splatted 64-bit pattern, the forms of
/// `ConstantBuildVector` in the pin's order; `bits` is the pattern of both
/// halves.
fn constant_build_vector(
    dag: &mut SelectionDAG,
    vt: EVT,
    bits: u64,
) -> Option<SDValue> {
    use llrm_codegen::backend::selectiondag::mvt::MVT;
    let nvcast = |dag: &mut SelectionDAG, mov: SDValue| {
        let from = dag.value_type(mov);
        if from == vt { mov } else { dag.get_node(ISD::Target(aarch64_isd::NVCAST), vt, &[mov]) }
    };
    let imm = |dag: &mut SelectionDAG, value: u64| dag.get_constant(u128::from(value), EVT::I32);
    // `tryAdvSIMDModImm64`
    let attempt = |dag: &mut SelectionDAG, bits: u64, inverted: bool| -> Option<SDValue> {
        let (opcode_shift, opcode_msl, opcode_byte) = if inverted {
            (aarch64_isd::MVNIshift, aarch64_isd::MVNImsl, None)
        } else {
            (aarch64_isd::MOVIshift, aarch64_isd::MOVImsl, Some(aarch64_isd::MOVI))
        };
        if !inverted && modimm::type10(bits) {
            let code = imm(dag, modimm::encode10(bits));
            let mov = dag.get_node(ISD::Target(aarch64_isd::MOVIedit), EVT::Simple(MVT::V2I64), &[code]);
            return Some(nvcast(dag, mov));
        }
        // `tryAdvSIMDModImm32`: types 1-4 shift the byte by 0, 8, 16, 24.
        let word = [
            (modimm::type1(bits), 0u64, bits & 0xff),
            (modimm::type2(bits), 8, (bits & 0xff00) >> 8),
            (modimm::type3(bits), 16, (bits & 0xff_0000) >> 16),
            (modimm::type4(bits), 24, (bits & 0xff00_0000) >> 24),
        ];
        if let Some(&(_, shift, value)) = word.iter().find(|(is, ..)| *is) {
            let (value, shift) = (imm(dag, value), imm(dag, shift));
            let mov = dag.get_node(ISD::Target(opcode_shift), EVT::Simple(MVT::V4I32), &[value, shift]);
            return Some(nvcast(dag, mov));
        }
        // `tryAdvSIMDModImm321s`: types 7 and 8, the shifted ones.
        let ones =
            [(modimm::type7(bits), 264u64, (bits & 0xff00) >> 8), (modimm::type8(bits), 272, (bits & 0xff_0000) >> 16)];
        if let Some(&(_, shift, value)) = ones.iter().find(|(is, ..)| *is) {
            let (value, shift) = (imm(dag, value), imm(dag, shift));
            let mov = dag.get_node(ISD::Target(opcode_msl), EVT::Simple(MVT::V4I32), &[value, shift]);
            return Some(nvcast(dag, mov));
        }
        // `tryAdvSIMDModImm16`: types 5 and 6.
        let half = [(modimm::type5(bits), 0u64, bits & 0xff), (modimm::type6(bits), 8, (bits & 0xff00) >> 8)];
        if let Some(&(_, shift, value)) = half.iter().find(|(is, ..)| *is) {
            let (value, shift) = (imm(dag, value), imm(dag, shift));
            let mov = dag.get_node(ISD::Target(opcode_shift), EVT::Simple(MVT::V8I16), &[value, shift]);
            return Some(nvcast(dag, mov));
        }
        // `tryAdvSIMDModImm8`: type 9, `MOVI` of a byte (the pin tries no
        // inverted byte form).
        if let Some(opcode) = opcode_byte
            && modimm::type9(bits)
        {
            let value = imm(dag, bits & 0xff);
            let mov = dag.get_node(ISD::Target(opcode), EVT::Simple(MVT::V16I8), &[value]);
            return Some(nvcast(dag, mov));
        }
        None
    };
    attempt(dag, bits, false).or_else(|| attempt(dag, !bits, true))
}

/// Whether every defined lane of the `BUILD_VECTOR` is a constant.
pub fn is_constant_build_vector(
    dag: &SelectionDAG,
    op: SDValue,
) -> bool {
    let lane_bits = dag.value_type(op).element().size_in_bits();
    dag.node(op).ops.iter().all(|&lane| dag.is_undef(lane) || lane_constant(dag, lane, lane_bits).is_some())
}

/// `LowerBUILD_VECTOR`.
pub fn lower_build_vector(
    dag: &mut SelectionDAG,
    op: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(op);
    let lane_bits = vt.element().size_in_bits();
    let ops = dag.node(op).ops.clone();
    let lanes: Vec<Option<SDValue>> = ops.iter().map(|&lane| (!dag.is_undef(lane)).then_some(lane)).collect();
    let defined: Vec<SDValue> = lanes.iter().flatten().copied().collect();
    let Some(&first) = defined.first() else {
        // "value undefined, creating undef node"
        return Some(dag.get_undef(vt));
    };
    // `isConstant()`: every defined lane is a constant.
    let constants: Option<Vec<u64>> = defined.iter().map(|&lane| lane_constant(dag, lane, lane_bits)).collect();
    if let Some(values) = constants {
        // Splat (the defined lanes agree): the zeros and ones pass through to
        // their patterns; the rest is `ConstantBuildVector`.
        if values.iter().all(|&value| value == values[0]) {
            let mask = (u128::MAX >> (128 - lane_bits)) as u64;
            let splat = values[0];
            if splat == 0 || splat == mask {
                return Some(op);
            }
            let unit = (0..64 / lane_bits).fold(0u64, |all, at| all | splat << (lane_bits * at));
            // "Otherwise, materialize the constant and splat it."
            return constant_build_vector(dag, vt, unit)
                .or_else(|| Some(dag.get_node(ISD::Target(aarch64_isd::DUP), vt, &[first])));
        }
        // Not a splat: the pin's `SDValue()`, the pool load of the legalizer.
        return None;
    }
    let low_only = lanes.iter().skip(1).all(Option::is_none);
    if low_only {
        // "isOnlyLowElement && !isIntOrFPConstant(Value)"
        return Some(dag.get_node(ISD::ScalarToVector, vt, &[first]));
    }
    let one_value = defined.iter().all(|&lane| lane == first);
    if one_value {
        // "Use DUP for non-constant splats."
        return Some(dag.get_node(ISD::Target(aarch64_isd::DUP), vt, &[first]));
    }
    // "If all else fails, just use a sequence of INSERT_VECTOR_ELT": lane 0 by
    // `SCALAR_TO_VECTOR`, the others inserted. DEVIATE VEC6 (owner
    // llrm-arm64-vec, exit 2026-11-27): the branches before it in the pin
    // (`PreferDUPAndInsert`, a constant splat with other lanes inserted, two
    // `DUP`s joined, `ReconstructShuffle`) are not translated.
    let index_ty = EVT::I64;
    let mut vec = dag.get_undef(vt);
    let mut at = 0;
    if lanes[0].is_some() {
        vec = dag.get_node(ISD::ScalarToVector, vt, &[ops[0]]);
        at = 1;
    }
    for lane in at..ops.len() {
        if let Some(value) = lanes[lane] {
            let index = dag.get_constant(lane as u128, index_ty);
            vec = dag.get_node(ISD::InsertVectorElt, vt, &[vec, value, index]);
        }
    }
    Some(vec)
}

/// `UnrollVectorOp`: one scalar operation per lane, put together by a
/// `BUILD_VECTOR`.
fn unroll_vector_op(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let opcode = dag.opcode(node);
    let operand = dag.operand(node, 0);
    let (from, to) = (dag.value_type(operand).element(), vt.element());
    let lanes = (0..vt.lanes())
        .map(|lane| {
            let index = dag.get_constant(u128::from(lane), EVT::I64);
            let element = dag.get_node(ISD::ExtractElement, from, &[operand, index]);
            dag.get_node(opcode, to, &[element])
        })
        .collect::<Vec<_>>();
    dag.get_build_vector(vt, &lanes)
}

/// `LowerVectorINT_TO_FP`. A conversion to a narrower float is unrolled when
/// the target is `f32` (the pin: a conversion through `f64` rounds twice) and
/// goes through the wider float otherwise; to a wider float the operand is
/// extended first; one of the same size is as it is.
///
/// DEVIATE VEC7 (owner llrm-arm64-vec, exit 2026-11-27): the `f16` target of
/// a split conversion, a conversion of a one-lane vector and the SVE forms are
/// not translated.
pub fn lower_vector_int_to_fp(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (vt, operand) = (dag.value_type(node), dag.operand(node, 0));
    let from = dag.value_type(operand);
    let opcode = dag.opcode(node);
    if vt.size_in_bits() < from.size_in_bits() {
        if vt.element() == EVT::Simple(MVT::F32) {
            return Some(unroll_vector_op(dag, node));
        }
        let wide = MVT::vector(MVT::F64, from.lanes())?;
        let converted = dag.get_node(opcode, EVT::Simple(wide), &[operand]);
        let exact = dag.get_constant(0, EVT::I64);
        return Some(dag.get_node(ISD::FpRound, vt, &[converted, exact]));
    }
    if vt.size_in_bits() > from.size_in_bits() {
        let extend = if opcode == ISD::SIntToFp { ISD::SignExtend } else { ISD::ZeroExtend };
        let int = MVT::vector(MVT::integer(vt.element().size_in_bits())?, vt.lanes())?;
        let extended = dag.get_node(extend, EVT::Simple(int), &[operand]);
        return Some(dag.get_node(opcode, vt, &[extended]));
    }
    Some(node)
}

/// `LowerVectorFP_TO_INT`, the conversions of one size and to a wider integer
/// the pin does with one instruction or a `fcvtl` first; anything else is
/// declined.
pub fn lower_vector_fp_to_int(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (vt, operand) = (dag.value_type(node), dag.operand(node, 0));
    (vt.size_in_bits() == dag.value_type(operand).size_in_bits()).then_some(node)
}

/// `LowerMUL`, the NEON path
/// ([L6202](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L6202)):
/// a multiply of extended operands is `SMULL`/`UMULL` of the narrow ones. A
/// multiply of another kind is as it is (legal) except `v2i64`, which
/// is expanded (`None`). DEVIATE MUL1 (owner llrm-arm64-vec, exit 2026-11-24):
/// `convertMulToShlAdd` (a `v2i64` multiply by a constant) is not translated.
pub fn lower_vector_mul(
    dag: &mut SelectionDAG,
    op: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(op);
    let (mut n0, mut n1) = (dag.operand(op, 0), dag.operand(op, 1));
    let mut multiplied = vt;
    if vt.size_in_bits() == 64 {
        // The low halves of two 128-bit operands: multiply those and take the
        // low half.
        let low_half = |dag: &SelectionDAG, n: SDValue| {
            dag.opcode(n) == ISD::ExtractSubvector
                && dag.constant(dag.operand(n, 1)) == Some(0)
                && dag.value_type(dag.operand(n, 0)).size_in_bits() == 128
        };
        if !(low_half(dag, n0) && low_half(dag, n1)) {
            // "Other vector multiplications are legal."
            return Some(op);
        }
        n0 = dag.operand(n0, 0);
        n1 = dag.operand(n1, 0);
        multiplied = dag.value_type(n0);
    }
    let Some((opcode, is_mla)) = select_umull_smull(dag, &mut n0, &mut n1) else {
        return if vt.element().size_in_bits() == 64 { None } else { Some(op) };
    };
    let op1 = skip_extension_for_vector_mull(dag, n1);
    let take_low = |dag: &mut SelectionDAG, wide: SDValue| {
        if multiplied == vt {
            wide
        } else {
            let zero = dag.get_constant(0, EVT::I64);
            dag.get_node(ISD::ExtractSubvector, vt, &[wide, zero])
        }
    };
    if !is_mla {
        let op0 = skip_extension_for_vector_mull(dag, n0);
        let product = dag.get_node(ISD::Target(opcode), multiplied, &[op0, op1]);
        return Some(take_low(dag, product));
    }
    // "(zext A + zext B) * C, to (S/UMULL A, C) + (S/UMULL B, C)": the
    // multiplies then accumulate back to back.
    let n00 = skip_extension_for_vector_mull(dag, dag.operand(n0, 0));
    let n01 = skip_extension_for_vector_mull(dag, dag.operand(n0, 1));
    let op1_vt = dag.value_type(op1);
    let a = dag.get_node(ISD::BitCast, op1_vt, &[n00]);
    let b = dag.get_node(ISD::BitCast, op1_vt, &[n01]);
    let first = dag.get_node(ISD::Target(opcode), multiplied, &[a, op1]);
    let second = dag.get_node(ISD::Target(opcode), multiplied, &[b, op1]);
    let sum = dag.get_node(dag.opcode(n0), multiplied, &[first, second]);
    let zero = dag.get_constant(0, EVT::I64);
    Some(dag.get_node(ISD::ExtractSubvector, vt, &[sum, zero]))
}

/// `selectUmullSmull`: the multiply `N0 * N1` is a `SMULL` or a `UMULL` when
/// both operands are extended alike or the other is known to fit; the flag
/// says it is the `(A + B) * C` form, which `n0` is the sum of.
fn select_umull_smull(
    dag: &mut SelectionDAG,
    n0: &mut SDValue,
    n1: &mut SDValue,
) -> Option<(u16, bool)> {
    use llrm_codegen::backend::selectiondag::known_bits::{masked_value_is_zero, num_sign_bits};
    let (is_n0_sext, is_n1_sext) = (is_sign_extended(dag, *n0), is_sign_extended(dag, *n1));
    if is_n0_sext && is_n1_sext {
        return Some((aarch64_isd::SMULL, false));
    }
    let (is_n0_zext, is_n1_zext) = (is_zero_extended(dag, *n0), is_zero_extended(dag, *n1));
    if is_n0_zext && is_n1_zext {
        return Some((aarch64_isd::UMULL, false));
    }
    // "Select UMULL if we can replace the other operand with an extend."
    let vt = dag.value_type(*n0);
    let elt_size = vt.element().size_in_bits();
    let high_half = (u128::MAX >> (128 - elt_size)) & !(u128::MAX >> (128 - elt_size / 2));
    if is_n0_zext || is_n1_zext {
        if masked_value_is_zero(dag, if is_n0_zext { *n1 } else { *n0 }, high_half) {
            return Some((aarch64_isd::UMULL, false));
        }
    } else if vt == EVT::Simple(MVT::V2I64)
        && masked_value_is_zero(dag, *n0, high_half)
        && masked_value_is_zero(dag, *n1, high_half)
    {
        // "For v2i64 we look more aggressively at both operands being zero, to
        // avoid scalarization."
        return Some((aarch64_isd::UMULL, false));
    }
    if is_n0_sext || is_n1_sext {
        if num_sign_bits(dag, if is_n0_sext { *n1 } else { *n0 }) > elt_size / 2 {
            return Some((aarch64_isd::SMULL, false));
        }
    } else if vt == EVT::Simple(MVT::V2I64)
        && num_sign_bits(dag, *n0) > elt_size / 2
        && num_sign_bits(dag, *n1) > elt_size / 2
    {
        return Some((aarch64_isd::SMULL, false));
    }
    if !is_n1_sext && !is_n1_zext {
        return None;
    }
    // "Look for (s/zext A + s/zext B) * (s/zext C)."
    if is_n1_sext && is_add_sub_extended(dag, *n0, is_sign_extended) {
        return Some((aarch64_isd::SMULL, true));
    }
    if is_n1_zext && is_add_sub_extended(dag, *n0, is_zero_extended) {
        return Some((aarch64_isd::UMULL, true));
    }
    if is_n0_zext && is_add_sub_extended(dag, *n1, is_zero_extended) {
        std::mem::swap(n0, n1);
        return Some((aarch64_isd::UMULL, true));
    }
    None
}

/// `isAddSubSExt` and `isAddSubZExt`: an add or a sub of two extended
/// operands, each used once.
fn is_add_sub_extended(
    dag: &SelectionDAG,
    n: SDValue,
    extended: fn(&SelectionDAG, SDValue) -> bool,
) -> bool {
    matches!(dag.opcode(n), ISD::Add | ISD::Sub)
        && [dag.operand(n, 0), dag.operand(n, 1)].into_iter().all(|one| dag.has_one_use(one.node) && extended(dag, one))
}

/// `isExtendedBUILD_VECTOR`: every lane a constant that fits half the lane.
fn is_extended_build_vector(
    dag: &SelectionDAG,
    n: SDValue,
    signed: bool,
) -> bool {
    if dag.opcode(n) != ISD::BuildVector {
        return false;
    }
    let lane_bits = dag.value_type(n).element().size_in_bits();
    let half = lane_bits / 2;
    (0..dag.node(n).ops.len()).all(|at| {
        let Some(bits) = dag.constant(dag.operand(n, at)) else { return false };
        let bits = bits & (u128::MAX >> (128 - lane_bits));
        if signed {
            // `isIntN(HalfSize, C->getSExtValue())`
            let value = ((bits << (128 - lane_bits)) as i128) >> (128 - lane_bits);
            -(1i128 << (half - 1)) <= value && value < 1i128 << (half - 1)
        } else {
            bits < 1u128 << half
        }
    })
}

fn is_sign_extended(
    dag: &SelectionDAG,
    n: SDValue,
) -> bool {
    matches!(dag.opcode(n), ISD::SignExtend | ISD::AnyExtend) || is_extended_build_vector(dag, n, true)
}

fn is_zero_extended(
    dag: &SelectionDAG,
    n: SDValue,
) -> bool {
    matches!(dag.opcode(n), ISD::ZeroExtend | ISD::AnyExtend) || is_extended_build_vector(dag, n, false)
}

/// `skipExtensionForVectorMULL`: the operand truncated to half the lane (the
/// combiner folds the extension away).
fn skip_extension_for_vector_mull(
    dag: &mut SelectionDAG,
    n: SDValue,
) -> SDValue {
    let half = match dag.value_type(n) {
        EVT::Simple(MVT::V8I16) => MVT::V8I8,
        EVT::Simple(MVT::V4I32) => MVT::V4I16,
        EVT::Simple(MVT::V2I64) => MVT::V2I32,
        vt => unreachable!("{vt:?} is not a vector MULL operand"),
    };
    dag.get_node(ISD::Truncate, EVT::Simple(half), &[n])
}

/// `performTruncateCombine`, the `DUP` arm
/// ([L23964](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L23964)):
/// a 64-bit truncate of a splat is the splat of the truncated scalar. DEVIATE
/// MUL2 (owner llrm-arm64-vec, exit 2026-11-27): the `ADDHN` and the
/// `extract_vector_elt` arms are not translated.
pub fn truncate_combine(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (vt, n0) = (dag.value_type(node), dag.operand(node, 0));
    if !(vt.is_vector() && vt.size_in_bits() == 64)
        || !dag.has_one_use(n0.node)
        || dag.opcode(n0) != ISD::Target(aarch64_isd::DUP)
    {
        return None;
    }
    let mut op = dag.operand(n0, 0);
    if vt.element() == EVT::I32 && dag.value_type(op) == EVT::I64 {
        op = dag.get_node(ISD::Truncate, EVT::I32, &[op]);
    }
    Some(dag.get_node(ISD::Target(aarch64_isd::DUP), vt, &[op]))
}

/// `performDUPCombine`, the first arm
/// ([L22164](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L22164)):
/// "If "v2i32 DUP(x)" and "v4i32 DUP(x)" both exist, use an extract from the
/// 128bit vector version." DEVIATE MUL3 (owner llrm-arm64-vec, exit
/// 2026-11-27): the `DUP` of a load, of a float compare or of an extracted
/// lane, and `performPostLD1Combine`, are not translated.
pub fn dup_combine(
    dag: &mut SelectionDAG,
    node: SDValue,
    after_legalize: bool,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if !(after_legalize && vt.is_vector() && vt.size_in_bits() == 64) {
        return None;
    }
    let doubled = double_lanes(vt)?;
    let ops = dag.node(node).ops.clone();
    let wide = dag.find_node(dag.opcode(node), doubled, &ops)?;
    let zero = dag.get_constant(0, EVT::I64);
    Some(dag.get_node(ISD::ExtractSubvector, vt, &[wide, zero]))
}

/// `getDoubleNumVectorElementsVT`: the 128-bit vector of a 64-bit one's lanes.
fn double_lanes(vt: EVT) -> Option<EVT> {
    Some(EVT::Simple(match vt {
        EVT::Simple(MVT::V8I8) => MVT::V16I8,
        EVT::Simple(MVT::V4I16) => MVT::V8I16,
        EVT::Simple(MVT::V2I32) => MVT::V4I32,
        EVT::Simple(MVT::V2F32) => MVT::V4F32,
        _ => return None,
    }))
}

/// `performMULLCombine`, `tryCombineLongOpWithDup`
/// ([L24761](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L24761)):
/// a long multiply of the high half of a vector and a splat takes the high
/// half of a wider splat, so that `SMULL2` matches. DEVIATE MUL4 (owner
/// llrm-arm64-vec, exit 2026-11-27): `tryCombineMULLWithUZP1` is not
/// translated.
pub fn mull_combine(
    dag: &mut SelectionDAG,
    node: SDValue,
    before_legalize_ops: bool,
) -> Option<SDValue> {
    if before_legalize_ops {
        return None;
    }
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let (lhs, rhs) = if is_essentially_extract_high_subvector(dag, lhs) {
        (lhs, try_extend_dup_to_extract_high(dag, rhs)?)
    } else if is_essentially_extract_high_subvector(dag, rhs) {
        (try_extend_dup_to_extract_high(dag, lhs)?, rhs)
    } else {
        return None;
    };
    let vt = dag.value_type(node);
    Some(dag.get_node(dag.opcode(node), vt, &[lhs, rhs]))
}

/// `isEssentiallyExtractHighSubvector`.
fn is_essentially_extract_high_subvector(
    dag: &SelectionDAG,
    mut n: SDValue,
) -> bool {
    if dag.opcode(n) == ISD::BitCast {
        n = dag.operand(n, 0);
    }
    dag.opcode(n) == ISD::ExtractSubvector
        && dag.constant(dag.operand(n, 1)) == Some(u128::from(dag.value_type(dag.operand(n, 0)).lanes() / 2))
}

/// `tryExtendDUPToExtractHigh`: a splat (or an immediate vector) of 64 bits
/// is the high half of the one of 128 bits.
fn try_extend_dup_to_extract_high(
    dag: &mut SelectionDAG,
    mut n: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(n);
    if dag.opcode(n) == ISD::ExtractSubvector && dag.constant(dag.operand(n, 1)) == Some(0) {
        n = dag.operand(n, 0);
    }
    use aarch64_isd::{
        DUP, DUPLANE8, DUPLANE16, DUPLANE32, DUPLANE64, MOVI, MOVIedit, MOVImsl, MOVIshift, MVNImsl, MVNIshift,
    };
    let ISD::Target(opcode) = dag.opcode(n) else { return None };
    if ![DUP, DUPLANE8, DUPLANE16, DUPLANE32, DUPLANE64, MOVI, MOVIshift, MOVIedit, MOVImsl, MVNIshift, MVNImsl]
        .contains(&opcode)
        || vt.size_in_bits() != 64
    {
        return None;
    }
    let lanes = vt.lanes();
    if dag.value_type(n).size_in_bits() == 64 {
        let wide = double_lanes(vt)?;
        let ops = dag.node(n).ops.clone();
        n = dag.get_node(ISD::Target(opcode), wide, &ops);
    }
    let at = dag.get_constant(u128::from(lanes), EVT::I64);
    Some(dag.get_node(ISD::ExtractSubvector, vt, &[n, at]))
}

/// `LowerVECTOR_SHUFFLE`, as far as a splat of a scalar.
pub fn lower_vector_shuffle(
    dag: &mut SelectionDAG,
    op: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(op);
    let mask = dag.shuffle_mask(op).to_vec();
    let v1 = dag.operand(op, 0);
    // `SVN->isSplat()`: every defined lane picks the one lane.
    let lane = mask.iter().copied().find(|&lane| lane >= 0).unwrap_or(0);
    if !mask.iter().all(|&one| one < 0 || one == lane) {
        return None;
    }
    // "If this is undef splat, generate it via "just" vdup, if possible."
    if lane == 0 && dag.opcode(v1) == ISD::ScalarToVector {
        return Some(dag.get_node(ISD::Target(aarch64_isd::DUP), vt, &[dag.operand(v1, 0)]));
    }
    // "V1 is a BUILD_VECTOR and the lane being referenced is a non-constant."
    if dag.opcode(v1) == ISD::BuildVector && dag.constant(dag.operand(v1, lane as usize)).is_none() {
        return Some(dag.get_node(ISD::Target(aarch64_isd::DUP), vt, &[dag.operand(v1, lane as usize)]));
    }
    // DEVIATE VEC1: `constructDup` (a lane of a vector) is not translated.
    None
}

/// The constant every lane of `operand` is, at the lane's width
/// (`isConstantSplatVector`): a `BUILD_VECTOR` of equal constants.
fn splat_constant(
    dag: &SelectionDAG,
    operand: SDValue,
) -> Option<u64> {
    if dag.opcode(operand) != ISD::BuildVector {
        return None;
    }
    let bits = dag.value_type(operand).element().size_in_bits();
    let lanes = dag.node(operand).ops.clone();
    let first = lane_constant(dag, *lanes.first()?, bits)?;
    lanes.iter().all(|&lane| lane_constant(dag, lane, bits) == Some(first)).then_some(first)
}

/// `LowerVectorSRA_SRL_SHL`
/// ([L18181](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L18181)):
/// a shift by a constant splat is `VSHL`, `VLSHR` or `VASHR` of the immediate
/// (`isVShiftLImm`: `0 <= n < esize`; `isVShiftRImm`: `1 <= n <= esize`).
///
/// DEVIATE VEC4: a shift by a vector that is no constant splat is the pin's
/// `ushl`/`sshl` intrinsic (and, to the right, of the negated amount), which
/// the DAG here has no node for; it is declined and the block refused. The
/// loop vectorizer widens only shifts by a constant. Exit: LV-X3, 2026-11-20.
pub fn lower_vector_shift(
    dag: &mut SelectionDAG,
    op: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(op);
    if !dag.value_type(dag.operand(op, 1)).is_vector() {
        return Some(op);
    }
    let esize = u64::from(vt.element().size_in_bits());
    let amount = splat_constant(dag, dag.operand(op, 1))?;
    let value = dag.operand(op, 0);
    let (opcode, valid) = match dag.opcode(op) {
        ISD::Shl => (aarch64_isd::VSHL, amount < esize),
        ISD::Srl => (aarch64_isd::VLSHR, (1..=esize).contains(&amount)),
        ISD::Sra => (aarch64_isd::VASHR, (1..=esize).contains(&amount)),
        _ => return None,
    };
    if !valid {
        return None;
    }
    let count = dag.get_constant_with(u128::from(amount), EVT::I32, true, false);
    let flags = dag.node(op).flags;
    Some(dag.get_node_flags(ISD::Target(opcode), vt, &[value, count], flags))
}
