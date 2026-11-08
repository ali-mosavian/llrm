//! `visitMULO`: `(umulo|smulo x, y)` folds.
//!
//! Translation of
//! [`DAGCombiner::visitMULO`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L6162)
//! at d1106deb71cc: constant operands fold, a constant goes right, `x * 0` is
//! `0` with no overflow, `x * 2` is `addo x, x`, a 1-bit `smulo` is an `and`,
//! and a product that cannot overflow is a plain `mul`.
//!
//! DEVIATE MULO1: `computeOverflowForUnsignedMul` takes its operand ranges
//! from the known bits only, not from `computeConstantRangeIncludingKnownBits`
//! (no constant-range analysis on DAG nodes). Owner wasm, exit 2026-11-22.
//! Vectors are not translated.

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::fold::mask;
use super::isd::{CondCode, ISD};
use super::known_bits::{known_bits, num_sign_bits};

fn signed(
    bits: u128,
    width: u32,
) -> i128 {
    let shift = 128 - width;
    ((bits << shift) as i128) >> shift
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_mulo(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let width = vt.size_in_bits();
        let carry_vt = self.dag.node(node).values[1];
        let is_signed = self.dag.opcode(node) == ISD::SMulO;
        let overflow = SDValue { node: node.node, res: 1 };
        let (c0, c1) = (self.dag.constant(n0), self.dag.constant(n1));

        if let (Some(a), Some(b)) = (c0, c1)
            && !self.dag.is_opaque(n0)
            && !self.dag.is_opaque(n1)
        {
            let (product, over) = if is_signed {
                let wide = signed(a, width).checked_mul(signed(b, width));
                let low = signed(a, width).wrapping_mul(signed(b, width)) as u128 & mask(width);
                (low, wide.is_none_or(|w| signed(w as u128 & mask(width), width) != w))
            } else {
                let wide = a.checked_mul(b);
                (a.wrapping_mul(b) & mask(width), wide.is_none_or(|w| w > mask(width)))
            };
            let value = self.dag.get_constant(product, vt);
            let flag = self.dag.get_bool_constant(over, carry_vt);
            self.dag.combine_to(node, value);
            self.dag.combine_to(overflow, flag);
            return None;
        }

        if c0.is_some() && c1.is_none() {
            return Some(self.dag.get_node_multi(self.dag.opcode(node), &[vt, carry_vt], &[n1, n0]));
        }

        if c1 == Some(0) {
            let zero = self.dag.get_constant(0, vt);
            let no_carry = self.dag.get_constant(0, carry_vt);
            self.dag.combine_to(node, zero);
            self.dag.combine_to(overflow, no_carry);
            return None;
        }

        if c1 == Some(2) && (!is_signed || width > 2) {
            let add = if is_signed { ISD::SAddO } else { ISD::UAddO };
            return Some(self.dag.get_node_multi(add, &[vt, carry_vt], &[n0, n0]));
        }

        if is_signed && width == 1 {
            let and = self.dag.get_node(ISD::And, vt, &[n0, n1]);
            let zero = self.dag.get_constant(0, vt);
            let flag = self.dag.get_setcc(carry_vt, and, zero, CondCode::SetNe);
            self.dag.combine_to(node, and);
            self.dag.combine_to(overflow, flag);
            return None;
        }

        if self.mul_never_overflows(is_signed, n0, n1) {
            let product = self.dag.get_node(ISD::Mul, vt, &[n0, n1]);
            let no_carry = self.dag.get_constant(0, carry_vt);
            self.dag.combine_to(node, product);
            self.dag.combine_to(overflow, no_carry);
            return None;
        }
        None
    }

    /// `willNotOverflowMul`: `computeOverflowForUnsignedMul` and
    /// `computeOverflowForSignedMul` are `OFK_Never`.
    fn mul_never_overflows(
        &self,
        is_signed: bool,
        a: SDValue,
        b: SDValue,
    ) -> bool {
        let width = self.dag.value_type(a).size_in_bits();
        if matches!(self.dag.constant(b), Some(0 | 1)) {
            return true;
        }
        if !is_signed {
            let (ka, kb) = (known_bits(self.dag, a), known_bits(self.dag, b));
            let (max_a, max_b) = (!ka.zero & mask(width), !kb.zero & mask(width));
            return max_a.checked_mul(max_b).is_some_and(|product| product <= mask(width));
        }
        let sign_bits = num_sign_bits(self.dag, a) + num_sign_bits(self.dag, b);
        if sign_bits > width + 1 {
            return true;
        }
        let non_negative = |value| known_bits(self.dag, value).zero >> (width - 1) & 1 == 1;
        sign_bits == width + 1 && (non_negative(a) || non_negative(b))
    }
}
