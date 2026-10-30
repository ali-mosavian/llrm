//! The `Select*` complex patterns of `AArch64DAGToDAGISel` the scalar table
//! names: arithmetic immediates, shifted registers and the load/store
//! addressing modes.
//!
//! # How it works
//! Each function reads a pointer or operand expression and, when it has the
//! shape, returns the operands the table expects, in the order of the pin's
//! signature (`SelectAddrModeIndexed`: base, scaled offset).
//!
//! DEVIATE: not translated, and answering false (the pattern after it, or a
//! refusal, follows):
//! `SelectExtractHigh`, `SelectDupZero`; `SelectCVTFixedPos*` reads only a
//! `ConstantFP`, not one loaded from the constant pool. `isWorthNegatingImm` is
//! false. Reason: the first loads and stores need none. Exit: pairs and
//! globals.
//!
//! # See also
//! `llvm/lib/Target/AArch64/AArch64ISelDAGToDAG.cpp`, pin d1106deb71cc.

use llrm_dagisel::dag::SDValue;
use llrm_dagisel::generated::aarch64::ids::isd as a64_isd;
use llrm_dagisel::generated::aarch64::ids::opcode as a64_opcode;
use llrm_dagisel::generated::aarch64::ids::reg::sub_32;
use llrm_dagisel::generated::ids::{isd, mvt, target_opcode};
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::x86_addr::AddrDag;

use super::globals::Globals;

/// The width of an integer value type, 0 for any other.
fn bits(vt: Vt) -> u32 {
    [(mvt::i1, 1), (mvt::i8, 8), (mvt::i16, 16), (mvt::i32, 32), (mvt::i64, 64)]
        .into_iter()
        .find(|(one, _)| *one == vt)
        .map_or(0, |(_, n)| n)
}

/// `getAsZExtVal` of a constant, at the width of its type.
pub fn zext<D: AddrDag>(
    dag: &D,
    value: SDValue,
) -> Option<u64> {
    let constant = dag.constant(value)? as u64;
    Some(match bits(dag.value_type(value)) {
        0 | 64 => constant,
        width => constant & ((1u64 << width) - 1),
    })
}

fn log2(size: u32) -> u32 {
    size.trailing_zeros()
}

/// `isLegalArithImmed`: 12 bits, or 12 bits shifted left by 12.
fn legal_arith_immed(immed: u64) -> bool {
    immed >> 12 == 0 || (immed & 0xfff == 0 && immed >> 24 == 0)
}

/// `SelectCVTFixedPosOperand` and `...RecipOperand`: `n` is a float constant that is
/// `2^fbits` (its reciprocal, for `reciprocal`), `1 <= fbits <= reg_width`; the operand is
/// `fbits`. `CheckFixedPointOperandConstant` in `AArch64BaseInfo.h`.
pub fn cvt_fixed_pos<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    reg_width: u32,
    reciprocal: bool,
) -> Option<[SDValue; 1]> {
    let bits = dag.fp_bits(n)?;
    let value = match dag.value_type(n) {
        t if t == mvt::f64 => f64::from_bits(bits as u64),
        t if t == mvt::f32 => f64::from(f32::from_bits(bits as u32)),
        _ => return None,
    };
    let value = if reciprocal { value.recip() } else { value };
    // A power of two has no fraction and exactly one bit set.
    if !(value >= 1.0 && value.is_finite() && value.fract() == 0.0 && value.to_bits() << 12 == 0) {
        return None;
    }
    let fbits = value.log2() as u32;
    if fbits == 0 || fbits > reg_width {
        return None;
    }
    Some([dag.target_constant(i64::from(fbits), mvt::i32)])
}

/// `SelectArithImmed`: the immediate and its shifter operand.
pub fn arith_immed<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 2]> {
    let immed = zext(dag, n)?;
    if !legal_arith_immed(immed) {
        return None;
    }
    // `getArithImmedShift`: 12 when the low 12 bits are clear and it is not already small.
    let shift = if immed >> 12 != 0 && immed & 0xfff == 0 { 12 } else { 0 };
    let value = dag.target_constant((immed >> shift) as i64, mvt::i32);
    let shifter = dag.target_constant(shift as i64, mvt::i32);
    Some([value, shifter])
}

