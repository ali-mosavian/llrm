//! `visitPHINode`'s argument folds: a phi of the same operation applied to
//! different values is the operation applied to a phi of those values.
//!
//! Translation of `foldPHIArgZextsIntoPHI` and `foldPHIArgOpIntoPHI`
//! ([InstCombinePHI.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombinePHI.cpp#L779))
//! at d1106deb71cc.
//!
//! # How it works
//! - Three or more incoming values that are `zext`s of one type (each with one
//!   user) and at least one constant that fits that type become a phi of the
//!   narrow type and one `zext` of it.
//! - Two or more incoming values that are all the same cast, or all the same
//!   binary operator or compare with one constant right operand, each with one
//!   user, become the operation applied to a phi of the left operands (or to
//!   the one left operand all of them share). The new operation goes at the top
//!   of the phi's block, after the phis.
//!
//! # Example
//! ```text
//! %a = zext i1 %x to i32 ; %b = zext i1 %y to i32
//! %p = phi i32 [ %a, %l ], [ %b, %r ]    ==>  %p.in = phi i1 [ %x, %l ], [ %y, %r ]
//!                                             %p = zext i1 %p.in to i32
//! ```
//!
//! MIRROR: the opcode and one-user tests, `isSameOperationAs` (opcode, type,
//! operand types, predicate), `shouldChangeType` for integer casts, the
//! constant right operand, the intersection of the binary operators' flags.
//!
//! DEVIATE 1: the rest of `visitPHINode` is not translated: the GEP, load,
//! `insertvalue`/`extractvalue` and binary-operator-of-two-variables forms of
//! `foldPHIArgOpIntoPHI`, `foldPHIArgIntToPtrToPHI`, the pointer-cast phi,
//! `foldDeadPhiWeb`, `foldIntegerTypedPHI`, `SliceUpIllegalIntegerPHI`,
//! and `foldDependentIVs`. Exit: later slices.
//!
//! DEVIATE 2: no check that the block's terminator is an EH pad, which MIR
//! cannot have a phi before; no debug locations.
//!
//! # Used by
//! `combine` (its `visit`, the `Phi` opcode).

use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{query, should_change_type};

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// Whether two opcodes are `Instruction::getOpcode` equal: a cast and a binary
/// operator by their operation, a compare by its kind alone.
fn same_opcode(
    a: &Opcode,
    b: &Opcode,
) -> bool {
    match (a, b) {
        (Opcode::Cast(x), Opcode::Cast(y)) => x == y,
        (Opcode::Binary(x), Opcode::Binary(y)) => x == y,
        (Opcode::ICmp(_), Opcode::ICmp(_)) => true,
        _ => false,
    }
}

