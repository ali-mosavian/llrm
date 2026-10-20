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
//! `foldBinopWithPhiOperands` (:2215) and `foldBinopWithRecurrence` (:2100):
//! a binary operator of two phis of one block.
//!
//! MIRROR: two simple recurrences of one operation, with constant starts and
//! steps, become one recurrence (`reduced.phi`); an identity constant on one
//! side of every incoming pair leaves the other side; a pair of constants for
//! one predecessor folds there and the operation moves to the other
//! predecessor, which must jump straight here.
//!
//! DEVIATE 4: `IRBuilder` here simplifies what it makes (`create_binop`), not
//! just constant-folds, as everywhere in `combine_build`.
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

use llrm_mir::apfloat::{ApFloat, IEEE_DOUBLE, IEEE_SINGLE};
use llrm_mir::constantfolding;
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::simplify::{simplify_binop_operands, simplify_instruction_operands_at};
use llrm_mir::types::{FloatKind, Type, TypeId};
use llrm_mir::valuetracking::context::is_implied_condition_cmp;
use llrm_mir::valuetracking::guarantee::is_guaranteed_to_transfer_execution_to_successor_range;
use llrm_mir::valuetracking::pattern::{self, CmpPredicate, Side};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

/// Metadata that says something about the value computed where the original
/// is, not where a copy is put (`dropUBImplyingAttrsAndMetadata`).
pub(crate) const UB_IMPLYING: [&str; 6] =
    ["range", "nonnull", "align", "dereferenceable", "dereferenceable_or_null", "noundef"];

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
            Some(terminator),
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

/// `Instruction::isAssociative`: the integer operations, and `fadd`/`fmul`
/// with `reassoc` and `nsz`.
pub(crate) fn is_associative(
    op: BinaryOp,
    flags: Flags,
) -> bool {
    match op {
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor => true,
        BinaryOp::FAdd | BinaryOp::FMul => flags.contains(Flags::REASSOC) && flags.contains(Flags::NSZ),
        _ => false,
    }
}

/// An `m_ImmConstant`: a constant that is not, and holds no, expression.
pub(crate) fn is_imm_constant(
    c: &Combiner,
    v: Operand,
) -> bool {
    matches!(
        v,
        Operand::Constant(id) if !matches!(c.context.get(id).kind, ConstantKind::Expr(_))
    )
}

