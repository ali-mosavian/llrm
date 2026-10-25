//! `ICFLoopSafetyInfo` and its helpers (`MustExecute.cpp`), over one loop.

use std::cell::RefCell;

use llrm_analysis::cfg;
use llrm_analysis::cfg::Shape;
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::memoryssa::Accesses;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Outer;

use super::calls;

/// [`ICFLoopSafetyInfo`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MustExecute.h#L111):
/// per block, its first instruction that may not transfer execution to its
/// successor (`ImplicitControlFlowTracking`) and its first that may write
/// memory (`MemoryWriteTracking`), found when asked.
pub struct LoopSafety<'a> {
    context: &'a llrm_mir::context::Context,
    layout: &'a llrm_mir::datalayout::DataLayout,
    function: &'a Function,
    accesses: &'a Accesses,
    outer: &'a Outer,
    loop_: &'a Loop,
    shape: &'a Shape,
    first_icf: RefCell<IdMap<BlockId, Option<usize>>>,
    first_write: RefCell<IdMap<BlockId, Option<usize>>>,
    guaranteed: RefCell<IdMap<BlockId, bool>>,
    may_throw: bool,
    allow_speculation: bool,
}

impl<'a> LoopSafety<'a> {
    /// [`computeLoopSafetyInfo`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MustExecute.cpp#L80).
    pub fn new(
        context: &'a llrm_mir::context::Context,
        layout: &'a llrm_mir::datalayout::DataLayout,
        function: &'a Function,
        accesses: &'a Accesses,
        outer: &'a Outer,
        loop_: &'a Loop,
        shape: &'a Shape,
    ) -> Self {
        let mut safety = Self {
            context,
            layout,
            function,
            accesses,
            outer,
            loop_,
            shape,
            first_icf: Default::default(),
            first_write: Default::default(),
            guaranteed: Default::default(),
            may_throw: false,
            allow_speculation: true,
        };
        safety.may_throw = loop_.body.iter().any(|&at| safety.block_may_throw(cfg::block(at)));
        safety
    }

    /// `AllowSpeculation` of the run these queries are asked in.
    pub fn allowing_speculation(
        mut self,
        allow: bool,
    ) -> Self {
        self.allow_speculation = allow;
        self
    }

    pub fn allow_speculation(&self) -> bool {
        self.allow_speculation
    }

    /// `anyBlockMayThrow`.
    pub fn any_block_may_throw(&self) -> bool {
        self.may_throw
    }

    /// `blockMayThrow`: `ICF.hasICF(BB)`.
    pub fn block_may_throw(
        &self,
        block: BlockId,
    ) -> bool {
        self.first_special(&self.first_icf, block, |inst| !self.transfers_execution(inst)).is_some()
    }

    /// `MW.mayWriteToMemory(BB)`.
    fn block_may_write(
        &self,
        block: BlockId,
    ) -> bool {
        self.first_special(&self.first_write, block, |inst| self.may_write_to_memory(inst)).is_some()
    }

    fn first_special(
        &self,
        found: &RefCell<IdMap<BlockId, Option<usize>>>,
        block: BlockId,
        special: impl Fn(InstId) -> bool,
    ) -> Option<usize> {
        if let Some(&first) = found.borrow().get(&block) {
            return first;
        }
        let first = self.function.iter_block(block).position(|inst| special(inst));
        found.borrow_mut().insert(block, first);
        first
    }

    /// `InstructionPrecedenceTracking::isPreceededBySpecialInstruction`: a
    /// special instruction comes strictly before `inst` in its block.
    fn preceded_by(
        &self,
        found: &RefCell<IdMap<BlockId, Option<usize>>>,
        inst: InstId,
        special: impl Fn(InstId) -> bool,
    ) -> bool {
        let block = self.function.parent(inst).expect("an instruction in a block");
        let Some(first) = self.first_special(found, block, special) else { return false };
        self.function.iter_block(block).position(|one| one == inst).is_some_and(|at| first < at)
    }

    /// [`isGuaranteedToTransferExecutionToSuccessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L8372).
    fn transfers_execution(
        &self,
        inst: InstId,
    ) -> bool {
        let instruction = self.function.instruction(inst);
        match &instruction.opcode {
            Opcode::Ret | Opcode::Unreachable => false,
            // `Instruction::mayThrow` and `willReturn`.
            Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Resume => {
                !calls::may_throw(self.outer, self.context, self.function, inst)
                    && calls::will_return(self.outer, self.context, self.function, inst)
            }
            // A volatile store is not guaranteed to return
            // (`Instruction::willReturn`).
            Opcode::Store { volatile, .. } => !volatile,
            _ => true,
        }
    }

