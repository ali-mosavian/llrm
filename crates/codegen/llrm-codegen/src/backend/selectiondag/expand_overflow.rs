//! `TargetLowering::expandUADDSUBO`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp>):
//! an unsigned add or subtract with overflow as the wrapped operation and a
//! compare. Used by the operation legalizer where the target leaves the
//! operation to be expanded.

use super::actions::TargetLowering;
use super::dag::{SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::mvt::EVT;

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

/// `TargetLowering::expandMULO`: a multiply with overflow as the product's two
/// halves, the overflow being a high half that is not zero (unsigned) or not
/// the sign of the low half (signed). The halves come from `UMUL_LOHI` or
/// `MULHU` where legal, from a legal wider multiply, or from
/// `forceExpandWideMUL`. A power of two is a shift.
pub fn expand_mulo(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
) -> Vec<SDValue> {
    let signed = dag.opcode(node) == ISD::SMulO;
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let bits = vt.size_in_bits();
    let bool_vt = tli.types.set_cc_result_type(vt);
    let result_type = dag.value_type(SDValue { node: node.node, res: 1 });

    // mulo(X, 1 << S) -> { X << S, (X << S) >> S != X }
    if let Some(c) = dag.constant(rhs).filter(|c| c.is_power_of_two()) {
        // smulo(x, signed_min) is the same as umulo(x, signed_min).
        let arithmetic = signed && c != 1u128 << (bits - 1);
        let amount = dag.get_constant(u128::from(c.trailing_zeros()), EVT::I8);
        let result = dag.get_node(ISD::Shl, vt, &[lhs, amount]);
        let back = dag.get_node(if arithmetic { ISD::Sra } else { ISD::Srl }, vt, &[result, amount]);
        let overflow = dag.get_setcc(bool_vt, back, lhs, CondCode::SetNe);
        return vec![result, dag.get_zext_or_trunc(overflow, result_type)];
    }

    let (lo_hi, high, extend) = if signed {
        (ISD::SMulLoHi, ISD::MulHS, ISD::SignExtend)
    } else {
        (ISD::UMulLoHi, ISD::MulHU, ISD::ZeroExtend)
    };
    let wide = EVT::integer(2 * bits);
    let (bottom, top) = if tli.is_operation_legal_or_custom(lo_hi, vt) {
        let both = dag.get_node_multi(lo_hi, &[vt, vt], &[lhs, rhs]);
        (both, SDValue { node: both.node, res: 1 })
    } else if tli.is_operation_legal_or_custom(high, vt) {
        (dag.get_node(ISD::Mul, vt, &[lhs, rhs]), dag.get_node(high, vt, &[lhs, rhs]))
    } else if tli.types.is_type_legal(wide) {
        let (l, r) = (dag.get_node(extend, wide, &[lhs]), dag.get_node(extend, wide, &[rhs]));
        let product = dag.get_node(ISD::Mul, wide, &[l, r]);
        let amount = dag.get_constant(u128::from(bits), EVT::I8);
        let shifted = dag.get_node(ISD::Srl, wide, &[product, amount]);
        (dag.get_node(ISD::Truncate, vt, &[product]), dag.get_node(ISD::Truncate, vt, &[shifted]))
    } else {
        super::expand_fixed::force_expand_multiply(dag, signed, lhs, rhs)
    };

    let overflow = if signed {
        let amount = dag.get_constant(u128::from(bits - 1), EVT::I8);
        let sign = dag.get_node(ISD::Sra, vt, &[bottom, amount]);
        dag.get_setcc(bool_vt, top, sign, CondCode::SetNe)
    } else {
        let zero = dag.get_constant(0, vt);
        dag.get_setcc(bool_vt, top, zero, CondCode::SetNe)
    };
    vec![bottom, dag.get_zext_or_trunc(overflow, result_type)]
}
