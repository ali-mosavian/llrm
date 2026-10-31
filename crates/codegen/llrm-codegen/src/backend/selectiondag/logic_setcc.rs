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
        if integer
            && self.hooks.convert_setcc_logic_to_bitwise_logic()
            && cc0 == cc1
            && self.has_one_use(n0)
            && self.has_one_use(n1)
        {
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

    /// `visitORLike`: `(or (and X, C1), (and Y, C2))` is `(and (or X, Y), C1 |
    /// C2)` when the bits of `X` set in `C2` alone, and of `Y` in `C1` alone,
    /// are zero; `(or (and X, M), (and X, N))` is `(and X, M | N)`; each only
    /// when one of the two `and`s has no other use.
    fn or_of_ands(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        if self.dag.opcode(n0) != ISD::And || self.dag.opcode(n1) != ISD::And {
            return None;
        }
        if !self.dag.has_one_use(n0.node) && !self.dag.has_one_use(n1.node) {
            return None;
        }
        let vt = self.dag.value_type(node);
        let all = super::fold::mask(vt.size_in_bits());
        let (x, y) = (self.dag.operand(n0, 0), self.dag.operand(n1, 0));
        let (m0, m1) = (self.dag.operand(n0, 1), self.dag.operand(n1, 1));
        if let (Some(c0), Some(c1)) = (
            self.dag.constant(m0).filter(|_| !self.dag.is_opaque(m0)),
            self.dag.constant(m1).filter(|_| !self.dag.is_opaque(m1)),
        ) {
            let zero = |dag: &super::dag::SelectionDAG, v: SDValue, mask: u128| {
                mask & all & !super::known_bits::known_bits(dag, v).zero == 0
            };
            if zero(self.dag, x, c1 & !c0) && zero(self.dag, y, c0 & !c1) {
                let joined = self.dag.get_node(ISD::Or, vt, &[x, y]);
                let mask = self.dag.get_constant(c0 | c1, vt);
                return Some(self.dag.get_node(ISD::And, vt, &[joined, mask]));
            }
        }
        if x == y {
            let joined = self.dag.get_node(ISD::Or, vt, &[m0, m1]);
            return Some(self.dag.get_node(ISD::And, vt, &[x, joined]));
        }
        None
    }

    /// `visitOR`, the compare folds.
    /// DEVIATE: a dword from two words (`BUILD_PAIR` expanded to `zext lo |
    /// (aext hi << 16)`, a LONG in `dx:ax`) is `fshr (hi, lo << 16, 16)`, a
    /// `shl` and a `shrd`: a byte and a clock under the pin's `movzx`,
    /// `shl`, `or` on the 486. Only once legal, and only when every user
    /// computes with the whole dword: a far pointer's halves are taken
    /// apart again, which the `or` form folds away. O(users).
    fn pack_words_by_shrd(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        if self.level != CombineLevel::AfterLegalizeDag || vt != EVT::I32 {
            return None;
        }
        let (a, b) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let (low, high) = if self.dag.opcode(a) == ISD::ZeroExtend { (a, b) } else { (b, a) };
        if self.dag.opcode(low) != ISD::ZeroExtend || self.dag.opcode(high) != ISD::Shl {
            return None;
        }
        let (lo, shifted) = (self.dag.operand(low, 0), self.dag.operand(high, 0));
        if self.dag.constant(self.dag.operand(high, 1)) != Some(16)
            || self.dag.opcode(shifted) != ISD::AnyExtend
            || self.dag.value_type(lo) != EVT::I16
        {
            return None;
        }
        // Only a value that is computed with stays whole; one that is stored,
        // passed on or taken apart (a far pointer's halves) is not packed.
        let computed = self
            .dag
            .users(node)
            .all(
                |user| matches!(
                    self.dag.opcode(SDValue { node: user, res: 0 }),
                    ISD::Add
                        | ISD::Sub
                        | ISD::Mul
                        | ISD::SDiv
                        | ISD::UDiv
                        | ISD::SRem
                        | ISD::URem
                        | ISD::Xor
                        | ISD::SetCC
                        | ISD::SIntToFp
                ),
            );
        if !computed {
            return None;
        }
        let sixteen = self.dag.get_constant(16, EVT::I8);
        let lo_wide = self.dag.get_node(ISD::AnyExtend, vt, &[lo]);
        let lo_high = self.dag.get_node(ISD::Shl, vt, &[lo_wide, sixteen]);
        Some(self.dag.get_node(ISD::FShr, vt, &[shifted, lo_high, sixteen]))
    }

    pub(super) fn visit_or(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(n0);
        let all = super::fold::mask(vt.size_in_bits());
        if n0 == n1 {
            return Some(n0);
        }
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_none() {
            return Some(self.dag.get_node(ISD::Or, vt, &[n1, n0]));
        }
        let c1 = self.dag.constant(n1).filter(|_| !self.dag.is_opaque(n1));
        if c1 == Some(all) {
            return Some(n1);
        }
        // `(or x, c)` is `c` when `x` is known zero wherever `c` is clear.
        if let Some(c) = c1 {
            if !c & all & !super::known_bits::known_bits(self.dag, n0).zero == 0 {
                return Some(n1);
            }
        }
        if let Some(folded) = self.fold_logic_of_setccs(false, n0, n1) {
            return Some(folded);
        }
        if let Some(folded) = self.or_of_ands(node) {
            return Some(folded);
        }
        if let Some(reassociated) = self.reassociate_ops(ISD::Or, n0, n1) {
            return Some(reassociated);
        }
        if let Some(rotated) = self.match_rotate(n0, n1) {
            return Some(rotated);
        }
        if let Some(packed) = self.pack_words_by_shrd(node) {
            return Some(packed);
        }
        if let Some(c1) = c1 {
            if let Some(folded) = self.or_with_shared_mask(n0, c1, vt) {
                return Some(folded);
            }
        }
        if self.hooks.fold_or_absorption() {
            if let Some(folded) = self.visit_or_commutative(n0, n1) {
                return Some(folded);
            }
            if let Some(folded) = self.visit_or_commutative(n1, n0) {
                return Some(folded);
            }
        }
        None
    }

    /// `(or (and x, c0), c1)` with `c0` and `c1` sharing a bit is
    /// `(and (or x, c1), c0 | c1)`.
    fn or_with_shared_mask(
        &mut self,
        n0: SDValue,
        c1: u128,
        vt: EVT,
    ) -> Option<SDValue> {
        let n1 = self.dag.get_constant(c1, vt);
        if self.dag.opcode(n0) == ISD::And && self.dag.has_one_use(n0.node) {
            let (x, mask) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            if let Some(c0) = self.dag.constant(mask) {
                if c0 & c1 != 0 {
                    let both = self.dag.get_constant(c0 | c1, vt);
                    let inner = self.dag.get_node(ISD::Or, vt, &[x, n1]);
                    self.add_to_worklist(inner.node);
                    return Some(self.dag.get_node(ISD::And, vt, &[both, inner]));
                }
            }
        }
        None
    }

    /// `visitORCommutative`, the folds of `or n0, n1` that look into `n0`:
    /// `(or (and x, y), x)` is `x`; `(or (and x, (not y)), y)` is `(or x, y)`;
    /// `(or (xor x, n1), n1)` is `(or x, n1)`; `(or (xor x, y), (and/or x, y))`
    /// is `(or x, y)`.
    ///
    /// DEVIATE: the resize peeking and `any_extend (not (truncate x))` of
    /// `getBitwiseNotOperand`, and the funnel-shift and `build_pair` folds, are
    /// not translated. Exit: those shapes.
    fn visit_or_commutative(
        &mut self,
        n0: SDValue,
        n1: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(n0);
        if self.dag.opcode(n0) == ISD::And {
            let (n00, n01) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            // fold or (and x, y), x --> x
            if n00 == n1 || n01 == n1 {
                return Some(n1);
            }
            // fold (or (and x, (xor y, -1)), y) -> (or x, y), and its mirror
            for (x, not) in [(n00, n01), (n01, n00)] {
                if self.bitwise_not_operand(not) == Some(n1) {
                    return Some(self.dag.get_node(ISD::Or, vt, &[x, n1]));
                }
            }
        }
        if self.dag.opcode(n0) == ISD::Xor {
            let (x, y) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            // fold or (xor x, n1), n1 --> or x, n1
            if y == n1 {
                return Some(self.dag.get_node(ISD::Or, vt, &[x, n1]));
            }
            // fold or (xor x, y), (x and/or y) --> or x, y
            let joins = |this: &Self, a: SDValue, b: SDValue| {
                matches!(this.dag.opcode(n1), ISD::And | ISD::Or) && {
                    let (l, r) = (this.dag.operand(n1, 0), this.dag.operand(n1, 1));
                    l == a && r == b
                }
            };
            if joins(self, x, y) {
                return Some(self.dag.get_node(ISD::Or, vt, &[x, y]));
            }
        }
        None
    }

    /// `isBitwiseNot`: the operand of `(xor v, -1)`.
    fn bitwise_not_operand(
        &self,
        v: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(v) != ISD::Xor {
            return None;
        }
        let vt = self.dag.value_type(v);
        (self.dag.constant(self.dag.operand(v, 1)) == Some(super::fold::mask(vt.size_in_bits())))
            .then(|| self.dag.operand(v, 0))
    }
}
