//! `foldOpIntoPhi`: an instruction that reads a phi becomes a phi of the
//! instruction applied to each incoming value.
//!
//! Translation of
//! [`InstCombinerImpl::foldOpIntoPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1943)
//! and `simplifyInstructionWithPHI` (:1859) at d1106deb71cc.
//!
//! MIRROR: the phi must have one user, or only users identical to the
//! instruction; every other operand must be phi-translatable; each incoming
//! value either simplifies with the operation applied, or (for at most one, on
//! an unconditional, reachable, non-back edge) gets a copy of the operation at
//! the end of its block; the implied-condition constant of an `icmp` on a
//! branch's edge.
//!
//! DEVIATE 1: the `icmp` of a `ucmp`/`scmp` intrinsic with a constant (the
//! first `WillFold` case) is not made: MIR has no `ucmp`/`scmp`.
//!
//! DEVIATE 2: the implied-condition test takes the compare's predicate without
//! `samesign` (MIR compares do not carry it), so it can only find less.
//!
//! DEVIATE 3: no debug-use rewrite (`replaceAllDbgUsesWith`): `replace_value`
//! moves debug records of the phi with its uses.
//!
//! # Used by
//! The `visit*` of the families that fold an operation into a phi
//! (`foldBinOpIntoSelectOrPhi`, `visitICmpInst`, `visitCastInst`...).
//!
//! # Example
//! ```text
//! %p = phi i16 [ 1, %a ], [ 2, %b ]          %q = phi i16 [ 6, %a ], [ 7, %b ]
//! %r = add i16 %p, 5                   ==>   (uses of %r read %q)
//! ```

use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, IntPredicate, Opcode};
use llrm_mir::simplify::simplify_instruction_operands_at;
use llrm_mir::valuetracking::context::is_implied_condition_cmp;
use llrm_mir::valuetracking::pattern::{self, CmpPredicate, Side};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

/// Metadata that says something about the value computed where the original
/// is, not where a copy is put (`dropUBImplyingAttrsAndMetadata`).
const UB_IMPLYING: [&str; 6] = ["range", "nonnull", "align", "dereferenceable", "dereferenceable_or_null", "noundef"];

