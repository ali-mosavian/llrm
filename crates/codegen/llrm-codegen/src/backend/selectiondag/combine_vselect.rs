//! `visitVSELECT`'s boolean folds: a vector select whose condition is a
//! constant or a `not`, and `combineVSelectWithAllOnesOrZeros`, a select with
//! an all-zero or all-ones arm over a condition as wide as the arms is a
//! bitwise `and`/`or` of the condition (`and`-not where the target has one).
//!
//! DEVIATE VSEL1 (owner arm64, exit 2026-11-24): the rest of `visitVSELECT`
//! is not translated: `simplifySelect`, `foldBoolSelectToLogic`, the abs,
//! min/max, saturating add/sub and `foldSelectToABD` folds,
//! `foldVSelectOfConstants`, `foldVSelectToSignBitSplatMask`,
//! `SimplifyDemandedVectorElts`; and `combineVSelectWithAllOnesOrZeros`'s
//! `setgt -1, 1, -1` to `sra`/`or` arm.
//!
//! DEVIATE VSEL2 (owner arm64, exit 2026-11-24): the pin hoists the `not` of
//! a compare above the truncates and concat that narrow its mask and makes one
//! `bic`; here the `not` stays under them (`mvn` x2, 13 instructions against
//! 11 on the `ugt` clamp). `freeze` is pushed through vector `truncate` and
//! `concat_vectors` as it is through the scalar nodes (`SD213` still holds for
//! the rest).
//!
//! # See also
//! LLVM's `DAGCombiner::visitVSELECT`
//! ([L14698](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L14698)),
//! `combineVSelectWithAllOnesOrZeros`.

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `extractBooleanFlip` for a vector: the `c` of `xor c, <-1, ...>`.
    fn vector_boolean_not(
        &self,
        value: SDValue,
    ) -> Option<SDValue> {
        (self.dag.opcode(value) == ISD::Xor && self.dag.is_build_vector_all(self.dag.operand(value, 1), true))
            .then(|| self.dag.operand(value, 0))
    }

    pub(super) fn visit_vselect(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1, n2) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let vt = self.dag.value_type(node);
        // `vselect (not Cond), N1, N2` is `vselect Cond, N2, N1`.
        if let Some(flipped) = self.vector_boolean_not(n0) {
            return Some(self.dag.get_node(ISD::VSelect, vt, &[flipped, n2, n1]));
        }
        // `vselect all_ones, N1, N2` is N1; `vselect all_zeros, N1, N2` is N2.
        if self.dag.is_build_vector_all(n0, true) {
            return Some(n1);
        }
        if self.dag.is_build_vector_all(n0, false) {
            return Some(n2);
        }
        self.vselect_with_all_ones_or_zeros(n0, n1, n2, vt)
    }

    /// `combineVSelectWithAllOnesOrZeros`.
    fn vselect_with_all_ones_or_zeros(
        &mut self,
        mut cond: SDValue,
        mut t_val: SDValue,
        mut f_val: SDValue,
        vt: EVT,
    ) -> Option<SDValue> {
        if !self.tli.is_some_and(|tli| tli.types.is_type_legal(vt)) {
            return None;
        }
        let cond_vt = self.dag.value_type(cond);
        let mut t_zero = self.dag.is_build_vector_all(t_val, false);
        let mut t_ones = self.dag.is_build_vector_all(t_val, true);
        let mut f_zero = self.dag.is_build_vector_all(f_val, false);
        let f_ones = self.dag.is_build_vector_all(f_val, true);
        if !t_zero && !t_ones && !f_zero && !f_ones {
            return None;
        }
        // `select Cond, 0, 0` is 0.
        if t_zero && f_zero {
            return Some(self.dag.get_constant(0, vt));
        }
        // The condition is a bitwise mask only when its lanes are as wide as
        // the arms' and every bit of a lane is its sign.
        if cond_vt.element().size_in_bits() != vt.element().size_in_bits() {
            return None;
        }
        if super::known_bits::num_sign_bits(self.dag, cond) != cond_vt.element().size_in_bits() {
            return None;
        }
        // Inverting a one-use compare and swapping the arms gives the
        // all-ones/all-zeros form.
        if !t_ones && !f_zero && self.dag.has_one_use(cond.node) && self.dag.opcode(cond) == ISD::SetCC {
            let super::dag::NodeData::CondCode(code) = self.dag.node(self.dag.operand(cond, 2)).data else {
                return None;
            };
            let operand_vt = self.dag.value_type(self.dag.operand(cond, 0));
            if self.tli.is_some_and(|tli| tli.types.set_cc_result_type(vt) == cond_vt) && (t_zero || f_ones) {
                let inverse = if operand_vt.element().is_floating_point() {
                    code.inverse_float()
                } else {
                    code.inverse_integer()
                };
                let (lhs, rhs) = (self.dag.operand(cond, 0), self.dag.operand(cond, 1));
                cond = self.dag.get_setcc(cond_vt, lhs, rhs, inverse);
                std::mem::swap(&mut t_val, &mut f_val);
                std::mem::swap(&mut t_zero, &mut f_zero);
                t_ones = f_ones;
            }
        }
        // `getBitcast`: the value itself where the type is the same.
        let bitcast = |this: &mut Self, value: SDValue, to: EVT| {
            if this.dag.value_type(value) == to { value } else { this.dag.get_node(ISD::BitCast, to, &[value]) }
        };
        let freeze_as = |this: &mut Self, value: SDValue| {
            let frozen = this.dag.get_node(ISD::Freeze, vt, &[value]);
            bitcast(this, frozen, cond_vt)
        };
        // `select Cond, -1, 0` is `Cond`.
        if t_ones && f_zero {
            return Some(bitcast(self, cond, vt));
        }
        // `select Cond, -1, x` is `or Cond, x`.
        if t_ones {
            let x = freeze_as(self, f_val);
            let or = self.dag.get_node(ISD::Or, cond_vt, &[cond, x]);
            return Some(bitcast(self, or, vt));
        }
        // `select Cond, x, 0` is `and Cond, x`.
        if f_zero {
            let x = freeze_as(self, t_val);
            let and = self.dag.get_node(ISD::And, cond_vt, &[cond, x]);
            return Some(bitcast(self, and, vt));
        }
        // `select Cond, 0, x` is `and (not Cond), x`.
        if t_zero && (self.vector_boolean_not(cond).is_some() || self.hooks.has_and_not(cond_vt, false)) {
            let x = freeze_as(self, f_val);
            let not = self.dag.get_not(cond, cond_vt);
            let and = self.dag.get_node(ISD::And, cond_vt, &[not, x]);
            return Some(bitcast(self, and, vt));
        }
        None
    }
}
