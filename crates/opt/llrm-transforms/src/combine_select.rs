//! `FoldOpIntoSelect`: an instruction that reads a select becomes a select of
//! the instruction applied to each arm.
//!
//! Translation of
//! [`InstCombinerImpl::FoldOpIntoSelect`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1793)
//! with `simplifyOperationIntoSelectOperand` (:1763) and
//! `foldOperationIntoSelectOperand` (:1787) at d1106deb71cc.
//!
//! MIRROR: a select with one user (or any, if asked); not `i1`; at least one
//! arm where the instruction simplifies (the select's own arm, the value an
//! `icmp eq`/`ne` condition says an operand is, `zext` of the condition as
//! 1/0); the other arm gets a copy of the instruction; the new select keeps the
//! select's `prof` and `unpredictable` metadata.
//!
//! DEVIATE 1: the min/max-intrinsic guard is not made: the callers pass binary
//! operators, compares, casts and address computations, and MIR has no
//! `MinMaxIntrinsic` among them.
//!
//! # Used by
//! `foldBinOpIntoSelectOrPhi` and the `visit*` of the families that fold an
//! operation into a select.
//!
//! # Example
//! ```text
//! %s = select i1 %c, i16 3, i16 4          %r = select i1 %c, i16 8, i16 9
//! %r = add i16 %s, 5                 ==>   (the add folds into both arms)
//! ```

