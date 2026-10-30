//! `AArch64DAGToDAGISel::tryBitfieldInsertInZeroOp`: `(and (shl x, n), mask)` of one contiguous run
//! of bits is `UBFM` (`UBFIZ`).
//!
//! DEVIATE: the non-zero bits come from `known_zero`, the structural subset of `computeKnownBits`
//! (SD40), and the `and (any_extend (shl ..))` form of an `i64` is not matched. Exit: with the DAG's
//! known bits and the `Widen` helper.
//!
//! # See also
//! `isBitfieldPositioningOp`, `AArch64ISelDAGToDAG.cpp`, pin d1106deb71cc.

use llrm_dagisel::dag::SDValue as PinValue;
use llrm_dagisel::generated::aarch64::ids::opcode as aarch64_ids;
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::Outcome;
use llrm_dagisel::x86_addr::{AddrDag, known_zero};

fn is_shifted_mask(x: u64) -> bool {
    x != 0 && {
        let filled = x | (x - 1);
        filled.wrapping_add(1) & filled == 0
    }
}

/// `tryBitfieldInsertInZeroOp`. `None` for any node it does not match.
pub fn select_bitfield_insert_in_zero(
    dag: &mut impl AddrDag,
    node: u32,
) -> Option<Outcome> {
    if dag.opcode(node) != isd::AND {
        return None;
    }
    let value = PinValue::new(node, 0);
    let vt = dag.value_type(value);
    let (bits, opcode) = match vt {
        mvt::i32 => (32u32, aarch64_ids::UBFMWri),
        mvt::i64 => (64, aarch64_ids::UBFMXri),
        _ => return None,
    };
    let all = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
    // `isBitfieldPositioningOp`: the bits not provably zero are one run.
    let non_zero = !known_zero(dag, value, 0) & all;
    if !is_shifted_mask(non_zero) {
        return None;
    }
    // `isBitfieldPositioningOpFromAnd`, for UBFIZ (`BiggerPattern` false).
    dag.constant(dag.operand(node, 1))?;
    let shl = dag.operand(node, 0);
    if dag.opcode(shl.node) != isd::SHL {
        return None;
    }
    let shl_imm = u64::try_from(dag.constant(dag.operand(shl.node, 1))?).ok()?;
    let source = dag.operand(shl.node, 0);
    if !dag.has_one_use(shl) {
        return None;
    }
    let dst_lsb = non_zero.trailing_zeros();
    let width = (non_zero >> dst_lsb).trailing_ones();
    if width >= bits || shl_imm != u64::from(dst_lsb) {
        return None;
    }
    let immr = (bits - dst_lsb) % bits;
    let imms = width - 1;
    let (immr, imms) = (dag.target_constant(i64::from(immr), vt), dag.target_constant(i64::from(imms), vt));
    dag.morph_node_to(node, opcode, &[vt], &[source, immr, imms]);
    Some(Outcome::Selected)
}
