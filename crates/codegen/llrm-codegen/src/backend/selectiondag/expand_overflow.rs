//! `TargetLowering::expandUADDSUBO`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp>):
//! an unsigned add or subtract with overflow as the wrapped operation and a
//! compare. Used by the operation legalizer where the target leaves the
//! operation to be expanded.

use super::actions::TargetLowering;
use super::dag::{SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};

/// The result and the overflow of a `UAddO` or `USubO`.
pub fn expand_uaddsubo(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
) -> Vec<SDValue> {
    let is_add = dag.opcode(node) == ISD::UAddO;
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let result_type = dag.value_type(SDValue { node: node.node, res: 1 });

    // If UADDO_CARRY/USUBO_CARRY is legal, use that instead.
    let carry_op = if is_add { ISD::UAddOCarry } else { ISD::USubOCarry };
    if tli.is_operation_legal_or_custom(carry_op, vt) {
        let carry_in = dag.get_constant(0, result_type);
        let carried = dag.get_node_multi(carry_op, &[vt, result_type], &[lhs, rhs, carry_in]);
        return vec![carried, SDValue { node: carried.node, res: 1 }];
    }

    let result = dag.get_node(if is_add { ISD::Add } else { ISD::Sub }, vt, &[lhs, rhs]);
    let bool_vt = tli.types.set_cc_result_type(vt);
    let zero = dag.get_constant(0, vt);
    let overflow = if is_add && dag.constant(rhs) == Some(1) {
        // uaddo X, 1 overflowed if X+1 is 0.
        dag.get_setcc(bool_vt, result, zero, CondCode::SetEq)
    } else if is_add && dag.constant(rhs) == Some(super::fold::mask(vt.size_in_bits())) {
        // uaddo X, -1 overflows if X != 0.
        dag.get_setcc(bool_vt, lhs, zero, CondCode::SetNe)
    } else {
        dag.get_setcc(bool_vt, result, lhs, if is_add { CondCode::SetULt } else { CondCode::SetUGt })
    };
    vec![result, dag.get_zext_or_trunc(overflow, result_type)]
}
