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

use super::dag::{MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_bitcast(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (vt, load) = (self.dag.value_type(node), self.dag.operand(node, 0));
        if self.dag.opcode(load) != ISD::Load || load.res != 0 || !self.dag.has_one_use(load.node) {
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
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: new.node, res: 1 });
        Some(new)
    }
}
