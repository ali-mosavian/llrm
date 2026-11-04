//! Reassociate: put commutative expressions in an order that promotes
//! constant folding, CSE, LICM and PRE.
//!
//! Translation of LLVM's
//! [`Reassociate.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp)
//! at d1106deb71cc, `ReassociatePass::runImpl` (:2737) and `OptimizeInst`
//! (:2292), `ReassociateExpression` (:2442).
//!
//! MIRROR: blocks in reverse post-order; each instruction is optimized once
//! (a dead one erased), the instructions the pass queued are then visited
//! again until none is left; a tree of one associative operation is
//! linearized, its leaves sorted by rank and optimized as a whole, and written
//! back into the nodes it came from.
//!
//! DEVIATE 1: no uniformity analysis. LLVM boosts the rank of divergent
//! leaves; a target without divergence (every llrm target) has none.
//!
//! DEVIATE 2: no vectors, and no debug locations (MIR carries none on an
//! instruction); a value's name is not carried to the instruction replacing it.
//!
//! DEVIATE 3: a value id is never reused, so the pair map has no stale-key
//! check (`PairMapValue::isValid`).
//!
//! DEVIATE 4: MIR has no constant `sub`: `NegateValue` of a constant
//! expression is a `sub 0, C` instruction (see `reassociate_negate`). Exit: a
//! constant `sub` in MIR.
//!
//! DEVIATE 5: a constant has no use list, so the negates of one are kept in
//! `constant_negates`, to find the one `NegateValue` reuses.
//!
//! DEVIATE 6: `reassociate` reports a change only if the body differs, not
//! `MadeChange` (which is also true for an add swapped and swapped back). The
//! pipeline repeats its body until no pass changes it, so a pass that reports
//! a no-op never settles it. Exit: the pipeline's rounds compare body
//! fingerprints themselves (as its cycle history already does), and
//! `reassociate` returns `MadeChange`.
//!
//! # How it works
//! The files are split by job: `reassociate_rank` (ranks),
//! `reassociate_tree` (linearize and rewrite a tree), `reassociate_optimize`
//! (the identities of one operation), `reassociate_negate` (subtract, negate,
//! shift and distribute into forms that reassociate), `reassociate_pairs`
//! (which pair to form first for CSE). This file is the driver and the shared
//! helpers; each of the others is an `impl` block of [`Reassociator`].
//!
//! # Used by
//! The pipeline's `reassociate` slot.
//!
//! # Example
//! `(x + 5) + 4` is `x + 9`; `(a * b) + (a * c)` is `a * (b + c)`.

use llrm_mir::constantfolding::Env;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::{Query, pattern};
use llrm_support::hash::HashMap;

use crate::reassociate_pairs::Pairs;
use crate::reassociate_rank::Ranks;

/// `reassociate::ValueEntry`: a leaf and its rank.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ValueEntry {
    pub rank: u32,
    pub op: Operand,
}

/// A map from a value or a constant. Hashed: built per tree, so a table paged
/// by id would cost the function's size each time (`SmallDenseMap` upstream).
pub(crate) struct OperandMap<V> {
    entries: HashMap<Operand, V>,
}

impl<V> OperandMap<V> {
    pub fn new() -> Self {
        Self { entries: HashMap::default() }
    }

    pub fn get(
        &self,
        v: Operand,
    ) -> Option<&V> {
        self.entries.get(&v)
    }

    pub fn get_mut(
        &mut self,
        v: Operand,
    ) -> Option<&mut V> {
        self.entries.get_mut(&v)
    }

    pub fn insert(
        &mut self,
        v: Operand,
        entry: V,
    ) {
        if !matches!(v, Operand::Block(_)) {
            self.entries.insert(v, entry);
        }
    }

    pub fn remove(
        &mut self,
        v: Operand,
    ) {
        self.entries.remove(&v);
    }

    /// The entry of `v`, `default` where it has none.
    pub fn get_or_insert(
        &mut self,
        v: Operand,
        default: V,
    ) -> &mut V {
        if self.get(v).is_none() {
            self.insert(v, default);
        }
        self.get_mut(v).expect("just made")
    }
}

/// `ReassociatePass::OrderedSet`: a set that remembers insertion order.
#[derive(Default)]
pub(crate) struct OrderedSet {
    order: Vec<Option<InstId>>,
    at: IdMap<InstId, usize>,
    head: usize,
}

