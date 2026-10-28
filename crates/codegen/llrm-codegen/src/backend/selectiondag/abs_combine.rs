//! `visitABS` and `visitABS_MIN_POISON`: the absolute value of an absolute
//! value, or of a value whose sign bit is clear, is the operand.
//!
//! DEVIATE SD246: no `foldABSToABD` (no `ABDS`/`ABDU` node) and no
//! `abs (sign_extend_inreg x)` narrowing (no `isTruncateFree`/`isZExtFree`
//! query). Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitABS`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L12603)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::known_bits::known_bits;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_abs(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let n0 = self.dag.operand(node, 0);
        let mut inner = n0;
        // `abs_min_poison` looks through a `freeze`; `abs` does not.
        if self.dag.opcode(node) == ISD::AbsMinPoison && self.dag.opcode(inner) == ISD::Freeze {
            inner = self.dag.operand(inner, 0);
        }
        if matches!(self.dag.opcode(inner), ISD::Abs | ISD::AbsMinPoison)
            && (inner == n0 || self.dag.opcode(node) == ISD::AbsMinPoison)
        {
            return Some(n0);
        }
        let bits = self.dag.value_type(n0).size_in_bits();
        (known_bits(self.dag, n0).zero >> (bits - 1) & 1 == 1).then_some(n0)
    }
}