use llrm_mir::module::{InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{CastOp, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::simplify::simplify_instruction_operands_at;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison;

use crate::combine::{Combiner, Visit};
use crate::combine_phi::UB_IMPLYING;

/// `CmpInst::isCommutative` of a float compare.
fn commutative(pred: FloatPredicate) -> bool {
    matches!(
        pred,
        FloatPredicate::Oeq
            | FloatPredicate::One
            | FloatPredicate::Ueq
            | FloatPredicate::Une
            | FloatPredicate::Ord
            | FloatPredicate::Uno
    )
}

impl Combiner<'_> {
    /// `Value::hasOneUser`: every use is by the same instruction.
    pub(crate) fn has_one_user(
        &self,
        value: ValueId,
    ) -> bool {
        let users = self.function.users(value);
        users.first().is_some_and(|first| users.iter().all(|one| one.user == first.user))
    }

    /// The definition of `v`, if an instruction defines it.
    fn definition(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = v else { return None };
        let ValueDef::Instruction(def) = self.function.value(value).def else { return None };
        Some(def)
    }

    /// `simplifyOperationIntoSelectOperand`.
    fn simplify_operation_into_select_operand(
        &mut self,
        inst: InstId,
        select: InstId,
        is_true_arm: bool,
    ) -> Option<Operand> {
        let select_value = Operand::Value(self.function.instruction(select).result.expect("a value"));
        let [condition, if_true, if_false] = self.function.instruction(select).operands[..] else { return None };
        let mut operands = Vec::new();
        for op in self.function.instruction(inst).operands.to_vec() {
            let value = if op == select_value {
                if is_true_arm { if_true } else { if_false }
            } else if let Some(known) = self.compared_operand(condition, op, is_true_arm) {
                known
            } else if self.definition(op).is_some_and(|def| {
                let zext = self.function.instruction(def);
                zext.opcode == Opcode::Cast(CastOp::ZExt) && zext.operands[..] == [condition]
            }) {
                let ty = self.type_of(op);
                self.int(ty, is_true_arm as u128)
            } else {
                op
            };
            operands.push(value);
        }
        let env = self.env;
        simplify_instruction_operands_at(self.context, &[], env.layout, None, self.function, None, inst, &operands)
    }

    /// `m_SpecificICmp(eq/ne, m_Specific(op), m_Value(V))` of the condition
    /// with `V` not undef or poison: the value `op` has on this arm.
    fn compared_operand(
        &self,
        condition: Operand,
        op: Operand,
        is_true_arm: bool,
    ) -> Option<Operand> {
        let compare = self.function.instruction(self.definition(condition)?);
        let wanted = if is_true_arm { IntPredicate::Eq } else { IntPredicate::Ne };
        let [lhs, rhs] = compare.operands[..] else { return None };
        if compare.opcode != Opcode::ICmp(wanted) || lhs != op {
            return None;
        }
        let q = Query::bare(self.context, self.function, self.env.layout);
        is_guaranteed_not_to_be_undef_or_poison(&q, rhs, 0).then_some(rhs)
    }

    /// `foldOperationIntoSelectOperand`: a copy of `inst` before it that reads
    /// `new` where it read the select.
    fn fold_operation_into_select_operand(
        &mut self,
        inst: InstId,
        select_value: Operand,
        new: Operand,
    ) -> Operand {
        let clone = self.function.clone_instruction(inst);
        for (slot, op) in self.function.instruction(inst).operands.to_vec().into_iter().enumerate() {
            if op == select_value {
                self.function.set_operand(clone, slot, new);
            }
        }
        for kind in UB_IMPLYING {
            self.function.unannotate(clone, kind);
        }
        self.function.insert(clone, llrm_mir::edit::Position::Before(inst)).expect("a placed instruction");
        self.worklist.add(clone);
        Operand::Value(self.function.instruction(clone).result.expect("a value"))
    }

    /// `FoldOpIntoSelect(Op, SI, FoldWithMultiUse, SimplifyBothArms)`.
    pub fn fold_op_into_select(
        &mut self,
        inst: InstId,
        select: InstId,
        fold_with_multi_use: bool,
        simplify_both_arms: bool,
    ) -> Visit {
        let select_result = self.function.instruction(select).result.expect("a value");
        // Don't modify shared selects unless asked.
        if !self.has_one_user(select_result) && !fold_with_multi_use {
            return Visit::None;
        }
        let [condition, if_true, if_false] = self.function.instruction(select).operands[..] else { return Visit::None };
        let ty = self.function.instruction(select).ty;
        // Bool selects with constant operands can be folded to logical ops.
        if self.context.types.int_bits(ty) == Some(1) {
            return Visit::None;
        }
        // A float compare used only by a select of its operands is a min or
        // max: left for the analyses that know the idiom.
        if let Some(def) = self.definition(condition)
            && let Opcode::FCmp(pred) = self.function.instruction(def).opcode
            && let Operand::Value(result) = condition
            && self.function.users(result).has_one_use()
        {
            let [lhs, rhs] = self.function.instruction(def).operands[..] else { return Visit::None };
            if ((if_true == lhs && if_false == rhs) || (if_false == lhs && if_true == rhs)) && !commutative(pred) {
                return Visit::None;
            }
        }
        // One of the arms must fold.
        let mut new_true = self.simplify_operation_into_select_operand(inst, select, true);
        let mut new_false = self.simplify_operation_into_select_operand(inst, select, false);
        if new_true.is_none() && new_false.is_none() {
            return Visit::None;
        }
        if simplify_both_arms && !(new_true.is_some() && new_false.is_some()) {
            return Visit::None;
        }
        // A copy for the arm that did not.
        let select_value = Operand::Value(select_result);
        if new_true.is_none() {
            new_true = Some(self.fold_operation_into_select_operand(inst, select_value, if_true));
        }
        if new_false.is_none() {
            new_false = Some(self.fold_operation_into_select_operand(inst, select_value, if_false));
        }
        let result_ty = self.function.instruction(inst).ty;
        let new = self.insert_before(
            inst,
            Opcode::Select,
            result_ty,
            vec![condition, new_true.expect("set"), new_false.expect("set")],
            Default::default(),
        );
        for (kind, node) in self.function.instruction(select).metadata.clone() {
            if kind == "prof" || kind == "unpredictable" {
                self.function.annotate(new, &kind, node);
            }
        }
        Visit::Replace(Operand::Value(self.function.instruction(new).result.expect("a value")))
    }
}

#[cfg(test)]
#[path = "combine_select_tests.rs"]
mod tests;
