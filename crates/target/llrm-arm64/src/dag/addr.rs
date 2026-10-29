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
//! refusal, follows): `SelectShiftedRegisterFromAnd`,
//! `SelectArithExtendedRegister`, `SelectArithUXTXRegister`,
//! `SelectCVTFixedPos*`, `SelectExtractHigh`, `SelectDupZero`, and
//! `SelectAddrModeIndexed7S*`. `isWorthNegatingImm` is false. Reason: the first
//! loads and stores need none. Exit: pairs and globals.
//!
//! # See also
//! `llvm/lib/Target/AArch64/AArch64ISelDAGToDAG.cpp`, pin d1106deb71cc.

use llrm_dagisel::dag::SDValue;
use llrm_dagisel::generated::aarch64::ids::isd as a64_isd;
use llrm_dagisel::generated::ids::{isd, mvt};
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

/// `SelectArithImmed`: the immediate and its shifter operand.
pub fn arith_immed<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 2]> {
    let immed = zext(dag, n)?;
    if !legal_arith_immed(immed) {
        return None;
    }
    // `getArithImmedShift`: 12 when the low 12 bits are clear and it is not
    // already small.
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
    let negated =
        if dag.value_type(n) == mvt::i32 { u64::from((immed as u32).wrapping_neg()) } else { immed.wrapping_neg() };
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

/// `SelectShiftedRegister` without `SelectShiftedRegisterFromAnd`.
pub fn shifted_register<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    allow_ror: bool,
) -> Option<[SDValue; 2]> {
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
        // A symbol the access is aligned to, at an offset it divides, folds as
        // a page offset.
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
    dag.users(v)
        .into_iter()
        .all(
            |(user, _)| is_mem_op_or_prefetch(dag, user)
                || dag.users(SDValue::new(user, 0)).into_iter().all(|(inner, _)| is_mem_op_or_prefetch(dag, inner)),
        )
}

/// `isWorthFoldingAddr(V, Size)` for a CPU with no `AddrLSLSlow14` and no size
/// goal.
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

/// `SelectExtendedSHL` with `WantExtend` false: `(shl x, c)` for the `XRO`
/// mode.
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

/// `SelectAddrModeXRO`: base, index register, sign-extend flag (0) and shift
/// flag.
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
    // A wide constant offset takes the register form after a `MOVi64imm`: not
    // translated.
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
