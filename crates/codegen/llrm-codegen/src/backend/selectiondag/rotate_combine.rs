//! `visitRotate`: a rotate by zero, or by a multiple of the width, is the
//! operand; an amount past the width is taken modulo it; `rotl i16 x, 8` is
//! a `bswap`; two rotates by constants are one.
//!
//! DEVIATE SD254: not the demanded-bits simplification of the operand and
//! not `distributeTruncateThroughAnd` (the amount is a constant in the cases
//! this target meets). Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitRotate`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L10823)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::known_bits::known_bits;

impl<H: CombineHooks> DagCombiner<'_, H> {
    fn has_rotate_operation(
        &self,
        op: ISD,
        vt: super::mvt::EVT,
    ) -> bool {
        self.level < super::dag_combiner::CombineLevel::AfterLegalizeVectorOps
            || self.tli.is_some_and(|tli| tli.is_operation_legal(op, vt))
    }

    pub(super) fn visit_rotate(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let bits = u128::from(vt.size_in_bits());
        // fold (rot x, 0) -> x
        if self.dag.constant(n1) == Some(0) {
            return Some(n0);
        }
        // fold (rot x, c) -> x iff (c % BitSize) == 0
        if bits.is_power_of_two() && bits > 1 {
            let amount = known_bits(self.dag, n1);
            if amount.zero & (bits - 1) == bits - 1 {
                return Some(n0);
            }
        }
        let amount_vt = self.dag.value_type(n1);
        // fold (rot x, c) -> (rot x, c % BitSize)
        if let Some(c) = self.dag.constant(n1) {
            if c >= bits {
                let reduced = self.dag.get_constant(c % bits, amount_vt);
                return Some(self.dag.get_node(op, vt, &[n0, reduced]));
            }
        }
        // rot i16 X, 8 --> bswap X
        if self.dag.constant(n1) == Some(8)
            && bits == 2 * u128::from(u8::BITS)
            && self.has_rotate_operation(ISD::BSwap, vt)
        {
            return Some(self.dag.get_node(ISD::BSwap, vt, &[n0]));
        }
        // fold (rot* (rot* x, c2), c1) -> (rot* x, ((c1 % bitsize) +- (c2 %
        // bitsize) + bitsize) % bitsize)
        let next = self.dag.opcode(n0);
        if matches!(next, ISD::Rotl | ISD::Rotr) {
            let inner = self.dag.operand(n0, 1);
            if let (Some(c1), Some(c2)) = (self.dag.constant(n1), self.dag.constant(inner)) {
                if amount_vt == self.dag.value_type(inner) {
                    let (c1, c2) = (c1 % bits, c2 % bits);
                    let combined = if op == next { c1 + c2 } else { c1 + bits - c2 };
                    let amount = self.dag.get_constant(combined % bits, amount_vt);
                    let x = self.dag.operand(n0, 0);
                    return Some(self.dag.get_node(op, vt, &[x, amount]));
                }
            }
        }
        None
    }
}
