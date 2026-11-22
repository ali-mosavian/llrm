//! `SimplifySetCC` of a compare against an immediate the target cannot
//! encode ([L5545 to
//! L5630](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L5545)):
//! the compare is made on the shifted value, whose constant is small.
//! `(x & -256) == 256` is `(x >> 8) == 1`; `x u< 0x4f000000` is `(x >> 24) u<
//! 0x4f`.
//!
//! `shouldAvoidTransformToShift` is false on every target here.

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::{CondCode, ISD};
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// The shifted compare of `setcc vt n0, c1, cond`, or `None`. `c1` is
    /// the constant, of the scalar integer type of `n0`.
    pub(super) fn fold_illegal_compare_immediate(
        &mut self,
        vt: EVT,
        n0: SDValue,
        c1: u128,
        cond: CondCode,
    ) -> Option<SDValue> {
        let op_type = self.dag.value_type(n0);
        let width = op_type.size_in_bits();
        let mask = super::fold::mask(width);
        let c1 = c1 & mask;
        if !fits_64(c1, width) || self.legal_immediate(c1, width) {
            return None;
        }
        if matches!(cond, CondCode::SetEq | CondCode::SetNe) {
            return self.and_mask_to_shift(vt, n0, c1, cond);
        }
        let adjust = matches!(cond, CondCode::SetULe | CondCode::SetUGt);
        if !adjust && !matches!(cond, CondCode::SetULt | CondCode::SetUGe) {
            return None;
        }
        // `x <= 0x0ffffffff` is `(x >> 32) < 1`, `x > 0x0ffffffff` is `(x >>
        // 32) >= 1`.
        let (shift, range, new_cond) = if adjust {
            let new_cond = if cond == CondCode::SetULe { CondCode::SetULt } else { CondCode::SetUGe };
            (c1.trailing_ones().min(width), c1.wrapping_add(1) & mask, new_cond)
        } else {
            (c1.trailing_zeros().min(width), c1, cond)
        };
        let shifted_c = if shift >= 128 { 0 } else { range >> shift };
        if shift == 0 || shift >= width || !fits_64(shifted_c, width) || !self.legal_immediate(shifted_c, width) {
            return None;
        }
        let new_c = self.dag.get_constant(shifted_c, op_type);
        // An offset range check keeps its add after the shift, not before it
        // as an add and a mask.
        if self.dag.opcode(n0) == ISD::Add
            && self.dag.has_one_use(n0.node)
            && let Some(add) = self.dag.constant(self.dag.operand(n0, 1))
        {
            let add = add & mask;
            let lower = add.wrapping_neg() & mask;
            let overflow = lower.checked_add(range).is_none_or(|sum| sum > mask);
            if add.trailing_zeros().min(width) >= shift && range != 0 && !overflow {
                let amount = self.shift_amount(op_type, shift);
                let shifted = self.dag.get_node(ISD::Srl, op_type, &[self.dag.operand(n0, 0), amount]);
                let offset = (lower >> shift).wrapping_neg() & mask;
                let offset = self.dag.get_constant(offset, op_type);
                let shifted_add = self.dag.get_node(ISD::Add, op_type, &[shifted, offset]);
                return Some(self.dag.get_setcc(vt, shifted_add, new_c, new_cond));
            }
        }
        let amount = self.shift_amount(op_type, shift);
        let shifted = self.dag.get_node(ISD::Srl, op_type, &[n0, amount]);
        Some(self.dag.get_setcc(vt, shifted, new_c, new_cond))
    }

    /// `(x & -256) == 256` is `(x >> 8) == 1`; an arithmetic shift where the
    /// logical one leaves an immediate the compare cannot take.
    fn and_mask_to_shift(
        &mut self,
        vt: EVT,
        n0: SDValue,
        c1: u128,
        cond: CondCode,
    ) -> Option<SDValue> {
        if self.dag.opcode(n0) != ISD::And || !self.dag.has_one_use(n0.node) {
            return None;
        }
        let op_type = self.dag.value_type(n0);
        let width = op_type.size_in_bits();
        let mask = super::fold::mask(width);
        let and_mask = self.dag.constant(self.dag.operand(n0, 1))? & mask;
        // `isNegatedPowerOf2`: ones from the top bit down to the lowest set
        // bit.
        let negated_power_of_two =
            and_mask != 0 && (and_mask << (128 - width)).leading_ones() + and_mask.trailing_zeros() == width;
        if !negated_power_of_two || c1 & !and_mask != 0 {
            return None;
        }
        let shift = and_mask.trailing_zeros();
        let x = self.dag.operand(n0, 0);
        let logical = c1 >> shift;
        if fits_64(logical, width) && !self.legal_immediate(logical, width) {
            let arithmetic = (((c1 << (128 - width)) as i128 >> (128 - width) >> shift) as u128) & mask;
            if fits_64(arithmetic, width) && self.legal_immediate(arithmetic, width) {
                let amount = self.shift_amount(op_type, shift);
                let shifted = self.dag.get_node(ISD::Sra, op_type, &[x, amount]);
                let rhs = self.dag.get_constant(arithmetic, op_type);
                return Some(self.dag.get_setcc(vt, shifted, rhs, cond));
            }
        }
        let amount = self.shift_amount(op_type, shift);
        let shifted = self.dag.get_node(ISD::Srl, op_type, &[x, amount]);
        let rhs = self.dag.get_constant(logical, op_type);
        Some(self.dag.get_setcc(vt, shifted, rhs, cond))
    }

    fn legal_immediate(
        &self,
        value: u128,
        width: u32,
    ) -> bool {
        self.hooks.legal_icmp_immediate(sign_extend(value, width) as i64)
    }
}

fn sign_extend(
    value: u128,
    width: u32,
) -> i128 {
    ((value << (128 - width)) as i128) >> (128 - width)
}

/// `getSignificantBits() <= 64`.
fn fits_64(
    value: u128,
    width: u32,
) -> bool {
    i64::try_from(sign_extend(value, width)).is_ok()
}
