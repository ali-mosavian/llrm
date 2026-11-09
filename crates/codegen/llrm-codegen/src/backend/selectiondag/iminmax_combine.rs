//! `visitIMINMAX`: `smin`/`smax`/`umin`/`umax` of equal operands is the
//! operand, a constant moves to the right, a sum of nested same-op constants
//! folds (`reassociateOps`), and where the target has no native instruction
//! `smax x, -1` is `x | (x >>s 31)` and `smin x, 0` is `x & (x >>s 31)`.
//! Operands of one known sign flip between `umin`/`umax` and `smin`/`smax`
//! where the op is illegal and the other legal, or where InstCombine broke a
//! saturation (`umin (smax x, c), c2`).
//!
//! DEVIATE SD252: not the FP-to-saturate folds, `reassociateReduction` nor the
//! `vscale` fold. Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitIMINMAX`
//! ([L6433](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L6433)).

use super::actions::Action;
use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::known_bits::known_bits;
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
        if let Some(reassociated) = self.reassociate_ops(op, n0, n1, self.dag.node(node).flags) {
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
        // If both operands are known to have the same sign (both non-negative
        // or both negative), flip between UMIN/UMAX and SMIN/SMAX
        // ([L6496](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L6496)).
        // Only if the current op isn't legal and the flipped is, or the
        // saturation pattern is broken by canonicalization in InstCombine.
        let legal = |op| {
            tli.types.is_type_legal(vt) && matches!(vt, EVT::Simple(_)) && tli.operation_action(op, vt) == Action::Legal
        };
        let is_op_illegal = !legal(op);
        let is_sat_broken = op == ISD::UMin && self.dag.opcode(n0) == ISD::SMax;
        if is_sat_broken || is_op_illegal {
            let sign_of = |dag: &mut super::dag::SelectionDAG, value: SDValue| {
                let known = known_bits(dag, value);
                let top = 1_u128 << (bits - 1);
                (known.zero & top != 0).then_some(false).or_else(|| (known.one & top != 0).then_some(true))
            };
            let same_sign = self.dag.is_undef(n0) || self.dag.is_undef(n1) || {
                let first = sign_of(self.dag, n0);
                first.is_some() && first == sign_of(self.dag, n1)
            };
            let flipped = match op {
                ISD::UMin => ISD::SMin,
                ISD::UMax => ISD::SMax,
                ISD::SMin => ISD::UMin,
                _ => ISD::UMax,
            };
            if same_sign && ((is_sat_broken && is_op_illegal) || legal(flipped)) {
                return Some(self.dag.get_node(flipped, vt, &[n0, n1]));
            }
        }
        None
    }
}
