//! `MatchRotate`: an `or` of two shifts that make a rotate or a funnel shift.
//!
//! `(or (shl x, 4), (srl y, 28))` at 32 bits is `(fshl x, y, 4)`, one `shld`
//! where the two shifts and the `or` were six instructions with their
//! copies; with `x == y` it is `(rotl x, 4)`. Either half may be masked by a
//! constant `and`; the mask moves to the result.
//!
//! A rotate by a variable amount is `MatchRotatePosNeg`: the two amounts are
//! `y` and `size - y`, either direct or through an extension or truncation.
//!
//! DEVIATE SD321: not translated: `extractShiftForRotate` (a shift hidden in
//! an `add`, `mul` or `udiv` by InstCombine), the disguised rotate of a
//! target without funnel shifts, the variable funnel shift
//! (`MatchFunnelPosNeg`) and the truncated rotate. `matchRotateSub` sees
//! through an `and` with the low-bits mask only, where the pin's
//! `SimplifyMultipleUseDemandedBits` sees through any operation that leaves
//! those bits alone. Exit: the variable funnel shift, for `shld r, r, cl`.
//! Owner sdag-c. Review 2026-11-27.
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
    /// `hasOperation(op, vt)`: once operations are legalized only a legal one
    /// counts, a custom one is not lowered again.
    fn has_operation(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        if self.legal_operations() { self.fp_op_legal(op, vt) } else { self.fp_op_legal_or_custom(op, vt) }
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
            (rotate_half(self, lhs)?, rotate_half(self, rhs)?);
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
        let is_rotate = left_arg == right_arg;
        let (Some(c1), Some(c2)) = (self.dag.constant(left_amount), self.dag.constant(right_amount)) else {
            return self.match_variable_rotate(
                left_arg,
                right_arg,
                left_amount,
                right_amount,
                left_mask.is_some() || right_mask.is_some(),
            );
        };
        if c1 + c2 != bits {
            return None;
        }
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
        // `ApplyMasks`: an `and` of either shifted operand applies to the
        // result.
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

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `MatchRotate` past the constant amounts: `MatchRotatePosNeg` for the
    /// rotate of one value by `y` and `size - y`.
    fn match_variable_rotate(
        &mut self,
        left_arg: SDValue,
        right_arg: SDValue,
        left_amount: SDValue,
        right_amount: SDValue,
        masked: bool,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(left_arg);
        if left_arg != right_arg || masked || !vt.is_integer() {
            return None;
        }
        let (has_rotl, has_rotr) = (self.has_operation(ISD::Rotl, vt), self.has_operation(ISD::Rotr, vt));
        if !has_rotl && !has_rotr {
            return None;
        }
        // an amount that is extended or truncated on both sides is compared
        // before the conversion
        let converts = |combiner: &Self, amount: SDValue| {
            matches!(
                combiner.dag.opcode(amount),
                ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend | ISD::Truncate
            )
        };
        let (left_inner, right_inner) = if converts(self, left_amount) && converts(self, right_amount) {
            (self.dag.operand(left_amount, 0), self.dag.operand(right_amount, 0))
        } else {
            (left_amount, right_amount)
        };
        let elt = vt.size_in_bits();
        if self.match_rotate_sub(left_inner, right_inner, elt) {
            let (op, amount) = if has_rotl { (ISD::Rotl, left_amount) } else { (ISD::Rotr, right_amount) };
            return Some(self.dag.get_node(op, vt, &[left_arg, amount]));
        }
        if self.match_rotate_sub(right_inner, left_inner, elt) {
            let (op, amount) = if has_rotr { (ISD::Rotr, right_amount) } else { (ISD::Rotl, left_amount) };
            return Some(self.dag.get_node(op, vt, &[left_arg, amount]));
        }
        None
    }

    /// `matchRotateSub` for a rotate: whether `neg == size - pos` wherever
    /// both are in range, so `(or (shift1 x, neg), (shift2 x, pos))` is a
    /// rotate.
    fn match_rotate_sub(
        &self,
        pos: SDValue,
        neg: SDValue,
        elt: u32,
    ) -> bool {
        // [A] for a power of two: only the low bits of the amount count, so
        // an `and` that keeps them is looked through on both sides
        let (mut pos, mut neg) = (pos, neg);
        let mut mask_lo_bits = 0;
        if elt.is_power_of_two() {
            let bits = elt.trailing_zeros();
            if self.dag.value_type(neg).size_in_bits() >= bits {
                let inner = self.strip_and_keeping(neg, bits);
                if inner != neg {
                    neg = inner;
                    mask_lo_bits = bits;
                }
            }
        }
        if self.dag.opcode(neg) != ISD::Sub {
            return false;
        }
        let Some(neg_c) = self.dag.constant(self.dag.operand(neg, 0)) else { return false };
        let neg_op1 = self.dag.operand(neg, 1);
        if mask_lo_bits != 0 && self.dag.value_type(pos).size_in_bits() >= mask_lo_bits {
            pos = self.strip_and_keeping(pos, mask_lo_bits);
        }
        let width_bits = self.dag.value_type(neg).size_in_bits();
        let wrap = |value: u128| if width_bits >= 128 { value } else { value & ((1u128 << width_bits) - 1) };
        let truncated_pos = self.dag.opcode(neg_op1) == ISD::Truncate && pos == self.dag.operand(neg_op1, 0);
        let width = if pos == neg_op1 || truncated_pos {
            wrap(neg_c)
        } else if self.dag.opcode(pos) == ISD::Add && self.dag.operand(pos, 0) == neg_op1 {
            match self.dag.constant(self.dag.operand(pos, 1)) {
                Some(pos_c) => wrap(pos_c.wrapping_add(neg_c)),
                None => return false,
            }
        } else {
            return false;
        };
        if mask_lo_bits != 0 {
            return width & ((1u128 << mask_lo_bits) - 1) == 0;
        }
        width == wrap(u128::from(elt))
    }

    /// The operand of an `and` whose constant mask keeps the low `bits` bits,
    /// else `value`: the part of `SimplifyMultipleUseDemandedBits` this needs.
    fn strip_and_keeping(
        &self,
        value: SDValue,
        bits: u32,
    ) -> SDValue {
        if self.dag.opcode(value) == ISD::And
            && let Some(mask) = self.dag.constant(self.dag.operand(value, 1))
            && mask & ((1u128 << bits) - 1) == (1u128 << bits) - 1
        {
            return self.dag.operand(value, 0);
        }
        value
    }
}
