//! `visitBSWAP`: a `bswap` of a value with one possibly-nonzero byte is a
//! shift to the mirror byte, and a `bswap` of a byte-multiple shift is the
//! opposite shift of the `bswap`.
//!
//! DEVIATE SD227: the half-width form of `bswap (shl x, c)` (needs a legal
//! half type and a free truncate) and the `bswap (bitreverse x)`
//! canonicalisation are not ported. Exit: both, when a target needs them.
//! Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitBSWAP`
//! ([L12530](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L12530)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::known_bits::known_bits;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_bswap(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let width = vt.size_in_bits();
        let src = self.dag.operand(node, 0);
        // bswap (srl x, c) --> shl (bswap x), c (and the other way), c a byte
        // multiple.
        if matches!(self.dag.opcode(src), ISD::Shl | ISD::Srl) && self.dag.has_one_use(src.node) {
            if let Some(by) = self.dag.constant(self.dag.operand(src, 1)) {
                if by < u128::from(width) && by % 8 == 0 {
                    let swapped = self.dag.get_node(ISD::BSwap, vt, &[self.dag.operand(src, 0)]);
                    let inverse = if self.dag.opcode(src) == ISD::Shl { ISD::Srl } else { ISD::Shl };
                    let amount = self.dag.operand(src, 1);
                    return Some(self.dag.get_node(inverse, vt, &[swapped, amount]));
                }
            }
        }
        let known = known_bits(self.dag, src);
        let all = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
        if known.zero & all == all {
            return Some(self.dag.get_constant(0, vt));
        }
        let trailing = known.zero.trailing_ones().min(width) & !7;
        let leading = (known.zero << (128 - width)).leading_ones().min(width) & !7;
        if width - (leading + trailing) == 8 {
            let shift = if leading > trailing { ISD::Shl } else { ISD::Srl };
            let legal = self.level < super::dag_combiner::CombineLevel::AfterLegalizeVectorOps
                || self.tli.is_some_and(|tli| tli.is_operation_legal(shift, vt));
            if legal {
                let by = leading.abs_diff(trailing);
                let amount_vt = self.shift_amount_vt(vt);
                let by = self.dag.get_constant(u128::from(by), amount_vt);
                return Some(self.dag.get_node(shift, vt, &[src, by]));
            }
        }
        None
    }
}
