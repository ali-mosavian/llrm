//! `TargetLowering::expandFunnelShift`
//! ([L8697](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L8697))
//! for scalars: the operation legalizer's expansion of `FSHL` and `FSHR`.

use super::actions::TargetLowering;
use super::dag::{SDValue, SelectionDAG};
use super::isd::ISD;

pub fn expand_funnel_shift(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let (mut x, mut y, mut z) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let bits = vt.size_in_bits();
    let shift_vt = dag.value_type(z);
    let op = dag.opcode(node);
    let is_fshl = op == ISD::FShl;
    // `isNonZeroModBitWidthOrUndef`, for a constant amount.
    let constant = dag.constant(z);
    let non_zero_mod = constant.is_some_and(|c| c % u128::from(bits) != 0);

    // If a funnel shift in the other direction is more supported, use it.
    let reverse = if is_fshl { ISD::FShr } else { ISD::FShl };
    if !tli.is_operation_legal_or_custom(op, vt)
        && tli.is_operation_legal_or_custom(reverse, vt)
        && bits.is_power_of_two()
    {
        if non_zero_mod {
            // fshl X, Y, Z -> fshr X, Y, -Z
            z = dag.get_negative(z, shift_vt);
        } else {
            // fshl X, Y, Z -> fshr (srl X, 1), (fshr X, Y, 1), ~Z
            // fshr X, Y, Z -> fshl (fshl X, Y, 1), (shl Y, 1), ~Z
            let one = dag.get_constant(1, shift_vt);
            if is_fshl {
                y = dag.get_node(reverse, vt, &[x, y, one]);
                x = dag.get_node(ISD::Srl, vt, &[x, one]);
            } else {
                x = dag.get_node(reverse, vt, &[x, y, one]);
                y = dag.get_node(ISD::Shl, vt, &[y, one]);
            }
            z = dag.get_not(z, shift_vt);
        }
        return dag.get_node(reverse, vt, &[x, y, z]);
    }

    let (shx, shy);
    if non_zero_mod {
        // fshl: X << C | Y >> (BW - C), fshr: X << (BW - C) | Y >> C, for
        // C = Z % BW not zero.
        let amount = u128::from(bits);
        let shift = constant.map_or(0, |c| c % amount);
        let (shift_amount, inverse) = (dag.get_constant(shift, shift_vt), dag.get_constant(amount - shift, shift_vt));
        shx = dag.get_node(ISD::Shl, vt, &[x, if is_fshl { shift_amount } else { inverse }]);
        shy = dag.get_node(ISD::Srl, vt, &[y, if is_fshl { inverse } else { shift_amount }]);
    } else {
        // fshl: X << (Z % BW) | Y >> 1 >> (BW - 1 - (Z % BW))
        // fshr: X << 1 << (BW - 1 - (Z % BW)) | Y >> (Z % BW)
        let mask = dag.get_constant(u128::from(bits - 1), shift_vt);
        let (shift_amount, inverse);
        if bits.is_power_of_two() {
            shift_amount = dag.get_node(ISD::And, shift_vt, &[z, mask]);
            let not = dag.get_not(z, shift_vt);
            inverse = dag.get_node(ISD::And, shift_vt, &[not, mask]);
        } else {
            let width = dag.get_constant(u128::from(bits), shift_vt);
            shift_amount = dag.get_node(ISD::URem, shift_vt, &[z, width]);
            inverse = dag.get_node(ISD::Sub, shift_vt, &[mask, shift_amount]);
        }
        let one = dag.get_constant(1, shift_vt);
        if is_fshl {
            shx = dag.get_node(ISD::Shl, vt, &[x, shift_amount]);
            let y1 = dag.get_node(ISD::Srl, vt, &[y, one]);
            shy = dag.get_node(ISD::Srl, vt, &[y1, inverse]);
        } else {
            let x1 = dag.get_node(ISD::Shl, vt, &[x, one]);
            shx = dag.get_node(ISD::Shl, vt, &[x1, inverse]);
            shy = dag.get_node(ISD::Srl, vt, &[y, shift_amount]);
        }
    }
    dag.get_node(ISD::Or, vt, &[shx, shy])
}

