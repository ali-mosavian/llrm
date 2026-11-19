//! `combineSelectToPseudoMinMax`: `x olt y ? x : y` is `pseudo_fmin` and
//! `x ogt y ? x : y` is `pseudo_fmax`, the nodes x86 selects as `minss` and
//! `maxss`.
//!
//! Translation of
//! [`combineSelectCCToPseudoMinMax`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13333)
//! and
//! [`combineSelectToPseudoMinMax`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13410),
//! called at the end of `visitSELECT` ([L13713]).
//!
//! DEVIATE 1: no strict (`STRICT_FSETCCS`) form: MIR has no constrained
//! compares. `isKnownNeverLogicalZero` is "a non-zero constant" (`fp_combine`).
//! Owner w10.
//!
//! [L13713]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13713

use super::dag::{NodeData, SDNodeFlags, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::{CondCode, ISD};

/// `getUnorderedFlavor(CC) == 1`: the code is true when either side is a NaN.
fn is_unordered(code: CondCode) -> bool {
    code as u8 >> 3 & 1 == 1
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `combineSelectCCToPseudoMinMax` for the compare `op0 cc op1` and the
    /// arms `lhs`, `rhs`: the opcode and its operands.
    fn select_cc_to_pseudo_min_max(
        &self,
        mut cc: CondCode,
        op0: SDValue,
        op1: SDValue,
        mut lhs: SDValue,
        mut rhs: SDValue,
        flags: SDNodeFlags,
    ) -> Option<(ISD, SDValue, SDValue)> {
        let vt = self.dag.value_type(lhs);
        if !vt.is_floating_point() {
            return None;
        }
        // Check for x CC y ? x : y.
        if lhs != op0 || rhs != op1 {
            if lhs != op1 || rhs != op0 {
                return None;
            }
            // Convert x CC y ? y : x to x inv(CC) y ? x : y.
            cc = cc.inverse_float();
            std::mem::swap(&mut lhs, &mut rhs);
        }
        // Convert x CC y ? x : y to y swap(inv(CC)) x ? y : x to convert an
        // unordered into an ordered comparison.
        if is_unordered(cc) {
            cc = cc.inverse_float().swapped_operands();
            std::mem::swap(&mut lhs, &mut rhs);
        }
        let nsz = flags.has(SDNodeFlags::NO_SIGNED_ZEROS);
        let signed_zero_safe = nsz || self.fp_known_never_zero(lhs) || self.fp_known_never_zero(rhs);
        let opcode = match cc {
            // Converting this to a min would handle comparisons between
            // positive and negative zero incorrectly.
            CondCode::SetOLe if signed_zero_safe => ISD::PseudoFMin,
            CondCode::SetLe => {
                // Convert setle to setlt via inv+swap.
                std::mem::swap(&mut lhs, &mut rhs);
                ISD::PseudoFMin
            }
            CondCode::SetOLt | CondCode::SetLt => ISD::PseudoFMin,
            CondCode::SetOGe if signed_zero_safe => ISD::PseudoFMax,
            CondCode::SetGe => {
                std::mem::swap(&mut lhs, &mut rhs);
                ISD::PseudoFMax
            }
            CondCode::SetOGt | CondCode::SetGt => ISD::PseudoFMax,
            _ => return None,
        };
        self.fp_op_legal_or_custom(opcode, vt).then_some((opcode, lhs, rhs))
    }

    /// `combineSelectToPseudoMinMax`: the select `node` as a min or a max.
    pub(super) fn select_to_pseudo_min_max(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (cond, lhs, rhs) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        if self.dag.opcode(cond) != ISD::SetCC {
            return None;
        }
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(cond, 2)).data else { return None };
        let (op0, op1) = (self.dag.operand(cond, 0), self.dag.operand(cond, 1));
        let flags = self.dag.node(node).flags;
        let (opcode, new_lhs, new_rhs) = self.select_cc_to_pseudo_min_max(cc, op0, op1, lhs, rhs, flags)?;
        // Propagate fast-math-flags.
        let vt = self.dag.value_type(node);
        Some(self.dag.get_node_flags(opcode, vt, &[new_lhs, new_rhs], flags))
    }
}
