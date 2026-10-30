//! `TargetLowering::expandFixedPointMul`
//! ([L12704](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L12704))
//! and `expandShlSat`
//! ([L12541](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L12541)),
//! for the operation legalizer.
//!
//! The vector forms are not translated; this target has no vector types.
//! `forceExpandWideMUL` (a product of a width with no wider multiply) is not
//! translated either: its node comes back `None` and is refused by name.

use super::actions::TargetLowering;
use super::dag::{SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::mvt::EVT;

fn mask(bits: u32) -> u128 {
    if bits == 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `expandFixedPointMul` for `SMULFIX`, `UMULFIX`, `SMULFIXSAT` and
/// `UMULFIXSAT`.
pub fn expand_fixed_point_mul(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
) -> Option<SDValue> {
    let op = dag.opcode(node);
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let scale = dag.constant(dag.operand(node, 2))? as u32;
    let saturating = matches!(op, ISD::SMulFixSat | ISD::UMulFixSat);
    let signed = matches!(op, ISD::SMulFix | ISD::SMulFixSat);
    let bool_vt = tli.types.set_cc_result_type(vt);
    let bits = vt.size_in_bits();
    let (min, max) = (1u128 << (bits - 1), (1u128 << (bits - 1)) - 1);

    if scale == 0 {
        // [us]mul.fix(a, b, 0) -> mul(a, b)
        if !saturating {
            if tli.is_operation_legal_or_custom(ISD::Mul, vt) {
                return Some(dag.get_node(ISD::Mul, vt, &[lhs, rhs]));
            }
        } else if signed && tli.is_operation_legal_or_custom(ISD::SMulO, vt) {
            let product = dag.get_node_multi(ISD::SMulO, &[vt, bool_vt], &[lhs, rhs]);
            let overflow = SDValue { node: product.node, res: 1 };
            let zero = dag.get_constant(0, vt);
            let (sat_min, sat_max) = (dag.get_constant(min, vt), dag.get_constant(max, vt));
            // The inputs' xor: sign bit clear, the product is positive.
            let xor = dag.get_node(ISD::Xor, vt, &[lhs, rhs]);
            let negative = dag.get_setcc(bool_vt, xor, zero, CondCode::SetLt);
            let saturated = dag.get_select(vt, negative, sat_min, sat_max);
            return Some(dag.get_select(vt, overflow, saturated, product));
        } else if !signed && tli.is_operation_legal_or_custom(ISD::UMulO, vt) {
            let product = dag.get_node_multi(ISD::UMulO, &[vt, bool_vt], &[lhs, rhs]);
            let overflow = SDValue { node: product.node, res: 1 };
            let sat_max = dag.get_all_ones_constant(vt);
            return Some(dag.get_select(vt, overflow, sat_max, product));
        }
    }

    // Select `sat` when `a <cc> b`, else `value` (`getSelectCC`).
    let saturating_select = |dag: &mut SelectionDAG, a, b, sat, value, cc| {
        let test = dag.get_setcc(bool_vt, a, b, cc);
        dag.get_select(vt, test, sat, value)
    };

    let (lo_hi, high_only) = if signed { (ISD::SMulLoHi, ISD::MulHS) } else { (ISD::UMulLoHi, ISD::MulHU) };
    let wide = EVT::integer(2 * bits);
    let (lo, hi) = if tli.is_operation_legal_or_custom(lo_hi, vt) {
        let both = dag.get_node_multi(lo_hi, &[vt, vt], &[lhs, rhs]);
        (both, SDValue { node: both.node, res: 1 })
    } else if tli.is_operation_legal_or_custom(high_only, vt) {
        (dag.get_node(ISD::Mul, vt, &[lhs, rhs]), dag.get_node(high_only, vt, &[lhs, rhs]))
    } else if tli.is_operation_legal_or_custom(ISD::Mul, wide) {
        // A multiplication in a wider type.
        let extend = if signed { ISD::SignExtend } else { ISD::ZeroExtend };
        let (l, r) = (dag.get_node(extend, wide, &[lhs]), dag.get_node(extend, wide, &[rhs]));
        let product = dag.get_node(ISD::Mul, wide, &[l, r]);
        let lo = dag.get_node(ISD::Truncate, vt, &[product]);
        let by = dag.get_constant(u128::from(bits), EVT::I8);
        let shifted = dag.get_node(ISD::Sra, wide, &[product, by]);
        (lo, dag.get_node(ISD::Truncate, vt, &[shifted]))
    } else {
        return None;
    };

    // The scale is the whole width: just the top half, which cannot overflow.
    if scale == bits {
        return Some(hi);
    }

    // Both operands are scaled, so shift the product right by the scale.
    let amount = dag.get_constant(u128::from(scale), EVT::I8);
    let result = dag.get_node(ISD::FShr, vt, &[hi, lo, amount]);
    if !saturating {
        return Some(result);
    }

    if !signed {
        // Overflowed if the upper (width - scale) bits are not all zero:
        // `hi > (1 << scale) - 1`.
        let low_mask = dag.get_constant(mask(scale), vt);
        let sat_max = dag.get_all_ones_constant(vt);
        return Some(saturating_select(dag, hi, low_mask, sat_max, result, CondCode::SetUGt));
    }

    // Overflowed if the upper (width - scale + 1) bits are not all ones or
    // all zeroes.
    let (sat_min, sat_max) = (dag.get_constant(min, vt), dag.get_constant(max, vt));
    if scale == 0 {
        let by = dag.get_constant(u128::from(bits - 1), EVT::I8);
        let sign = dag.get_node(ISD::Sra, vt, &[lo, by]);
        let overflow = dag.get_setcc(bool_vt, hi, sign, CondCode::SetNe);
        // Negative wide product saturates to the minimum, positive to the
        // maximum, but only if it overflowed.
        let zero = dag.get_constant(0, vt);
        let if_overflow = saturating_select(dag, hi, zero, sat_min, sat_max, CondCode::SetLt);
        return Some(dag.get_select(vt, overflow, if_overflow, result));
    }

    // `hi > (1 << (scale - 1)) - 1` saturates to the maximum, and
    // `hi < (-1 << (scale - 1))` to the minimum.
    let low_mask = dag.get_constant(mask(scale - 1), vt);
    let high_mask = dag.get_constant(mask(bits) & !mask(scale - 1), vt);
    let result = saturating_select(dag, hi, low_mask, sat_max, result, CondCode::SetGt);
    Some(saturating_select(dag, hi, high_mask, sat_min, result, CondCode::SetLt))
}

/// `expandShlSat`: the shift, and the saturated value where shifting back
/// does not return the operand.
pub fn expand_shl_sat(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
) -> SDValue {
    let signed = dag.opcode(node) == ISD::SShlSat;
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let bits = vt.size_in_bits();
    let bool_vt = tli.types.set_cc_result_type(vt);
    // The shift amount is a byte.
    let amount = dag.get_node(ISD::Truncate, EVT::I8, &[rhs]);
    let result = dag.get_node(ISD::Shl, vt, &[lhs, amount]);
    let original = dag.get_node(if signed { ISD::Sra } else { ISD::Srl }, vt, &[result, amount]);
    let saturated = if signed {
        let (min, max) = (1u128 << (bits - 1), (1u128 << (bits - 1)) - 1);
        let (sat_min, sat_max) = (dag.get_constant(min, vt), dag.get_constant(max, vt));
        let zero = dag.get_constant(0, vt);
        let negative = dag.get_setcc(bool_vt, lhs, zero, CondCode::SetLt);
        dag.get_select(vt, negative, sat_min, sat_max)
    } else {
        dag.get_all_ones_constant(vt)
    };
    let overflowed = dag.get_setcc(bool_vt, lhs, original, CondCode::SetNe);
    dag.get_select(vt, overflowed, saturated, result)
}

/// `forceExpandMultiply` ([L12579][pin]) with no high operands: the product of
/// two `lhs`-wide values as two `lhs`-wide words, from Hacker's Delight (the
/// arithmetic right shifts carry the signs for a signed product).
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L12579
pub fn force_expand_multiply(
    dag: &mut SelectionDAG,
    signed: bool,
    lhs: SDValue,
    rhs: SDValue,
) -> (SDValue, SDValue) {
    let vt = dag.value_type(lhs);
    let bits = vt.size_in_bits();
    let half = bits / 2;
    let mask = dag.get_constant(mask(half), vt);
    let shift = dag.get_constant(u128::from(half), EVT::I8);
    let (ll, rl) = (dag.get_node(ISD::And, vt, &[lhs, mask]), dag.get_node(ISD::And, vt, &[rhs, mask]));
    let t = dag.get_node(ISD::Mul, vt, &[ll, rl]);
    let tl = dag.get_node(ISD::And, vt, &[t, mask]);
    // Always an unsigned shift.
    let th = dag.get_node(ISD::Srl, vt, &[t, shift]);
    let shift_op = if signed { ISD::Sra } else { ISD::Srl };
    let (lh, rh) = (dag.get_node(shift_op, vt, &[lhs, shift]), dag.get_node(shift_op, vt, &[rhs, shift]));
    let lh_rl = dag.get_node(ISD::Mul, vt, &[lh, rl]);
    let u = dag.get_node(ISD::Add, vt, &[lh_rl, th]);
    let ul = dag.get_node(ISD::And, vt, &[u, mask]);
    let uh = dag.get_node(shift_op, vt, &[u, shift]);
    let ll_rh = dag.get_node(ISD::Mul, vt, &[ll, rh]);
    let v = dag.get_node(ISD::Add, vt, &[ll_rh, ul]);
    let vh = dag.get_node(shift_op, vt, &[v, shift]);
    let v_up = dag.get_node(ISD::Shl, vt, &[v, shift]);
    let lo = dag.get_node(ISD::Add, vt, &[tl, v_up]);
    let lh_rh = dag.get_node(ISD::Mul, vt, &[lh, rh]);
    let carries = dag.get_node(ISD::Add, vt, &[uh, vh]);
    (lo, dag.get_node(ISD::Add, vt, &[lh_rh, carries]))
}
