//! `GVNPass`, the scalar part: a translation of the driver, `processInstruction`,
//! `propagateEquality`, `findLeader` and the dead-block handling of
//! [`GVN.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp)
//! at d1106deb71cc. Loads, load PRE and scalar PRE are the later stages of
//! `docs/architecture/gvn-port.md`; where `processInstruction` would call
//! them a load is numbered and entered as a leader, as LLVM does when
//! `processLoad` finds nothing.
//!
//! DEVIATE 1, the analyses: `DominatorTree` is rebuilt after a pass of CFG
//! edits (`MergeBlockIntoPredecessor`, a split critical edge); LLVM keeps it
//! with `DomTreeUpdater`. `LoopInfo`, `MemorySSAUpdater` and
//! `ImplicitControlFlowTracking` are not kept: nothing in this part reads
//! them (the last is read by PRE, stage 5).
//!
//! DEVIATE 3, `salvageAndRemoveInstruction` removes the instruction: MIR has no
//! debug-value use list to salvage into, and `salvageKnowledge` reads
//! operand bundles, which MIR has none of.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dominators::DominatorTree;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use llrm_support::hash::{HashMap, HashSet, IndexMap};

use super::table::{Env, ExprOp, LeaderMap, ValueTable};
use crate::local;

/// What `propagateEquality` is rooted at: the edge from a block to a
/// successor, or an instruction (an assume).
#[derive(Clone, Copy, Debug)]
enum Root {
    Edge(BlockId, BlockId),
    Inst(InstId),
}

pub struct GvnPass<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    outer: &'a Outer,
    dt: DominatorTree,
    vn: ValueTable,
    leader_table: LeaderMap,
    dead_blocks: IndexMap<BlockId, ()>,
}

/// `runImpl`'s scalar part. Whether anything changed.
pub fn run_impl(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let dt = DominatorTree::new(function);
    let mut pass = GvnPass {
        context,
        layout,
        function,
        outer,
        dt,
        vn: ValueTable::new(),
        leader_table: LeaderMap::default(),
        dead_blocks: IndexMap::default(),
    };
    pass.run_impl()
}

