//! `visitBITCAST`'s load fold: `(conv (load x))` is `(load (conv*)x)`, so a
//! float argument read as its bits is one integer load, not `fld; fstp; mov`.
//! And `foldBitcastedFPLogic` (a port): a sign-bit `xor`/`and`/`or` of a
//! float's bits, cast back, is `fneg`/`fabs`/`fneg (fabs)` (`__negdf2` on wasm:
//! `f64.neg`, 7 B, not reinterpret, const, xor, reinterpret, 20 B).
//!
//! DEVIATE SD548: `foldBitcastedFPLogic` takes the operand only when it is a
//! cast from the result type; the target's `isFNegFree`/`isFAbsFree` second
//! way (no target here answers true) and vector splat masks are not ported.
//! Owner softfloat. Exit: when a target answers `isFNegFree`.
//!
//! DEVIATE SD251: only scalar casts of a plain load (no vector, `freeze`,
//! `AssertNoFPClass` or range metadata; `isLoadBitCastBeneficial` is true for
//! scalars on x86). Owner sdag-b.
//!
//! # See also
//! LLVM's `DAGCombiner::visitBITCAST`
//! ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L18314));
//! `foldBitcastedFPLogic`
//! ([L18247](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L18247)).

use super::dag::{MemBase, MemInfo, NodeData, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    pub(super) fn visit_bitcast(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if let Some(fp) = self.fold_bitcasted_fp_logic(node) {
            return Some(fp);
        }
        let (vt, load) = (self.dag.value_type(node), self.dag.operand(node, 0));
        if self.dag.opcode(load) != ISD::Load || load.res != 0 {
            return None;
        }
        // DEVIATE SD256: a shared load is read again as the integer when every
        // user of the cast is a call argument's store.
        let shared = !self.dag.has_one_use(load.node);
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

    /// `foldBitcastedFPLogic`: `(bitcast fp (xor (bitcast int x) signmask))` is
    /// `(fneg x)`; `and` with the complement of the mask is `fabs`, `or` with
    /// the mask is `fneg (fabs x)`.
    fn fold_bitcasted_fp_logic(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let logic = self.dag.operand(node, 0);
        let width = self.dag.value_type(logic).size_in_bits();
        if !vt.is_floating_point() || vt.size_in_bits() != width || width > 64 {
            return None;
        }
        let sign = 1u128 << (width - 1);
        let all = (1u128 << width) - 1;
        let (fp_op, mask) = match self.dag.opcode(logic) {
            ISD::And => (ISD::FAbs, all & !sign),
            ISD::Xor => (ISD::FNeg, sign),
            ISD::Or => (ISD::FAbs, sign),
            _ => return None,
        };
        let legal_operations = self.level >= CombineLevel::AfterLegalizeVectorOps;
        if legal_operations && !self.tli.is_some_and(|tli| tli.is_operation_legal(fp_op, vt)) {
            return None;
        }
        if self.dag.constant(self.dag.operand(logic, 1))? & all != mask {
            return None;
        }
        let bits = self.dag.operand(logic, 0);
        if self.dag.opcode(bits) != ISD::BitCast || self.dag.value_type(self.dag.operand(bits, 0)) != vt {
            return None;
        }
        let fp = self.dag.get_node(fp_op, vt, &[self.dag.operand(bits, 0)]);
        Some(if self.dag.opcode(logic) == ISD::Or { self.dag.get_node(ISD::FNeg, vt, &[fp]) } else { fp })
    }
}
