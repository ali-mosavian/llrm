//! `reassociateOps` for add, mul, and, or and xor: a commutative `op` of an
//! `op` and a third value, grouped so constants meet and a node already built
//! is reused.
//!
//! `(add (add x, 4), y)` is `(add (add x, y), 4)`: the constant moves out where
//! the matcher folds it into one `lea`. `(add (add x, 4), 5)` is `(add x, 9)`.
//!
//! DEVIATE SD211: the node flags (`nuw`, `disjoint`) the pin carries to the new
//! nodes are dropped, and the compare-pair rule for `and`/`or` of `setcc`s
//! runs only where `reassociate_compare_pairs` says so (x86 judged later).
//! Exit: flags on the combiner's node builder. Owner w12.
//! Review 2026-12-10.
//!
//! # See also
//! LLVM's `DAGCombiner::reassociateOps`
//! ([L1336](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L1336)).

use super::dag::SDValue;
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    fn plain_constant(
        &self,
        value: SDValue,
    ) -> bool {
        self.dag.constant(value).is_some() && !self.dag.is_opaque(value)
    }

    /// `reassociateOps` for a commutative integer `opc`.
    pub(super) fn reassociate_ops(
        &mut self,
        opc: ISD,
        n0: SDValue,
        n1: SDValue,
    ) -> Option<SDValue> {
        self.reassociate_ops_commutative(opc, n0, n1).or_else(|| self.reassociate_ops_commutative(opc, n1, n0))
    }

    /// `reassociationCanBreakAddressingModePattern` for `ADD`
    /// ([L1109](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L1109)):
    /// whether reassociating `(add n0, n1)` would undo a split of an address
    /// into a base with an offset that a load or store folds.
    ///
    /// DEVIATE SD210: the vscale half, the global-address offset test and
    /// `hasUniqueMemOperand` are not translated (x86 has no scalable
    /// addressing here, and every access counts as unique); an offset is legal
    /// when it fits a signed 32-bit displacement
    /// (`X86::isOffsetSuitableForCodeModel`, small model). Exit: those. Owner
    /// sdag-b. Review 2026-12-10.
    pub(super) fn reassociation_can_break_addressing_mode_pattern(
        &self,
        node: SDValue,
        n0: SDValue,
        n1: SDValue,
    ) -> bool {
        if self.dag.opcode(n0) != ISD::Add {
            return false;
        }
        let legal = |offset: i128| i128::from(i32::MIN) <= offset && offset <= i128::from(i32::MAX);
        let signed = |value: u128, vt: EVT| {
            let bits = vt.size_in_bits();
            let shift = 128 - bits;
            ((value << shift) as i128) >> shift
        };
        let vt = self.dag.value_type(n1);
        let Some(c2) = self.dag.constant(n1).map(|value| signed(value, vt)) else { return false };
        let is_memory = |user: u32| matches!(
            self.dag.opcode(SDValue { node: user, res: 0 }),
            ISD::Load | ISD::Store
        );
        if let Some(c1) = self.dag.constant(self.dag.operand(n0, 1)).map(|value| signed(value, vt)) {
            if self.dag.has_one_use(n0.node) {
                return false;
            }
            for user in self.dag.users(node) {
                // x[offset2] already not legal: nothing to break; else would
                // x[offset1 + offset2] still be?
                if is_memory(user) && legal(c2) && !legal(c1 + c2) {
                    return true;
                }
            }
            return false;
        }
        // `(add (add x, y), c2)` only used by accesses with `x[c2]` legal.
        self.dag.users(node).all(|user| is_memory(user) && legal(c2))
    }

    fn reassociate_ops_commutative(
        &mut self,
        opc: ISD,
        n0: SDValue,
        n1: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(n0) != opc {
            return None;
        }
        let vt = self.dag.value_type(n0);
        let (n00, n01) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
        // `isReassocProfitable`: the default, `N0->hasOneUse()`.
        let profitable = self.dag.has_one_use(n0.node);
        if self.plain_constant(n01) {
            if self.plain_constant(n1) {
                // `(op (op x, c1), c2)` is `(op x, (op c1, c2))`.
                let folded = self.dag.get_node(opc, vt, &[n01, n1]);
                if self.dag.constant(folded).is_none() {
                    return None;
                }
                return Some(self.dag.get_node(opc, vt, &[n00, folded]));
            }
            if profitable {
                // `(op (op x, c1), y)` is `(op (op x, y), c1)`.
                let inner = self.dag.get_node(opc, vt, &[n00, n1]);
                self.add_to_worklist(inner.node);
                return Some(self.dag.get_node(opc, vt, &[inner, n01]));
            }
        }
        match opc {
            ISD::And | ISD::Or if n1 == n00 || n1 == n01 => return Some(n0),
            ISD::Xor if n1 == n00 => return Some(n01),
            ISD::Xor if n1 == n01 => return Some(n00),
            _ => {}
        }
        if profitable {
            let exists = |this: &Self, a: SDValue, b: SDValue| this.dag.get_node_if_exists(opc, &[vt], &[a, b]);
            if n1 != n01 {
                if let Some(found) = exists(self, n00, n1) {
                    let found = SDValue { node: found, res: 0 };
                    if exists(self, found, n01).is_none() {
                        return Some(self.dag.get_node(opc, vt, &[found, n01]));
                    }
                }
            }
            if n1 != n00 {
                if let Some(found) = exists(self, n01, n1) {
                    let found = SDValue { node: found, res: 0 };
                    if exists(self, found, n00).is_none() {
                        return Some(self.dag.get_node(opc, vt, &[found, n00]));
                    }
                }
            }
            // `(or (or a, b), c)` of compares where `c` and `b` share a
            // predicate: pair those two.
            if matches!(opc, ISD::And | ISD::Or)
                && self.hooks.reassociate_compare_pairs()
                && [n1, n00, n01].iter().all(|&v| self.dag.opcode(v) == ISD::SetCC)
            {
                let code = |this: &Self, v: SDValue| this.dag.node(this.dag.operand(v, 2)).data;
                let (cc1, cc00, cc01) = (code(self, n1), code(self, n00), code(self, n01));
                if cc1 == cc00 && cc1 != cc01 {
                    let inner = self.dag.get_node(opc, vt, &[n00, n1]);
                    return Some(self.dag.get_node(opc, vt, &[inner, n01]));
                }
                if cc1 == cc01 && cc1 != cc00 {
                    let inner = self.dag.get_node(opc, vt, &[n01, n1]);
                    return Some(self.dag.get_node(opc, vt, &[inner, n00]));
                }
            }
        }
        None
    }
}
