//! InstCombine's `freeze`: `InstCombinerImpl::visitFreeze`
//! ([InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5402)).
//!
//! # How it works
//! A freeze of a phi goes into the phi's incoming values; a freeze of an
//! instruction whose only possibly-poison operand is one value moves to that
//! operand (`pushFreezeToPreventPoisonFromPropagating`, L5210); a freeze of
//! `undef` or `poison` becomes the one constant its users prefer: all ones
//! for an `or`, true for a select condition with a constant arm, the common
//! constant of a phi, else zero. A freeze of a recurrence's phi moves to its
//! start value, dropping the flags that could make the steps poison
//! (`foldFreezeIntoRecurrence`, L5271); the uses a freeze dominates read the
//! freeze (`freezeOtherUses`, L5339).
//!
//! # Example
//! ```text
//! %f = freeze i32 %a              ==>   %x.fr = freeze i32 %x
//! ; %a = add i32 %x, 1                  %a = add i32 %x.fr, 1
//! ```
//!
//! MIRROR: the order of the folds and each guard.
//!
//! DEVIATE 1: no vectors, so `getFreezeVectorReplacement` and the shuffle
//! guard are not here.
//!
//! # Used by
//! `combine` (its dispatch on `Opcode::Freeze`).

use std::collections::BTreeSet;

use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, Operand, Use, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::valuetracking::guarantee::{
    Operator, can_create_undef_or_poison, is_guaranteed_not_to_be_undef_or_poison,
};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{query, simplify_visited};
use crate::local::dominates_instruction_use;

