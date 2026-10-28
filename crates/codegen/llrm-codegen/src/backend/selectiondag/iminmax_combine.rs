//! `visitIMINMAX`: `smin`/`smax`/`umin`/`umax` of equal operands is the
//! operand, a constant moves to the right, a sum of nested same-op constants
//! folds (`reassociateOps`), and where the target has no native instruction
//! `smax x, -1` is `x | (x >>s 31)` and `smin x, 0` is `x & (x >>s 31)`.
//!
//! DEVIATE SD252: not the FP-to-saturate folds, `reassociateReduction`, the
//! `vscale` fold, nor the flip to the opposite signedness (it needs the
//! opposite op to be legal, and none is on this target). Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitIMINMAX`
//! ([L6433](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L6433)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_iminmax(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        // If the operands are the same, this is a no-op.
        if n0 == n1 {
            return Some(n0);
        }
        // canonicalize constant to RHS
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_none() {
            return Some(self.dag.get_node(op, vt, &[n1, n0]));
        }
        if let Some(reassociated) = self.reassociate_ops(op, n0, n1) {
            return Some(reassociated);
        }
        // Fold sign-extension masks using an arithmetic shift.
        let tli = self.tli?;
        let bits = vt.size_in_bits();
        let mask = super::fold::mask(bits);
        if tli.types.is_type_legal(vt) && matches!(vt, EVT::Simple(_)) {
            let expand = |op| tli.operation_action(op, vt) == super::actions::Action::Expand;
            let constant = self.dag.constant(n1).map(|c| c & mask);
            let by = |dag: &mut super::dag::SelectionDAG| dag.get_constant(u128::from(bits - 1), EVT::I8);
            if op == ISD::SMax
                && expand(ISD::SMax)
                && !matches!(self.dag.opcode(n0), ISD::SMin | ISD::SignExtend)
                && constant == Some(mask)
            {
                let amount = by(self.dag);
                let sign = self.dag.get_node(ISD::Sra, vt, &[n0, amount]);
                return Some(self.dag.get_node(ISD::Or, vt, &[n0, sign]));
            }
            if op == ISD::SMin && expand(ISD::SMin) && self.dag.opcode(n0) != ISD::SMax && constant == Some(0) {
                let amount = by(self.dag);
                let sign = self.dag.get_node(ISD::Sra, vt, &[n0, amount]);
                return Some(self.dag.get_node(ISD::And, vt, &[n0, sign]));
            }
        }
        None
    }
}