impl OrderedSet {
    pub fn insert(
        &mut self,
        inst: InstId,
    ) {
        if self.at.get(&inst).is_none() {
            self.at.insert(inst, self.order.len());
            self.order.push(Some(inst));
        }
    }

    pub fn remove(
        &mut self,
        inst: InstId,
    ) {
        if let Some(at) = self.at.remove(&inst) {
            self.order[at] = None;
        }
    }

    /// `pop_back_val`.
    pub fn pop_back(&mut self) -> Option<InstId> {
        while let Some(last) = self.order.pop() {
            if let Some(inst) = last {
                self.at.remove(&inst);
                return Some(inst);
            }
        }
        None
    }

    /// `front` and `erase(begin())`.
    pub fn pop_front(&mut self) -> Option<InstId> {
        while self.head < self.order.len() {
            let at = self.head;
            self.head += 1;
            if let Some(inst) = self.order[at].take() {
                self.at.remove(&inst);
                return Some(inst);
            }
        }
        None
    }

    pub fn copy(&self) -> OrderedSet {
        let mut copy = OrderedSet::default();
        for inst in self.order[self.head..].iter().flatten() {
            copy.insert(*inst);
        }
        copy
    }
}

pub struct Reassociate;

impl FunctionPass for Reassociate {
    fn name(&self) -> &'static str {
        "reassociate"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer();
        if reassociate(unit.context, unit.layout, &outer, unit.function) {
            PreservedAnalyses::none().preserve::<llrm_mir::passes::Dominators>().preserve::<llrm_mir::passes::Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// The pass over `function`: whether anything changed.
pub fn reassociate(
    context: &Context,
    layout: &DataLayout,
    outer: &Outer,
    function: &mut Function,
) -> bool {
    let env = Env { globals: &outer.globals, layout };
    // Upstream's `MadeChange` is also true when the pass swaps an add's
    // operands and swaps them back (a body already in its order): the
    // pipeline repeats until no pass changes a body, so report a change
    // only if there is one.
    let before = digest(function);
    Reassociator::new(context, env, outer, function).run();
    digest(function) != before
}

/// `ReversePostOrderTraversal<Function *>`: the blocks `entry` reaches, each
/// before its successors but for back edges. (`llrm_support::graph`'s appends
/// the blocks it cannot reach, which Reassociate must never see: a block no
/// path reaches may hold an instruction that reads itself.)
fn reverse_post_order(
    function: &Function,
    entry: BlockId,
) -> Vec<BlockId> {
    let mut seen = llrm_mir::dense::IdSet::<BlockId>::new();
    seen.insert(entry);
    let mut post: Vec<BlockId> = Vec::new();
    // The first successor is walked first, so a stack of the rest reversed.
    let successors_of = |block: BlockId| {
        let mut list: Vec<BlockId> = function.successors(block).into_iter().collect();
        list.reverse();
        list
    };
    let mut stack: Vec<(BlockId, Vec<BlockId>)> = vec![(entry, successors_of(entry))];
    while let Some((block, successors)) = stack.last_mut() {
        match successors.pop() {
            Some(next) => {
                if seen.insert(next) {
                    stack.push((next, successors_of(next)));
                }
            }
            None => {
                post.push(*block);
                stack.pop();
            }
        }
    }
    post.reverse();
    post
}

/// A hash of the body: what each instruction is and reads, in order. (Not
/// `print::fingerprint`, which the pipeline counts as a hashed body.) Fx, not
/// SipHash: 2.0 M of the 3.5 M Ir of one run on d_faces -O2 was SipHash.
fn digest(function: &Function) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = llrm_mir::hash::Fx::default();
    for (block, inst) in function.walk() {
        block.hash(&mut hasher);
        let instruction = function.instruction(inst);
        instruction.opcode.hash(&mut hasher);
        instruction.operands.hash(&mut hasher);
        instruction.flags.hash(&mut hasher);
        instruction.ty.hash(&mut hasher);
    }
    hasher.finish()
}

/// One run of the pass over a function.
pub(crate) struct Reassociator<'a> {
    pub context: &'a Context,
    pub env: Env<'a>,
    pub outer: &'a Outer,
    pub function: &'a mut Function,
    pub ranks: Ranks,
    pub pairs: Pairs,
    pub redo: OrderedSet,
    /// The negates of each constant, in program order: a constant has no use
    /// list to scan (`NegateValue` looks for a negate of the value).
    pub constant_negates: IdMap<usize, Vec<InstId>>,
    /// `MadeChange`.
    pub changed: bool,
    /// The climb to a tree's root in `EraseInst` marks the nodes it passes
    /// with the stamp of the call.
    stamp: u32,
    visited: IdMap<InstId, u32>,
    /// Steps taken, for the slope test: each loop that could be super-linear
    /// counts one per turn.
    work: std::cell::Cell<u64>,
}

