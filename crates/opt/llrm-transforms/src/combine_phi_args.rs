//! `visitPHINode`'s argument folds: a phi of the same operation applied to
//! different values is the operation applied to a phi of those values.
//!
//! Translation of `foldPHIArgZextsIntoPHI`, `foldPHIArgLoadIntoPHI`,
//! `foldPHIArgGEPIntoPHI` and `foldPHIArgOpIntoPHI`
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
//! - Two or more incoming values that are simple loads with one user each and
//!   no memory write after them in their block
//!   (`isSafeAndProfitableToSinkLoad`) become one load of a phi of the
//!   addresses.
//! - Two or more incoming values that are `getelementptr`s of one source type
//!   and operand count, each with one user, whose operands agree but for one
//!   that is no constant and has one type in all of them, become one
//!   `getelementptr` of a phi of that operand (`foldPHIArgGEPIntoPHI`), at the
//!   flags all of them have. Not where every other base is a frame slot indexed
//!   by constants.
//! - `foldDeadPhiWeb`
//!   ([:59](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombinePHI.cpp#L59)):
//!   phis used only by other phis, fewer than 16 of them, are deleted.
//! - The unused induction variable
//!   ([:1445](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombinePHI.cpp#L1445)):
//!   a phi whose one user is a binary operator, `fneg` or GEP whose one user is
//!   the phi is poison, which leaves the user dead.
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
//! DEVIATE 1: the rest of `visitPHINode` is not translated: the
//! `insertvalue`/`extractvalue` and binary-operator-of-two-variables forms of
//! `foldPHIArgOpIntoPHI`, `foldPHIArgIntToPtrToPHI`, the pointer-cast phi,
//! `foldIntegerTypedPHI`, `SliceUpIllegalIntegerPHI`, and
//! `foldDependentIVs`. Exit: later slices.
//!
//! DEVIATE 2: no check that the block's terminator is an EH pad, which MIR
//! cannot have a phi before; no debug locations.
//!
//! # Used by
//! `combine` (its `visit`, the `Phi` opcode).

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::valuetracking::guarantee::may_write_to_memory;
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
        (Opcode::Load { .. }, Opcode::Load { .. }) => true,
        (Opcode::GetElementPtr { .. }, Opcode::GetElementPtr { .. }) => true,
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
            && let Some(folded) = match self.function.instruction(first).opcode {
                Opcode::Load { .. } => self.fold_phi_arg_load_into_phi(phi, &incoming),
                Opcode::GetElementPtr { .. } => self.fold_phi_arg_gep_into_phi(phi, &incoming),
                _ => self.fold_phi_arg_op_into_phi(phi, &incoming, first),
            }
        {
            return Visit::Replace(folded);
        }
        if self.fold_dead_phi_web(phi) {
            return Visit::Erased;
        }
        if let Some(poison) = self.unused_induction_variable(phi) {
            return Visit::Replace(poison);
        }
        if let Some(folded) = self.simplify_using_control_flow(phi, &incoming) {
            return Visit::Replace(folded);
        }
        Visit::None
    }

    /// `foldDeadPhiWeb`: the phis `phi` reaches through phi users, when
    /// every user on the way is a phi and there are fewer than 16, are dead.
    fn fold_dead_phi_web(
        &mut self,
        phi: InstId,
    ) -> bool {
        let mut web = vec![phi];
        let mut stack = vec![phi];
        while let Some(one) = stack.pop() {
            let result = self.function.instruction(one).result.expect("a phi's value");
            let users: Vec<InstId> = self.function.users(result).iter().map(|use_| use_.user).collect();
            for user in users {
                if !matches!(self.function.instruction(user).opcode, Opcode::Phi) {
                    return false;
                }
                if web.contains(&user) {
                    continue;
                }
                web.push(user);
                // Early stop if the set of PHIs is large.
                if web.len() >= 16 {
                    return false;
                }
                stack.push(user);
            }
        }
        for &one in &web {
            let poison = self.poison_of(one);
            self.replace_inst_with(one, poison);
        }
        for one in web {
            self.erase(one);
        }
        true
    }

    /// The `visitPHINode` rule for a phi whose one user computes the next
    /// iteration's value and is used only by the phi (`for (j = 0;; ++j)`):
    /// poison, which leaves the user dead.
    fn unused_induction_variable(
        &mut self,
        phi: InstId,
    ) -> Option<Operand> {
        let result = self.function.instruction(phi).result?;
        let user = self.function.users(result).only()?.user;
        let next = self.function.instruction(user);
        if !matches!(
            next.opcode,
            Opcode::Binary(_) | Opcode::FNeg | Opcode::GetElementPtr { .. }
        ) {
            return None;
        }
        let next_result = next.result?;
        (self.function.users(next_result).only()?.user == phi).then(|| self.poison_of(phi))
    }

    fn poison_of(
        &self,
        inst: InstId,
    ) -> Operand {
        let ty = self.function.instruction(inst).ty;
        Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Poison }))
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

    /// `foldPHIArgLoadIntoPHI`: a phi of simple loads is one load of a phi of
    /// the addresses (the address itself where they agree), at the least
    /// alignment, with the metadata all of them keep.
    fn fold_phi_arg_load_into_phi(
        &mut self,
        phi: InstId,
        incoming: &[(Operand, BlockId)],
    ) -> Option<Operand> {
        let first = self.definition_of(incoming[0].0)?;
        let first_inst = self.function.instruction(first).clone();
        let Opcode::Load { align, volatile: false } = first_inst.opcode else { return None };
        let layout = self.env.layout;
        let mut least = align.unwrap_or_else(|| layout.align(&self.context.types, first_inst.ty));
        let pointer_ty = self.type_of(first_inst.operands[0]);
        let mut loads = vec![first];
        // We can't sink the load if the loaded value could be modified
        // between the load and the phi.
        if self.function.parent(first) != Some(incoming[0].1) || !self.is_safe_and_profitable_to_sink_load(first) {
            return None;
        }
        for &(value, from) in &incoming[1..] {
            let def = self.definition_of(value)?;
            let Operand::Value(user_of) = value else { return None };
            let inst = self.function.instruction(def);
            let Opcode::Load { align, volatile: false } = inst.opcode else { return None };
            if !self.has_one_user(user_of) || self.type_of(inst.operands[0]) != pointer_ty {
                return None;
            }
            least = least.min(align.unwrap_or_else(|| layout.align(&self.context.types, inst.ty)));
            if self.function.parent(def) != Some(from) || !self.is_safe_and_profitable_to_sink_load(def) {
                return None;
            }
            loads.push(def);
        }
        let pointers: Vec<Operand> = loads.iter().map(|&one| self.function.instruction(one).operands[0]).collect();
        let pointer = if pointers.iter().all(|&one| one == pointers[0]) {
            pointers[0]
        } else {
            let operands: Vec<Operand> =
                pointers.iter().zip(incoming).flat_map(|(&value, &(_, from))| [value, Operand::Block(from)]).collect();
            let new_phi = self.insert_before(phi, Opcode::Phi, pointer_ty, operands, Flags::default());
            Operand::Value(self.function.instruction(new_phi).result.expect("a value"))
        };
        let at = self.after_phis(phi);
        let align = (least != layout.align(&self.context.types, first_inst.ty)).then_some(least);
        let new = self.insert_before(
            at,
            Opcode::Load { align, volatile: false },
            first_inst.ty,
            vec![pointer],
            Flags::default(),
        );
        for (kind, node) in first_inst.metadata.iter().filter(|(kind, _)| kind != "dbg") {
            self.function.annotate(new, kind, *node);
        }
        for &other in &loads[1..] {
            crate::local::combine_metadata_for_cse(self.env.metadata, self.function, new, other, true);
        }
        Some(Operand::Value(self.function.instruction(new).result.expect("a value")))
    }

    /// `foldPHIArgGEPIntoPHI`: a phi of `getelementptr`s that differ in one
    /// operand is one `getelementptr` of a phi of that operand. A second
    /// differing operand would add a phi, a constant one would make a
    /// cheaper index a variable, and bases that are all frame slots indexed by
    /// constants are cheaper left in the predecessors.
    fn fold_phi_arg_gep_into_phi(
        &mut self,
        phi: InstId,
        incoming: &[(Operand, BlockId)],
    ) -> Option<Operand> {
        let first = self.definition_of(incoming[0].0)?;
        let first_inst = self.function.instruction(first).clone();
        let Opcode::GetElementPtr { source } = first_inst.opcode else { return None };
        // The operand that needs a phi, if any.
        let mut varying: Option<usize> = None;
        let mut flags = first_inst.flags;
        let mut all_slots = true;
        let mut geps = vec![first];
        for &(value, _) in &incoming[1..] {
            let def = self.definition_of(value)?;
            let Operand::Value(user_of) = value else { return None };
            let inst = self.function.instruction(def);
            let Opcode::GetElementPtr { source: other } = inst.opcode else { return None };
            if !self.has_one_user(user_of) || other != source || inst.operands.len() != first_inst.operands.len() {
                return None;
            }
            flags = flags.intersect(inst.flags);
            // The pin looks at the other GEPs only, not the first.
            let on_slot = self
                .definition_of(inst.operands[0])
                .is_some_and(|base| matches!(self.function.instruction(base).opcode, Opcode::Alloca { .. }));
            if !(on_slot && inst.operands[1..].iter().all(|one| matches!(one, Operand::Constant(_)))) {
                all_slots = false;
            }
            for (at, (&one, &other)) in first_inst.operands.iter().zip(&inst.operands).enumerate() {
                if one == other {
                    continue;
                }
                if matches!(one, Operand::Constant(_))
                    || matches!(other, Operand::Constant(_))
                    || self.type_of(one) != self.type_of(other)
                    || varying.is_some()
                {
                    return None;
                }
                varying = Some(at);
            }
            geps.push(def);
        }
        if all_slots {
            return None;
        }
        let mut operands = first_inst.operands.clone();
        if let Some(at) = varying {
            let ty = self.type_of(first_inst.operands[at]);
            let merged: Vec<Operand> = geps
                .iter()
                .zip(incoming)
                .flat_map(|(&gep, &(_, from))| [self.function.instruction(gep).operands[at], Operand::Block(from)])
                .collect();
            let new_phi = self.insert_before(phi, Opcode::Phi, ty, merged, Flags::default());
            operands[at] = Operand::Value(self.function.instruction(new_phi).result.expect("a value"));
        }
        let at = self.after_phis(phi);
        let ty = self.function.instruction(phi).ty;
        let new = self.insert_before(at, Opcode::GetElementPtr { source }, ty, operands, flags);
        Some(Operand::Value(self.function.instruction(new).result.expect("a value")))
    }

    /// `isSafeAndProfitableToSinkLoad`: nothing after the load in its block
    /// writes memory (a call that only touches inaccessible memory does not),
    /// and its address is not a frame slot the code would only load the
    /// address of in each predecessor.
    fn is_safe_and_profitable_to_sink_load(
        &self,
        load: InstId,
    ) -> bool {
        let q = query(self, None);
        let mut scan = self.function.next_in_block(load);
        while let Some(one) = scan {
            if may_write_to_memory(&q, one) && !self.only_accesses_inaccessible_memory(&q, one) {
                return false;
            }
            scan = self.function.next_in_block(one);
        }
        let is_static_alloca = |def: InstId| {
            matches!(self.function.instruction(def).opcode, Opcode::Alloca { .. })
                && llrm_mir::instructioncost::is_static_alloca(self.function, def)
        };
        let Some(address) = self.definition_of(self.function.instruction(load).operands[0]) else { return true };
        match self.function.instruction(address).opcode {
            // A frame slot nothing takes the address of is not worth the
            // address in a register.
            Opcode::Alloca { .. } if is_static_alloca(address) => {
                let result = self.function.instruction(address).result.expect("a value");
                let taken =
                    self.function.users(result).iter().any(|one| match self.function.instruction(one.user).opcode {
                        Opcode::Load { .. } => false,
                        Opcode::Store { .. } => one.index != 1,
                        _ => true,
                    });
                taken
            }
            Opcode::GetElementPtr { .. } => {
                let operands = &self.function.instruction(address).operands;
                let on_slot = self.definition_of(operands[0]).is_some_and(is_static_alloca);
                !(on_slot && operands[1..].iter().all(|one| matches!(one, Operand::Constant(_))))
            }
            _ => true,
        }
    }

    /// `CallBase::onlyAccessesInaccessibleMemory`: what the site or the
    /// callee states allows no access through a pointer or to other memory.
    fn only_accesses_inaccessible_memory(
        &self,
        q: &llrm_mir::valuetracking::Query,
        inst: InstId,
    ) -> bool {
        let instruction = self.function.instruction(inst);
        let Some(info) = pattern::call_info(instruction) else { return false };
        let none = |attrs: &[llrm_mir::opcode::Attribute]| {
            let (through_pointers, elsewhere) = llrm_mir::memory::located(attrs);
            through_pointers == llrm_mir::memory::Effects::NONE && elsewhere == llrm_mir::memory::Effects::NONE
        };
        none(&info.attrs) || pattern::called_function(q, instruction).is_some_and(|callee| none(&callee.attrs))
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
