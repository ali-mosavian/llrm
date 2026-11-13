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
//! lowered; any other (the pin loads it from a constant pool, or builds it
//! with `INSERT_VECTOR_ELT`) is declined and the block refused. A constant
//! with undefined lanes is tried only as the splat of its defined lanes
//! (`UndefBits` of `ConstantBuildVector` is not tried), and `FMOV`, the
//! `FNEG` forms and SVE are not made. A shuffle is lowered where it is a
//! splat of a scalar (`DUP`); `DUPLANE` and the other masks are declined.
//! Exit: the rest of both lowerings, with reverse (`REV64`) first.

use llrm_codegen::backend::selectiondag::SDValue;
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
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
        // DEVIATE VEC1: a constant that is not a splat is loaded in the pin.
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