impl Combiner<'_> {
    /// `visitFreeze`.
    pub(crate) fn visit_freeze(
        &mut self,
        freeze: InstId,
    ) -> Visit {
        let operand = self.function.instruction(freeze).operands[0];
        if let Some(found) = simplify_visited(self, freeze) {
            return found;
        }
        // freeze (phi const, x) --> phi const, (freeze x)
        if let Some((phi, instruction)) = query(self, Some(freeze)).instruction_of(operand)
            && instruction.opcode == Opcode::Phi
        {
            let found = self.fold_op_into_phi(freeze, phi, false);
            if !matches!(found, Visit::None) {
                return found;
            }
            if let Some(recurrence) = self.fold_freeze_into_recurrence(freeze, phi) {
                return Visit::Replace(recurrence);
            }
        }
        if let Some(pushed) = self.push_freeze_to_prevent_poison_from_propagating(freeze) {
            return Visit::Replace(pushed);
        }
        if self.is_undef_or_poison(operand) {
            let replacement = self.undef_replacement(freeze);
            return Visit::Replace(replacement);
        }
        // Replace uses of Op with freeze(Op).
        if self.freeze_other_uses(freeze) {
            return Visit::Changed;
        }
        Visit::None
    }

    /// `foldFreezeIntoRecurrence` (L5271): `freeze (phi start, step...)` where
    /// every step reaches the phi again through instructions that cannot make
    /// poison without their flags is the phi itself, the flags dropped and the
    /// start frozen if it may be poison.
    fn fold_freeze_into_recurrence(
        &mut self,
        freeze: InstId,
        phi: InstId,
    ) -> Option<Operand> {
        let phi_block = self.function.parent(phi)?;
        let phi_value = self.value_of(phi);
        let incoming: Vec<(Operand, BlockId)> = pattern::phi_incoming(self.function.instruction(phi)).collect();
        let mut start: Option<(usize, Operand, BlockId)> = None;
        let mut worklist: Vec<Operand> = Vec::new();
        {
            let tree = self.env.analyses.dominator_tree();
            for (at, &(value, block)) in incoming.iter().enumerate() {
                if tree.dominates(phi_block, block) {
                    // Add backedge value to worklist.
                    worklist.push(value);
                    continue;
                }
                // Don't bother handling multiple start values.
                if start.is_some() {
                    return None;
                }
                start = Some((at, value, block));
            }
        }
        let (start_at, start_value, start_block) = start?;
        if worklist.is_empty() {
            return None; // Not a recurrence.
        }
        let q = query(self, None);
        let start_needs_freeze = !is_guaranteed_not_to_be_undef_or_poison(&q, start_value, 0);
        // We can't insert freeze if the start value is the result of the
        // terminator (e.g. an invoke).
        if start_needs_freeze
            && q.instruction_of(start_value).is_some_and(|(def, _)| self.function.terminator(start_block) == Some(def))
        {
            return None;
        }
        let mut visited: BTreeSet<Operand> = BTreeSet::new();
        let mut drop_flags: Vec<InstId> = Vec::new();
        while let Some(value) = worklist.pop() {
            if !visited.insert(value) {
                continue;
            }
            if visited.len() > 32 {
                return None; // Limit the total number of values we inspect.
            }
            // Assume that PN is non-poison, because it will be after the
            // transform.
            if value == phi_value || is_guaranteed_not_to_be_undef_or_poison(&q, value, 0) {
                continue;
            }
            let (def, instruction) = q.instruction_of(value)?;
            if can_create_undef_or_poison(&q, Operator::Instruction(def), false) {
                return None;
            }
            drop_flags.push(def);
            worklist
                .extend(instruction.operands.iter().copied().filter(|operand| !matches!(operand, Operand::Block(_))));
        }
        for def in drop_flags {
            self.function.drop_poison_generating_annotations(def);
        }
        if start_needs_freeze {
            let terminator = self.function.terminator(start_block).expect("a terminated block");
            let ty = self.type_of(start_value);
            let frozen = self.insert_before(terminator, Opcode::Freeze, ty, vec![start_value], Flags::default());
            let frozen = self.value_of(frozen);
            self.function.set_operand(phi, 2 * start_at, frozen);
        }
        Some(phi_value)
    }

    /// `freezeOtherUses` (L5339): the freeze moves right after the definition
    /// of its operand, and every use it dominates reads the freeze, so all of
    /// them see one value.
    fn freeze_other_uses(
        &mut self,
        freeze: InstId,
    ) -> bool {
        let operand = self.function.instruction(freeze).operands[0];
        let Operand::Value(value) = operand else { return false };
        if self.has_one_use(operand) {
            return false;
        }
        // Move the freeze directly after the definition of its operand, so
        // that it dominates the maximum number of uses.
        let position = match self.function.value(value).def {
            ValueDef::Argument(_) => {
                let entry = self.function.entry().expect("a body");
                let first = self
                    .function
                    .iter_block(entry)
                    .find(|&one| !matches!(
                        self.function.instruction(one).opcode,
                        Opcode::Phi | Opcode::Alloca { .. }
                    ))
                    .expect("a terminated block");
                Position::Before(first)
            }
            ValueDef::Instruction(def) => match self.function.instruction(def).opcode {
                // `getInsertionPointAfterDef`: after the phis for a phi, none
                // for a terminator.
                Opcode::Phi => Position::Before(self.first_insertion_point(self.function.parent(def).expect("placed"))),
                ref opcode if opcode.is_terminator() => return false,
                _ => Position::After(def),
            },
            _ => return false,
        };
        let mut changed = false;
        let already = match position {
            Position::Before(at) => at == freeze,
            Position::After(def) => self.function.next_in_block(def) == Some(freeze),
            Position::End(_) => false,
        };
        if !already {
            self.function.move_to(freeze, position).expect("a placed freeze");
            changed = true;
        }
        let frozen = self.value_of(freeze);
        let uses: Vec<Use> = self.function.users(value).iter().copied().collect();
        let mut users: Vec<InstId> = Vec::new();
        for at in uses {
            if at.user == freeze {
                continue;
            }
            let tree = self.env.analyses.dominator_tree();
            if !dominates_instruction_use(tree, self.function, freeze, at) {
                continue;
            }
            users.push(at.user);
            self.function.set_operand(at.user, at.index as usize, frozen);
            changed = true;
        }
        for user in users {
            // Re-queue U and its users: freezing U's operand can expose a fold
            // on a user of U (e.g. a freeze of U can now be pushed
            // through it).
            if let Some(result) = self.function.instruction(user).result {
                let readers: Vec<InstId> = self.function.users(result).iter().map(|used| used.user).collect();
                for reader in readers {
                    self.add_to_worklist(reader);
                }
            }
            self.add_to_worklist(user);
        }
        changed
    }

    fn is_undef_or_poison(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Constant(id) if matches!(self.context.get(id).kind, ConstantKind::Undef | ConstantKind::Poison)
        )
    }

    /// `pushFreezeToPreventPoisonFromPropagating` (L5210).
    fn push_freeze_to_prevent_poison_from_propagating(
        &mut self,
        freeze: InstId,
    ) -> Option<Operand> {
        let original = self.function.instruction(freeze).operands[0];
        // Only a freeze uses it, or the other users would lose their flags.
        let Operand::Value(value) = original else { return None };
        let def = match self.function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(def) => def,
            _ => return None,
        };
        if !self.has_one_use(original) || self.function.instruction(def).opcode == Opcode::Phi {
            return None;
        }
        let q = query(self, Some(freeze));
        // An instruction that can itself create poison cannot take the freeze
        // through; flags alone can be dropped, only the freeze uses it.
        if can_create_undef_or_poison(&q, Operator::Instruction(def), false) {
            return None;
        }
        let operands = self.function.instruction(def).operands.to_vec();
        let mut maybe_poison: Option<Operand> = None;
        for v in operands {
            if matches!(v, Operand::Block(_))
                || is_guaranteed_not_to_be_undef_or_poison(&q, v, 0)
                || maybe_poison == Some(v)
            {
                continue;
            }
            if maybe_poison.is_some() {
                return None;
            }
            maybe_poison = Some(v);
        }
        self.function.drop_poison_generating_annotations(def);
        let Some(operand) = maybe_poison else { return Some(original) };
        let ty = self.type_of(operand);
        let frozen = self.insert_before(def, Opcode::Freeze, ty, vec![operand], Flags::default());
        let frozen = self.value_of(frozen);
        let at = self.function.instruction(def).operands.iter().position(|&v| v == operand).expect("an operand");
        let count = self.function.instruction(def).operands.len();
        for index in at..count {
            if self.function.instruction(def).operands[index] == operand {
                self.function.set_operand(def, index, frozen);
            }
        }
        Some(original)
    }

    /// `getUndefReplacement`: the one constant every user of the freeze of
    /// undef can take.
    fn undef_replacement(
        &mut self,
        freeze: InstId,
    ) -> Operand {
        let ty = self.function.instruction(freeze).ty;
        let result = self.function.instruction(freeze).result.expect("a value");
        let null = self.int(ty, 0);
        let users: Vec<InstId> = self.function.users(result).iter().map(|used| used.user).collect();
        let mut best: Option<Operand> = None;
        for user in users {
            let instruction = self.function.instruction(user);
            let mut chosen = null;
            if matches!(instruction.opcode, Opcode::Binary(BinaryOp::Or)) {
                chosen = self.all_ones(ty);
            } else if instruction.opcode == Opcode::Select
                && instruction.operands[0] == Operand::Value(result)
                && matches!(instruction.operands[1], Operand::Constant(_))
            {
                chosen = self.int(ty, 1);
            } else if instruction.opcode == Opcode::Select && instruction.operands[0] == Operand::Value(result) {
                // select (freeze undef), a, b: a guaranteed-defined arm
                // decides.
                let q = query(self, Some(freeze));
                let (a, b) = (instruction.operands[1], instruction.operands[2]);
                chosen = null;
                for arm in [a, b] {
                    if arm != Operand::Value(result) && is_guaranteed_not_to_be_undef_or_poison(&q, arm, 0) {
                        chosen = arm;
                        break;
                    }
                }
            } else if instruction.opcode == Opcode::Phi
                && let Some(common) = self.common_constant_of_phi(user)
            {
                chosen = common;
            }
            match best {
                None => best = Some(chosen),
                Some(previous) if previous != chosen => best = Some(null),
                _ => {}
            }
        }
        best.unwrap_or(null)
    }

    /// `pickCommonConstantFromPHI`: `phi(freeze(undef), C, C)` is `C`.
    fn common_constant_of_phi(
        &self,
        phi: InstId,
    ) -> Option<Operand> {
        let q = query(self, Some(phi));
        let mut best: Option<Operand> = None;
        for (value, _) in pattern::phi_incoming(self.function.instruction(phi)) {
            if let Some((_, frozen)) = q.instruction_of(value)
                && frozen.opcode == Opcode::Freeze
                && self.is_undef_or_poison(frozen.operands[0])
            {
                continue;
            }
            let Operand::Constant(_) = value else { return None };
            if !is_guaranteed_not_to_be_undef_or_poison(&q, value, 0) {
                return None;
            }
            if best.is_some_and(|previous| previous != value) {
                return None;
            }
            best = Some(value);
        }
        best
    }
}

#[cfg(test)]
#[path = "combine_freeze_tests.rs"]
mod tests;
