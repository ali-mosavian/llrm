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
//! `SimplifySelectsFeedingBinaryOp` (:1356) and `foldBinOpSelectBinOp`
//! (:1907): a binary operator of selects becomes a select of the operator on
//! each arm, and `(select c, x, x op k1) op k2` a select of constants on `x
//! op`.
//!
//! MIRROR: the three shapes (two selects on one condition, either operand a
//! select with one use), the arm of an inner select an outer condition
//! implies, `(c ? y : -n) + z` as `c ? y : z - n`; the new select takes the
//! operation's name and the old select's `prof` and `unpredictable`.
//!
//! DEVIATE 2: the new select and operators carry the operation's fast-math
//! flags (`IRBuilder::FastMathFlagGuard`) only on a float type; vectors do not
//! exist in MIR.
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
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::simplify::{simplify_binop_with_flags_at, simplify_instruction_operands_at};
use llrm_mir::valuetracking::context::is_implied_condition;
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison;
use llrm_mir::valuetracking::{Query, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_phi::{UB_IMPLYING, is_associative, is_imm_constant};
use crate::combine_shared::query;

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

    /// `foldBinOpIntoSelectOrPhi`
    /// ([:2329](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L2329)):
    /// the binary operator folded into a select or a phi it reads, operand 0
    /// first.
    pub fn fold_bin_op_into_select_or_phi(
        &mut self,
        inst: InstId,
    ) -> Visit {
        let operands = self.function.instruction(inst).operands.to_vec();
        let is_constant = |op: Operand| matches!(op, Operand::Constant(_));
        for index in 0..2 {
            let Some(def) = self.definition(operands[index]) else { continue };
            let other_is_constant = is_constant(operands[1 - index]);
            match self.function.instruction(def).opcode {
                Opcode::Select => return self.fold_op_into_select(inst, def, false, !other_is_constant),
                Opcode::Phi => return self.fold_op_into_phi(inst, def, false),
                _ => {}
            }
        }
        Visit::None
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

    /// `m_Select(m_Value(A), m_Value(B), m_Value(C))`.
    pub(crate) fn select_of(
        &self,
        v: Operand,
    ) -> Option<(InstId, [Operand; 3])> {
        let def = self.definition(v)?;
        let instruction = self.function.instruction(def);
        let [a, b, c] = instruction.operands[..] else { return None };
        (instruction.opcode == Opcode::Select).then_some((def, [a, b, c]))
    }

    /// The new select the folds here make: before `at`, with `select`'s
    /// `prof` and `unpredictable` (`CreateSelect(.., MDFrom = SI)`).
    pub(crate) fn select_like(
        &mut self,
        at: InstId,
        select: InstId,
        operands: [Operand; 3],
        flags: Flags,
    ) -> Operand {
        let ty = self.type_of(operands[1]);
        let new = self.insert_before(at, Opcode::Select, ty, operands.to_vec(), flags);
        for (kind, node) in self.function.instruction(select).metadata.clone() {
            if kind == "prof" || kind == "unpredictable" {
                self.function.annotate(new, &kind, node);
            }
        }
        Operand::Value(self.function.instruction(new).result.expect("a value"))
    }

    /// `SimplifySelectsFeedingBinaryOp(I, LHS, RHS)`: the value `inst`, a
    /// binary operator, equals as a select of the operator on each arm.
    pub fn simplify_selects_feeding_binary_op(
        &mut self,
        inst: InstId,
        lhs: Operand,
        rhs: Operand,
    ) -> Option<Operand> {
        let (lhs_select, rhs_select) = (self.select_of(lhs), self.select_of(rhs));
        let (select, ..) = lhs_select.or(rhs_select)?;
        let Opcode::Binary(op) = self.function.instruction(inst).opcode else { return None };
        let ty = self.function.instruction(inst).ty;
        let fmf = if matches!(self.context.types.get(ty), llrm_mir::types::Type::Float(_)) {
            self.function.instruction(inst).flags.intersect(Flags::FAST)
        } else {
            Flags::default()
        };
        let one_use = |c: &Self, v: Operand| matches!(v, Operand::Value(v) if c.function.users(v).has_one_use());

        // `simplifyBinOp(Opcode, a, b, FMF, SQ.getWithInstruction(&I))`.
        let simplify = |c: &Self, a: Operand, b: Operand| {
            let env = c.env;
            simplify_binop_with_flags_at(
                c.context,
                env.globals,
                env.layout,
                Some(&env.analyses),
                c.function,
                inst,
                op,
                fmf,
                a,
                b,
            )
        };
        // A select whose condition `cond` implies resolves to the arm for it.
        let with_implied_cond = |c: &Self, v: Operand, cond: Operand, cond_is_true: bool| {
            let Some((_, [inner, if_true, if_false])) = c.select_of(v) else { return v };
            if c.type_of(cond) != c.type_of(inner) {
                return v;
            }
            let q = query(c, Some(inst));
            match is_implied_condition(&q, cond, inner, cond_is_true, 0) {
                Some(true) => if_true,
                Some(false) => if_false,
                None => v,
            }
        };

        let (cond, mut if_true, mut if_false);
        match (lhs_select, rhs_select) {
            // (A ? B : C) op (A ? E : F) -> A ? (B op E) : (C op F)
            (Some((_, [a, b, c])), Some((_, [d, e, f]))) if a == d => {
                cond = a;
                if_true = simplify(self, b, e);
                if_false = simplify(self, c, f);
                if one_use(self, lhs) && one_use(self, rhs) {
                    match (if_true, if_false) {
                        (None, Some(_)) => if_true = Some(self.create_binop(inst, op, b, e, fmf)),
                        (Some(_), None) => if_false = Some(self.create_binop(inst, op, c, f, fmf)),
                        _ => {}
                    }
                }
            }
            // (A ? B : C) op Y -> A ? (B op Y) : (C op Y)
            (Some((_, [a, b, c])), _) if one_use(self, lhs) => {
                cond = a;
                let true_rhs = with_implied_cond(self, rhs, cond, true);
                let false_rhs = with_implied_cond(self, rhs, cond, false);
                if_true = simplify(self, b, true_rhs);
                if_false = simplify(self, c, false_rhs);
                if let Some(folded) = self.fold_add_negate(inst, select, cond, (b, c), rhs, (if_true, if_false), op) {
                    return Some(folded);
                }
            }
            // X op (D ? E : F) -> D ? (X op E) : (X op F)
            (_, Some((_, [d, e, f]))) if one_use(self, rhs) => {
                cond = d;
                let true_lhs = with_implied_cond(self, lhs, cond, true);
                let false_lhs = with_implied_cond(self, lhs, cond, false);
                if_true = simplify(self, true_lhs, e);
                if_false = simplify(self, false_lhs, f);
                if let Some(folded) = self.fold_add_negate(inst, select, cond, (e, f), lhs, (if_true, if_false), op) {
                    return Some(folded);
                }
            }
            _ => return None,
        }
        let (if_true, if_false) = (if_true?, if_false?);
        let new = self.select_like(inst, select, [cond, if_true, if_false], fmf);
        if let (Operand::Value(from), Operand::Value(to)) = (self.value_operand(inst), new) {
            self.function.take_name(from, to);
        }
        Some(new)
    }

    /// The result of `inst` as an operand.
    fn value_operand(
        &self,
        inst: InstId,
    ) -> Operand {
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    /// `foldAddNegate`: `(cond ? y : -n) + z` is `cond ? y : z - n` where the
    /// other arm simplified (and the mirror), on exactly one arm.
    fn fold_add_negate(
        &mut self,
        inst: InstId,
        select: InstId,
        cond: Operand,
        (t_val, f_val): (Operand, Operand),
        z: Operand,
        (simplified_true, simplified_false): (Option<Operand>, Option<Operand>),
        op: BinaryOp,
    ) -> Option<Operand> {
        if op != BinaryOp::Add {
            return None;
        }
        // The arm that simplified keeps its value; the other, a negation `-n`,
        // becomes `z - n`.
        let (done, other, flipped) = match (simplified_true, simplified_false) {
            (Some(done), None) => (done, f_val, false),
            (None, Some(done)) => (done, t_val, true),
            _ => return None,
        };
        let (_, n, _) = pattern::neg(&query(self, None), other)?;
        let sub = self.create_sub(inst, z, n);
        let operands = if flipped { [cond, sub, done] } else { [cond, done, sub] };
        let new = self.select_like(inst, select, operands, Flags::default());
        if let (Operand::Value(from), Operand::Value(to)) = (self.value_operand(inst), new) {
            self.function.take_name(from, to);
        }
        Some(new)
    }

    /// `foldBinOpSelectBinOp(Op)`: `(c ? x : x op k1) op k2` is `(c ? k2 :
    /// k1 op k2) op x` for an associative `op` and constants `k1`, `k2`.
    pub fn fold_bin_op_select_bin_op(
        &mut self,
        inst: InstId,
    ) -> Visit {
        let instruction = self.function.instruction(inst);
        let Opcode::Binary(op) = instruction.opcode else { return Visit::None };
        let [first, constant] = instruction.operands[..] else { return Visit::None };
        if !is_associative(op, instruction.flags) {
            return Visit::None;
        }
        let one_use = |c: &Self, v: Operand| matches!(v, Operand::Value(v) if c.function.users(v).has_one_use());
        let Some((select, [cond, if_true, if_false])) = self.select_of(first) else { return Visit::None };
        if !is_imm_constant(self, constant) || !one_use(self, self.value_operand(inst)) || !one_use(self, first) {
            return Visit::None;
        }
        // `m_BinOp(Opcode, m_Specific(other), m_ImmConstant(Const2))`
        let with_constant = |c: &Self, arm: Operand, other: Operand| {
            let def = c.definition(arm).filter(|_| one_use(c, arm))?;
            let arm = c.function.instruction(def);
            let [x, k2] = arm.operands[..] else { return None };
            (arm.opcode == Opcode::Binary(op) && x == other && is_imm_constant(c, k2)).then_some(k2)
        };
        let (new_true, new_false, input) = if let Some(k2) = with_constant(self, if_true, if_false) {
            (self.fold_constant_binop(op, constant, k2), Some(constant), if_false)
        } else if let Some(k2) = with_constant(self, if_false, if_true) {
            (Some(constant), self.fold_constant_binop(op, constant, k2), if_true)
        } else {
            return Visit::None;
        };
        let (Some(new_true), Some(new_false)) = (new_true, new_false) else { return Visit::None };
        let new_select = self.select_like(inst, select, [cond, new_true, new_false], Flags::default());
        self.replace_with_binop(inst, op, new_select, input, Flags::default())
    }
}

#[cfg(test)]
#[path = "combine_select_tests.rs"]
mod tests;