impl Combiner<'_> {
    /// `Value::DoPHITranslation(block, pred)`: the incoming value from `pred`
    /// if `op` is a phi of `block`, else `op`.
    fn do_phi_translation(
        &self,
        op: Operand,
        block: BlockId,
        pred: BlockId,
    ) -> Operand {
        if let Operand::Value(value) = op
            && let ValueDef::Instruction(def) = self.function.value(value).def
            && self.function.instruction(def).opcode == Opcode::Phi
            && self.function.parent(def) == Some(block)
            && let Some((incoming, _)) =
                pattern::phi_incoming(self.function.instruction(def)).find(|&(_, from)| from == pred)
        {
            incoming
        } else {
            op
        }
    }

    /// `Instruction::isIdenticalTo` of two instructions: the same opcode, type
    /// and operands.
    fn is_identical_to(
        &self,
        this: InstId,
        other: InstId,
    ) -> bool {
        let (this, other) = (self.function.instruction(this), self.function.instruction(other));
        this.opcode == other.opcode && this.ty == other.ty && this.operands == other.operands
    }

    /// `simplifyInstructionWithPHI`: what `inst` is with `phi` replaced by its
    /// incoming `value` from `from`, if that is a value already.
    fn simplify_instruction_with_phi(
        &mut self,
        inst: InstId,
        phi: InstId,
        value: Operand,
        from: BlockId,
    ) -> Option<Operand> {
        let phi_block = self.function.parent(phi).expect("a placed phi");
        let phi_value = self.function.instruction(phi).result.expect("a phi's value");
        let operands: Vec<Operand> = self
            .function
            .instruction(inst)
            .operands
            .to_vec()
            .into_iter()
            .map(
                |op| if op == Operand::Value(phi_value) { value } else { self.do_phi_translation(op, phi_block, from) },
            )
            .collect();
        let terminator = self.function.terminator(from)?;
        let env = self.env;
        let found = simplify_instruction_operands_at(
            self.context,
            env.globals,
            env.layout,
            Some(&env.analyses),
            self.function,
            terminator,
            inst,
            &operands,
        );
        // A constant expression is an instruction in hiding; the phi itself
        // cannot be removed.
        if let Some(found) = found
            && found != Operand::Value(phi_value)
            && !matches!(
                found,
                Operand::Constant(id) if matches!(self.context.get(id).kind, ConstantKind::Expr(_))
            )
        {
            return Some(found);
        }
        // The value the edge's branch condition implies for the compare.
        let Opcode::ICmp(pred) = self.function.instruction(inst).opcode else { return None };
        let branch = self.function.instruction(terminator);
        let [cond, Operand::Block(taken), Operand::Block(other)] = branch.operands[..] else { return None };
        if branch.opcode != Opcode::Br || taken == other {
            return None;
        }
        let q = query(self, Some(terminator));
        let implied = is_implied_condition_cmp(
            &q,
            cond,
            CmpPredicate::new(pred),
            Side::of(&q, operands[0]),
            Side::of(&q, operands[1]),
            taken == phi_block,
            0,
        )?;
        let ty = self.function.instruction(inst).ty;
        Some(Operand::Constant(self.context.int(ty, implied as i128)))
    }

    /// `foldOpIntoPhi(I, PN, AllowMultipleUses)`.
    pub fn fold_op_into_phi(
        &mut self,
        inst: InstId,
        phi: InstId,
        allow_multiple_uses: bool,
    ) -> Visit {
        let incoming: Vec<(Operand, BlockId)> = pattern::phi_incoming(self.function.instruction(phi)).collect();
        if incoming.is_empty() {
            return Visit::None;
        }
        let phi_block = self.function.parent(phi).expect("a placed phi");
        let phi_value = self.function.instruction(phi).result.expect("a phi's value");

        // Only a phi with one user, or whose users are all the same operation.
        let one_use = self.function.users(phi_value).has_one_use();
        let mut identical_users = false;
        if !allow_multiple_uses && !one_use {
            if self
                .function
                .users(phi_value)
                .iter()
                .any(|one| one.user != inst && !self.is_identical_to(inst, one.user))
            {
                return Visit::None;
            }
            identical_users = true;
        }

        // Every operand is phi-translatable.
        let tree = self.env.analyses.dominator_tree();
        for &op in self.function.instruction(inst).operands.iter() {
            if op == Operand::Value(phi_value) {
                continue;
            }
            let Operand::Value(value) = op else { continue };
            let ValueDef::Instruction(def) = self.function.value(value).def else { continue };
            let Some(def_block) = self.function.parent(def) else { continue };
            if self.function.instruction(def).opcode == Opcode::Phi && def_block == phi_block {
                continue;
            }
            if def_block != phi_block && tree.dominates(def_block, phi_block) {
                continue;
            }
            return Visit::None;
        }

        // Which incoming values fold, and which get a copy of the operation.
        let mut new_values: Vec<Option<Operand>> = Vec::new();
        let mut move_to_incoming: Vec<usize> = Vec::new();
        let mut seen_unsimplified = false;
        for (index, &(value, from)) in incoming.iter().enumerate() {
            if let Some(found) = self.simplify_instruction_with_phi(inst, phi, value, from) {
                new_values.push(Some(found));
                continue;
            }
            if self.will_fold(inst, phi_value, value) {
                move_to_incoming.push(index);
                new_values.push(None);
                continue;
            }
            if !one_use && !identical_users {
                return Visit::None;
            }
            if seen_unsimplified {
                return Visit::None;
            }
            seen_unsimplified = true;
            // The copy goes where it runs on this edge alone: the block must
            // go straight to the phi's, and be reachable.
            let Some(terminator) = self.function.terminator(from) else { return Visit::None };
            let branch = self.function.instruction(terminator);
            if branch.opcode != Opcode::Br
                || branch.operands.len() != 1
                || !self.env.analyses.dominator_tree().is_reachable(from)
            {
                return Visit::None;
            }
            new_values.push(None);
            move_to_incoming.push(index);
            // Not across a back edge: the combine would not end, and it is
            // not worth it.
            if self.is_back_edge(from, phi_block) {
                return Visit::None;
            }
        }

        // The copies, one per incoming block.
        let mut clones: Vec<(BlockId, InstId)> = Vec::new();
        for index in move_to_incoming {
            let (value, from) = incoming[index];
            let clone = match clones.iter().find(|(block, _)| *block == from) {
                Some(&(_, clone)) => clone,
                None => {
                    let clone = self.function.clone_instruction(inst);
                    for (slot, op) in self.function.instruction(inst).operands.to_vec().into_iter().enumerate() {
                        let with = if op == Operand::Value(phi_value) {
                            value
                        } else {
                            self.do_phi_translation(op, phi_block, from)
                        };
                        self.function.set_operand(clone, slot, with);
                    }
                    let terminator = self.function.terminator(from).expect("a terminated block");
                    self.function.insert(clone, Position::Before(terminator)).expect("a placed instruction");
                    self.worklist.add(clone);
                    for kind in UB_IMPLYING {
                        self.function.unannotate(clone, kind);
                    }
                    clones.push((from, clone));
                    clone
                }
            };
            new_values[index] = Some(Operand::Value(self.function.instruction(clone).result.expect("a value")));
        }

        // The new phi, where the old is.
        let ty = self.function.instruction(inst).ty;
        let mut operands = Vec::with_capacity(incoming.len() * 2);
        for (&(_, from), value) in incoming.iter().zip(&new_values) {
            operands.push(value.expect("every incoming value has its new value"));
            operands.push(Operand::Block(from));
        }
        let new_phi = self.insert_before(phi, Opcode::Phi, ty, operands, Default::default());
        let new_value = Operand::Value(self.function.instruction(new_phi).result.expect("a phi's value"));

        if identical_users {
            let mut users: Vec<InstId> = Vec::new();
            for one in self.function.users(phi_value).iter() {
                if one.user != inst && !users.contains(&one.user) {
                    users.push(one.user);
                }
            }
            for user in users {
                self.replace_inst_with(user, new_value);
                self.erase(user);
            }
        }
        Visit::Replace(new_value)
    }

    /// The `WillFold` lambda: a case that does not simplify yet, but where the
    /// copy and the incoming value fold into one instruction.
    fn will_fold(
        &self,
        inst: InstId,
        phi_value: llrm_mir::module::ValueId,
        value: Operand,
    ) -> bool {
        let Operand::Value(in_value) = value else { return false };
        let ValueDef::Instruction(def) = self.function.value(in_value).def else { return false };
        if !self.function.users(in_value).has_one_use() {
            return false;
        }
        // `icmp eq zext(i1 b), 0` is `!b`.
        let source = self.function.instruction(def);
        let Opcode::Cast(CastOp::ZExt) = source.opcode else { return false };
        let narrow = source.operands.first().and_then(|&op| self.function.operand_type(self.context, op));
        let is_bool = narrow.is_some_and(|ty| self.context.types.int_bits(ty) == Some(1));
        let compare = self.function.instruction(inst);
        let zero = Operand::Constant(self.context.int(self.function.instruction(def).ty, 0));
        is_bool
            && compare.opcode == Opcode::ICmp(IntPredicate::Eq)
            && compare.operands == [Operand::Value(phi_value), zero]
    }
}

#[cfg(test)]
#[path = "combine_phi_tests.rs"]
mod tests;