/// `SelectNegArithImmed`: `-immediate` as an arithmetic immediate.
pub fn neg_arith_immed<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 2]> {
    let immed = zext(dag, n)?;
    if immed == 0 {
        return None;
    }
    let negated = if dag.value_type(n) == mvt::i32 { u64::from((immed as u32).wrapping_neg()) } else { immed.wrapping_neg() };
    if negated & 0xffff_ffff_ff00_0000 != 0 {
        return None;
    }
    let negated = negated & 0xff_ffff;
    if !legal_arith_immed(negated) {
        return None;
    }
    let shift = if negated >> 12 != 0 && negated & 0xfff == 0 { 12 } else { 0 };
    let value = dag.target_constant((negated >> shift) as i64, mvt::i32);
    let shifter = dag.target_constant(shift as i64, mvt::i32);
    Some([value, shifter])
}

/// `getShifterImm(type, amount)`: the type in bits 6.., the amount below.
fn shifter_imm(
    shift_type: u32,
    amount: u32,
) -> i64 {
    i64::from((shift_type << 6) | amount)
}

/// `isWorthFoldingALU(V, LSL)` on a CPU with neither `ALULSLFast` nor a size
/// goal: `V` has one use.
fn worth_folding_alu<D: AddrDag>(
    dag: &D,
    v: SDValue,
) -> bool {
    dag.has_one_use(v)
}

/// `SelectShiftedRegisterFromAnd`: `and (shl|srl|sra x, c), mask` for a shifted
/// mask is one bitfield move and a `lsl` of the register operand.
fn shifted_register_from_and<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 2]> {
    let vt = dag.value_type(n);
    let width = bits(vt);
    if (width != 32 && width != 64) || dag.opcode(n.node) != isd::AND || !dag.node_has_one_use(n.node) {
        return None;
    }
    let lhs = dag.operand(n.node, 0);
    let shift_op = dag.opcode(lhs.node);
    if !dag.has_one_use(lhs) || ![isd::SHL, isd::SRL, isd::SRA].contains(&shift_op) {
        return None;
    }
    let shift = zext(dag, dag.operand(lhs.node, 1))?;
    let mask = zext(dag, dag.operand(n.node, 1))?;
    let low_zero = mask.trailing_zeros();
    let mask_len = (mask >> low_zero.min(63)).trailing_ones();
    if mask == 0 || (mask >> low_zero) >> mask_len != 0 {
        return None;
    }
    let wide = width == 64;
    let (new_shift, opcode) = if shift_op == isd::SHL {
        // `LowZBits <= ShiftAmtC` is a bitfield positioning op.
        if u64::from(low_zero) <= shift || width != low_zero + mask_len {
            return None;
        }
        (u64::from(low_zero) - shift, if wide { a64_opcode::UBFMXri } else { a64_opcode::UBFMWri })
    } else {
        if low_zero == 0 {
            return None;
        }
        let new_shift = u64::from(low_zero) + shift;
        if new_shift >= u64::from(width)
            || (shift_op == isd::SRA && width != low_zero + mask_len)
            || (shift_op == isd::SRL && u64::from(width) > new_shift + u64::from(mask_len))
        {
            return None;
        }
        let opcode = match (shift_op == isd::SRL, wide) {
            (true, true) => a64_opcode::UBFMXri,
            (true, false) => a64_opcode::UBFMWri,
            (false, true) => a64_opcode::SBFMXri,
            (false, false) => a64_opcode::SBFMWri,
        };
        (new_shift, opcode)
    };
    let amount = dag.target_constant(new_shift as i64, vt);
    let last = dag.target_constant(i64::from(width) - 1, vt);
    let source = dag.operand(lhs.node, 0);
    let reg = SDValue::new(dag.machine_node(opcode, &[vt], &[source, amount, last]), 0);
    let shifter = dag.target_constant(shifter_imm(0, low_zero), mvt::i32);
    Some([reg, shifter])
}