impl GvnPass<'_> {
    fn env(&mut self) -> (Env<'_>, &mut ValueTable, &LeaderMap) {
        (
            Env { context: &mut *self.context, function: &*self.function, layout: self.layout, outer: self.outer },
            &mut self.vn,
            &self.leader_table,
        )
    }

    fn lookup_or_add(
        &mut self,
        value: Operand,
    ) -> u32 {
        let (mut env, vn, _) = self.env();
        vn.lookup_or_add(&mut env, value)
    }

    fn refresh_dominators(&mut self) {
        self.dt = DominatorTree::new(self.function);
    }

    /// `runImpl`
    /// ([GVN.cpp:3483](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3483)).
    fn run_impl(&mut self) -> bool {
        let mut changed = false;
        // Merge unconditional branches, allowing PRE to catch more
        // optimization opportunities.
        for block in self.function.layout().to_vec() {
            if !self.function.layout().contains(&block) {
                continue;
            }
            changed |= local::merge_block_into_predecessor(self.function, block);
        }
        if changed {
            self.refresh_dominators();
        }

        let mut should_continue = true;
        while should_continue {
            should_continue = self.iterate_on_function();
            changed |= should_continue;
        }

        self.cleanup_global_sets();
        // Do not cleanup DeadBlocks in cleanupGlobalSets() as it's called for
        // each iteration.
        self.dead_blocks.clear();
        changed
    }

    /// `iterateOnFunction`
    /// ([GVN.cpp:3862](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3862)).
    fn iterate_on_function(&mut self) -> bool {
        self.cleanup_global_sets();
        // The order is fixed before any block is processed, as RPOT's is.
        let mut changed = false;
        for block in reverse_postorder(self.function) {
            changed |= self.process_block(block);
        }
        changed
    }

    /// `cleanupGlobalSets`
    /// ([GVN.cpp:3878](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3878)).
    fn cleanup_global_sets(&mut self) {
        self.vn.clear();
        self.leader_table.clear();
    }

    /// `processBlock`
    /// ([GVN.cpp:3567](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3567)).
    fn process_block(
        &mut self,
        block: BlockId,
    ) -> bool {
        if self.dead_blocks.contains_key(&block) {
            return false;
        }
        // Since we may not have visited the input blocks of the phis, we can't
        // use our normal hash approach for phis. Instead, simply look for
        // obvious duplicates.
        let (mut changed, to_remove) = local::eliminate_duplicate_phi_nodes(self.function, block);
        for phi in to_remove {
            self.remove_instruction(phi);
        }
        for inst in self.function.block(block).instructions().to_vec() {
            if self.function.is_erased(inst) {
                continue;
            }
            changed |= self.process_instruction(inst);
        }
        changed
    }

    /// `removeInstruction`
    /// ([GVN.cpp:3886](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3886)).
    fn remove_instruction(
        &mut self,
        inst: InstId,
    ) {
        if let Some(result) = self.function.instruction(inst).result {
            let is_phi = self.function.instruction(inst).opcode == Opcode::Phi;
            self.vn.erase(Operand::Value(result), is_phi);
            self.vn.verify_removed(Operand::Value(result));
        }
        self.function.erase(inst).expect("an instruction nothing uses");
    }

    /// `salvageAndRemoveInstruction` (DEVIATE 3).
    fn salvage_and_remove_instruction(
        &mut self,
        inst: InstId,
    ) {
        self.remove_instruction(inst);
    }

    /// `findLeader`
    /// ([GVN.cpp:3072](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3072)).
    pub fn find_leader(
        &self,
        block: BlockId,
        number: u32,
    ) -> Option<Operand> {
        let mut found = None;
        for entry in self.leader_table.get_leaders(number) {
            if self.dt.dominates(entry.block, block) {
                found = Some(entry.value);
                if matches!(entry.value, Operand::Constant(_)) {
                    return found;
                }
            }
        }
        found
    }

    fn is_constant(
        &self,
        value: Operand,
    ) -> bool {
        matches!(value, Operand::Constant(_))
    }

    fn is_argument(
        &self,
        value: Operand,
    ) -> bool {
        matches!(value, Operand::Value(id) if matches!(self.function.value(id).def, ValueDef::Argument(_)))
    }

    fn is_instruction(
        &self,
        value: Operand,
    ) -> bool {
        matches!(value, Operand::Value(id) if matches!(self.function.value(id).def, ValueDef::Instruction(_)))
    }

    fn defining(
        &self,
        value: Operand,
    ) -> Option<InstId> {
        match value {
            Operand::Value(id) => match self.function.value(id).def {
                ValueDef::Instruction(inst) => Some(inst),
                ValueDef::Argument(_) => None,
            },
            _ => None,
        }
    }

    /// The number of a constant integer.
    fn const_int(
        &self,
        value: Operand,
    ) -> Option<u128> {
        let Operand::Constant(id) = value else { return None };
        match self.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            ConstantKind::Zero | ConstantKind::Null if self.context.types.int_bits(self.context.get(id).ty).is_some() => {
                Some(0)
            }
            _ => None,
        }
    }

    fn type_of(
        &self,
        value: Operand,
    ) -> llrm_mir::types::TypeId {
        self.function.operand_type(self.context, value).expect("a value")
    }

    fn int_constant(
        &mut self,
        ty: llrm_mir::types::TypeId,
        value: u128,
    ) -> Operand {
        Operand::Constant(self.context.int(ty, value as i128))
    }

    fn has_one_use(
        &self,
        value: Operand,
    ) -> bool {
        matches!(value, Operand::Value(id) if self.function.users(id).len() == 1)
    }

    /// `isOnlyReachableViaThisEdge`
    /// ([GVN.cpp:3092](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3092)).
    fn is_only_reachable_via_this_edge(
        &self,
        start: BlockId,
        end: BlockId,
    ) -> bool {
        let pred = crate::edges::single_predecessor(self.function, end);
        assert!(pred.is_none_or(|pred| pred == start), "No edge between these basic blocks!");
        pred.is_some()
    }

    /// `propagateEquality`
    /// ([GVN.cpp:3119](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3119)):
    /// `lhs` and `rhs` are equal in every use `root` dominates.
    fn propagate_equality(
        &mut self,
        lhs: Operand,
        rhs: Operand,
        root: Root,
    ) -> bool {
        let mut worklist = vec![(lhs, rhs)];
        let mut visited: HashSet<(Operand, Operand)> = HashSet::default();
        let mut changed = false;
        let mut dominated_blocks: Vec<BlockId> = Vec::new();
        match root {
            Root::Edge(start, end) => {
                // For speed, compute a conservative fast approximation to
                // DT->dominates(Root, Root.getEnd()).
                if self.is_only_reachable_via_this_edge(start, end) {
                    dominated_blocks.push(end);
                }
            }
            Root::Inst(inst) => {
                let parent = self.function.parent(inst).expect("a placed assume");
                for &block in self.function.layout() {
                    if self.dt.immediate_dominator(block) == Some(parent) {
                        dominated_blocks.push(block);
                    }
                }
            }
        }

        while let Some((mut lhs, mut rhs)) = worklist.pop() {
            if lhs == rhs {
                continue;
            }
            // Don't try to propagate equalities between constants.
            if self.is_constant(lhs) && self.is_constant(rhs) {
                continue;
            }
            // Prefer a constant on the right-hand side, or an Argument if no
            // constants.
            if self.is_constant(lhs) || (self.is_argument(lhs) && !self.is_constant(rhs)) {
                std::mem::swap(&mut lhs, &mut rhs);
            }
            assert!(self.is_argument(lhs) || self.is_instruction(lhs), "Unexpected value!");

            // If there is no obvious reason to prefer the left-hand side over
            // the right-hand side, ensure the longest lived term is on the
            // right-hand side, so the shortest lived term will be replaced by
            // the longest lived. This tends to expose more simplifications.
            let mut lvn = self.lookup_or_add(lhs);
            if (self.is_argument(lhs) && self.is_argument(rhs)) || (self.is_instruction(lhs) && self.is_instruction(rhs))
            {
                // Move the 'oldest' value to the right-hand side, using the
                // value number as a proxy for age.
                let rvn = self.lookup_or_add(rhs);
                if lvn < rvn {
                    std::mem::swap(&mut lhs, &mut rhs);
                    lvn = rvn;
                }
            }
            if !visited.insert((lhs, rhs)) {
                continue;
            }

            // If value numbering later sees that an instruction in the scope
            // is equal to 'LHS' then ensure it will be turned into 'RHS'.
            // The leader table only tracks basic blocks, not edges. Only add
            // to if we have the simple case where the edge dominates the end.
            if !self.is_instruction(rhs) && local::can_replace_pointers_if_equal(self.context, self.layout, lhs, rhs) {
                for &block in &dominated_blocks {
                    self.leader_table.insert(lvn, rhs, block);
                }
            }

            // Replace all occurrences of 'LHS' with 'RHS' everywhere in the
            // scope. As LHS always has at least one use that is not dominated
            // by Root, this will never do anything if LHS has only one use.
            if !self.has_one_use(lhs) {
                let replaced = self.replace_dominated_uses_with_if(lhs, rhs, root);
                if replaced > 0 {
                    changed = true;
                }
            }

            // Now try to deduce additional equalities from this one. For
            // example, if the known equality was "(A != B)" == "false" then it
            // follows that A and B are equal in the scope. Only boolean
            // equalities with an explicit true or false RHS are currently
            // supported.
            if self.context.types.int_bits(self.type_of(rhs)) != Some(1) {
                continue;
            }
            let Some(bits) = self.const_int(rhs) else { continue };
            // Whether RHS equals 'true'. Otherwise it equals 'false'.
            let is_known_true = bits & 1 == 1;
            let is_known_false = !is_known_true;

            // If "A && B" is known true then both A and B are known true. If
            // "A || B" is known false then both A and B are known false.
            if let Some((a, b)) = self.logical(lhs, is_known_true) {
                worklist.push((a, rhs));
                worklist.push((b, rhs));
                continue;
            }

            // If we are propagating an equality like "(A == B)" == "true" then
            // also propagate the equality A == B. When propagating a
            // comparison such as "(A >= B)" == "true", replace all instances
            // of "A < B" with "false".
            if let Some(cmp) = self.defining(lhs).filter(|&inst| {
                matches!(self.function.instruction(inst).opcode, Opcode::ICmp(_) | Opcode::FCmp(_))
            }) {
                let instruction = self.function.instruction(cmp);
                let (op0, op1) = (instruction.operands[0], instruction.operands[1]);
                let (opcode, flags, ty) = (instruction.opcode.clone(), instruction.flags, instruction.ty);

                // If "A == B" is known true, or "A != B" is known false, then
                // replace A with B everywhere in the scope. For floating point
                // operations, we have to be careful since equality does not
                // always imply equivalance.
                if self.is_equivalence(&opcode, flags, op0, op1, is_known_false) {
                    worklist.push((op0, op1));
                }

                // If "A >= B" is known true, replace "A < B" with false
                // everywhere.
                let not_pred = match &opcode {
                    Opcode::ICmp(predicate) => ExprOp::ICmp(predicate.inverse()),
                    Opcode::FCmp(predicate) => ExprOp::FCmp(predicate.inverse()),
                    _ => unreachable!("a compare"),
                };
                let not_val = self.int_constant(ty, u128::from(is_known_false));
                // Since we don't have the instruction "A < B" immediately to
                // hand, work out the value number that it would have and use
                // that to find an appropriate instruction (if any).
                let next_num = self.vn.next_unused_value_number();
                let num = {
                    let (mut env, vn, _) = self.env();
                    vn.lookup_or_add_cmp(&mut env, not_pred, op0, op1)
                };
                // If the number we were assigned was brand new then there is no
                // point in looking for an instruction realizing it: there
                // cannot be one!
                if num < next_num {
                    for entry in self.leader_table.get_leaders(num).to_vec() {
                        // Only look at leaders that either dominate the start
                        // of the edge, or are dominated by the end. This check
                        // is not necessary for correctness, it only discards
                        // cases for which the following use replacement will
                        // not work anyway.
                        match root {
                            Root::Edge(start, end) => {
                                if !self.dt.dominates(entry.block, start) && !self.dt.dominates(end, entry.block) {
                                    continue;
                                }
                            }
                            Root::Inst(inst) => {
                                let block = self.function.parent(inst).expect("a placed assume");
                                if !self.dt.dominates(entry.block, block) && !self.dt.dominates(block, entry.block) {
                                    continue;
                                }
                            }
                        }
                        let not_cmp = entry.value;
                        if self.is_instruction(not_cmp) {
                            let replaced = self.replace_dominated_uses_with(not_cmp, not_val, root);
                            changed |= replaced > 0;
                        }
                    }
                }
                // Ensure that any instruction in scope that gets the "A < B"
                // value number is replaced with false. The leader table only
                // tracks basic blocks, not edges. Only add to if we have the
                // simple case where the edge dominates the end.
                for &block in &dominated_blocks {
                    self.leader_table.insert(num, not_val, block);
                }
                continue;
            }

            // Propagate equalities that results from truncation with no
            // unsigned wrap like (trunc nuw i64 %v to i1) == "true" or
            // (trunc nuw i64 %v to i1) == "false".
            if let Some(inst) = self.defining(lhs) {
                let instruction = self.function.instruction(inst);
                if instruction.opcode == Opcode::Cast(CastOp::Trunc) && instruction.flags.contains(Flags::NUW) {
                    let a = instruction.operands[0];
                    let ty = self.type_of(a);
                    let constant = self.int_constant(ty, u128::from(is_known_true));
                    worklist.push((a, constant));
                    continue;
                }
                // m_Not: xor with all ones.
                if let Opcode::Binary(BinaryOp::Xor) = instruction.opcode {
                    let [x, y] = instruction.operands[..] else { unreachable!("two operands") };
                    let all_ones = |value: Operand| self.const_int(value) == Some(1);
                    let a = if all_ones(y) {
                        Some(x)
                    } else if all_ones(x) {
                        Some(y)
                    } else {
                        None
                    };
                    if let Some(a) = a {
                        let ty = self.type_of(a);
                        let constant = self.int_constant(ty, u128::from(!is_known_true));
                        worklist.push((a, constant));
                        continue;
                    }
                }
            }
        }
        changed
    }

    /// `m_LogicalAnd` when `and` is set, `m_LogicalOr` otherwise: `A && B` is
    /// `and i1 A, B` or `select A, B, false`; `A || B` is `or i1 A, B` or
    /// `select A, true, B`.
    fn logical(
        &self,
        value: Operand,
        and: bool,
    ) -> Option<(Operand, Operand)> {
        let inst = self.defining(value)?;
        let instruction = self.function.instruction(inst);
        if self.context.types.int_bits(instruction.ty) != Some(1) {
            return None;
        }
        match (&instruction.opcode, and) {
            (Opcode::Binary(BinaryOp::And), true) | (Opcode::Binary(BinaryOp::Or), false) => {
                Some((instruction.operands[0], instruction.operands[1]))
            }
            (Opcode::Select, true) if self.const_int(instruction.operands[2]) == Some(0) => {
                Some((instruction.operands[0], instruction.operands[1]))
            }
            (Opcode::Select, false) if self.const_int(instruction.operands[1]) == Some(1) => {
                Some((instruction.operands[0], instruction.operands[2]))
            }
            _ => None,
        }
    }

    /// `CmpInst::isEquivalence`
    /// ([Instructions.cpp:3657](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L3657)).
    fn is_equivalence(
        &self,
        opcode: &Opcode,
        flags: Flags,
        op0: Operand,
        op1: Operand,
        invert: bool,
    ) -> bool {
        match opcode {
            Opcode::ICmp(predicate) => (if invert { predicate.inverse() } else { *predicate }) == IntPredicate::Eq,
            Opcode::FCmp(predicate) => {
                let predicate = if invert { predicate.inverse() } else { *predicate };
                match predicate {
                    FloatPredicate::Ueq if !flags.contains(Flags::NNAN) => false,
                    FloatPredicate::Ueq | FloatPredicate::Oeq => {
                        self.non_zero_fp(op0) || self.non_zero_fp(op1)
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// `hasNonZeroFPOperands`: a floating constant that is not a zero.
    fn non_zero_fp(
        &self,
        value: Operand,
    ) -> bool {
        let Operand::Constant(id) = value else { return false };
        match self.context.get(id).kind {
            ConstantKind::Float(bits) => {
                // The sign bit apart, an IEEE zero is all zero bits.
                let sign = match self.context.types.get(self.context.get(id).ty) {
                    llrm_mir::types::Type::Float(llrm_mir::types::FloatKind::Float) => 1_u64 << 31,
                    _ => 1_u64 << 63,
                };
                bits & !sign != 0
            }
            _ => false,
        }
    }

    /// `replaceDominatedUsesWithIf` from `root`, with `canReplacePointersInUseIfEqual`.
    fn replace_dominated_uses_with_if(
        &mut self,
        from: Operand,
        to: Operand,
        root: Root,
    ) -> usize {
        let Operand::Value(from) = from else { return 0 };
        let (context, layout) = (&*self.context, self.layout);
        let allowed = |function: &Function, user: llrm_mir::module::Use, to: Operand| {
            local::can_replace_pointers_in_use_if_equal(context, layout, function, user, to)
        };
        match root {
            Root::Edge(start, end) => local::replace_dominated_uses_with_if_edge(
                self.function,
                &self.dt,
                from,
                to,
                (start, end),
                &allowed,
            ),
            Root::Inst(inst) => {
                local::replace_dominated_uses_with_if_inst(self.function, &self.dt, from, to, inst, &allowed)
            }
        }
    }

    /// `replaceDominatedUsesWith` from `root`.
    fn replace_dominated_uses_with(
        &mut self,
        from: Operand,
        to: Operand,
        root: Root,
    ) -> usize {
        let Operand::Value(from) = from else { return 0 };
        let always = |_: &Function, _: llrm_mir::module::Use, _: Operand| true;
        match root {
            Root::Edge(start, end) => {
                local::replace_dominated_uses_with_if_edge(self.function, &self.dt, from, to, (start, end), &always)
            }
            Root::Inst(inst) => {
                local::replace_dominated_uses_with_if_inst(self.function, &self.dt, from, to, inst, &always)
            }
        }
    }

    /// `processInstruction`
    /// ([GVN.cpp:3327](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3327)).
    fn process_instruction(
        &mut self,
        inst: InstId,
    ) -> bool {
        // If the instruction can be easily simplified then do so now in
        // preference to value numbering it. Value numbering often exposes
        // redundancies, for example if it determines that %y is equal to %x
        // then the instruction "%z = and i32 %x, %y" becomes "%z = and i32 %x,
        // %x" which we now simplify.
        if let Some(simplified) = llrm_mir::simplify::simplify_instruction(self.context, self.function, inst) {
            let mut changed = false;
            let result = self.function.instruction(inst).result;
            if let Some(result) = result
                && !self.function.users(result).is_empty()
            {
                self.function.replace_all_uses_with(result, simplified);
                changed = true;
            }
            if local::is_instruction_trivially_dead(self.context, self.outer, self.function, inst) {
                self.salvage_and_remove_instruction(inst);
                changed = true;
            }
            if changed {
                return true;
            }
        }

        let instruction = self.function.instruction(inst);
        let parent = self.function.parent(inst).expect("a placed instruction");

        if self.is_assume(inst) {
            return self.process_assume_intrinsic(inst);
        }

        if matches!(instruction.opcode, Opcode::Load { .. }) {
            // Stage 4 calls `processLoad` here.
            let value = Operand::Value(instruction.result.expect("a load has a value"));
            let num = self.lookup_or_add(value);
            self.leader_table.insert(num, value, parent);
            return false;
        }

        // For conditional branches, we can perform simple conditional
        // propagation on the condition value itself.
        if instruction.opcode == Opcode::Br && instruction.operands.len() == 3 {
            let [condition, Operand::Block(true_succ), Operand::Block(false_succ)] = instruction.operands[..] else {
                unreachable!("a conditional branch")
            };
            if matches!(condition, Operand::Constant(_)) {
                return self.process_foldable_cond_br(inst);
            }
            // Avoid multiple edges early.
            if true_succ == false_succ {
                return false;
            }
            let bool_ty = self.context.types.int(1);
            let true_val = self.int_constant(bool_ty, 1);
            let mut changed = self.propagate_equality(condition, true_val, Root::Edge(parent, true_succ));
            let false_val = self.int_constant(bool_ty, 0);
            changed |= self.propagate_equality(condition, false_val, Root::Edge(parent, false_succ));
            return changed;
        }

        // For switches, propagate the case values into the case destinations.
        if instruction.opcode == Opcode::Switch {
            let operands = instruction.operands.clone();
            let condition = operands[0];
            let mut edges: HashMap<BlockId, u32> = HashMap::default();
            for operand in &operands {
                if let Operand::Block(block) = operand {
                    *edges.entry(*block).or_default() += 1;
                }
            }
            let mut changed = false;
            // Operands: value, default, then (case value, destination) pairs.
            for case in operands[2..].chunks(2) {
                let Operand::Block(destination) = case[1] else { continue };
                // If there is only a single edge, propagate the case value into
                // it.
                if edges[&destination] == 1 {
                    changed |= self.propagate_equality(condition, case[0], Root::Edge(parent, destination));
                }
            }
            return changed;
        }

        // Instructions with void type don't return a value, so there's no point
        // in trying to find redundancies in them.
        let Some(result) = instruction.result else { return false };
        let is_terminator = instruction.opcode.is_terminator();
        let is_alloca = matches!(instruction.opcode, Opcode::Alloca { .. });
        let is_phi = instruction.opcode == Opcode::Phi;
        let value = Operand::Value(result);

        let next_num = self.vn.next_unused_value_number();
        let num = self.lookup_or_add(value);

        // Allocations are always uniquely numbered, so we can save time and
        // memory by fast failing them.
        if is_alloca || is_terminator || is_phi {
            self.leader_table.insert(num, value, parent);
            return false;
        }

        // If the number we were assigned was a brand new VN, then we don't need
        // to do a lookup to see if the number already exists somewhere in the
        // domtree: it can't!
        if num >= next_num {
            self.leader_table.insert(num, value, parent);
            return false;
        }

        // Perform fast-path value-number based elimination of values inherited
        // from dominators.
        let Some(repl) = self.find_leader(parent, num) else {
            // Failure, just remember this instance for future use.
            self.leader_table.insert(num, value, parent);
            return false;
        };
        if repl == value {
            // If I was the result of a shortcut PRE, it might already be in the
            // table and the best replacement for itself. Nothing to do.
            return false;
        }

        // Remove it!
        local::patch_and_replace_all_uses_with(self.function, inst, repl);
        self.salvage_and_remove_instruction(inst);
        true
    }

    fn is_assume(
        &self,
        inst: InstId,
    ) -> bool {
        let Some(callee) = llrm_mir::memory::callee(self.context, self.function, inst) else { return false };
        self.outer.globals[callee.0 as usize].name.as_deref().and_then(Intrinsic::named) == Some(Intrinsic::Assume)
    }

    /// `processAssumeIntrinsic`
    /// ([GVN.cpp:2180](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2180)).
    /// An assume has no operand bundles in MIR, so `isAssumeWithEmptyBundle`
    /// holds.
    fn process_assume_intrinsic(
        &mut self,
        assume: InstId,
    ) -> bool {
        let value = self.function.instruction(assume).operands[0];
        if let Some(cond) = self.const_int(value) {
            if cond & 1 == 0 {
                // Insert a new store to null instruction before the assume to
                // indicate that this code is not reachable.
                let i8_ty = self.context.types.int(8);
                let poison = Operand::Constant(self.context.constant(Constant { ty: i8_ty, kind: ConstantKind::Poison }));
                let ptr_ty = self.context.types.ptr(0);
                let null = Operand::Constant(self.context.constant(Constant { ty: ptr_ty, kind: ConstantKind::Null }));
                let void = self.context.types.void();
                let store = self.function.create_instruction(
                    Opcode::Store { align: None, volatile: false },
                    void,
                    vec![poison, null],
                    Flags::default(),
                    None,
                );
                self.function.insert(store, Position::Before(assume)).expect("a placed assume");
            }
            self.salvage_and_remove_instruction(assume);
            return true;
        }
        if matches!(value, Operand::Constant(_)) {
            // If it's not false, and constant, it must evaluate to true. This
            // means our assume is assume(true), and thus, pointless, and we
            // don't want to do anything more here.
            return false;
        }
        let bool_ty = self.context.types.int(1);
        let truth = self.int_constant(bool_ty, 1);
        self.propagate_equality(value, truth, Root::Inst(assume))
    }

    /// `processFoldableCondBr`
    /// ([GVN.cpp:3997](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3997)).
    fn process_foldable_cond_br(
        &mut self,
        branch: InstId,
    ) -> bool {
        let operands = self.function.instruction(branch).operands.clone();
        let [condition, Operand::Block(true_succ), Operand::Block(false_succ)] = operands[..] else {
            unreachable!("a conditional branch")
        };
        // If a branch has two identical successors, we cannot declare either
        // dead.
        if true_succ == false_succ {
            return false;
        }
        let Some(cond) = self.const_int(condition) else { return false };
        let mut dead_root = if cond & 1 == 1 { false_succ } else { true_succ };
        if self.dead_blocks.contains_key(&dead_root) {
            return false;
        }
        if crate::edges::single_predecessor(self.function, dead_root).is_none() {
            let parent = self.function.parent(branch).expect("a placed branch");
            let Some(split) = self.split_critical_edges(parent, dead_root) else { return false };
            dead_root = split;
        }
        self.add_dead_block(dead_root);
        true
    }

    /// `splitCriticalEdges`
    /// ([GVN.cpp:3826](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3826)).
    fn split_critical_edges(
        &mut self,
        pred: BlockId,
        succ: BlockId,
    ) -> Option<BlockId> {
        let block = crate::edges::split_critical_edge(self.function, pred, succ)?;
        self.refresh_dominators();
        Some(block)
    }

    /// `addDeadBlock`
    /// ([GVN.cpp:3909](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L3909)).
    fn add_dead_block(
        &mut self,
        block: BlockId,
    ) {
        let mut new_dead = vec![block];
        let mut frontier: IndexMap<BlockId, ()> = IndexMap::default();
        while let Some(dead) = new_dead.pop() {
            if self.dead_blocks.contains_key(&dead) {
                continue;
            }
            // All blocks dominated by D are dead.
            let dominated: Vec<BlockId> = self
                .function
                .layout()
                .iter()
                .copied()
                .filter(|&b| self.dt.is_reachable(b) && self.dt.dominates(dead, b))
                .collect();
            for &b in &dominated {
                self.dead_blocks.insert(b, ());
            }
            // Figure out the dominance-frontier(D).
            for &b in &dominated {
                for successor in self.function.successors(b) {
                    if self.dead_blocks.contains_key(&successor) {
                        continue;
                    }
                    let all_pred_dead =
                        self.function.predecessors(successor).iter().all(|p| self.dead_blocks.contains_key(p));
                    if !all_pred_dead {
                        // S could be proved dead later on. That is why we don't
                        // update phi operands at this moment.
                        frontier.insert(successor, ());
                    } else {
                        // While S is not dominated by D, it is dead by now.
                        // This could take place if S already have a dead
                        // predecessor before D is declared dead.
                        new_dead.push(successor);
                    }
                }
            }
        }

        // For the dead blocks' live successors, update their phi nodes by
        // replacing the operands corresponding to dead blocks with poison.
        for (b, ()) in frontier {
            if self.dead_blocks.contains_key(&b) {
                continue;
            }
            // First, split the critical edges.
            let preds: Vec<BlockId> = self.function.predecessors(b).to_vec();
            for p in preds {
                if !self.dead_blocks.contains_key(&p) {
                    continue;
                }
                if self.function.successors(p).contains(&b) && crate::edges::is_critical_edge(self.function, p, b) {
                    if let Some(split) = self.split_critical_edges(p, b) {
                        self.dead_blocks.insert(split, ());
                    }
                }
            }
            // Now poison the incoming values from the dead predecessors.
            for p in self.function.predecessors(b).to_vec() {
                if !self.dead_blocks.contains_key(&p) {
                    continue;
                }
                for phi in crate::edges::phis(self.function, b) {
                    let ty = self.function.instruction(phi).ty;
                    let poison = Operand::Constant(self.context.constant(Constant { ty, kind: ConstantKind::Poison }));
                    let operands = self.function.instruction(phi).operands.clone();
                    for (at, pair) in operands.chunks(2).enumerate() {
                        if pair[1] == Operand::Block(p) {
                            self.function.set_operand(phi, at * 2, poison);
                        }
                    }
                }
            }
        }
    }
}

/// The reachable blocks in reverse postorder, `ReversePostOrderTraversal`.
fn reverse_postorder(function: &Function) -> Vec<BlockId> {
    let Some(entry) = function.entry() else { return Vec::new() };
    let mut seen: HashSet<BlockId> = HashSet::default();
    let mut order = Vec::new();
    let mut stack = vec![(entry, function.successors(entry).to_vec(), 0_usize)];
    seen.insert(entry);
    while let Some((block, successors, next)) = stack.last_mut() {
        if let Some(&successor) = successors.get(*next) {
            *next += 1;
            if seen.insert(successor) {
                let below = function.successors(successor).to_vec();
                stack.push((successor, below, 0));
            }
        } else {
            order.push(*block);
            stack.pop();
        }
    }
    order.reverse();
    order
}
