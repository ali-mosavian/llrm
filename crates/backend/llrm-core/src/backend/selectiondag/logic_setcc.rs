//! `foldLogicOfSetCCs`: an `and` or `or` of two compares as one compare.
//!
//! `(and (seteq a, 104), (seteq b, 100))` is `(seteq (or (xor a, 104), (xor b,
//! 100)), 0)`: two `xor`s, an `or` and one `setcc` in place of two `setcc`s and
//! an `and`.
//!
//! DEVIATE SD207: only `SETCC` is a compare (`isSetCCEquivalent` also takes a
//! `SELECT_CC` of constants, which this target never forms), and
//! `isKnownToBeAPowerOfTwo` knows constants and `shl 1, x`. Exit: a
//! known-bits power-of-two query. Owner sdag-b. Review 2026-11-27.
//!
//! # See also
//! LLVM's `DAGCombiner::foldLogicOfSetCCs`
//! ([L6772](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L6772)).

use super::dag::{NodeData, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::{CondCode, ISD};
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    fn compare_of(
        &self,
        value: SDValue,
    ) -> Option<(SDValue, SDValue, CondCode)> {
        if self.dag.opcode(value) != ISD::SetCC {
            return None;
        }
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(value, 2)).data else { return None };
        Some((self.dag.operand(value, 0), self.dag.operand(value, 1), cc))
    }

    /// `isKnownToBeAPowerOfTwo`, for what folds ask: a constant power of two,
    /// or `shl 1, x`.
    fn is_power_of_two(
        &self,
        value: SDValue,
    ) -> bool {
        if let Some(c) = self.dag.constant(value) {
            return c.is_power_of_two();
        }
        self.dag.opcode(value) == ISD::Shl && self.dag.constant(self.dag.operand(value, 0)) == Some(1)
    }

    fn has_one_use(
        &self,
        value: SDValue,
    ) -> bool {
        self.dag.users(value).count() == 1
    }

    /// `foldLogicOfSetCCs`.
    pub(super) fn fold_logic_of_setccs(
        &mut self,
        is_and: bool,
        n0: SDValue,
        n1: SDValue,
    ) -> Option<SDValue> {
        let (ll, lr, cc0) = self.compare_of(n0)?;
        let (rl, rr, cc1) = self.compare_of(n1)?;
        let vt = self.dag.value_type(n0);
        let op_vt = self.dag.value_type(ll);
        let legal_ops = self.level >= CombineLevel::AfterLegalizeVectorOps;
        if (legal_ops || vt != EVT::I1) && vt != EVT::I8 {
            return None;
        }
        if op_vt != self.dag.value_type(rl) {
            return None;
        }
        let integer = op_vt.is_integer();
        let ones = if integer { super::fold::mask(op_vt.size_in_bits()) } else { 0 };
        if lr == rr && cc0 == cc1 && integer {
            let is_zero = self.dag.is_null_constant(lr);
            let is_neg1 = self.dag.constant(lr) == Some(ones);
            // All bits clear / sign bits clear / any bit set / any sign set.
            let by_or = (is_and && cc1 == CondCode::SetEq && is_zero)
                || (is_and && cc1 == CondCode::SetGt && is_neg1)
                || (!is_and && cc1 == CondCode::SetNe && is_zero)
                || (!is_and && cc1 == CondCode::SetLt && is_zero);
            if by_or {
                let or = self.dag.get_node(ISD::Or, op_vt, &[ll, rl]);
                self.add_to_worklist(or.node);
                return Some(self.dag.get_setcc(vt, or, lr, cc1));
            }
            // All bits set / all sign bits set / any bit clear / any sign
            // clear.
            let by_and = (is_and && cc1 == CondCode::SetEq && is_neg1)
                || (is_and && cc1 == CondCode::SetLt && is_zero)
                || (!is_and && cc1 == CondCode::SetNe && is_neg1)
                || (!is_and && cc1 == CondCode::SetGt && is_neg1);
            if by_and {
                let and = self.dag.get_node(ISD::And, op_vt, &[ll, rl]);
                self.add_to_worklist(and.node);
                return Some(self.dag.get_setcc(vt, and, lr, cc1));
            }
        }
        // `(and (setne (and x, a), 0), (setne (and x, b), 0))` tests both bits.
        if self.dag.opcode(ll) == ISD::And
            && self.dag.opcode(rl) == ISD::And
            && self.dag.is_null_constant(lr)
            && self.dag.is_null_constant(rr)
            && cc0 == cc1
            && matches!(cc0, CondCode::SetNe | CondCode::SetEq)
        {
            let (ll0, ll1) = (self.dag.operand(ll, 0), self.dag.operand(ll, 1));
            let (rl0, rl1) = (self.dag.operand(rl, 0), self.dag.operand(rl, 1));
            if ll0 == rl0 && self.is_power_of_two(ll1) && self.is_power_of_two(rl1) {
                let or = self.dag.get_node(ISD::Or, op_vt, &[ll1, rl1]);
                let and = self.dag.get_node(ISD::And, op_vt, &[ll0, or]);
                let cc = if is_and { CondCode::SetEq } else { CondCode::SetNe };
                return Some(self.dag.get_setcc(vt, and, or, cc));
            }
        }
        // `(and (setne x, 0), (setne x, -1))` is `x + 1 >=u 2`.
        if ll == rl
            && cc0 == cc1
            && op_vt.size_in_bits() > 1
            && integer
            && ((is_and && cc0 == CondCode::SetNe) || (!is_and && cc0 == CondCode::SetEq))
            && ((self.dag.is_null_constant(lr) && self.dag.constant(rr) == Some(ones))
                || (self.dag.constant(lr) == Some(ones) && self.dag.is_null_constant(rr)))
        {
            let (one, two) = (self.dag.get_constant(1, op_vt), self.dag.get_constant(2, op_vt));
            let add = self.dag.get_node(ISD::Add, op_vt, &[ll, one]);
            self.add_to_worklist(add.node);
            let cc = if is_and { CondCode::SetUGe } else { CondCode::SetULt };
            return Some(self.dag.get_setcc(vt, add, two, cc));
        }
        // The more general forms, if the compares have no other user.
        if integer && cc0 == cc1 && self.has_one_use(n0) && self.has_one_use(n1) {
            if (is_and && cc1 == CondCode::SetEq) || (!is_and && cc1 == CondCode::SetNe) {
                let xor_left = self.dag.get_node(ISD::Xor, op_vt, &[ll, lr]);
                let xor_right = self.dag.get_node(ISD::Xor, op_vt, &[rl, rr]);
                let or = self.dag.get_node(ISD::Or, op_vt, &[xor_left, xor_right]);
                let zero = self.dag.get_constant(0, op_vt);
                return Some(self.dag.get_setcc(vt, or, zero, cc1));
            }
            // Constants a single bit apart: `x != 5 && x != 4` is `(x - 4) & ~1
            // != 0`.
            if (is_and && cc1 == CondCode::SetNe) || (!is_and && cc1 == CondCode::SetEq) {
                if let (true, Some(c0), Some(c1)) = (ll == rl, self.dag.constant(lr), self.dag.constant(rr)) {
                    let (high, low) = (c0.max(c1), c0.min(c1));
                    if !self.dag.is_opaque(lr) && !self.dag.is_opaque(rr) && (high - low).is_power_of_two() {
                        let max = self.dag.get_node(ISD::UMax, op_vt, &[lr, rr]);
                        let min = self.dag.get_node(ISD::UMin, op_vt, &[lr, rr]);
                        let offset = self.dag.get_node(ISD::Sub, op_vt, &[ll, min]);
                        let diff = self.dag.get_node(ISD::Sub, op_vt, &[max, min]);
                        let mask = self.dag.get_not(diff, op_vt);
                        let and = self.dag.get_node(ISD::And, op_vt, &[offset, mask]);
                        let zero = self.dag.get_constant(0, op_vt);
                        return Some(self.dag.get_setcc(vt, and, zero, cc0));
                    }
                }
            }
        }
        // Canonicalize equivalent operands to `ll == rl`.
        let (mut rl, mut rr, mut cc1) = (rl, rr, cc1);
        if ll == rr && lr == rl {
            cc1 = cc1.swapped_operands();
            std::mem::swap(&mut rl, &mut rr);
        }
        // Two compares of the same operands: one compare of the combined code.
        if ll == rl && lr == rr {
            let combined = if is_and { cc0.and_operation(cc1, integer) } else { cc0.or_operation(cc1, integer) };
            if let Some(code) = combined {
                let legal = !legal_ops
                    || match (self.tli, op_vt) {
                        (Some(tli), EVT::Simple(mvt)) => {
                            tli.cond_code_action(code, mvt) != super::actions::Action::Expand
                                && tli.is_operation_legal(ISD::SetCC, op_vt)
                        }
                        _ => false,
                    };
                if legal {
                    return Some(self.dag.get_setcc(vt, ll, lr, code));
                }
            }
        }
        None
    }

    /// `visitOR`, the compare folds.
    pub(super) fn visit_or(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        self.fold_logic_of_setccs(false, n0, n1)
    }
}
