//! `visitBITCAST`'s load fold: `(conv (load x))` is `(load (conv*)x)`, so a
//! float argument read as its bits is one integer load, not `fld; fstp; mov`.
//!
//! DEVIATE SD251: only scalar casts of a plain load (no vector, `freeze`,
//! `AssertNoFPClass` or range metadata; `isLoadBitCastBeneficial` is true for
//! scalars on x86). Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitBITCAST`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L18314)).

use super::dag::{MemBase, MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_bitcast(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (vt, load) = (self.dag.value_type(node), self.dag.operand(node, 0));
        if self.dag.opcode(load) != ISD::Load || load.res != 0 {
            return None;
        }
        // DEVIATE SD256: a shared load is read again as the integer when every
        // user of the cast is a call argument's store.
        let shared = self.dag.use_count(load.node) != 1;
        if shared && !self.only_call_argument_stores(node) {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data.clone() else { return None };
        if info.ext.is_some() || !matches!(vt, EVT::Simple(_)) {
            return None;
        }
        // "(LegalOperations || !LN0->isSimple()) && !isOperationLegal(LOAD,
        // VT)"
        let legal_operations = self.level >= CombineLevel::AfterLegalizeVectorOps;
        if (legal_operations || info.volatile) && !self.tli.is_some_and(|tli| tli.is_operation_legal(ISD::Load, vt)) {
            return None;
        }
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let cast = MemInfo { mem_vt: vt, ..info };
        let new = self.dag.get_load(vt, chain, ptr, cast);
        let old_chain = SDValue { node: load.node, res: 1 };
        if shared {
            // What was ordered after the float load is ordered after both
            // loads (`ReplaceAllUsesOfValueWith` of the old chain by a
            // `TokenFactor` that still reads it).
            let both = self.dag.get_token_factor(&[old_chain, SDValue { node: new.node, res: 1 }]);
            self.replace_value(old_chain, both);
            self.dag.update_node_operands(both.node, &[old_chain, SDValue { node: new.node, res: 1 }]);
        } else {
            self.replace_value(old_chain, SDValue { node: new.node, res: 1 });
        }
        Some(new)
    }

    /// Whether every user of `cast` stores it to a call's argument slot.
    fn only_call_argument_stores(
        &self,
        cast: SDValue,
    ) -> bool {
        let mut users = self.dag.users(cast).peekable();
        users.peek().is_some()
            && users.all(|user| {
                let user = SDValue { node: user, res: 0 };
                self.dag.opcode(user) == ISD::Store
                    && matches!(
                        self.dag.node(user).data,
                        NodeData::Mem(MemInfo { base: MemBase::CallArgument(_), .. })
                    )
            })
    }
}