/// `SelectShiftedRegister`.
pub fn shifted_register<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    allow_ror: bool,
) -> Option<[SDValue; 2]> {
    if let Some(found) = shifted_register_from_and(dag, n) {
        return Some(found);
    }
    // LSL 0, LSR 1, ASR 2, ROR 3.
    let shift_type = match dag.opcode(n.node) {
        o if o == isd::SHL => 0,
        o if o == isd::SRL => 1,
        o if o == isd::SRA => 2,
        o if o == isd::ROTR && allow_ror => 3,
        _ => return None,
    };
    let amount = zext(dag, dag.operand(n.node, 1))?;
    let width = bits(dag.value_type(n));
    let reg = dag.operand(n.node, 0);
    let shift = dag.target_constant(shifter_imm(shift_type, (amount & u64::from(width - 1)) as u32), mvt::i32);
    worth_folding_alu(dag, n).then_some([reg, shift])
}

/// `isBaseWithConstantOffset`: an `ADD` (or an `OR` of disjoint bits) of a
/// constant.
fn base_with_constant_offset<D: AddrDag>(
    dag: &D,
    n: SDValue,
) -> bool {
    let opcode = dag.opcode(n.node);
    (opcode == isd::ADD || (opcode == isd::OR && dag.is_add_like(n))) && dag.constant(dag.operand(n.node, 1)).is_some()
}

/// A frame index operand as its target form, any other as it is.
fn base_operand<D: AddrDag>(
    dag: &mut D,
    base: SDValue,
) -> SDValue {
    match dag.frame_index(base) {
        Some(index) => dag.target_frame_index(index, mvt::i64),
        None => base,
    }
}

fn valid_as_scaled_immediate(
    offset: i64,
    range: u64,
    size: u32,
) -> bool {
    (offset & (i64::from(size) - 1)) == 0 && offset >= 0 && (offset as u64) < (range << log2(size))
}

/// `isWorthFoldingADDlow`: every user is a plain load or store.
fn worth_folding_addlow<D: AddrDag>(
    dag: &D,
    n: SDValue,
) -> bool {
    dag.users(n).into_iter().all(|(user, _)| matches!(dag.opcode(user), o if o == isd::LOAD || o == isd::STORE))
}

/// `SelectAddrModeIndexed`: base and 12-bit scaled offset.
pub fn addr_mode_indexed<D: AddrDag>(
    dag: &mut D,
    globals: &Globals,
    n: SDValue,
    size: u32,
) -> Option<[SDValue; 2]> {
    if let Some(index) = dag.frame_index(n) {
        let base = dag.target_frame_index(index, mvt::i64);
        return Some([base, dag.target_constant(0, mvt::i64)]);
    }
    if dag.opcode(n.node) == a64_isd::ADDlow && worth_folding_addlow(dag, n) {
        let (base, low) = (dag.operand(n.node, 0), dag.operand(n.node, 1));
        // A symbol the access is aligned to, at an offset it divides, folds as a page offset.
        let aligned = dag.wrapped_symbol(low).map(|w| {
            w.offset % i64::from(size) == 0 && globals.get(w.symbol.id).is_some_and(|g| g.align >= u64::from(size))
        });
        if aligned != Some(false) {
            return Some([base, low]);
        }
    }
    if base_with_constant_offset(dag, n) {
        if let Some(offset) = zext(dag, dag.operand(n.node, 1)).map(|c| c as i64) {
            if valid_as_scaled_immediate(offset, 0x1000, size) {
                let base = base_operand(dag, dag.operand(n.node, 0));
                return Some([base, dag.target_constant(offset >> log2(size), mvt::i64)]);
            }
        }
    }
    // The unscaled forms are preferred for an offset the scaled one lacks.
    if addr_mode_unscaled(dag, n).is_some() {
        return None;
    }
    Some([n, dag.target_constant(0, mvt::i64)])
}

