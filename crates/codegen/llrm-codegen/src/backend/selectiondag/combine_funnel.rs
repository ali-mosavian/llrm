//! `visitFunnelShift`, the folds of a constant amount: `fsh* x, y, c` with `c`
//! a multiple of the width is `x` or `y`, a `c` past the width is `c % width`,
//! an undef or zero operand makes it a plain shift, and a funnel of two loads
//! at consecutive addresses by a whole number of bytes is one load at the
//! byte offset (`fshr (load p+4), (load p), 16` is `load p+2`).
//!
//! DEVIATE SD105: the identity fold (`fshl (srl x, c0), (shl x, c1)`
//! is `x`), the variable-amount range folds and the rotate fold are not
//! ported. Exit: the three, when a program asks.
//!
//! # See also
//! LLVM's `DAGCombiner::visitFunnelShift`
//! ([L12083](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L12083)),
//! `SelectionDAG::makeEquivalentMemoryOrdering`
//! ([L13618](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L13618)),
//! `areNonVolatileConsecutiveLoads`
//! ([L14396](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L14396)).

use super::address_analysis::BaseIndexOffset;
use super::dag::{MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_funnel_shift(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let (vt, n0, n1, n2) = (
            self.dag.value_type(node),
            self.dag.operand(node, 0),
            self.dag.operand(node, 1),
            self.dag.operand(node, 2),
        );
        let is_fshl = op == ISD::FShl;
        let width = u128::from(vt.size_in_bits());
        let amount_vt = self.dag.value_type(n2);
        let amount = self.dag.constant(n2)?;
        if amount >= width {
            let reduced = self.dag.get_constant(amount % width, amount_vt);
            return Some(self.dag.get_node(op, vt, &[n0, n1, reduced]));
        }
        if amount == 0 {
            return Some(if is_fshl { n0 } else { n1 });
        }
        let undef_or_zero =
            |this: &Self, value: SDValue| this.dag.is_undef(value) || this.dag.constant(value) == Some(0);
        if undef_or_zero(self, n0) {
            let by = self.dag.get_constant(if is_fshl { width - amount } else { amount }, amount_vt);
            return Some(self.dag.get_node(ISD::Srl, vt, &[n1, by]));
        }
        if undef_or_zero(self, n1) {
            let by = self.dag.get_constant(if is_fshl { amount } else { width - amount }, amount_vt);
            return Some(self.dag.get_node(ISD::Shl, vt, &[n0, by]));
        }
        if width % 8 == 0 && amount % 8 == 0 && vt.is_integer() {
            return self.funnel_of_consecutive_loads(is_fshl, vt.size_in_bits(), amount, n0, n1);
        }
        None
    }

    /// `fshl ld1, ld0, c` and `fshr ld1, ld0, c` of loads at consecutive
    /// addresses: the load of the bytes the funnel keeps, `ld0`'s address plus
    /// the offset.
    fn funnel_of_consecutive_loads(
        &mut self,
        is_fshl: bool,
        bits: u32,
        amount: u128,
        n0: SDValue,
        n1: SDValue,
    ) -> Option<SDValue> {
        let simple = |this: &Self, load: SDValue| -> Option<MemInfo> {
            if this.dag.opcode(load) != ISD::Load || load.res != 0 {
                return None;
            }
            let NodeData::Mem(info) = this.dag.node(load).data else { return None };
            (!info.volatile && info.ext.is_none()).then_some(info)
        };
        let (lhs, rhs) = (simple(self, n0)?, simple(self, n1)?);
        let single_use = |this: &Self, load: SDValue| this.dag.users(load).count() == 1;
        if lhs.space != rhs.space || !(single_use(self, n0) || single_use(self, n1)) {
            return None;
        }
        // `areNonVolatileConsecutiveLoads(LHS, RHS, BitWidth / 8, 1)`: the
        // same chain, `BitWidth / 8` bytes each, the first that far above.
        let bytes = i64::from(bits / 8);
        if self.dag.operand(n0, 0) != self.dag.operand(n1, 0)
            || i64::from(lhs.mem_vt.size_in_bits() / 8) != bytes
            || i64::from(rhs.mem_vt.size_in_bits() / 8) != bytes
        {
            return None;
        }
        let (lhs_at, rhs_at) = (BaseIndexOffset::matching(self.dag, n0), BaseIndexOffset::matching(self.dag, n1));
        if rhs_at.equal_base_index(&lhs_at, self.dag) != Some(bytes) {
            return None;
        }
        let offset = if is_fshl { ((u128::from(bits) - amount) % u128::from(bits)) / 8 } else { amount / 8 } as u32;
        // `commonAlignment(RHS->getAlign(), PtrOff)`; the target loads
        // unaligned scalars at full speed (`allowsMemoryAccess`).
        let align = rhs.align.min(1 << offset.trailing_zeros().min(31));
        let (chain, ptr) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1));
        let step = self.dag.get_constant(u128::from(offset), self.dag.value_type(ptr));
        let moved = self.dag.get_mem_base_plus_offset(ptr, step, Default::default());
        self.add_to_worklist(moved.node);
        let load = self.dag.get_load(self.dag.value_type(n1), chain, moved, MemInfo { align, ..rhs });
        for old in [n0, n1] {
            self.make_equivalent_memory_ordering(
                SDValue { node: old.node, res: 1 },
                SDValue { node: load.node, res: 1 },
            );
        }
        Some(load)
    }

    /// `makeEquivalentMemoryOrdering`: what read the old load's chain reads
    /// it and the new access's, so the new one takes the old one's place.
    pub(super) fn make_equivalent_memory_ordering(
        &mut self,
        old: SDValue,
        new: SDValue,
    ) {
        if old == new || self.dag.users(old).next().is_none() {
            return;
        }
        let factor = self.dag.get_token_factor(&[old, new]);
        self.replace_value(old, factor);
        self.dag.update_node_operands(factor.node, &[old, new]);
    }
}