impl<'a> Reassociator<'a> {
    fn new(
        context: &'a Context,
        env: Env<'a>,
        outer: &'a Outer,
        function: &'a mut Function,
    ) -> Self {
        Self {
            context,
            env,
            outer,
            function,
            ranks: Ranks::empty(),
            pairs: Pairs::default(),
            redo: OrderedSet::default(),
            constant_negates: IdMap::new(),
            changed: false,
            stamp: 0,
            visited: IdMap::new(),
            work: std::cell::Cell::new(0),
        }
    }

    /// `runImpl`.
    fn run(&mut self) -> bool {
        let Some(entry) = self.function.entry() else { return false };
        let order = reverse_post_order(self.function, entry);
        let pinned = self.pinned_instructions(&order);
        self.ranks = Ranks::build(self.function, &order, |inst| pinned.contains(&inst));
        self.pairs = Pairs::build(self, &order);
        for &block in &order {
            for inst in self.function.iter_block(block).collect::<Vec<_>>() {
                if self.function.instruction(inst).result.is_some() {
                    self.note_constant_negate(inst);
                }
            }
        }
        self.changed = false;
        for &block in &order {
            self.visit_block(block);
        }
        self.changed
    }

    fn visit_block(
        &mut self,
        block: BlockId,
    ) {
        for inst in self.function.iter_block(block).collect::<Vec<_>>() {
            if self.function.is_erased(inst) || self.function.parent(inst).is_none() {
                continue;
            }
            self.tick();
            if self.is_trivially_dead(inst) {
                self.erase_inst(inst);
            } else {
                self.optimize_inst(inst);
            }
        }
        let mut to_redo = self.redo.copy();
        while let Some(inst) = to_redo.pop_back() {
            if self.is_trivially_dead(inst) {
                self.recursively_erase_dead_insts(inst, &mut to_redo);
                self.changed = true;
            }
        }
        while let Some(inst) = self.redo.pop_front() {
            if self.function.is_erased(inst) {
                continue;
            }
            if self.is_trivially_dead(inst) {
                self.erase_inst(inst);
            } else {
                self.optimize_inst(inst);
            }
        }
    }

    // ---- shared helpers -------------------------------------------------

    /// One step of a loop that could be super-linear.
    pub fn tick(&self) {
        self.work.set(self.work.get() + 1);
    }

    /// The steps taken so far, the ranks' too.
    #[cfg(test)]
    pub fn work_done(&self) -> u64 {
        self.work.get() + self.ranks.steps()
    }