    /// `Instruction::mayWriteToMemory`, as the accesses state it.
    fn may_write_to_memory(
        &self,
        inst: InstId,
    ) -> bool {
        let unit = llrm_analysis::memory::Unit::over(
            self.context,
            self.layout,
            &self.outer.metadata,
            &self.outer.globals,
            self.function,
        );
        self.accesses.writes_in(&unit, inst).is_none_or(|writes| !writes.is_empty())
    }

    /// [`ICFLoopSafetyInfo::isGuaranteedToExecute`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MustExecute.cpp#L304).
    pub fn is_guaranteed_to_execute(
        &self,
        inst: InstId,
    ) -> bool {
        let block = self.function.parent(inst).expect("an instruction in a block");
        !self.preceded_by(&self.first_icf, inst, |one| !self.transfers_execution(one))
            && self.all_loop_paths_lead_to_block(block)
    }

    /// [`ICFLoopSafetyInfo::doesNotWriteMemoryBefore`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MustExecute.cpp#L329).
    pub fn does_not_write_memory_before(
        &self,
        inst: InstId,
    ) -> bool {
        let block = self.function.parent(inst).expect("an instruction in a block");
        !self.preceded_by(&self.first_write, inst, |one| self.may_write_to_memory(one))
            && self.does_not_write_memory_before_block(block)
    }

    fn does_not_write_memory_before_block(
        &self,
        block: BlockId,
    ) -> bool {
        // Fast path: there are no instructions before the header.
        if cfg::id(block) == self.loop_.header {
            return true;
        }
        !self.collect_transitive_predecessors(block).iter().any(|pred| self.block_may_write(pred))
    }

    /// [`collectTransitivePredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MustExecute.cpp#L169):
    /// the blocks of the loop on every path from the header (inclusive) to
    /// `block` (exclusive); none for the header.
    fn collect_transitive_predecessors(
        &self,
        block: BlockId,
    ) -> IdSet<BlockId> {
        let mut found: IdSet<BlockId> = IdSet::new();
        if cfg::id(block) == self.loop_.header {
            return found;
        }
        let mut work = Vec::new();
        for pred in self.function.predecessors(block) {
            if self.loop_.body.contains(&cfg::id(pred)) && found.insert(pred) {
                work.push(pred);
            }
        }
        while let Some(pred) = work.pop() {
            // We are not interested in backedges and we don't want to leave
            // the loop.
            if cfg::id(pred) == self.loop_.header {
                continue;
            }
            for next in self.function.predecessors(pred) {
                if self.loop_.body.contains(&cfg::id(next)) && found.insert(next) {
                    work.push(next);
                }
            }
        }
        found
    }

    /// [`allLoopPathsLeadToBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MustExecute.cpp#L211).
    fn all_loop_paths_lead_to_block(
        &self,
        block: BlockId,
    ) -> bool {
        // Fast path: the header is always reached once the loop is entered.
        if cfg::id(block) == self.loop_.header {
            return true;
        }
        if let Some(&known) = self.guaranteed.borrow().get(&block) {
            return known;
        }
        let answer = self.all_loop_paths_lead_to_block_impl(block);
        self.guaranteed.borrow_mut().insert(block, answer);
        answer
    }

    fn all_loop_paths_lead_to_block_impl(
        &self,
        block: BlockId,
    ) -> bool {
        let predecessors = self.collect_transitive_predecessors(block);
        // Bail out if a latch block is part of the predecessor set: we may
        // take the backedge to the header and not execute other latch
        // successors.
        if self.function.predecessors(cfg::block(self.loop_.header)).iter().any(|pred| predecessors.contains(pred)) {
            return false;
        }
        let mut checked: IdSet<BlockId> = IdSet::new();
        for pred in predecessors.iter() {
            // Predecessor block may throw, so it has a side exit.
            if self.block_may_throw(pred) {
                return false;
            }
            // BB dominates Pred, so if Pred runs, BB must run.
            if self.shape.dominance.dominates(self.function, cfg::id(block), cfg::id(pred)) {
                continue;
            }
            for succ in self.function.successors(pred) {
                if checked.insert(succ)
                    && succ != block
                    && !predecessors.contains(&succ)
                    && (self.loop_.body.contains(&cfg::id(succ)) || !self.cannot_take_first_iteration(succ))
                {
                    return false;
                }
            }
        }
        true
    }

