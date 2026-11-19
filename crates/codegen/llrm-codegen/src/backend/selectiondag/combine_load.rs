//! `visitLOAD`: a simple load nothing reads is bypassed in the chain, and a
//! load right after a store to its address is the stored value
//! (`ForwardStoreValueToDirectLoad`).
//!
//! DEVIATE SD216: the rest of `visitLOAD` (alignment inference, indexed and
//! sliced loads) is not translated, and the forwarding has no vector, float
//! or same-size bitcast cases (the
//! combiner holds no vectors or float memory types of another size). Exit:
//! those ports. Owner w10. Review 2026-11-13.
//!
//! DEVIATE LS1 (owner m16, exit 2026-11-17): [`DagCombiner::split_dword_load`]
//! splits a plain `i32` load read only as its two words. The pin's
//! `ReduceLoadWidth`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L17095))
//! narrows a load only when nothing else reads it; m16 only
//! (`split_dword_loads`).
//!
//! # See also
//! LLVM's `DAGCombiner::visitLOAD`
//! ([L22110](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L22110)).

use super::actions::LoadExt;
use super::address_analysis::BaseIndexOffset;
use super::dag::{MemBase, MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_load(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        // The root has no users but is not dead (the pin's `HandleSDNode` holds
        // it); revisiting it would re-queue it through its own chain.
        if info.volatile || node.node == self.dag.get_root().node {
            return None;
        }
        // A load whose value is read: the chain step, not the dead-load one.
        if self.dag.users(SDValue { node: node.node, res: 0 }).next().is_some() {
            // If this load is directly stored, replace the load value with the
            // stored value.
            if let Some(forwarded) = self.forward_store_value_to_direct_load(node) {
                return Some(forwarded);
            }
            self.refine_alignment(node);
            if let Some(split) = self.split_dword_load(node) {
                return Some(split);
            }
            return self.improve_load_chain(node);
        }
        // "It's not safe to use the two value CombineTo variant here": only
        // the chain result moves, to the load's input chain.
        let chain = self.dag.operand(node, 0);
        self.replace_value(SDValue { node: node.node, res: 1 }, chain);
        let users: Vec<u32> = self.dag.users(chain).collect();
        for user in users {
            self.add_to_worklist(user);
        }
        Some(SDValue { node: node.node, res: 0 })
    }

    /// DEVIATE LS1 (`split_dword_loads`): `trunc i16 L` and `trunc i16 (srl L,
    /// 16)` of a plain `i32` load `L` are loads of its two words. A use that is
    /// not a half keeps the one load.
    fn split_dword_load(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        let value = SDValue { node: node.node, res: 0 };
        if !self.hooks.split_dword_loads()
            || self.dag.value_type(node) != EVT::I32
            || info.mem_vt != EVT::I32
            || info.ext.is_some()
            || info.ordering.is_some()
            || info.volatile
        {
            return None;
        }
        let (mut lows, mut highs, mut shifts) = (Vec::new(), Vec::new(), Vec::new());
        for user in self.dag.users(value).collect::<Vec<_>>() {
            let user = SDValue { node: user, res: 0 };
            match self.dag.opcode(user) {
                ISD::Truncate if self.dag.value_type(user) == EVT::I16 => lows.push(user),
                ISD::Srl
                    if self.dag.constant(self.dag.operand(user, 1)) == Some(16)
                        && self.dag.operand(user, 0) == value =>
                {
                    shifts.push(user);
                    for half in self.dag.users(user).collect::<Vec<_>>() {
                        let half = SDValue { node: half, res: 0 };
                        if self.dag.opcode(half) != ISD::Truncate || self.dag.value_type(half) != EVT::I16 {
                            return None;
                        }
                        highs.push(half);
                    }
                }
                _ => return None,
            }
        }
        if lows.is_empty() || highs.is_empty() {
            return None;
        }
        let (chain, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let low = self.dag.get_load(EVT::I16, chain, ptr, MemInfo { mem_vt: EVT::I16, ..info });
        let high_ptr = self.dag.get_object_ptr_offset(ptr, 2);
        let base = match info.base {
            MemBase::Stack(at) => MemBase::Stack(at + 2),
            MemBase::CallArgument(at) => MemBase::CallArgument(at + 2),
            other => other,
        };
        let high_info = MemInfo { mem_vt: EVT::I16, align: info.align.min(2), base, ..info };
        let high = self.dag.get_load(EVT::I16, chain, high_ptr, high_info);
        let chains = [SDValue { node: low.node, res: 1 }, SDValue { node: high.node, res: 1 }];
        let joined = self.dag.get_token_factor(&chains);
        self.replace_value(SDValue { node: node.node, res: 1 }, joined);
        for &half in &lows {
            self.replace_value(half, low);
        }
        for &half in &highs {
            self.replace_value(half, high);
        }
        // The replaced halves stay in the arena reading the load; left so, the
        // load would be split again.
        for dead in lows.iter().chain(&highs).chain(&shifts) {
            self.dag.delete_dead(dead.node);
        }
        Some(value)
    }

    /// `isTypeLegal`: every type is fine before type legalization.
    fn is_type_legal(
        &self,
        vt: EVT,
    ) -> bool {
        self.level < CombineLevel::AfterLegalizeTypes || self.tli.is_some_and(|tli| tli.types.is_type_legal(vt))
    }

    /// `getUniqueStoreFeeding`: the one store the load reads, and the bytes
    /// from its address to the load's.
    fn unique_store_feeding(
        &self,
        load: SDValue,
    ) -> Option<(SDValue, i64)> {
        let mut chain = self.dag.operand(load, 0);
        // Look through CALLSEQ_START.
        if self.dag.opcode(chain) == ISD::CallSeqStart {
            chain = self.dag.operand(chain, 0);
        }
        let at = BaseIndexOffset::matching(self.dag, load);
        let offset_of = |store: SDValue| BaseIndexOffset::matching(self.dag, store).equal_base_index(&at, self.dag);
        if self.dag.opcode(chain) == ISD::TokenFactor {
            // Look for a unique store within the token factor.
            for op in self.dag.node(chain).ops.clone() {
                if self.dag.opcode(op) != ISD::Store {
                    continue;
                }
                let Some(offset) = offset_of(op) else { continue };
                // The store must not alias any other node of the token factor.
                let aliases = self.gather_all_aliases(op, chain);
                return (aliases.is_empty() || (aliases.len() == 1 && aliases[0].node == op.node))
                    .then_some((op, offset));
            }
            return None;
        }
        if self.dag.opcode(chain) == ISD::Store {
            return offset_of(chain).map(|offset| (chain, offset));
        }
        None
    }

    /// `ForwardStoreValueToDirectLoad`: `CombineTo(LD, Val, Chain)` when the
    /// store the load follows covers it.
    fn forward_store_value_to_direct_load(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if self.opt_none {
            return None;
        }
        let NodeData::Mem(ld) = self.dag.node(node).data else { return None };
        let (store, offset) = self.unique_store_feeding(node)?;
        let NodeData::Mem(st) = self.dag.node(store).data else { return None };
        if ld.volatile || st.volatile || st.space != ld.space {
            return None;
        }
        let ld_type = self.dag.value_type(node);
        let (ld_mem, st_mem) = (ld.mem_vt, st.mem_vt);
        let value = self.dag.operand(store, 1);
        let st_type = self.dag.value_type(value);
        // An x87 value lives on a stack that one stored value read twice cannot
        // share: floats are forwarded only where they are in registers, and
        // then only as the same type (the pin's bitcast cases are not
        // modelled).
        if ![ld_type, ld_mem, st_type, st_mem].iter().all(|vt| vt.is_integer()) {
            let plain_float = ld_type.is_floating_point()
                && ld_type == st_type
                && ld_mem == st_mem
                && ld_type == ld_mem
                && ld.ext.is_none();
            if self.dag.config().fp_stack || !plain_float {
                return None;
            }
        }
        // Normalize for endianness: offset 0 is then the stored value's least
        // significant byte.
        let store_bits = |vt: EVT| i64::from(vt.size_in_bits().div_ceil(8) * 8);
        let offset =
            if self.dag.config().big_endian { (store_bits(st_mem) - store_bits(ld_mem)) / 8 - offset } else { offset };
        // Check that the stored value covers all bits that are loaded.
        let covers = offset >= 0 && offset * 8 + i64::from(ld_mem.size_in_bits()) <= i64::from(st_mem.size_in_bits());
        if !covers {
            return None;
        }
        let forwarded = self.stored_value_as_loaded(ld.ext, ld_type, ld_mem, st_type, st_mem, value, offset)?;
        // `CombineTo(LD, Val, Chain)`: the value and the chain are replaced
        // separately, and the load is not looked at again.
        let chain = self.dag.operand(node, 0);
        self.replace_value(SDValue { node: node.node, res: 0 }, forwarded);
        self.replace_value(SDValue { node: node.node, res: 1 }, chain);
        // `CombineTo` removes the load at once.
        self.dag.delete_replaced(node.node);
        self.queue_survivors();
        Some(node)
    }

    /// The stored `value` as the load sees it, or `None` for a shape the
    /// forwarding does not model.
    #[allow(clippy::too_many_arguments)]
    fn stored_value_as_loaded(
        &mut self,
        ext: Option<LoadExt>,
        ld_type: EVT,
        ld_mem: EVT,
        st_type: EVT,
        st_mem: EVT,
        value: SDValue,
        offset: i64,
    ) -> Option<SDValue> {
        // Memory as copy space (potentially masked).
        if offset == 0 && ld_type == st_type && st_mem == ld_mem {
            // Simple case: direct non-truncating forwarding.
            if ld_type.size_in_bits() == ld_mem.size_in_bits() {
                return Some(value);
            }
            // Model the truncate and extension with an and mask.
            if st_type.is_integer() && ld_mem.is_integer() && ext != Some(LoadExt::SExtLoad) {
                let mask = (1u128 << st_mem.size_in_bits()) - 1;
                let mask = self.dag.get_constant(mask, st_type);
                return Some(self.dag.get_node(ISD::And, ld_type, &[value, mask]));
            }
        }
        // TODO (the pin's): deal with nonzero offset.
        if offset != 0 {
            return None;
        }
        // Truncate the value to the stored memory size
        // (`getTruncatedStoreValue`).
        let mut value = value;
        if st_type != st_mem {
            if self.is_type_legal(st_mem) || !(st_type.is_integer() && st_mem.is_integer()) {
                return None;
            }
            value = self.dag.get_node(ISD::Truncate, st_mem, &[value]);
        }
        if !self.is_type_legal(ld_mem) {
            return None;
        }
        if st_mem != ld_mem {
            // A same-size reinterpretation is a bitcast the pin gates on the
            // target: not modelled.
            if ld_mem.size_in_bits() == st_mem.size_in_bits() || !(st_mem.is_integer() && ld_mem.is_integer()) {
                return None;
            }
            value = self.dag.get_node(ISD::Truncate, ld_mem, &[value]);
        }
        // `extendLoadedValueToExtension`.
        if ld_type == ld_mem {
            return Some(value);
        }
        if !(ld_mem.is_integer() && ld_type.is_integer()) {
            return None;
        }
        let opcode = match ext? {
            LoadExt::ExtLoad => ISD::AnyExtend,
            LoadExt::ZExtLoad => ISD::ZeroExtend,
            LoadExt::SExtLoad => ISD::SignExtend,
        };
        Some(self.dag.get_node(opcode, ld_type, &[value]))
    }
}
