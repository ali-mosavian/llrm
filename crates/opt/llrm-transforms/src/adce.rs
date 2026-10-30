//! Aggressive dead code elimination: a translation of LLVM's ADCE
//! (`ADCE.cpp` at d1106deb71cc). Unlike [`crate::dead`], which keeps whatever
//! feeds a branch, it assumes every instruction dead and proves live only the
//! ones with effects, the ones they read, and the branches whose outcome
//! decides whether a live block runs. A dead branch becomes a jump to its
//! successor closest to the exit, and the code that only fed it goes.
//!
//! # How it works
//! Roots are the instructions with side effects, the exits that are not
//! branches, the terminators of back edges (a loop might not end), and the
//! blocks that cannot reach a return. Liveness flows backwards to operands;
//! a live phi makes control-live the predecessors it reads from. When the
//! data flow settles, the reverse iterated dominance frontier
//! ([`llrm_mir::idf`]) of the newly live blocks, pruned to blocks whose branch
//! is still dead, is the set of branches that decide them: those become live
//! and the walk resumes. What is left dead is removed; each dead branch is
//! made unconditional to the successor with the highest post-order number of
//! the reverse CFG, so a path to the exit remains.
//!
//! ```text
//!  b0: br %c, b1, b2        b0: br b2      (b1 only computes a dead value)
//!  b1: %d = add ..; br b2   b2: ret
//!  b2: ret
//! ```
//!
//! # DEVIATEs
//! 1. No debug-info scopes: `AliveScopes`, `collectLiveScopes` and the dropping
//!    of debug records outside them are not translated; MIR debug records name
//!    values, not scopes, and `erase` forgets the ones whose value goes.
//! 2. No `DomTreeUpdater`: llrm's pass manager recomputes dominators and loops
//!    when the CFG changed, and the post-dominator tree built for the run is
//!    used only before the first edge goes.
//! 3. The instrumentation-profile call test (`isInstrumentsConstant`): MIR has
//!    no value-profiling calls.
//! 4. The marking walk is a loop over a stack where LLVM's `markLive` recurses
//!    through jump chains; the set it reaches is the same.
//! 5. `RemoveControlFlowFlag` is true and `RemoveLoops` false, the pass's
//!    defaults at -O1 and above.
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i1 %c, i16 %x) {
//! b0:
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   %d = add i16 %x, 1
//!   br label %b2
//!
//! b2:
//!   ret i16 %x
//! }
//! ```
//! After:
//! ```mir
//! define i16 @f(i1 %c, i16 %x) {
//! b0:
//!   br label %b2
//!
//! b1:
//!   br label %b2
//!
//! b2:
//!   ret i16 %x
//! }
//! ```
//! (`b1` is unreachable and left to `cfg` cleanup, as LLVM leaves it to
//! SimplifyCFG.)
//!
//! # Runs
//! LLVM runs it after SCCP, BDCE and before MemCpyOpt (slot 35). Not in llrm's
//! pipeline yet: `llrm-pass adce FILE` runs it alone.
//!
//! # See also
//! [`crate::dead`] is what it replaces; [`crate::bdce`] is the bit-level one.

use llrm_analysis::cfgwalk::{find_function_backedges, inverse_post_order_ext};
use llrm_mir::context::Context;
use llrm_mir::dense::IdSet;
use llrm_mir::domtree::PostDomTree;
use llrm_mir::edit::Position;
use llrm_mir::memory;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

pub struct Adce;

impl FunctionPass for Adce {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "adce"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("adce")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let changed = aggressive_dead_code_elimination(unit.context, analyses.outer().callees(), unit.function);
        match (changed.anything, changed.control_flow) {
            (false, _) => PreservedAnalyses::all(),
            (true, false) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            (true, true) => PreservedAnalyses::none(),
        }
    }
}

/// `ADCEChanged`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Changed {
    pub anything: bool,
    pub control_flow: bool,
}

/// `BlockInfoType`.
#[derive(Clone, Copy, Default)]
struct BlockInfo {
    /// The block holds a live instruction.
    live: bool,
    has_live_phi_nodes: bool,
    /// The sources of its control dependences must be live.
    control_flow_live: bool,
    /// Post-order number in the reverse CFG.
    post_order: u32,
}

/// What `markLive` still has to do.
enum Mark {
    Instruction(InstId),
    Block(BlockId),
}

struct Aggressive<'a> {
    context: &'a mut Context,
    callees: &'a memory::Callees,
    function: &'a mut Function,
    post_dominators: PostDomTree,
    block_info: Vec<BlockInfo>,
    live: IdSet<InstId>,
    /// Instructions known live whose operands are yet to be marked.
    worklist: Vec<InstId>,
    blocks_with_dead_terminators: IdSet<BlockId>,
    new_live_blocks: IdSet<BlockId>,
}

