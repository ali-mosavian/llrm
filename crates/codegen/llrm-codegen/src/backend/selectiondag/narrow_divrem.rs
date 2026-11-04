//! A wide signed divide or remainder whose operands are sign extensions of a
//! narrower value divides at the narrower width.
//!
//! Not a pin site, an improvement over it: `ExpandIntRes_SDIV`/`SREM`
//! ([L4916](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L4916),
//! [L5218](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L5218))
//! make the full-width libcall, and the pin's O1 has no correlated-propagation
//! to narrow the node first. On a target whose registers are the narrow width
//! that is `__modsi3` for a `srem i32` of two values that fit 16 bits (+434 B
//! on recpow).
//!
//! The rule is `narrowSDivOrSRem`
//! ([L775](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L775)):
//! the narrow quotient of `INT_MIN / -1` overflows where the wide one does not,
//! so the width is bumped by a bit unless the dividend has a spare sign bit or
//! the divisor is a constant other than -1. That holds for a remainder too,
//! unless the target says its narrow remainder is defined there
//! ([`DagConfig::narrow_srem_overflow_defined`](super::dag::DagConfig)).

use super::dag::{SDValue, SelectionDAG};
use super::isd::ISD;
use super::known_bits::{known_bits, num_sign_bits};
use super::mvt::EVT;

/// `node` (a `SDIV` or `SREM`) as `sext(op(trunc a, trunc b))` at `narrow`,
/// when both operands fit `narrow` signed. `None` otherwise; the caller keeps
/// its wide lowering.
pub fn narrow_sdivrem(
    dag: &mut SelectionDAG,
    node: SDValue,
    narrow: EVT,
) -> Option<SDValue> {
    let op = dag.opcode(node);
    if !matches!(op, ISD::SDiv | ISD::SRem) {
        return None;
    }
    let vt = dag.value_type(node);
    let (wide_bits, narrow_bits) = (vt.size_in_bits(), narrow.size_in_bits());
    if narrow_bits >= wide_bits {
        return None;
    }
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    let fits = wide_bits - narrow_bits + 1;
    if num_sign_bits(dag, a) < fits || num_sign_bits(dag, b) < fits {
        return None;
    }
    if op == ISD::SDiv || !dag.config().narrow_srem_overflow_defined {
        let spare_bit = num_sign_bits(dag, a) > fits;
        let not_minus_one = dag.constant(b).is_some_and(|c| c != super::fold::mask(wide_bits));
        if !spare_bit && !not_minus_one {
            return None;
        }
    }
    let (a, b) = (dag.get_node(ISD::Truncate, narrow, &[a]), dag.get_node(ISD::Truncate, narrow, &[b]));
    let narrowed = dag.get_node(op, narrow, &[a, b]);
    Some(dag.get_node(ISD::SignExtend, vt, &[narrowed]))
}

/// `node` (a `UDIV` or `UREM`) as `zext(op(trunc a, trunc b))` at `narrow`,
/// when both operands have their upper bits known zero: the pin's
/// `narrowUDivOrURem`
/// ([L903](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L903)),
/// which only the O2 pipeline runs, made at the DAG for the narrow-register
/// targets (nbody_fixed at 6502 -O1, +312 B: `ONE / (d >> 18) + 1`). No
/// overflow case: unsigned.
pub fn narrow_udivrem(
    dag: &mut SelectionDAG,
    node: SDValue,
    narrow: EVT,
) -> Option<SDValue> {
    let op = dag.opcode(node);
    if !matches!(op, ISD::UDiv | ISD::URem) {
        return None;
    }
    let vt = dag.value_type(node);
    let (wide_bits, narrow_bits) = (vt.size_in_bits(), narrow.size_in_bits());
    if narrow_bits >= wide_bits {
        return None;
    }
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    let fits = |value: SDValue| {
        let known = known_bits(dag, value);
        (narrow_bits..wide_bits).all(|bit| known.zero >> bit & 1 == 1)
    };
    if !fits(a) || !fits(b) {
        return None;
    }
    let (a, b) = (dag.get_node(ISD::Truncate, narrow, &[a]), dag.get_node(ISD::Truncate, narrow, &[b]));
    let narrowed = dag.get_node(op, narrow, &[a, b]);
    Some(dag.get_node(ISD::ZeroExtend, vt, &[narrowed]))
}
