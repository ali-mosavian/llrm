//! `TargetLowering::expandAddSubSat`
//! ([L12375](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L12375)):
//! a saturating add or subtract as the wrapped one and a select on its
//! overflow. Shared by the operation legalizer (a legal width) and the type
//! legalizer (a width it has expanded or promoted).
//!
//! The vector forms are not translated; this target has no vector types.

use super::actions::TargetLowering;
use super::dag::{SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::known_bits::known_bits;
use super::mvt::EVT;

pub fn expand_add_sub_sat(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
) -> SDValue {
    let op = dag.opcode(node);
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let bits = vt.size_in_bits();
    let bool_vt = tli.types.set_cc_result_type(vt);

    // usub.sat(a, b) -> umax(a, b) - b
    if op == ISD::USubSat && tli.is_operation_legal(ISD::UMax, vt) {
        let max = dag.get_node(ISD::UMax, vt, &[lhs, rhs]);
        return dag.get_node(ISD::Sub, vt, &[max, rhs]);
    }
    // usub.sat(a, 1) -> sub(a, zext(a != 0)), where there is no cheap borrow.
    if op == ISD::USubSat && dag.constant(rhs) == Some(1) && !tli.is_operation_legal_or_custom(ISD::USubOCarry, vt) {
        let lhs = dag.get_node(ISD::Freeze, vt, &[lhs]);
        let zero = dag.get_constant(0, vt);
        let non_zero = dag.get_setcc(bool_vt, lhs, zero, CondCode::SetNe);
        let flag = match bool_vt.size_in_bits().cmp(&bits) {
            std::cmp::Ordering::Less => dag.get_node(ISD::ZeroExtend, vt, &[non_zero]),
            std::cmp::Ordering::Greater => dag.get_node(ISD::Truncate, vt, &[non_zero]),
            std::cmp::Ordering::Equal => non_zero,
        };
        let one = dag.get_constant(1, vt);
        let subtrahend = dag.get_node(ISD::And, vt, &[flag, one]);
        return dag.get_node(ISD::Sub, vt, &[lhs, subtrahend]);
    }
    // uadd.sat(a, b) -> umin(a, ~b) + b
    if op == ISD::UAddSat && tli.is_operation_legal(ISD::UMin, vt) {
        let inverse = dag.get_not(rhs, vt);
        let min = dag.get_node(ISD::UMin, vt, &[lhs, inverse]);
        return dag.get_node(ISD::Add, vt, &[min, rhs]);
    }

    let overflow_op = match op {
        ISD::SAddSat => ISD::SAddO,
        ISD::UAddSat => ISD::UAddO,
        ISD::SSubSat => ISD::SSubO,
        _ => ISD::USubO,
    };
    let result = dag.get_node_multi(overflow_op, &[vt, bool_vt], &[lhs, rhs]);
    let (sum_diff, overflow) = (result, SDValue { node: result.node, res: 1 });
    // Booleans here are zero or one, so the select forms of the pin apply.
    match op {
        ISD::UAddSat => {
            let ones = dag.get_all_ones_constant(vt);
            return dag.get_select(vt, overflow, ones, sum_diff);
        }
        ISD::USubSat => {
            let zero = dag.get_constant(0, vt);
            return dag.get_select(vt, overflow, zero, sum_diff);
        }
        _ => {}
    }

    let min = 1u128 << (bits - 1);
    let max = min - 1;
    // A known operand sign fixes the direction of any overflow; for a
    // subtract the subtrahend's sign is flipped.
    let (known_lhs, known_rhs) = (known_bits(dag, lhs), known_bits(dag, rhs));
    let sign =
        |known: &super::known_bits::KnownBits| (known.zero >> (bits - 1) & 1 == 1, known.one >> (bits - 1) & 1 == 1);
    let ((lhs_non_negative, lhs_negative), (rhs_non_negative, rhs_negative)) = (sign(&known_lhs), sign(&known_rhs));
    let (rhs_up, rhs_down) =
        if op == ISD::SAddSat { (rhs_non_negative, rhs_negative) } else { (rhs_negative, rhs_non_negative) };
    if lhs_non_negative || rhs_up {
        let saturated = dag.get_constant(max, vt);
        return dag.get_select(vt, overflow, saturated, sum_diff);
    }
    if lhs_negative || rhs_down {
        let saturated = dag.get_constant(min, vt);
        return dag.get_select(vt, overflow, saturated, sum_diff);
    }
    // Overflow ? (SumDiff >> BW) ^ MinVal : SumDiff
    let sat_min = dag.get_constant(min, vt);
    let amount = dag.get_constant(u128::from(bits - 1), EVT::I8);
    let shifted = dag.get_node(ISD::Sra, vt, &[sum_diff, amount]);
    let saturated = dag.get_node(ISD::Xor, vt, &[shifted, sat_min]);
    dag.get_select(vt, overflow, saturated, sum_diff)
}