/// `SelectAddrModeUnscaled`: base and signed 9-bit offset.
pub fn addr_mode_unscaled<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 2]> {
    if !base_with_constant_offset(dag, n) {
        return None;
    }
    let offset = dag.constant(dag.operand(n.node, 1))?;
    if !(-256..256).contains(&offset) {
        return None;
    }
    let base = base_operand(dag, dag.operand(n.node, 0));
    Some([base, dag.target_constant(offset, mvt::i64)])
}

/// `isMemOpOrPrefetch`.
fn is_mem_op_or_prefetch<D: AddrDag>(
    dag: &D,
    node: u32,
) -> bool {
    dag.mem_operands(node).is_some() || dag.opcode(node) == a64_isd::PREFETCH
}

/// `isWorthFoldingSHL`: a shift of up to three that no non-memory user keeps.
fn worth_folding_shl<D: AddrDag>(
    dag: &D,
    v: SDValue,
) -> bool {
    match dag.constant(dag.operand(v.node, 1)) {
        Some(amount) if (0..=3).contains(&amount) => {}
        _ => return false,
    }
    dag.users(v).into_iter().all(|(user, _)| {
        is_mem_op_or_prefetch(dag, user)
            || dag.users(SDValue::new(user, 0)).into_iter().all(|(inner, _)| is_mem_op_or_prefetch(dag, inner))
    })
}

/// `isWorthFoldingAddr(V, Size)` for a CPU with no `AddrLSLSlow14` and no size goal.
fn worth_folding_addr<D: AddrDag>(
    dag: &D,
    v: SDValue,
) -> bool {
    if dag.has_one_use(v) {
        return true;
    }
    if dag.opcode(v.node) == isd::SHL && worth_folding_shl(dag, v) {
        return true;
    }
    if dag.opcode(v.node) == isd::ADD {
        let (lhs, rhs) = (dag.operand(v.node, 0), dag.operand(v.node, 1));
        return [lhs, rhs].into_iter().any(|side| dag.opcode(side.node) == isd::SHL && worth_folding_shl(dag, side));
    }
    false
}

/// `SelectExtendedSHL` with `WantExtend` false: `(shl x, c)` for the `XRO` mode.
fn extended_shl<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    size: u32,
) -> Option<SDValue> {
    let amount = zext(dag, dag.operand(n.node, 1))?;
    if amount & 7 != amount || (amount != 0 && amount != u64::from(log2(size))) {
        return None;
    }
    worth_folding_addr(dag, n).then(|| dag.operand(n.node, 0))
}

/// `SelectAddrModeXRO`: base, index register, sign-extend flag (0) and shift flag.
pub fn addr_mode_xro<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    size: u32,
) -> Option<[SDValue; 4]> {
    if dag.opcode(n.node) != isd::ADD {
        return None;
    }
    let (lhs, rhs) = (dag.operand(n.node, 0), dag.operand(n.node, 1));
    if dag.users(n).into_iter().any(|(user, _)| !is_mem_op_or_prefetch(dag, user)) {
        return None;
    }
    // A wide constant offset takes the register form after a `MOVi64imm`: not translated.
    if dag.constant(rhs).is_some() {
        return None;
    }
    let worth = worth_folding_addr(dag, n);
    let flag = |dag: &mut D, on: i64| dag.target_constant(on, mvt::i32);
    for (shl, base) in [(rhs, lhs), (lhs, rhs)] {
        if worth && dag.opcode(shl.node) == isd::SHL {
            if let Some(offset) = extended_shl(dag, shl, size) {
                let (sign, shift) = (flag(dag, 0), flag(dag, 1));
                return Some([base, offset, sign, shift]);
            }
        }
    }
    let (sign, shift) = (flag(dag, 0), flag(dag, 0));
    Some([lhs, rhs, sign, shift])
}

