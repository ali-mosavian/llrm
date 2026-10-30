//! `visitCTLZ`, `visitCTTZ`, `visitCTPOP`: a count of a value known non-zero
//! drops the zero case; a population count looks through a shift that moves
//! no set bit.
//!
//! DEVIATE SD245: no `foldCTLZToCTLS` (no `CTLS` node), no half-width
//! `ctpop` of a value with a zero upper half (needs `isTruncateFree` and
//! `isZExtFree`), and "non-zero" is what known bits prove, not the full
//! `isKnownNeverZero`. Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitCTLZ`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L12685)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::known_bits::known_bits;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `ctlz`/`cttz` of a value with a known set bit is the `_zero_poison`
    /// form.
    pub(super) fn visit_ctlz_cttz(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let poison = match self.dag.opcode(node) {
            ISD::CtLz => ISD::CtLzZeroPoison,
            ISD::CtTz => ISD::CtTzZeroPoison,
            _ => return None,
        };
        let vt = self.dag.value_type(node);
        let src = self.dag.operand(node, 0);
        let legal = self.level < CombineLevel::AfterLegalizeVectorOps
            || self.tli.is_some_and(|tli| tli.is_operation_legal(poison, vt));
        if legal && known_bits(self.dag, src).one != 0 {
            return Some(self.dag.get_node(poison, vt, &[src]));
        }
        None
    }

    pub(super) fn visit_ctpop(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let width = vt.size_in_bits();
        let src = self.dag.operand(node, 0);
        // A shift that moves no active bit leaves the count alone.
        let op = self.dag.opcode(src);
        if matches!(op, ISD::Srl | ISD::Shl) {
            let by = self.dag.constant(self.dag.operand(src, 1))?;
            if by < u128::from(width) {
                let inner = self.dag.operand(src, 0);
                let zero = known_bits(self.dag, inner).zero;
                let by = by as u32;
                let free = if op == ISD::Srl { zero.trailing_ones() } else { (zero << (128 - width)).leading_ones() };
                if by <= free.min(width) {
                    return Some(self.dag.get_node(ISD::CtPop, vt, &[inner]));
                }
            }
        }
        None
    }
}