impl Combiner<'_> {
    /// `ConstantExpr::getBinOpIdentity(op, ty, allow_rhs_constant)` (`NSZ`
    /// false): the identity of a commutative operation, and with
    /// `allow_rhs_constant` the right-hand one of the others.
    pub(crate) fn bin_op_identity(
        &self,
        op: BinaryOp,
        ty: TypeId,
        allow_rhs_constant: bool,
    ) -> Option<Operand> {
        Some(match op {
            BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor => self.int(ty, 0),
            BinaryOp::Mul => self.int(ty, 1),
            BinaryOp::And => self.all_ones(ty),
            BinaryOp::Sub | BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr if allow_rhs_constant => self.int(ty, 0),
            BinaryOp::UDiv | BinaryOp::SDiv if allow_rhs_constant => self.int(ty, 1),
            BinaryOp::FAdd | BinaryOp::FMul => {
                let Type::Float(kind) = *self.context.types.get(ty) else { return None };
                let sem = if kind == FloatKind::Float { IEEE_SINGLE } else { IEEE_DOUBLE };
                let value =
                    if op == BinaryOp::FAdd { ApFloat::get_zero(sem, true) } else { ApFloat::get_one(sem, false) };
                Operand::Constant(
                    self.context.constant(Constant { ty, kind: ConstantKind::Float(value.to_bits() as u64) }),
                )
            }
            _ => return None,
        })
    }

    /// `A == B` for two constants, a zero spelled `Null`/`Zero` or as a
    /// number alike.
    pub(crate) fn same_constant(
        &self,
        a: Operand,
        b: Operand,
    ) -> bool {
        let (Operand::Constant(x), Operand::Constant(y)) = (a, b) else { return a == b };
        let normal = |id| {
            let constant = self.context.get(id);
            match constant.kind {
                ConstantKind::Null | ConstantKind::Zero => match self.context.types.get(constant.ty) {
                    Type::Float(_) => ConstantKind::Float(0),
                    _ => ConstantKind::Int(0),
                },
                ref kind => kind.clone(),
            }
        };
        x == y || (self.context.get(x).ty == self.context.get(y).ty && normal(x) == normal(y))
    }

    /// `ConstantFoldBinaryOpOperands(op, a, b)`, the floating operations by
    /// what `simplifyBinOp` finds of two constants.
    pub(crate) fn fold_constant_binop(
        &self,
        op: BinaryOp,
        a: Operand,
        b: Operand,
    ) -> Option<Operand> {
        let (Operand::Constant(x), Operand::Constant(y)) = (a, b) else { return None };
        if matches!(
            op,
            BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem
        ) {
            let found =
                simplify_binop_operands(self.context, self.env.layout, self.function, op, Flags::default(), a, b)?;
            return matches!(found, Operand::Constant(_)).then_some(found);
        }
        let env = constantfolding::Env { globals: self.env.globals, layout: self.env.layout };
        constantfolding::constant_fold_binary_op_operands(self.context, env, op, x, y).map(Operand::Constant)
    }

    fn defining_inst(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = v else { return None };
        match self.function.value(value).def {
            ValueDef::Instruction(def) if self.function.parent(def).is_some() => Some(def),
            _ => None,
        }
    }

    /// `foldBinopWithRecurrence`.
    fn fold_binop_with_recurrence(
        &mut self,
        bo: InstId,
    ) -> Visit {
        let instruction = self.function.instruction(bo);
        let Opcode::Binary(op) = instruction.opcode else { return Visit::None };
        let flags = instruction.flags;
        if !is_associative(op, flags) {
            return Visit::None;
        }
        let [lhs, rhs] = instruction.operands[..] else { return Visit::None };
        let (Some(bo0), Some(bo1)) = (self.defining_inst(lhs), self.defining_inst(rhs)) else { return Visit::None };
        let (i0, i1) = (self.function.instruction(bo0), self.function.instruction(bo1));
        let exactly_two = |c: &Self, inst: InstId| {
            let result = c.function.instruction(inst).result.expect("a value");
            c.function.users(result).iter().count() == 2
        };
        if i0.opcode != Opcode::Binary(op)
            || i1.opcode != Opcode::Binary(op)
            || !is_associative(op, i0.flags)
            || !is_associative(op, i1.flags)
            || !exactly_two(self, bo0)
            || !exactly_two(self, bo1)
            || self.function.parent(bo0) != self.function.parent(bo1)
        {
            return Visit::None;
        }
        let (flags0, flags1) = (i0.flags, i1.flags);

        // The matching phis, forming the recurrences.
        let q = query(self, None);
        let Some((pn0, start0, step0)) = pattern::match_simple_recurrence_of(&q, bo0) else { return Visit::None };
        let Some((pn1, start1, step1)) = pattern::match_simple_recurrence_of(&q, bo1) else { return Visit::None };
        let one_use = |c: &Self, phi: InstId| {
            c.function.users(c.function.instruction(phi).result.expect("a value")).has_one_use()
        };
        if !one_use(self, pn0) || !one_use(self, pn1) || self.function.parent(pn0) != self.function.parent(pn1) {
            return Visit::None;
        }
        if ![start0, start1, step0, step1].iter().all(|&v| matches!(v, Operand::Constant(_))) {
            return Visit::None;
        }

        // The recurrence constants.
        let Some(init) = self.fold_constant_binop(op, start0, start1) else { return Visit::None };
        let Some(step) = self.fold_constant_binop(op, step0, step1) else { return Visit::None };

        // The reduced phi, and the operation on it.
        let ty = self.function.instruction(pn0).ty;
        let incoming: Vec<(Operand, BlockId)> = pattern::phi_incoming(self.function.instruction(pn0)).collect();
        let mut operands = Vec::new();
        for &(value, from) in &incoming {
            operands.push(if value == start0 { init } else { value });
            operands.push(Operand::Block(from));
        }
        let new_phi = self.insert_before(pn0, Opcode::Phi, ty, operands, Flags::default());
        let new_phi_value = Operand::Value(self.function.instruction(new_phi).result.expect("a value"));
        let new_flags = if matches!(op, BinaryOp::FAdd | BinaryOp::FMul) {
            // The fast-math flags all three have.
            let fast = Flags::FAST;
            let both = flags0.intersect(flags1).intersect(flags);
            both.intersect(fast)
        } else {
            // `OverflowTracking`, with nothing known non-negative or non-zero.
            let (nuw, nsw, disjoint) = [flags0, flags1, flags].iter().fold((true, true, true), |(u, s, d), f| {
                (u && f.contains(Flags::NUW), s && f.contains(Flags::NSW), d && f.contains(Flags::DISJOINT))
            });
            let mut out = Flags::default();
            if op == BinaryOp::Add {
                if nuw {
                    out.insert(Flags::NUW);
                }
                if nsw && nuw {
                    out.insert(Flags::NSW);
                }
            }
            if op == BinaryOp::Or && disjoint {
                out.insert(Flags::DISJOINT);
            }
            out
        };
        let new_bo = self.insert_before(bo0, Opcode::Binary(op), ty, vec![new_phi_value, step], new_flags);
        let new_bo_value = Operand::Value(self.function.instruction(new_bo).result.expect("a value"));
        let result = self.function.instruction(bo).result.expect("a value");
        let new_result = self.function.instruction(new_bo).result.expect("a value");
        self.function.take_name(result, new_result);
        for (slot, &(value, _)) in incoming.iter().enumerate() {
            if value != start0 {
                self.function.set_operand(new_phi, slot * 2, new_bo_value);
            }
        }

        // The new recurrence replaces the old, which is dead.
        let poison = |c: &Self, inst: InstId| {
            let ty = c.function.instruction(inst).ty;
            Operand::Constant(c.context.constant(Constant { ty, kind: ConstantKind::Poison }))
        };
        for dead in [bo0, bo1] {
            let with = poison(self, dead);
            self.replace_inst_with(dead, with);
            self.erase(dead);
        }
        self.erase(pn0);
        self.erase(pn1);
        Visit::Replace(new_bo_value)
    }

    /// `foldBinopWithPhiOperands`.
    pub fn fold_binop_with_phi_operands(
        &mut self,
        bo: InstId,
    ) -> Visit {
        let instruction = self.function.instruction(bo);
        let Opcode::Binary(op) = instruction.opcode else { return Visit::None };
        let [lhs, rhs] = instruction.operands[..] else { return Visit::None };
        let bo_flags = instruction.flags;
        let ty = instruction.ty;

        // Operands that are simple recurrences.
        let recurrence = self.fold_binop_with_recurrence(bo);
        if !matches!(recurrence, Visit::None) {
            return recurrence;
        }

        let is_phi =
            |c: &Self, v: Operand| c.defining_inst(v).filter(|&def| c.function.instruction(def).opcode == Opcode::Phi);
        let (Some(phi0), Some(phi1)) = (is_phi(self, lhs), is_phi(self, rhs)) else { return Visit::None };
        let one_use = |c: &Self, phi: InstId| {
            c.function.users(c.function.instruction(phi).result.expect("a value")).has_one_use()
        };
        if !one_use(self, phi0)
            || !one_use(self, phi1)
            || self.function.instruction(phi0).operands.len() != self.function.instruction(phi1).operands.len()
        {
            return Visit::None;
        }
        let block = self.function.parent(bo);
        if block != self.function.parent(phi0) || block != self.function.parent(phi1) {
            return Visit::None;
        }

        // An identity constant on one side of every incoming pair.
        let in0: Vec<(Operand, BlockId)> = pattern::phi_incoming(self.function.instruction(phi0)).collect();
        let in1: Vec<(Operand, BlockId)> = pattern::phi_incoming(self.function.instruction(phi1)).collect();
        if let Some(identity) = self.bin_op_identity(op, ty, false) {
            let mut values = Vec::with_capacity(in0.len());
            let all = in0
                .iter()
                .zip(&in1)
                .all(
                    |(&(a, from_a), &(b, from_b))| {
                        if from_a != from_b {
                            return false;
                        }
                        if self.same_constant(a, identity) {
                            values.push(b);
                        } else if self.same_constant(b, identity) {
                            values.push(a);
                        } else {
                            return false;
                        }
                        true
                    },
                );
            if all {
                let mut operands = Vec::new();
                for (value, &(_, from)) in values.into_iter().zip(&in0) {
                    operands.push(value);
                    operands.push(Operand::Block(from));
                }
                let new_phi = self.insert_before(phi0, Opcode::Phi, ty, operands, Flags::default());
                return Visit::Replace(Operand::Value(self.function.instruction(new_phi).result.expect("a value")));
            }
        }

        if in0.len() != 2 || in1.len() != 2 {
            return Visit::None;
        }

        // A pair of incoming constants for one of the predecessors.
        let (c0, const_bb, other_bb) = if is_imm_constant(self, in0[0].0) {
            (in0[0].0, in0[0].1, in0[1].1)
        } else if is_imm_constant(self, in0[1].0) {
            (in0[1].0, in0[1].1, in0[0].1)
        } else {
            return Visit::None;
        };
        let Some(&(c1, _)) = in1.iter().find(|&&(_, from)| from == const_bb) else { return Visit::None };
        if !is_imm_constant(self, c1) {
            return Visit::None;
        }

        // The block that gets the operation must reach here unconditionally,
        // else a non-speculatable op could run.
        let Some(branch) = self.function.terminator(other_bb) else { return Visit::None };
        let jump = self.function.instruction(branch);
        if jump.opcode != Opcode::Br
            || jump.operands.len() != 1
            || !self.env.analyses.dominator_tree().is_reachable(other_bb)
        {
            return Visit::None;
        }

        // TODO in LLVM: only the div/rem need this.
        let parent = block.expect("a placed instruction");
        let q = query(self, None);
        let reaches = is_guaranteed_to_transfer_execution_to_successor_range(
            &q,
            self.function.iter_block(parent).take_while(|&one| one != bo),
            u32::MAX,
        );
        if !reaches {
            return Visit::None;
        }

        // The constants fold for the predecessor that has them.
        let Some(new_c) = self.fold_constant_binop(op, c0, c1) else { return Visit::None };

        // The operation on the other incoming values, in the other predecessor.
        let value_from = |incoming: &[(Operand, BlockId)], from: BlockId| {
            incoming.iter().find(|&&(_, b)| b == from).expect("a predecessor").0
        };
        let (a, b) = (value_from(&in0, other_bb), value_from(&in1, other_bb));
        let new_bo = self.create_binop(branch, op, a, b, bo_flags);

        // A phi of the new values. The old phis are dead.
        let operands = vec![new_bo, Operand::Block(other_bb), new_c, Operand::Block(const_bb)];
        let new_phi = self.insert_before(phi0, Opcode::Phi, ty, operands, Flags::default());
        Visit::Replace(Operand::Value(self.function.instruction(new_phi).result.expect("a value")))
    }
}

#[cfg(test)]
#[path = "combine_phi_tests.rs"]
mod tests;