    pub fn query(&self) -> Query<'_> {
        Query::bare(self.context, self.function, self.env.layout)
    }

    /// The placed instruction that computes `v`.
    pub fn inst_of(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = v else { return None };
        match self.function.value(value).def {
            ValueDef::Instruction(inst) if self.function.parent(inst).is_some() => Some(inst),
            _ => None,
        }
    }

    pub fn result(
        &self,
        inst: InstId,
    ) -> Operand {
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    pub fn operand(
        &self,
        inst: InstId,
        index: usize,
    ) -> Operand {
        self.function.instruction(inst).operands[index]
    }

    pub fn flags(
        &self,
        inst: InstId,
    ) -> Flags {
        self.function.instruction(inst).flags
    }

    pub fn type_of(
        &self,
        v: Operand,
    ) -> TypeId {
        self.function.operand_type(self.context, v).expect("a value or a constant")
    }

    pub fn is_float(
        &self,
        ty: TypeId,
    ) -> bool {
        matches!(self.context.types.get(ty), Type::Float(_))
    }

    /// The operation of a binary operator.
    pub fn binop(
        &self,
        inst: InstId,
    ) -> Option<BinaryOp> {
        match self.function.instruction(inst).opcode {
            Opcode::Binary(op) => Some(op),
            _ => None,
        }
    }

    /// `m_OneUse`: the value has exactly one use.
    pub fn has_one_use(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Value(value) if self.function.users(value).has_one_use()
        )
    }

    /// `user_back()` of a value with one use.
    pub fn only_user(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = v else { return None };
        self.function.users(value).only().map(|one| one.user)
    }

    /// The value of the one user of `v`, where it has one and has a value.
    pub fn only_user_value(
        &self,
        v: Operand,
    ) -> Option<Operand> {
        let user = self.only_user(v)?;
        self.function.instruction(user).result.map(Operand::Value)
    }

    pub fn is_constant(
        &self,
        v: Operand,
    ) -> bool {
        matches!(v, Operand::Constant(_))
    }

    /// `isa<ConstantInt>` as a number and its width.
    pub fn int_constant(
        &self,
        v: Operand,
    ) -> Option<(u128, u32)> {
        self.query().int_constant(v)
    }

    /// `hasFPAssociativeFlags`: `reassoc` and `nsz`.
    pub fn has_fp_associative_flags(
        &self,
        inst: InstId,
    ) -> bool {
        let flags = self.flags(inst);
        flags.contains(Flags::REASSOC) && flags.contains(Flags::NSZ)
    }

    /// `isa<FPMathOperator>`.
    pub fn is_fp_math(
        &self,
        inst: InstId,
    ) -> bool {
        match self.function.instruction(inst).opcode {
            Opcode::Binary(op) => {
                matches!(
                    op,
                    BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem
                )
            }
            Opcode::FNeg => true,
            _ => false,
        }
    }

    /// `isReassociableOp(V, Opcode...)`: `v` is a one-use binary operator of
    /// one of `ops`, a floating one with the flags that allow it.
    pub fn reassociable(
        &self,
        v: Operand,
        ops: &[BinaryOp],
    ) -> Option<InstId> {
        let inst = self.inst_of(v)?;
        let op = self.binop(inst)?;
        (self.has_one_use(v) && ops.contains(&op) && (!self.is_fp_math(inst) || self.has_fp_associative_flags(inst)))
            .then_some(inst)
    }

    /// `Instruction::isAssociative`.
    pub fn is_associative(
        &self,
        inst: InstId,
    ) -> bool {
        match self.binop(inst) {
            Some(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) => true,
            Some(BinaryOp::FAdd | BinaryOp::FMul) => self.has_fp_associative_flags(inst),
            _ => false,
        }
    }

    /// `Instruction::isCommutative` of a binary operator.
    pub fn is_commutative(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(
            self.binop(inst),
            Some(
                BinaryOp::Add
                    | BinaryOp::Mul
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
                    | BinaryOp::FAdd
                    | BinaryOp::FMul
            )
        )
    }

    /// `m_Neg`.
    pub fn match_neg(
        &self,
        v: Operand,
    ) -> Option<Operand> {
        pattern::neg(&self.query(), v).map(|(_, x, _)| x)
    }

    /// `m_Not`.
    pub fn match_not(
        &self,
        v: Operand,
    ) -> Option<Operand> {
        pattern::not(&self.query(), v)
    }

    /// `m_FNeg`.
    pub fn match_fneg(
        &self,
        v: Operand,
    ) -> Option<Operand> {
        pattern::fneg(&self.query(), v)
    }

    /// `getRank(V)`.
    pub fn rank(
        &mut self,
        v: Operand,
    ) -> u32 {
        self.ranks.of(self.context, self.env.layout, self.function, v)
    }

    /// `ValueEntry(getRank(V), V)`.
    pub fn entry(
        &mut self,
        v: Operand,
    ) -> ValueEntry {
        ValueEntry { rank: self.rank(v), op: v }
    }

    /// `mayHaveNonDefUseDependency` of each instruction in `order`'s blocks.
    fn pinned_instructions(
        &self,
        order: &[BlockId],
    ) -> llrm_mir::dense::IdSet<InstId> {
        let mut pinned = llrm_mir::dense::IdSet::new();
        for &block in order {
            for inst in self.function.iter_block(block) {
                if self.may_have_non_def_use_dependency(inst) {
                    pinned.insert(inst);
                }
            }
        }
        pinned
    }

    /// `mayHaveNonDefUseDependency`
    /// ([ValueTracking.cpp:7683](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7683)).
    fn may_have_non_def_use_dependency(
        &self,
        inst: InstId,
    ) -> bool {
        let access = llrm_mir::memory::of(self.context, self.outer.callees(), self.function, inst);
        access.reads
            || access.writes
            || !crate::licm::hoist::is_safe_to_speculatively_execute_in(
                self.context,
                self.env.layout,
                self.function,
                self.outer,
                inst,
            )
            || !crate::local::is_guaranteed_to_transfer_execution_to_successor(
                self.context,
                &self.outer.globals,
                self.function,
                inst,
            )
    }

    /// `isInstructionTriviallyDead`.
    pub fn is_trivially_dead(
        &self,
        inst: InstId,
    ) -> bool {
        crate::local::is_instruction_trivially_dead(
            self.context,
            self.outer.callees(),
            &self.outer.globals,
            self.function,
            inst,
        )
    }

    /// A new instruction placed at `position`.
    pub fn insert_at(
        &mut self,
        position: Position,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
    ) -> InstId {
        let new = self.function.create_instruction(opcode, ty, operands, flags, None);
        self.function.insert(new, position).expect("a placed position");
        new
    }

    /// A new instruction placed before `at`.
    pub fn insert_before(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
    ) -> InstId {
        self.insert_at(Position::Before(at), opcode, ty, operands, flags)
    }

    /// `isa<Argument>`.
    pub fn is_argument(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Value(value) if matches!(self.function.value(value).def, ValueDef::Argument(_))
        )
    }

    /// `isa<UndefValue>` (an `undef` or a `poison`).
    pub fn is_undef_operand(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Constant(c) if matches!(self.context.get(c).kind, ConstantKind::Undef | ConstantKind::Poison)
        )
    }

    /// `Value::replaceAllUsesWith`.
    pub fn replace_all_uses(
        &mut self,
        inst: InstId,
        with: Operand,
    ) {
        let result = self.function.instruction(inst).result.expect("a value");
        self.function.replace_all_uses_with(result, with);
    }

    /// The constant `0` of `ty`, when a use of it stands for dropping a use.
    pub fn zero_of(
        &self,
        ty: TypeId,
    ) -> Operand {
        if self.is_float(ty) {
            crate::reassociate_fp::float_constant(self.context, ty, 0.0)
        } else {
            Operand::Constant(self.context.int(ty, 0))
        }
    }

    /// `PoisonValue::get(ty)`.
    pub fn poison_of(
        &self,
        ty: TypeId,
    ) -> Operand {
        Operand::Constant(self.context.constant(llrm_mir::context::Constant { ty, kind: ConstantKind::Poison }))
    }

    // ---- erasing --------------------------------------------------------

    /// `EraseInst`: zap `inst` and queue the root of the tree each operand is
    /// in.
    fn erase_inst(
        &mut self,
        inst: InstId,
    ) {
        let operands = self.function.instruction(inst).operands.clone();
        self.forget(inst);
        self.function.erase(inst).expect("nothing reads a dead instruction");
        // Detect self-referential nodes with a stamp, not a set made per call.
        self.stamp += 1;
        for operand in operands {
            let Some(mut op) = self.inst_of(operand) else { continue };
            let opcode = self.function.instruction(op).opcode.clone();
            while let Some(user) = self.only_user(self.result(op)) {
                self.tick();
                if self.function.instruction(user).opcode != opcode || self.visited.get(&op) == Some(&self.stamp) {
                    break;
                }
                self.visited.insert(op, self.stamp);
                op = user;
            }
            // A node of a dead block is not ranked: its tree is not visited.
            if self.ranks.knows(self.function.instruction(op).result.expect("a value")) {
                self.redo.insert(op);
            }
        }
        self.changed = true;
    }

    /// `ValueRankMap.erase(I)` and `RedoInsts.remove(I)`.
    fn forget(
        &mut self,
        inst: InstId,
    ) {
        if let Some(result) = self.function.instruction(inst).result {
            self.ranks.forget(result);
        }
        self.redo.remove(inst);
    }

    /// `RecursivelyEraseDeadInsts`: erase `inst`, and queue in `insts` an
    /// operand it was the last use of.
    fn recursively_erase_dead_insts(
        &mut self,
        inst: InstId,
        insts: &mut OrderedSet,
    ) {
        let operands = self.function.instruction(inst).operands.clone();
        insts.remove(inst);
        self.forget(inst);
        self.function.erase(inst).expect("nothing reads a dead instruction");
        for operand in operands {
            if let Some(op) = self.inst_of(operand)
                && let Operand::Value(value) = operand
                && self.function.users(value).is_empty()
            {
                insts.insert(op);
            }
        }
    }
}

#[cfg(test)]
#[path = "reassociate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "reassociate_lit_tests.rs"]
mod lit_tests;
