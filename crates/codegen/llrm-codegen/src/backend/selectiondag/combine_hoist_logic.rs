//! `hoistLogicOpWithSameOpcodeHands`: `and`, `or` and `xor` of two nodes with
//! the same opcode sink that opcode below the logic operation, `xor (zext a),
//! (zext b)` is `zext (xor a, b)`.
//!
//! Ported: the extension hands (`zext`, `sext`, `anyext`, `sext_inreg`),
//! `trunc`, the shifts and `and` by a shared amount, `bswap`, the funnel shifts
//! and an integer `bitcast`. Not ported: the vector hands (`scalar_to_vector`,
//! `vector_shuffle`, `*_extend_vector_inreg`). DEVIATE HLO1 (owner m64-b, exit
//! 2026-11-27): `anyext` is not hoisted after type legalization, where the pin
//! asks `isTypeDesirableForOp`.
//!
//! # See also
//! LLVM's `DAGCombiner::hoistLogicOpWithSameOpcodeHands`
//! ([L6588](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L6588)).

use super::dag::{SDNodeFlags, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::isd::ISD;
use super::mvt::EVT;

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `isOperationLegalOrCustom`; false without a table.
    fn logic_legal_or_custom(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        self.tli.is_some_and(|tli| tli.is_operation_legal_or_custom(op, vt))
    }

    pub(super) fn hoist_logic_op_with_same_opcode_hands(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(n0);
        let logic = self.dag.opcode(node);
        let hand = self.dag.opcode(n0);
        if hand != self.dag.opcode(n1) || self.dag.node(n0).ops.is_empty() {
            return None;
        }
        let (x, y) = (self.dag.operand(n0, 0), self.dag.operand(n1, 0));
        let x_vt = self.dag.value_type(x);
        let both_used = !self.dag.has_one_use(n0.node) && !self.dag.has_one_use(n1.node);
        let either_shared = !self.dag.has_one_use(n0.node) || !self.dag.has_one_use(n1.node);
        let is_ext = matches!(hand, ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend);
        if is_ext || (hand == ISD::SignExtendInReg && self.dag.operand(n0, 1) == self.dag.operand(n1, 1)) {
            // Both operands have other uses: this creates an instruction and
            // removes none.
            if both_used || x_vt != self.dag.value_type(y) {
                return None;
            }
            // No illegal operation during or after legalization, no unsupported
            // vector one.
            if (vt.is_vector() || self.legal_operations()) && !self.logic_legal_or_custom(logic, x_vt) {
                return None;
            }
            if hand == ISD::AnyExtend && self.level >= CombineLevel::AfterLegalizeTypes {
                return None;
            }
            let disjoint = self.dag.node(node).flags.has(SDNodeFlags::DISJOINT) && is_ext;
            let flags = if disjoint { SDNodeFlags::DISJOINT } else { SDNodeFlags(0) };
            let inner = self.dag.get_node_flags(logic, x_vt, &[x, y], flags);
            return Some(if hand == ISD::SignExtendInReg {
                self.dag.get_node(hand, vt, &[inner, self.dag.operand(n0, 1)])
            } else {
                self.dag.get_node(hand, vt, &[inner])
            });
        }
        if hand == ISD::Truncate {
            if both_used || x_vt != self.dag.value_type(y) {
                return None;
            }
            if self.legal_operations() && !self.tli.is_some_and(|tli| tli.is_operation_legal(logic, x_vt)) {
                return None;
            }
            // Sinking a free truncate gains nothing; no logic operation on an
            // illegal type.
            if self.hooks.is_zext_free(vt, x_vt) && self.hooks.is_truncate_free(x_vt, vt) {
                return None;
            }
            if !self.tli.is_some_and(|tli| tli.types.is_type_legal(x_vt)) {
                return None;
            }
            let inner = self.dag.get_node(logic, x_vt, &[x, y]);
            return Some(self.dag.get_node(hand, vt, &[inner]));
        }
        if matches!(hand, ISD::Shl | ISD::Srl | ISD::Sra | ISD::And)
            && self.dag.operand(n0, 1) == self.dag.operand(n1, 1)
        {
            if either_shared {
                return None;
            }
            let inner = self.dag.get_node(logic, x_vt, &[x, y]);
            return Some(self.dag.get_node(hand, vt, &[inner, self.dag.operand(n0, 1)]));
        }
        if hand == ISD::BSwap {
            if either_shared {
                return None;
            }
            let inner = self.dag.get_node(logic, x_vt, &[x, y]);
            return Some(self.dag.get_node(hand, vt, &[inner]));
        }
        if matches!(hand, ISD::FShl | ISD::FShr) && self.dag.operand(n0, 2) == self.dag.operand(n1, 2) {
            if either_shared {
                return None;
            }
            let (x1, y1) = (self.dag.operand(n0, 1), self.dag.operand(n1, 1));
            let low = self.dag.get_node(logic, vt, &[x, y]);
            let high = self.dag.get_node(logic, vt, &[x1, y1]);
            return Some(self.dag.get_node(hand, vt, &[low, high, self.dag.operand(n0, 2)]));
        }
        // Integer sources of one type, up to type legalization (a vector
        // bitcast of a scalar the target cannot hold stays).
        if hand == ISD::BitCast && self.level <= CombineLevel::AfterLegalizeTypes {
            let legal = |v| self.tli.is_some_and(|tli| tli.types.is_type_legal(v));
            if x_vt.is_integer()
                && x_vt == self.dag.value_type(y)
                && !(vt.is_vector() && legal(vt) && !x_vt.is_vector() && !legal(x_vt))
            {
                let inner = self.dag.get_node(logic, x_vt, &[x, y]);
                return Some(self.dag.get_node(hand, vt, &[inner]));
            }
        }
        None
    }
}