/// `ADCEPass::run`'s body: whether and how the function changed.
pub fn aggressive_dead_code_elimination(
    context: &mut Context,
    callees: &memory::Callees,
    function: &mut Function,
) -> Changed {
    let mut post_dominators = PostDomTree::new();
    post_dominators.recalculate(&*function);
    let mut pass = Aggressive {
        context,
        callees,
        post_dominators,
        block_info: vec![BlockInfo::default(); function.block_capacity()],
        function,
        live: IdSet::new(),
        worklist: Vec::new(),
        blocks_with_dead_terminators: IdSet::new(),
        new_live_blocks: IdSet::new(),
    };
    pass.initialize();
    pass.mark_live_instructions();
    pass.remove_dead_instructions()
}

impl Aggressive<'_> {
    /// `AggressiveDeadCodeElimination::initialize`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ADCE.cpp#L205>
    fn initialize(&mut self) {
        // The roots known live.
        let roots: Vec<InstId> =
            self.function.walk().map(|(_, inst)| inst).filter(|&inst| self.is_always_live(inst)).collect();
        for inst in roots {
            self.mark(Mark::Instruction(inst));
        }
        // Back edges: a loop might not end.
        for (source, _) in find_function_backedges(self.function) {
            let terminator = self.function.terminator(source).expect("a block with a successor ends");
            self.mark(Mark::Instruction(terminator));
        }
        // A block with no path to a return: the subtree of a post-dominator
        // root child that does not end in one.
        for child in self.post_dominators.children(None) {
            let last = self.function.terminator(child);
            if last.is_some_and(|last| self.function.instruction(last).opcode == Opcode::Ret) {
                continue;
            }
            for block in self.post_dominators.descendants(child).into_iter().flatten() {
                if let Some(terminator) = self.function.terminator(block) {
                    self.mark(Mark::Instruction(terminator));
                }
            }
        }
        // The entry block is always live.
        let entry = self.function.entry().expect("a function with a body");
        self.block_info[entry.0 as usize].live = true;
        if let Some(jump) = self.unconditional_branch(entry) {
            self.mark(Mark::Instruction(jump));
        }
        for &block in self.function.layout() {
            let live = self.function.terminator(block).is_some_and(|last| self.live.contains(&last));
            if !live {
                self.blocks_with_dead_terminators.insert(block);
            }
        }
    }

    /// `isAlwaysLive`: an effect, or an exit that is not a branch or a switch.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ADCE.cpp#L246>
    fn is_always_live(
        &self,
        inst: InstId,
    ) -> bool {
        let opcode = &self.function.instruction(inst).opcode;
        match opcode {
            Opcode::Br | Opcode::Switch => false,
            opcode if opcode.is_terminator() => true,
            _ => !memory::only_value(self.context, self.callees, self.function, inst),
        }
    }

    /// The block's terminator if it is an unconditional branch.
    fn unconditional_branch(
        &self,
        block: BlockId,
    ) -> Option<InstId> {
        let terminator = self.function.terminator(block)?;
        let instruction = self.function.instruction(terminator);
        (instruction.opcode == Opcode::Br && instruction.operands.len() == 1).then_some(terminator)
    }

    /// `markLive(Instruction *)` and `markLive(BasicBlock *)`, run to the end
    /// of the chain of marks each starts.
    fn mark(
        &mut self,
        start: Mark,
    ) {
        let mut pending = vec![start];
        while let Some(next) = pending.pop() {
            match next {
                Mark::Instruction(inst) => {
                    if !self.live.insert(inst) {
                        continue;
                    }
                    self.worklist.push(inst);
                    let block = self.function.parent(inst).expect("a placed instruction");
                    if self.function.terminator(block) == Some(inst) {
                        self.blocks_with_dead_terminators.remove(&block);
                        // A live terminator keeps its destinations.
                        if self.unconditional_branch(block) != Some(inst) {
                            for operand in &self.function.instruction(inst).operands {
                                if let Operand::Block(successor) = *operand {
                                    pending.push(Mark::Block(successor));
                                }
                            }
                        }
                    }
                    pending.push(Mark::Block(block));
                }
                Mark::Block(block) => {
                    let info = &mut self.block_info[block.0 as usize];
                    if info.live {
                        continue;
                    }
                    info.live = true;
                    if !info.control_flow_live {
                        info.control_flow_live = true;
                        self.new_live_blocks.insert(block);
                    }
                    // Nothing to decide later for a jump at the end of a live
                    // block.
                    if let Some(jump) = self.unconditional_branch(block) {
                        pending.push(Mark::Instruction(jump));
                    }
                }
            }
        }
    }

    /// `markLiveInstructions`.
    fn mark_live_instructions(&mut self) {
        loop {
            while let Some(live) = self.worklist.pop() {
                let operands = self.function.instruction(live).operands.clone();
                for operand in operands {
                    if let Operand::Value(value) = operand
                        && let llrm_mir::module::ValueDef::Instruction(def) = self.function.value(value).def
                    {
                        self.mark(Mark::Instruction(def));
                    }
                }
                if self.function.instruction(live).opcode == Opcode::Phi {
                    self.mark_phi_live(live);
                }
            }
            // The branches the live instructions' execution depends on.
            self.mark_live_branches_from_control_dependences();
            if self.worklist.is_empty() {
                break;
            }
        }
    }

    /// `markPhiLive`: the predecessors a live phi reads from are control-live.
    fn mark_phi_live(
        &mut self,
        phi: InstId,
    ) {
        let block = self.function.parent(phi).expect("a placed phi");
        if std::mem::replace(&mut self.block_info[block.0 as usize].has_live_phi_nodes, true) {
            return;
        }
        for &predecessor in self.function.predecessors(block).iter() {
            let info = &mut self.block_info[predecessor.0 as usize];
            if !info.control_flow_live {
                info.control_flow_live = true;
                self.new_live_blocks.insert(predecessor);
            }
        }
    }

    /// `markLiveBranchesFromControlDependences`: the reverse dominance
    /// frontier of a live block is what it is control dependent on.
    fn mark_live_branches_from_control_dependences(&mut self) {
        if self.blocks_with_dead_terminators.is_empty() {
            return;
        }
        let frontier = llrm_mir::idf::calculate(
            &self.post_dominators,
            &*self.function,
            &self.new_live_blocks,
            Some(&self.blocks_with_dead_terminators),
        );
        self.new_live_blocks.clear();
        for block in frontier {
            let terminator = self.function.terminator(block).expect("a block ends");
            self.mark(Mark::Instruction(terminator));
        }
    }

    /// `removeDeadInstructions`.
    fn remove_dead_instructions(&mut self) -> Changed {
        let control_flow = self.update_dead_regions();
        let dead: Vec<InstId> =
            self.function.walk().map(|(_, inst)| inst).filter(|inst| !self.live.contains(inst)).collect();
        for &inst in &dead {
            self.function.set_operands(inst, Vec::new());
        }
        for &inst in dead.iter().rev() {
            self.function.erase(inst).expect("only the dead read the dead");
        }
        Changed { anything: control_flow || !dead.is_empty(), control_flow }
    }

    /// `updateDeadRegions`: each dead branch jumps to the successor closest to
    /// the exit.
    fn update_dead_regions(&mut self) -> bool {
        let blocks: Vec<BlockId> =
            self.function.layout().iter().copied().filter(|b| self.blocks_with_dead_terminators.contains(b)).collect();
        let mut have_post_order = false;
        let mut changed = false;
        for block in blocks {
            if let Some(jump) = self.unconditional_branch(block) {
                self.live.insert(jump);
                continue;
            }
            if !have_post_order {
                self.compute_reverse_post_order();
                have_post_order = true;
            }
            // The successor closest to the end of the function ensures a path
            // to the exit for each live edge.
            let edges: Vec<BlockId> = self
                .function
                .instruction(self.function.terminator(block).expect("a block ends"))
                .operands
                .iter()
                .filter_map(|operand| if let Operand::Block(target) = operand { Some(*target) } else { None })
                .collect();
            let mut preferred = None;
            let mut preferred_order = 0;
            for &successor in &edges {
                let order = self.block_info[successor.0 as usize].post_order;
                if preferred_order < order {
                    preferred = Some(successor);
                    preferred_order = order;
                }
            }
            let preferred = preferred.expect("Failed to find safe successor for dead branch");
            // Every edge but the first to the preferred successor goes.
            let mut first = true;
            for &successor in &edges {
                if !first || successor != preferred {
                    self.function.predecessor_removed(self.context, successor, block, false);
                } else {
                    first = false;
                }
            }
            self.make_unconditional(block, preferred);
            changed = true;
        }
        changed
    }

    /// `computeReversePostOrder`: a post-order numbering of the reverse CFG,
    /// from each block without successors.
    fn compute_reverse_post_order(&mut self) {
        let mut visited = IdSet::<BlockId>::new();
        let mut post_order = 0;
        for block in self.function.layout().to_vec() {
            if !self.function.successors(block).is_empty() {
                continue;
            }
            for reached in inverse_post_order_ext(self.function, block, &mut visited) {
                self.block_info[reached.0 as usize].post_order = post_order;
                post_order += 1;
            }
        }
    }

    /// `makeUnconditional`: `block` ends in a jump to `target`.
    fn make_unconditional(
        &mut self,
        block: BlockId,
        target: BlockId,
    ) {
        let old = self.function.terminator(block).expect("a block ends");
        if let Some(jump) = self.unconditional_branch(block) {
            self.function.set_operands(jump, vec![Operand::Block(target)]);
            self.live.insert(jump);
            return;
        }
        let ty = self.function.instruction(old).ty;
        let jump =
            self.function.create_instruction(Opcode::Br, ty, vec![Operand::Block(target)], Flags::default(), None);
        self.function.insert(jump, Position::Before(old)).expect("placed before the old terminator");
        self.live.insert(jump);
        self.function.set_operands(old, Vec::new());
        self.function.erase(old).expect("a terminator nothing reads");
    }
}

#[cfg(test)]
#[path = "adce_tests.rs"]
mod tests;
