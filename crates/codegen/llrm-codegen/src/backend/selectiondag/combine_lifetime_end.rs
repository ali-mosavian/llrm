//! `visitLIFETIME_END`: a store to an object just before its lifetime ends is
//! dead, and is removed
//! ([DAGCombiner.cpp#L24933](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L24933)).
//!
//! # How it works
//! The walk goes up the chains from the `LIFETIME_END` through the nodes with
//! a single user: a token factor is followed into every operand, a lifetime
//! marker it cannot alias is passed, and a simple store that lies wholly
//! inside the object is replaced by its own chain. Anything else ends that
//! branch. m32 `variadic` -Os: the two stores of the advanced `va_list`
//! pointer into its slot, which nothing reads, stayed (+38 B against the pin's
//! backend on the same IR).
//!
//! DEVIATE CC2 (`chain_combine`) holds: a marker is the whole frame object;
//! the object's size is [`SelectionDAG::frame_object_bytes`], set by the
//! builder from the alloca. An object with no size recorded is left alone.
//!
//! DEVIATE LE1 (no number yet): the pin skips indexed and scalable stores;
//! this DAG has neither (stores are unindexed, no scalable types), so the
//! checks are absent.
//!
//! # Used by
//! `DagCombiner::visit`.

use super::address_analysis::BaseIndexOffset;
use super::dag::{NodeData, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};
use super::isd::ISD;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_lifetime_end(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let NodeData::FrameIndex(index) = self.dag.node(self.dag.operand(node, 1)).data else { return None };
        let bytes = *self.dag.frame_object_bytes.get(usize::try_from(index).ok()?)?;
        if bytes == 0 {
            return None;
        }
        let end = BaseIndexOffset::matching(&self.dag, node);
        // We walk up the chains to find stores.
        let mut chains = vec![self.dag.operand(node, 0)];
        while let Some(chain) = chains.pop() {
            if !self.dag.has_one_use(chain.node) {
                continue;
            }
            match self.dag.opcode(chain) {
                ISD::TokenFactor => {
                    // `Chains.push_back(operand(--Nops))`: the first operand
                    // is walked first.
                    let operands = self.dag.node(chain).ops.clone();
                    chains.extend(operands.iter().rev().copied());
                }
                // We can forward past any lifetime start/end that can be
                // proven not to alias the node.
                ISD::LifetimeStart | ISD::LifetimeEnd => {
                    if !self.may_alias(chain, node) {
                        chains.push(self.dag.operand(chain, 0));
                    }
                }
                ISD::Store => {
                    let NodeData::Mem(info) = self.dag.node(chain).data else { continue };
                    if info.volatile || info.ordering.is_some() {
                        continue;
                    }
                    let store = BaseIndexOffset::matching(&self.dag, chain);
                    let store_bits = i64::from(info.mem_vt.size_in_bits());
                    // If we store purely within object bounds just before its
                    // lifetime ends, we can remove the store.
                    if end.contains(&self.dag, i64::try_from(bytes).ok()? * 8, &store, store_bits) {
                        let before = self.dag.operand(chain, 0);
                        self.dag.combine_to(chain, before);
                        return Some(node);
                    }
                }
                _ => {}
            }
        }
        None
    }
}