/// `AArch64_AM::ShiftExtendType`, by `getExtendEncoding`.
const UXTB: u8 = 0;
const UXTH: u8 = 1;
const UXTW: u8 = 2;
const UXTX: u8 = 3;
const SXTB: u8 = 4;
const SXTH: u8 = 5;
const SXTW: u8 = 6;

/// `getExtendTypeForNode(N, false)`: the extend a node performs, if it is one.
fn extend_type<D: AddrDag>(
    dag: &D,
    n: SDValue,
) -> Option<u8> {
    let op = dag.opcode(n.node);
    let from = |sign: bool, src: Vt| match bits(src) {
        8 => Some(if sign { SXTB } else { UXTB }),
        16 => Some(if sign { SXTH } else { UXTH }),
        32 => Some(if sign { SXTW } else { UXTW }),
        _ => None,
    };
    if op == isd::SIGN_EXTEND {
        from(true, dag.value_type(dag.operand(n.node, 0)))
    } else if op == isd::SIGN_EXTEND_INREG {
        from(true, dag.value_type_node(dag.operand(n.node, 1))?)
    } else if op == isd::ZERO_EXTEND || op == isd::ANY_EXTEND {
        from(false, dag.value_type(dag.operand(n.node, 0)))
    } else if op == isd::AND {
        match zext(dag, dag.operand(n.node, 1))? {
            0xff => Some(UXTB),
            0xffff => Some(UXTH),
            0xffff_ffff => Some(UXTW),
            _ => None,
        }
    } else {
        None
    }
}

/// `narrowIfNeeded`: a 64-bit register read as its low 32 bits.
fn narrow_if_needed<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> SDValue {
    if dag.value_type(n) == mvt::i32 {
        return n;
    }
    let index = dag.target_constant(i64::from(sub_32), mvt::i32);
    SDValue::new(dag.machine_node(target_opcode::EXTRACT_SUBREG, &[mvt::i32], &[n, index]), 0)
}

/// `getExtendTypeForNode(N, true)`: the extends a load or store index takes
/// (`sxtw`, `uxtw`).
fn index_extend_type<D: AddrDag>(
    dag: &D,
    n: SDValue,
) -> Option<u8> {
    extend_type(dag, n).filter(|&ext| ext == SXTW || ext == UXTW)
}

/// `SelectExtendedSHL` with `WantExtend` true: `(shl (extend x), c)`.
fn extended_shl_of_extend<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    size: u32,
) -> Option<[SDValue; 2]> {
    let amount = zext(dag, dag.operand(n.node, 1))?;
    if amount & 7 != amount {
        return None;
    }
    let extend = dag.operand(n.node, 0);
    let ext = index_extend_type(dag, extend)?;
    let offset = narrow_if_needed(dag, dag.operand(extend.node, 0));
    let sign = dag.target_constant(i64::from(ext == SXTW), mvt::i32);
    if amount != 0 && amount != u64::from(log2(size)) {
        return None;
    }
    worth_folding_addr(dag, n).then_some([offset, sign])
}

/// `SelectAddrModeWRO`: base, a 32-bit index register, sign-extend flag and
/// shift flag.
pub fn addr_mode_wro<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    size: u32,
) -> Option<[SDValue; 4]> {
    if dag.opcode(n.node) != isd::ADD {
        return None;
    }
    let (lhs, rhs) = (dag.operand(n.node, 0), dag.operand(n.node, 1));
    if dag.constant(lhs).is_some() || dag.constant(rhs).is_some() {
        return None;
    }
    if dag.users(n).into_iter().any(|(user, _)| !is_mem_op_or_prefetch(dag, user)) {
        return None;
    }
    let worth = worth_folding_addr(dag, n);
    let flag = |dag: &mut D, on: i64| dag.target_constant(on, mvt::i32);
    for (shl, base) in [(rhs, lhs), (lhs, rhs)] {
        if worth && dag.opcode(shl.node) == isd::SHL {
            if let Some([offset, sign]) = extended_shl_of_extend(dag, shl, size) {
                return Some([base, offset, sign, flag(dag, 1)]);
            }
        }
    }
    for (extend, base) in [(lhs, rhs), (rhs, lhs)] {
        if !worth {
            break;
        }
        if let Some(ext) = index_extend_type(dag, extend) {
            let offset = narrow_if_needed(dag, dag.operand(extend.node, 0));
            let sign = flag(dag, i64::from(ext == SXTW));
            if worth_folding_addr(dag, extend) {
                return Some([base, offset, sign, flag(dag, 0)]);
            }
        }
    }
    None
}

