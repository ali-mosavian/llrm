//! `MatchRotate`: an `or` of two shifts that make a rotate or a funnel shift.
//!
//! `(or (shl x, 4), (srl y, 28))` at 32 bits is `(fshl x, y, 4)`, one `shld`
//! where the two shifts and the `or` were six instructions with their
//! copies; with `x == y` it is `(rotl x, 4)`. Either half may be masked by a
//! constant `and`; the mask moves to the result.
//!
//! DEVIATE SD460: only the constant shift amounts are matched. Not translated:
//! `extractShiftForRotate` (a shift hidden in an `add`, `mul` or `udiv` by
//! InstCombine), the disguised rotate of a target without funnel shifts, the
//! variable amounts (`MatchRotatePosNeg`, `MatchFunnelPosNeg`) and the
//! truncated rotate. Exit: the variable amounts, which the x86 table needs for
//! `rol r, cl`. Owner sdag-c. Review 2026-11-27.
//!
//! # See also
//! LLVM's `DAGCombiner::MatchRotate`
//! ([L9464](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L9464)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

/// `stripConstantMask` and `matchRotateHalf`: the shift under an optional
/// `and` with a constant, and that mask.
fn rotate_half<H: CombineHooks>(
    combiner: &DagCombiner<'_, H>,
    op: SDValue,
) -> Option<(SDValue, Option<SDValue>)> {
    let (inner, mask) = match combiner.dag.opcode(op) {
        ISD::And if combiner.dag.constant(combiner.dag.operand(op, 1)).is_some() => {
            (combiner.dag.operand(op, 0), Some(combiner.dag.operand(op, 1)))
        }
        _ => (op, None),
    };
    matches!(combiner.dag.opcode(inner), ISD::Shl | ISD::Srl).then_some((inner, mask))
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `hasOperation(op, vt)`.
    fn has_operation(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        self.fp_op_legal_or_custom(op, vt)
    }

    /// `MatchRotate(LHS, RHS)` for constant shift amounts.
    pub(super) fn match_rotate(
        &mut self,
        lhs: SDValue,
        rhs: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(lhs);
        let bits = u128::from(vt.size_in_bits());
        let (has_rotl, has_rotr) = (self.has_operation(ISD::Rotl, vt), self.has_operation(ISD::Rotr, vt));
        let (has_fshl, has_fshr) = (self.has_operation(ISD::FShl, vt), self.has_operation(ISD::FShr, vt));
        if self.legal_operations() && !has_rotl && !has_rotr && !has_fshl && !has_fshr {
            return None;
        }
        let ((mut left, mut left_mask), (mut right, mut right_mask)) =
            (rotate_half(self, lhs).map(|(s, m)| (s, m))?, rotate_half(self, rhs)?);
        if self.dag.opcode(left) == self.dag.opcode(right) {
            return None;
        }
        // Canonicalize the `shl` to the left.
        if self.dag.opcode(right) == ISD::Shl {
            std::mem::swap(&mut left, &mut right);
            std::mem::swap(&mut left_mask, &mut right_mask);
        }
        let (left_arg, left_amount) = (self.dag.operand(left, 0), self.dag.operand(left, 1));
        let (right_arg, right_amount) = (self.dag.operand(right, 0), self.dag.operand(right, 1));
        let (c1, c2) = (self.dag.constant(left_amount)?, self.dag.constant(right_amount)?);
        if c1 + c2 != bits {
            return None;
        }
        let is_rotate = left_arg == right_arg;
        if !is_rotate && !(has_fshl || has_fshr) {
            return None;
        }
        let result = if is_rotate && (has_rotl || has_rotr || !(has_fshl || has_fshr)) {
            let use_rotl = !self.legal_operations() || has_rotl;
            let amount = if use_rotl { left_amount } else { right_amount };
            self.dag.get_node(if use_rotl { ISD::Rotl } else { ISD::Rotr }, vt, &[left_arg, amount])
        } else {
            let use_fshl = !self.legal_operations() || has_fshl;
            let amount = if use_fshl { left_amount } else { right_amount };
            self.dag.get_node(if use_fshl { ISD::FShl } else { ISD::FShr }, vt, &[left_arg, right_arg, amount])
        };
        if left_mask.is_none() && right_mask.is_none() {
            return Some(result);
        }
        // `ApplyMasks`: an `and` of either shifted operand applies to the result.
        let all_ones = self.dag.get_constant(super::fold::mask(vt.size_in_bits()), vt);
        let mut mask = all_ones;
        if let Some(left_mask) = left_mask {
            let right_bits = self.dag.get_node(ISD::Srl, vt, &[all_ones, right_amount]);
            let either = self.dag.get_node(ISD::Or, vt, &[left_mask, right_bits]);
            mask = self.dag.get_node(ISD::And, vt, &[mask, either]);
        }
        if let Some(right_mask) = right_mask {
            let left_bits = self.dag.get_node(ISD::Shl, vt, &[all_ones, left_amount]);
            let either = self.dag.get_node(ISD::Or, vt, &[right_mask, left_bits]);
            mask = self.dag.get_node(ISD::And, vt, &[mask, either]);
        }
        Some(self.dag.get_node(ISD::And, vt, &[result, mask]))
    }
}