impl Combiner<'_> {
    /// `visitPHINode`, for the folds translated so far.
    pub fn visit_phi(
        &mut self,
        phi: InstId,
    ) -> Visit {
        let incoming: Vec<(Operand, BlockId)> = pattern::phi_incoming(self.function.instruction(phi)).collect();
        if let Some(folded) = self.fold_phi_arg_zexts_into_phi(phi, &incoming) {
            return Visit::Replace(folded);
        }
        if incoming.len() >= 2
            && let (Some(first), Some(second)) = (self.definition_of(incoming[0].0), self.definition_of(incoming[1].0))
            && same_opcode(&self.function.instruction(first).opcode, &self.function.instruction(second).opcode)
            && let Operand::Value(value) = incoming[0].0
            && self.has_one_user(value)
            && let Some(folded) = self.fold_phi_arg_op_into_phi(phi, &incoming, first)
        {
            return Visit::Replace(folded);
        }
        if let Some(folded) = self.simplify_using_control_flow(phi, &incoming) {
            return Visit::Replace(folded);
        }
        Visit::None
    }

    /// `simplifyUsingControlFlow`: a phi of constants whose inputs the edges
    /// of its immediate dominator's branch or switch dominate is that
    /// condition, or its negation.
    fn simplify_using_control_flow(
        &mut self,
        phi: InstId,
        incoming: &[(Operand, BlockId)],
    ) -> Option<Operand> {
        let ty = self.function.instruction(phi).ty;
        let bits = self.context.types.int_bits(ty)?;
        let mut inputs = Vec::with_capacity(incoming.len());
        for &(value, from) in incoming {
            let (constant, _) = pattern::apint(&query(self, None), value)?;
            inputs.push((constant, from));
        }
        let block = self.function.parent(phi).expect("a placed phi");
        let tree = self.env.analyses.dominator_tree();
        if !tree.is_reachable_from_entry(block) {
            return None;
        }
        let idom = tree.immediate_dominator(block)?;
        let branch = self.function.terminator(idom)?;
        let instruction = self.function.instruction(branch);
        // Which successor each condition value takes, and how many edges
        // each successor has from the terminator.
        let mut succ_for: Vec<(u128, BlockId)> = Vec::new();
        let mut edges: Vec<(BlockId, u32)> = Vec::new();
        let mut count = |successor: BlockId| match edges.iter_mut().find(|(one, _)| *one == successor) {
            Some((_, n)) => *n += 1,
            None => edges.push((successor, 1)),
        };
        let cond = match (&instruction.opcode, &instruction.operands[..]) {
            (Opcode::Br, &[cond, Operand::Block(taken), Operand::Block(other)]) => {
                succ_for.extend([(1, taken), (0, other)]);
                count(taken);
                count(other);
                cond
            }
            (Opcode::Switch, [cond, Operand::Block(default), rest @ ..]) => {
                count(*default);
                for pair in rest.chunks(2) {
                    let &[case, Operand::Block(successor)] = pair else { return None };
                    let (value, _) = pattern::apint(&query(self, None), case)?;
                    succ_for.push((value, successor));
                    count(successor);
                }
                *cond
            }
            _ => return None,
        };
        if self.type_of(cond) != ty {
            return None;
        }
        let function = &*self.function;
        let mask = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
        // `BasicBlockEdge(IDom, successor)` dominates `BasicBlockEdge(pred,
        // block)`.
        let correct = |value: u128, from: BlockId| {
            let Some(&(_, successor)) = succ_for.iter().rev().find(|&&(one, _)| one == value) else { return false };
            edges.iter().any(|&(one, n)| one == successor && n == 1)
                && ((idom == from && successor == block) || tree.dominates_edge(function, idom, successor, from))
        };
        let mut invert = None;
        for &(value, from) in &inputs {
            let needs = if correct(value, from) {
                false
            } else if correct(!value & mask, from) {
                true
            } else {
                return None;
            };
            if *invert.get_or_insert(needs) != needs {
                return None;
            }
        }
        if invert? {
            let at = self.after_phis(phi);
            return Some(self.create_not(at, cond));
        }
        Some(cond)
    }

    /// The first instruction of `phi`'s block that is not a phi: where the
    /// replacement of a phi goes.
    fn after_phis(
        &self,
        phi: InstId,
    ) -> InstId {
        let block = self.function.parent(phi).expect("a placed phi");
        self.first_insertion_point(block)
    }

    /// `foldPHIArgZextsIntoPHI`.
    fn fold_phi_arg_zexts_into_phi(
        &mut self,
        phi: InstId,
        incoming: &[(Operand, BlockId)],
    ) -> Option<Operand> {
        if incoming.len() < 3 {
            return None;
        }
        let narrow = incoming
            .iter()
            .find_map(
                |&(value, _)| {
                    let def = self.definition_of(value)?;
                    let inst = self.function.instruction(def);
                    (inst.opcode == Opcode::Cast(CastOp::ZExt)).then(|| self.type_of(inst.operands[0]))
                },
            )?;
        let narrow_bits = self.context.types.int_bits(narrow)?;
        let (mut zexts, mut constants) = (0, 0);
        let mut narrowed = Vec::with_capacity(incoming.len());
        for &(value, _) in incoming {
            if let Some(def) = self.definition_of(value)
                && self.function.instruction(def).opcode == Opcode::Cast(CastOp::ZExt)
            {
                let source = self.function.instruction(def).operands[0];
                let Operand::Value(user_of) = value else { return None };
                if self.type_of(source) != narrow || !self.has_one_user(user_of) {
                    return None;
                }
                narrowed.push(source);
                zexts += 1;
            } else if matches!(value, Operand::Constant(_)) {
                let (bits, _) = pattern::apint(&query(self, None), value)?;
                if bits & !mask(narrow_bits) != 0 {
                    return None;
                }
                narrowed.push(self.int(narrow, bits));
                constants += 1;
            } else {
                return None;
            }
        }
        if constants == 0 || zexts < 2 {
            return None;
        }
        let operands: Vec<Operand> =
            narrowed.iter().zip(incoming).flat_map(|(&value, &(_, from))| [value, Operand::Block(from)]).collect();
        let new_phi = self.insert_before(phi, Opcode::Phi, narrow, operands, Flags::default());
        let narrow_value = Operand::Value(self.function.instruction(new_phi).result.expect("a value"));
        let ty = self.function.instruction(phi).ty;
        let at = self.after_phis(phi);
        let cast = self.insert_before(at, Opcode::Cast(CastOp::ZExt), ty, vec![narrow_value], Flags::default());
        Some(Operand::Value(self.function.instruction(cast).result.expect("a value")))
    }

    /// `foldPHIArgOpIntoPHI`, for casts and for binary operators and compares
    /// with a constant right operand.
    fn fold_phi_arg_op_into_phi(
        &mut self,
        phi: InstId,
        incoming: &[(Operand, BlockId)],
        first: InstId,
    ) -> Option<Operand> {
        let first_inst = self.function.instruction(first).clone();
        let phi_ty = self.function.instruction(phi).ty;
        let (cast_source, constant) = match first_inst.opcode {
            Opcode::Cast(_) => {
                let source = self.type_of(first_inst.operands[0]);
                if let (Some(to), Some(from)) =
                    (self.context.types.int_bits(phi_ty), self.context.types.int_bits(source))
                    && !should_change_type(self.env.layout, to, from)
                {
                    return None;
                }
                (Some(source), None)
            }
            Opcode::Binary(_) | Opcode::ICmp(_) => {
                let rhs = *first_inst.operands.get(1)?;
                if !matches!(rhs, Operand::Constant(_)) {
                    return None;
                }
                (None, Some(rhs))
            }
            _ => return None,
        };
        let mut flags = first_inst.flags;
        for &(value, _) in &incoming[1..] {
            let def = self.definition_of(value)?;
            let Operand::Value(user_of) = value else { return None };
            let inst = self.function.instruction(def);
            if !self.has_one_user(user_of) || inst.opcode != first_inst.opcode || inst.ty != first_inst.ty {
                return None;
            }
            match (cast_source, constant) {
                (Some(source), _) if self.type_of(inst.operands[0]) != source => return None,
                (None, Some(constant)) if inst.operands.get(1) != Some(&constant) => return None,
                _ => {}
            }
            flags = flags.intersect(inst.flags);
        }
        let left: Vec<Operand> = incoming
            .iter()
            .map(|&(value, _)| {
                let def = self.definition_of(value).expect("an instruction");
                self.function.instruction(def).operands[0]
            })
            .collect();
        let phi_value = if left.iter().all(|&one| one == left[0]) {
            left[0]
        } else {
            let operands: Vec<Operand> =
                left.iter().zip(incoming).flat_map(|(&value, &(_, from))| [value, Operand::Block(from)]).collect();
            let ty = self.type_of(left[0]);
            let new_phi = self.insert_before(phi, Opcode::Phi, ty, operands, Flags::default());
            Operand::Value(self.function.instruction(new_phi).result.expect("a value"))
        };
        let at = self.after_phis(phi);
        let operands = match constant {
            Some(constant) => vec![phi_value, constant],
            None => vec![phi_value],
        };
        let op_flags = if matches!(first_inst.opcode, Opcode::Binary(_)) { flags } else { Flags::default() };
        let new = self.insert_before(at, first_inst.opcode, phi_ty, operands, op_flags);
        Some(Operand::Value(self.function.instruction(new).result.expect("a value")))
    }
}

#[cfg(test)]
#[path = "combine_phi_args_tests.rs"]
mod tests;
