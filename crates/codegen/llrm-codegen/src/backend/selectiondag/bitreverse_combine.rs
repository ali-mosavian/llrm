//! `visitBITREVERSE`: a `bitreverse` of a `bitreverse` is the value, and a
//! shift of a `bitreverse` reversed again is the opposite shift of the value.
//!
//! DEVIATE SD236: the `clmul` fold is not ported (no carry-less multiply
//! here). Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitBITREVERSE`
//! ([L12617](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L12617)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_bitreverse(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let src = self.dag.operand(node, 0);
        // fold (bitreverse (bitreverse x)) -> x
        if self.dag.opcode(src) == ISD::BitReverse {
            return Some(self.dag.operand(src, 0));
        }
        // fold (bitreverse (lshr (bitreverse x), y)) -> (shl x, y), and
        // (bitreverse (shl (bitreverse x), y)) -> (lshr x, y)
        let inverse = match self.dag.opcode(src) {
            ISD::Srl => ISD::Shl,
            ISD::Shl => ISD::Srl,
            _ => return None,
        };
        let inner = self.dag.operand(src, 0);
        if self.dag.opcode(inner) != ISD::BitReverse {
            return None;
        }
        let legal = self.level < CombineLevel::AfterLegalizeVectorOps
            || self.tli.is_some_and(|tli| tli.is_operation_legal(inverse, vt));
        legal.then(|| {
            let (x, by) = (self.dag.operand(inner, 0), self.dag.operand(src, 1));
            self.dag.get_node(inverse, vt, &[x, by])
        })
    }
}
