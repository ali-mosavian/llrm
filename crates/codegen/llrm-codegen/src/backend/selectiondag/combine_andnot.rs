//! The folds the pin makes where the target has an and-not instruction
//! (`TLI.hasAndNot`, [`CombineHooks::has_and_not`]): `unfoldMaskedMerge` and
//! `foldBitwiseOpWithNeg`. x86 has one only with BMI (`andn`), so no baseline
//! x86 row moves; AArch64 (`bic`) turns it on.
//!
//! Scalars only: the vector arms (splat constants) are not ported.
//!
//! # See also
//! LLVM's `DAGCombiner::unfoldMaskedMerge`
//! ([L10421](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L10421))
//! and
//! `foldBitwiseOpWithNeg`
//! ([L12899](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L12899)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;

impl<H: CombineHooks> DagCombiner<'_, H> {
    fn all_ones(
        &self,
        value: SDValue,
    ) -> bool {
        self.dag.constant(value) == Some(super::fold::mask(self.dag.width(value)))
    }

    /// `isBitwiseNot`: `xor x, -1`, and `x`.
    fn bitwise_not_of(
        &self,
        value: SDValue,
    ) -> Option<SDValue> {
        (self.dag.opcode(value) == ISD::Xor && self.all_ones(self.dag.operand(value, 1)))
            .then(|| self.dag.operand(value, 0))
    }

    pub(super) fn has_and_not(
        &self,
        value: SDValue,
    ) -> bool {
        let vt = self.dag.value_type(value);
        vt.is_integer() && self.hooks.has_and_not(vt, self.dag.constant(value).is_some() && !self.dag.is_opaque(value))
    }

    /// `unfoldMaskedMerge`: `((x ^ y) & m) ^ y` is `(x & m) | (y & ~m)`.
    pub(super) fn unfold_masked_merge(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        if !vt.is_integer() || self.all_ones(n1) {
            return None;
        }
        // Three commutable operators: eight variants of the pattern.
        let matched = [(n0, 0, n1), (n0, 1, n1), (n1, 0, n0), (n1, 1, n0)]
            .into_iter()
            .find_map(
                |(and, at, other)| {
                    if self.dag.opcode(and) != ISD::And || !self.dag.has_one_use(and.node) {
                        return None;
                    }
                    let xor = self.dag.operand(and, at);
                    if self.dag.opcode(xor) != ISD::Xor || !self.dag.has_one_use(xor.node) {
                        return None;
                    }
                    let (mut x, mut y) = (self.dag.operand(xor, 0), self.dag.operand(xor, 1));
                    if self.all_ones(y) {
                        return None;
                    }
                    if other == x {
                        std::mem::swap(&mut x, &mut y);
                    }
                    (other == y).then(|| (x, y, self.dag.operand(and, 1 - at)))
                },
            );
        let (x, y, m) = matched?;
        if self.dag.constant(m).is_some() || !self.has_and_not(m) {
            return None;
        }
        if !self.has_and_not(y) && self.bitwise_not_of(m).is_none() {
            // Y is a constant and M no `not`: keep `andn` working on
            // immediates.
            let not_x = self.dag.get_not(x, vt);
            let lhs = self.dag.get_node(ISD::And, vt, &[not_x, m]);
            let not_lhs = self.dag.get_not(lhs, vt);
            let rhs = self.dag.get_node(ISD::Or, vt, &[m, y]);
            return Some(self.dag.get_node(ISD::And, vt, &[not_lhs, rhs]));
        }
        if !self.has_and_not(x) {
            if let Some(not_m) = self.bitwise_not_of(m) {
                let lhs = self.dag.get_node(ISD::Or, vt, &[x, not_m]);
                let not_y = self.dag.get_not(y, vt);
                let rhs = self.dag.get_node(ISD::And, vt, &[not_m, not_y]);
                let not_rhs = self.dag.get_not(rhs, vt);
                return Some(self.dag.get_node(ISD::And, vt, &[lhs, not_rhs]));
            }
        }
        let lhs = self.dag.get_node(ISD::And, vt, &[x, m]);
        let not_m = self.dag.get_not(m, vt);
        let rhs = self.dag.get_node(ISD::And, vt, &[y, not_m]);
        Some(self.dag.get_node(ISD::Or, vt, &[lhs, rhs]))
    }

    /// `foldBitwiseOpWithNeg`, for `and`: `x & (add (not y), z)` is
    /// `x & ~(y - z)` and `x & (sub (one-use not y), z)` is `x & ~(y + z)`.
    pub(super) fn fold_bitwise_op_with_neg(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        if !vt.is_integer() || !self.has_and_not(node) {
            return None;
        }
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        for (x, other) in [(n0, n1), (n1, n0)] {
            if !matches!(self.dag.opcode(other), ISD::Add | ISD::Sub) {
                continue;
            }
            let (a, b) = (self.dag.operand(other, 0), self.dag.operand(other, 1));
            match self.dag.opcode(other) {
                ISD::Add => {
                    // m_Add is commutative: the `not` may be either operand.
                    for (not_y, z) in [(a, b), (b, a)] {
                        if let Some(y) = self.bitwise_not_of(not_y) {
                            let sub = self.dag.get_node(ISD::Sub, vt, &[y, z]);
                            let not = self.dag.get_not(sub, vt);
                            return Some(self.dag.get_node(ISD::And, vt, &[x, not]));
                        }
                    }
                }
                ISD::Sub => {
                    if let Some(y) = self.bitwise_not_of(a).filter(|_| self.dag.has_one_use(a.node)) {
                        let add = self.dag.get_node(ISD::Add, vt, &[y, b]);
                        let not = self.dag.get_not(add, vt);
                        return Some(self.dag.get_node(ISD::And, vt, &[x, not]));
                    }
                }
                _ => {}
            }
        }
        None
    }
}
