//! `DAGCombiner::CombineToPreIndexedLoadStore`
//! ([L21466][pre]) and `CombineToPostIndexedLoadStore` ([L21775][post]): a
//! load or store whose address is an add or subtract with other users takes
//! the update itself (`ldr x, [p, #8]!`, `ldr x, [p], #8`).
//!
//! DEVIATE: no masked loads or stores (no vectors). Off unless the target
//! says an indexed access is legal (`CombineHooks::is_indexed_legal`); x86 has
//! none. Exit: none.
//!
//! [pre]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L21466
//! [post]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L21775

use super::dag::{NodeData, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;

/// `SelectionDAG::getHasPredecessorMaxSteps`.
const MAX_STEPS: usize = 8192;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `getCombineLoadStoreParts`: whether `n` is an unindexed load or store
    /// the target can index, and its address.
    fn combine_load_store_parts(
        &self,
        n: SDValue,
        post: bool,
    ) -> Option<(bool, SDValue)> {
        let is_load = match self.dag.opcode(n) {
            ISD::Load => true,
            ISD::Store => false,
            _ => return None,
        };
        let NodeData::Mem(info) = self.dag.node(n).data else { return None };
        if !self.hooks.is_indexed_legal(is_load, info.mem_vt, post) {
            return None;
        }
        let ptr = self.dag.operand(n, if is_load { 1 } else { 2 });
        Some((is_load, ptr))
    }

    /// `SDNode::hasPredecessorHelper`: whether `target` is an operand of a node
    /// reachable through operands from `starts`, not going through
    /// `visited`. Giving up after the step limit counts as reachable.
    fn reaches_operand(
        &self,
        target: u32,
        starts: &[u32],
        mut visited: std::collections::HashSet<u32>,
    ) -> bool {
        if visited.contains(&target) {
            return true;
        }
        let mut work: Vec<u32> = starts.to_vec();
        let mut steps = 0;
        while let Some(node) = work.pop() {
            let mut found = false;
            for op in &self.dag.node(SDValue { node, res: 0 }).ops {
                if visited.insert(op.node) {
                    work.push(op.node);
                }
                found |= op.node == target;
            }
            if found {
                return true;
            }
            steps += 1;
            if steps >= MAX_STEPS {
                return true;
            }
        }
        false
    }

    /// `canFoldInAddressingMode`: `user` is an unindexed access that takes `n`
    /// (an add or subtract) as its address and the target can fold it.
    fn can_fold_in_addressing_mode(
        &self,
        n: SDValue,
        user: u32,
    ) -> bool {
        let user = SDValue { node: user, res: 0 };
        let (is_load, address_at) = match self.dag.opcode(user) {
            ISD::Load => (true, 1),
            ISD::Store => (false, 2),
            _ => return false,
        };
        let _ = is_load;
        if self.dag.operand(user, address_at).node != n.node {
            return false;
        }
        let NodeData::Mem(info) = self.dag.node(user).data else { return false };
        let negate = match self.dag.opcode(n) {
            ISD::Add => false,
            ISD::Sub => true,
            _ => return false,
        };
        let offset = self.dag.operand(n, 1);
        let (base_offs, scale) = match self.dag.constant(offset) {
            Some(_) => {
                let value = self.signed_constant(offset);
                (if negate { -value } else { value }, 0)
            }
            None => (0, 1),
        };
        self.hooks.is_legal_addressing_mode(info.mem_vt, base_offs, scale)
    }

    /// `DAG.getConstant(offset, LHS->getValueType(0))`: the offset as a
    /// constant of the type of the update's second operand.
    fn offset_constant(
        &mut self,
        address: SDValue,
        offset: i64,
    ) -> SDValue {
        let vt = self.dag.value_type(self.dag.operand(address, 1));
        self.dag.get_constant(offset as u128 & super::fold::mask(vt.size_in_bits()), vt)
    }

    /// `ConstantSDNode::getSExtValue`.
    fn signed_constant(
        &self,
        value: SDValue,
    ) -> i64 {
        let bits = self.dag.value_type(value).size_in_bits();
        let raw = self.dag.constant(value).unwrap_or(0);
        let shift = 128 - bits;
        (((raw << shift) as i128) >> shift) as i64
    }

    fn is_frame_or_register(
        &self,
        value: SDValue,
    ) -> bool {
        matches!(self.dag.opcode(value), ISD::FrameIndex | ISD::Register)
    }

    /// `CombineToPreIndexedLoadStore`.
    pub(super) fn combine_to_pre_indexed_load_store(
        &mut self,
        n: SDValue,
    ) -> Option<SDValue> {
        if self.level < CombineLevel::AfterLegalizeDag {
            return None;
        }
        let (is_load, ptr) = self.combine_load_store_parts(n, false)?;
        // "If the pointer is not an add/sub, or if it doesn't have multiple
        // uses, bail out."
        if !matches!(self.dag.opcode(ptr), ISD::Add | ISD::Sub) || self.dag.has_one_use(ptr.node) {
            return None;
        }
        let (mut base, signed_offset) = self.hooks.indexed_address_parts(self.dag, n, ptr, false)?;
        let mut offset = self.offset_constant(ptr, signed_offset);
        // "Backends without true r+i pre-indexed forms may need to pass a
        // constant base with a variable offset".
        let mut swapped = false;
        if self.dag.constant(base).is_some() {
            std::mem::swap(&mut base, &mut offset);
            swapped = true;
        }
        // "Don't create a indexed load / store with zero offset."
        if self.dag.constant(offset) == Some(0) {
            return None;
        }
        // Check #1: a frame index or register base would need a copy.
        if self.is_frame_or_register(base) {
            return None;
        }
        // Check #2.
        if !is_load {
            let value = self.dag.operand(n, 1);
            if value == base || value == ptr || self.reaches_operand(ptr.node, &[value.node], Default::default()) {
                return None;
            }
        }
        // "The offset is a constant: other adds of constants on the base fold
        // into the update."
        let mut other_uses: Vec<u32> = Vec::new();
        if self.dag.constant(offset).is_some() {
            for user in self.dag.users(base).collect::<Vec<_>>() {
                if user == ptr.node {
                    continue;
                }
                if self.reaches_operand(user, &[n.node], Default::default()) {
                    continue;
                }
                let user_value = SDValue { node: user, res: 0 };
                if !matches!(self.dag.opcode(user_value), ISD::Add | ISD::Sub) {
                    other_uses.clear();
                    break;
                }
                let other = if self.dag.operand(user_value, 0) == base {
                    self.dag.operand(user_value, 1)
                } else {
                    self.dag.operand(user_value, 0)
                };
                if self.dag.constant(other).is_none() || self.dag.value_type(other) != self.dag.value_type(offset) {
                    other_uses.clear();
                    break;
                }
                other_uses.push(user);
            }
        }
        if swapped {
            std::mem::swap(&mut base, &mut offset);
        }
        // Checks #3 and #4.
        let mut real_use = false;
        for user in self.dag.users(ptr).collect::<Vec<_>>() {
            if user == n.node {
                continue;
            }
            if self.reaches_operand(user, &[n.node], Default::default()) {
                return None;
            }
            if !self.can_fold_in_addressing_mode(ptr, user) {
                real_use = true;
            }
        }
        if !real_use {
            return None;
        }
        let result = if is_load {
            self.dag.get_indexed_load(n, base, offset, false)
        } else {
            self.dag.get_indexed_store(n, base, offset, false)
        };
        let new_base = SDValue { node: result.node, res: if is_load { 1 } else { 0 } };
        if is_load {
            self.replace_value(SDValue { node: n.node, res: 0 }, SDValue { node: result.node, res: 0 });
            self.replace_value(SDValue { node: n.node, res: 1 }, SDValue { node: result.node, res: 2 });
        } else {
            self.replace_value(SDValue { node: n.node, res: 0 }, SDValue { node: result.node, res: 1 });
        }
        // "Replace other uses of BasePtr that can be updated to use Ptr."
        for other in other_uses {
            let other_value = SDValue { node: other, res: 0 };
            let offset_at =
                if self.dag.operand(other_value, 1) == base || self.dag.operand(other_value, 1).node == base.node {
                    0
                } else {
                    1
                };
            let c0 = self.signed_constant(self.dag.operand(other_value, offset_at));
            let c1 = self.signed_constant(offset);
            let is_sub = self.dag.opcode(other_value) == ISD::Sub;
            let x0 = if is_sub && offset_at == 1 { -1i64 } else { 1 };
            let y0 = if is_sub && offset_at == 0 { -1i64 } else { 1 };
            // Only PRE_INC is made: x1 = y1 = 1.
            let opcode = if y0 < 0 { ISD::Sub } else { ISD::Add };
            let mut cnv = c0;
            if x0 < 0 {
                cnv = -cnv;
            }
            cnv = if y0 < 0 { cnv + c1 } else { cnv - c1 };
            let vt = self.dag.value_type(self.dag.operand(other_value, offset_at));
            let mask = super::fold::mask(vt.size_in_bits());
            let new_offset = self.dag.get_constant(cnv as u128 & mask, vt);
            let ovt = self.dag.value_type(other_value);
            let new_use = self.dag.get_node(opcode, ovt, &[new_offset, new_base]);
            self.replace_value(other_value, new_use);
        }
        self.replace_value(ptr, new_base);
        Some(result)
    }

    /// `shouldCombineToPostInc`.
    fn should_combine_to_post_inc(
        &self,
        n: SDValue,
        ptr: SDValue,
        ptr_use: u32,
    ) -> Option<(SDValue, i64)> {
        let use_value = SDValue { node: ptr_use, res: 0 };
        if ptr_use == n.node || !matches!(self.dag.opcode(use_value), ISD::Add | ISD::Sub) {
            return None;
        }
        let (base, offset) = self.hooks.indexed_address_parts(self.dag, n, use_value, true)?;
        if offset == 0 || self.is_frame_or_register(base) {
            return None;
        }
        for user in self.dag.users(base).collect::<Vec<_>>() {
            if user == ptr.node {
                continue;
            }
            // "No if there's a later user which could perform the index
            // instead."
            if self.combine_load_store_parts(SDValue { node: user, res: 0 }, true).is_some()
                && self.reaches_operand(n.node, &[user], Default::default())
            {
                return None;
            }
            // "If all the uses are load / store addresses, then don't do the
            // transformation."
            let user_value = SDValue { node: user, res: 0 };
            if matches!(self.dag.opcode(user_value), ISD::Add | ISD::Sub) {
                for user_user in self.dag.users(user_value).collect::<Vec<_>>() {
                    if self.can_fold_in_addressing_mode(user_value, user_user) {
                        return None;
                    }
                }
            }
        }
        Some((base, offset))
    }

    /// `CombineToPostIndexedLoadStore`.
    pub(super) fn combine_to_post_indexed_load_store(
        &mut self,
        n: SDValue,
    ) -> Option<SDValue> {
        if self.level < CombineLevel::AfterLegalizeDag {
            return None;
        }
        let (is_load, ptr) = self.combine_load_store_parts(n, true)?;
        if self.dag.has_one_use(ptr.node) {
            return None;
        }
        let mut found = None;
        for op in self.dag.users(ptr).collect::<Vec<_>>() {
            // Check for #1.
            let Some((base, signed_offset)) = self.should_combine_to_post_inc(n, ptr, op) else { continue };
            // Check for #2: "Op must be independent of N".
            let seeded: std::collections::HashSet<u32> = [ptr.node].into_iter().collect();
            if !self.reaches_operand(n.node, &[n.node, op], seeded.clone())
                && !self.reaches_operand(op, &[n.node, op], seeded)
            {
                found = Some((op, base, signed_offset, op));
                break;
            }
        }
        let (op, base, signed_offset, address) = found?;
        let offset = self.offset_constant(SDValue { node: address, res: 0 }, signed_offset);
        let result = if is_load {
            self.dag.get_indexed_load(n, base, offset, true)
        } else {
            self.dag.get_indexed_store(n, base, offset, true)
        };
        if is_load {
            self.replace_value(SDValue { node: n.node, res: 0 }, SDValue { node: result.node, res: 0 });
            self.replace_value(SDValue { node: n.node, res: 1 }, SDValue { node: result.node, res: 2 });
        } else {
            self.replace_value(SDValue { node: n.node, res: 0 }, SDValue { node: result.node, res: 1 });
        }
        self.replace_value(
            SDValue { node: op, res: 0 },
            SDValue { node: result.node, res: if is_load { 1 } else { 0 } },
        );
        Some(result)
    }
}