/// `TargetLowering::expandROT`
/// ([L8782](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L8782))
/// for scalars: the operation legalizer's expansion of `ROTL` and `ROTR`.
pub fn expand_rotate(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let (x, amount) = (dag.operand(node, 0), dag.operand(node, 1));
    let bits = vt.size_in_bits();
    let shift_vt = dag.value_type(amount);
    let op = dag.opcode(node);
    let is_left = op == ISD::Rotl;

    // If a rotate in the other direction is more supported, use it.
    let reverse = if is_left { ISD::Rotr } else { ISD::Rotl };
    if !tli.is_operation_legal_or_custom(op, vt) && tli.is_operation_legal_or_custom(reverse, vt) && bits.is_power_of_two()
    {
        let negated = dag.get_negative(amount, shift_vt);
        return dag.get_node(reverse, vt, &[x, negated]);
    }

    let (shift_op, half_op) = if is_left { (ISD::Shl, ISD::Srl) } else { (ISD::Srl, ISD::Shl) };
    let mask = dag.get_constant(u128::from(bits - 1), shift_vt);
    let (shifted, halved);
    if bits.is_power_of_two() {
        // (rotl x, c) -> x << (c & (w - 1)) | x >> (-c & (w - 1))
        // (rotr x, c) -> x >> (c & (w - 1)) | x << (-c & (w - 1))
        let negated = dag.get_negative(amount, shift_vt);
        let shift_amount = dag.get_node(ISD::And, shift_vt, &[amount, mask]);
        shifted = dag.get_node(shift_op, vt, &[x, shift_amount]);
        let half_amount = dag.get_node(ISD::And, shift_vt, &[negated, mask]);
        halved = dag.get_node(half_op, vt, &[x, half_amount]);
    } else {
        // (rotl x, c) -> x << (c % w) | x >> 1 >> (w - 1 - (c % w))
        // (rotr x, c) -> x >> (c % w) | x << 1 << (w - 1 - (c % w))
        let width = dag.get_constant(u128::from(bits), shift_vt);
        let shift_amount = dag.get_node(ISD::URem, shift_vt, &[amount, width]);
        shifted = dag.get_node(shift_op, vt, &[x, shift_amount]);
        let half_amount = dag.get_node(ISD::Sub, shift_vt, &[mask, shift_amount]);
        let one = dag.get_constant(1, shift_vt);
        let once = dag.get_node(half_op, vt, &[x, one]);
        halved = dag.get_node(half_op, vt, &[once, half_amount]);
    }
    dag.get_node(ISD::Or, vt, &[shifted, halved])
}

/// `TargetLowering::expandShiftParts`
/// ([L9224](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L9224)):
/// `SHL_PARTS`, `SRL_PARTS` and
/// `SRA_PARTS` as funnel shifts of the halves, with a select for an amount of
/// a part's width or more. `cc_vt` is the target's `getSetCCResultType`. The
/// low and the high result.
pub fn expand_shift_parts(
    dag: &mut SelectionDAG,
    node: SDValue,
    cc_vt: super::mvt::EVT,
) -> (SDValue, SDValue) {
    let vt = dag.value_type(node);
    let (lo_in, hi_in, amount) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let amount_vt = dag.value_type(amount);
    let bits = vt.size_in_bits();
    let op = dag.opcode(node);
    let (is_shl, is_sra) = (op == ISD::ShlParts, op == ISD::SraParts);

    let width_mask = dag.get_constant(u128::from(bits - 1), amount_vt);
    let safe_amount = dag.get_node(ISD::And, amount_vt, &[amount, width_mask]);
    let tmp1 = if is_sra {
        let top = dag.get_constant(u128::from(bits - 1), amount_vt);
        dag.get_node(ISD::Sra, vt, &[hi_in, top])
    } else {
        dag.get_constant(0, vt)
    };
    let (tmp2, tmp3) = if is_shl {
        (dag.get_node(ISD::FShl, vt, &[hi_in, lo_in, amount]), dag.get_node(ISD::Shl, vt, &[lo_in, safe_amount]))
    } else {
        let shift = if is_sra { ISD::Sra } else { ISD::Srl };
        (dag.get_node(ISD::FShr, vt, &[hi_in, lo_in, amount]), dag.get_node(shift, vt, &[hi_in, safe_amount]))
    };
    // An amount of a part's width or more does not use the funnel shift.
    let width = dag.get_constant(u128::from(bits), amount_vt);
    let and_node = dag.get_node(ISD::And, amount_vt, &[amount, width]);
    let zero = dag.get_constant(0, amount_vt);
    let wide = dag.get_setcc(cc_vt, and_node, zero, super::isd::CondCode::SetNe);
    if is_shl {
        (dag.get_select(vt, wide, tmp1, tmp3), dag.get_select(vt, wide, tmp3, tmp2))
    } else {
        (dag.get_select(vt, wide, tmp3, tmp2), dag.get_select(vt, wide, tmp1, tmp3))
    }
}