    /// [`CanProveNotTakenFirstIteration`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MustExecute.cpp#L121):
    /// the exit edge into `exit` is not taken on the first iteration. DEVIATE:
    /// `simplifyCmpInst` is a fold of two constants.
    fn cannot_take_first_iteration(
        &self,
        exit: BlockId,
    ) -> bool {
        let function = self.function;
        let preds = function.predecessors(exit);
        let [exiting] = preds[..] else { return false };
        let Some(branch) = function.terminator(exiting) else { return false };
        let instruction = function.instruction(branch);
        if !matches!(instruction.opcode, Opcode::Br) || instruction.operands.len() != 3 {
            return false;
        }
        let (on_true, on_false) = (instruction.operands[1], instruction.operands[2]);
        let (Operand::Block(on_true), Operand::Block(on_false)) = (on_true, on_false) else { return false };
        // A constant condition: the successor not taken is the exit.
        if let Operand::Constant(condition) = instruction.operands[0] {
            let llrm_mir::context::ConstantKind::Int(bits) = self.outer_constant(condition) else { return false };
            return (if bits & 1 == 1 { on_false } else { on_true }) == exit;
        }
        let Operand::Value(condition) = instruction.operands[0] else { return false };
        let llrm_mir::module::ValueDef::Instruction(compare) = function.value(condition).def else { return false };
        let compare_instruction = function.instruction(compare);
        let Opcode::ICmp(mut predicate) = compare_instruction.opcode else { return false };
        let header = cfg::block(self.loop_.header);
        let is_header_phi = |operand: Operand| match operand {
            Operand::Value(value) => match function.value(value).def {
                llrm_mir::module::ValueDef::Instruction(def) => {
                    matches!(function.instruction(def).opcode, Opcode::Phi) && function.parent(def) == Some(header)
                }
                _ => false,
            },
            _ => false,
        };
        let (mut lhs, mut rhs) = (compare_instruction.operands[0], compare_instruction.operands[1]);
        if !is_header_phi(lhs) {
            predicate = predicate.swapped();
            (lhs, rhs) = (rhs, lhs);
            if !is_header_phi(lhs) {
                return false;
            }
        }
        // The induction variable starts at the value coming into the header
        // from outside the loop (`getLoopPredecessor`).
        let entering: Vec<BlockId> = function
            .predecessors(header)
            .into_iter()
            .filter(|pred| !self.loop_.body.contains(&cfg::id(*pred)))
            .collect();
        let [predecessor] = entering[..] else { return false };
        let Operand::Value(phi_value) = lhs else { return false };
        let llrm_mir::module::ValueDef::Instruction(phi) = function.value(phi_value).def else { return false };
        let phi_instruction = function.instruction(phi);
        let start = phi_instruction
            .operands
            .chunks(2)
            .find_map(|pair| (pair.get(1) == Some(&Operand::Block(predecessor))).then_some(pair[0]));
        let (Some(start), Operand::Constant(rhs)) = (start, rhs) else { return false };
        let Operand::Constant(start) = start else { return false };
        let (llrm_mir::context::ConstantKind::Int(x), llrm_mir::context::ConstantKind::Int(y)) =
            (self.outer_constant(start), self.outer_constant(rhs))
        else {
            return false;
        };
        let width = self.compare_width(compare);
        let truth = predicate.holds(x, y, width);
        // The exit is `on_true` when the condition holds.
        if exit == on_true { !truth } else { truth }
    }

    fn outer_constant(
        &self,
        id: llrm_mir::context::ConstantId,
    ) -> llrm_mir::context::ConstantKind {
        self.context.get(id).kind.clone()
    }

    fn compare_width(
        &self,
        compare: InstId,
    ) -> u32 {
        let operand = self.function.instruction(compare).operands[0];
        let ty = match operand {
            Operand::Value(value) => self.function.value(value).ty,
            Operand::Constant(constant) => self.context.get(constant).ty,
            Operand::Block(_) => return 64,
        };
        self.context.types.int_bits(ty).unwrap_or(64)
    }
}