/// `SelectArithExtendedRegister`: an extend, with an optional shift of up to
/// four, as an `add`/`sub`/`cmp` operand: the register and the encoded
/// extend.
///
/// DEVIATE: the extend of a vector lane answers false (the pin refuses only a
/// fixed-length vector). Exit: vectors.
pub fn arith_extended_register<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 2]> {
    let (ext, shift, reg) = if dag.opcode(n.node) == isd::SHL {
        let shift = zext(dag, dag.operand(n.node, 1)).filter(|&s| s <= 4)?;
        let extend = dag.operand(n.node, 0);
        (extend_type(dag, extend)?, shift, dag.operand(extend.node, 0))
    } else {
        let ext = extend_type(dag, n)?;
        let mut inner = dag.operand(n.node, 0);
        if ext >= SXTB {
            if dag.opcode(inner.node) == isd::ANY_EXTEND {
                inner = dag.operand(inner.node, 0);
            }
            if dag.opcode(inner.node) == isd::EXTRACT_VECTOR_ELT {
                return None;
            }
        }
        let reg = dag.operand(n.node, 0);
        // A 32-bit definition is already zero-extended for free.
        let opcode = dag.opcode(reg.node);
        let is_def32 = ![isd::TRUNCATE, isd::CopyFromReg, isd::AssertSext, isd::AssertZext, isd::AssertAlign, isd::FREEZE]
            .contains(&opcode)
            && !(dag.is_machine(reg.node) && opcode == target_opcode::EXTRACT_SUBREG);
        if ext == UXTW && dag.value_type(reg) == mvt::i32 && is_def32 {
            return None;
        }
        (ext, 0, reg)
    };
    let reg = narrow_if_needed(dag, reg);
    let imm = dag.target_constant((i64::from(ext) << 3) | shift as i64, mvt::i32);
    worth_folding_alu(dag, n).then_some([reg, imm])
}

/// `SelectArithUXTXRegister`: `(shl x, c)` for `c <= 4` as `uxtx #c`, the form
/// an operand next to `sp` needs.
pub fn arith_uxtx_register<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 2]> {
    if dag.opcode(n.node) != isd::SHL {
        return None;
    }
    let shift = zext(dag, dag.operand(n.node, 1)).filter(|&s| s <= 4)?;
    let imm = dag.target_constant((i64::from(UXTX) << 3) | shift as i64, mvt::i32);
    worth_folding_alu(dag, n).then(|| [dag.operand(n.node, 0), imm])
}

/// `SelectAddrModeIndexedBitWidth` (signed, 7 bits): the `ldp`/`stp` offset,
/// `base + size * imm7`; any other address is the base alone.
pub fn addr_mode_indexed_7s<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    size: u32,
) -> [SDValue; 2] {
    if let Some(index) = dag.frame_index(n) {
        let base = dag.target_frame_index(index, mvt::i64);
        return [base, dag.target_constant(0, mvt::i64)];
    }
    if base_with_constant_offset(dag, n) {
        if let Some(offset) = dag.constant(dag.operand(n.node, 1)) {
            let scale = log2(size);
            let range = 64i64 << scale;
            if offset & (i64::from(size) - 1) == 0 && (-range..range).contains(&offset) {
                let base = base_operand(dag, dag.operand(n.node, 0));
                return [base, dag.target_constant(offset >> scale, mvt::i64)];
            }
        }
    }
    [n, dag.target_constant(0, mvt::i64)]
}
