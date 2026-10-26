//! `visitLOAD`, the dead-load half: a simple load nothing reads is bypassed
//! in the chain.
//!
//! DEVIATE SD321: the rest of `visitLOAD` (store-to-load forwarding,
//! `FindBetterChain`, alignment inference, indexed and sliced loads) is not
//! translated: it needs `BaseIndexOffset` and the alias queries. Exit: those
//! ports. Owner w10. Review 2026-11-13.
//!
//! # See also
//! LLVM's `DAGCombiner::visitLOAD`
//! ([L22110](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L22110)).

use super::dag::{NodeData, SDValue};
use super::dag_combiner::{CombineHooks, DagCombiner};

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_load(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        // The root has no users but is not dead (the pin's `HandleSDNode` holds
        // it); revisiting it would re-queue it through its own chain.
        if info.volatile
            || node.node == self.dag.get_root().node
            || self.dag.users(SDValue { node: node.node, res: 0 }).next().is_some()
        {
            return None;
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
}
