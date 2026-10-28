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
