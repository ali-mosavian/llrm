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
//! # See also
//! LLVM's `DAGCombiner::visitLOAD`
//! ([L22110](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L22110)).

use super::actions::LoadExt;
use super::address_analysis::BaseIndexOffset;
use super::dag::{NodeData, SDValue};
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
        // share: only integers are forwarded.
        if ![ld_type, ld_mem, st_type, st_mem].iter().all(|vt| vt.is_integer()) {
            return None;
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
