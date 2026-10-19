//! Jump threading: a block that ends in a branch or switch on a condition some
//! of its predecessors decide is bypassed for those predecessors, which jump
//! straight to the target; as LLVM's `jump-threading`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `JumpThreading.cpp`, one Rust function
//! per C++ function, each linking its line. This is stage S1 of
//! `docs/architecture/jumpthreading-translation.md`: `runImpl`,
//! `findLoopHeaders`, `processBlock`, `computeValueKnownInPredecessors`,
//! `processThreadableEdges`, `tryThreadEdge`, `threadEdge`, `splitBlockPreds`,
//! `cloneInstructions`, `updateSSA` and `maybeMergeBasicBlockIntoOnlyPred`.
//! For each block the pass folds a terminator whose condition is a constant,
//! asks what the condition is on each edge into the block (a `phi` of
//! constants, `LazyValueInfo` on the edge, a compare of a `phi`, `and`/`or` of
//! known parts, a `select`), and copies the block (without its terminator)
//! for the predecessors that agree on where it goes next: the copy jumps to
//! that target and the predecessors jump to the copy.
//!
//! ```text
//!   p1: br b            p1: br b.thread       b.thread: ..; br t
//!   p2: br b      ==>   p2: br b              b: %c = phi [1, p1] [%x, p2]
//!   b:  %c = phi [1, p1] [%x, p2]                 br i1 %c, t, e
//!       br i1 %c, t, e
//! ```
//!
//! DEVIATE (J1..J9; stages S2-S4 are not ported yet, which is not a deviation
//! of the finished pass):
//! - J1. No `indirectbr` or `blockaddress` in MIR: `processBlock`'s
//!   `IndirectBrInst` arm, `hasAddressTakenAndUsed` and the block-address
//!   preference of `getKnownConstant` are absent.
//! - J2. No guards: `processGuards` is skipped; no input has
//!   `llvm.experimental.guard`.
//! - J3. No `!prof`, `BlockFrequencyInfo` or `BranchProbabilityInfo` updates
//!   (`updateBlockFreqAndEdgeWeight`, `getBPI`).
//! - J4. `DomTreeUpdater` is not used: the pass returns `none()` when it
//!   changed anything and the manager rebuilds the dominator tree. The
//!   reachable set at the start is a walk from the entry.
//! - J5. The sweep visits the blocks of the layout as it stood when the sweep
//!   began; a block made during it is met by the next sweep (LLVM's list
//!   iterator meets one placed after the current block in the same sweep).
//! - J6. `fcmp` is not looked through (`CmpInst` is integer compares only); MIR
//!   has no vectors.
//! - J7. Debug records are not retargeted (`DbgVariableRecord`s in `updateSSA`
//!   and `cloneInstructions`); `clone_instruction` carries the records of the
//!   original.
//! - J8. No `convergent` tokens (`HasLoopOrEntryConvergenceToken`), no noalias
//!   scopes to clone (`cloneNoAliasScopes`), no source atoms.
//! - J9. LVI is the pass's own `LazyValueInfo` (as `Cvp`'s), not the one
//!   function analysis the manager would share between the passes.
//! - Not yet ported (later stages): `tryToUnfoldSelect*`, `processBranchOnPHI`,
//!   `processBranchOnXOR`, `maybethreadThroughTwoBasicBlocks`,
//!   `duplicateCondBranchOnPHIIntoPred`, `processImpliedCondition`,
//!   `simplifyPartiallyRedundantLoad`.
//!
//! # Example
//! `%c` is `true` from `p1`, so `p1` skips `b`:
//! ```mir
//! p1:
//!   br label %b
//! b:
//!   %c = phi i1 [ true, %p1 ], [ %x, %p2 ]
//!   br i1 %c, label %t, label %e
//! ```
//!
//! # Used by
//! `pipeline` (`jumpthreading@668`, `jumpthreading@809`), the `Partial`
//! slots of the parity registry.

use std::rc::Rc;

use llrm_analysis::ssaupdater::SsaUpdater;
use llrm_mir::constantfolding::{self, Env as FoldEnv};
use llrm_mir::constantrange::ConstantRange;
use llrm_mir::constfold;
use llrm_mir::context::{Constant, ConstantId, ConstantKind};
use llrm_mir::dense::IdSet;
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::hash::HashSet;
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::lazyvalueinfo::{Env, LazyValueInfo};
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand, Use, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::simplify::{simplify_icmp_operands, simplify_instruction};
use llrm_mir::tti::{Cost, CostKind, Tti};
use llrm_mir::valuelattice::LatticeConstant;
use llrm_mir::valuetracking::guarantee::{
    is_guaranteed_not_to_be_undef_or_poison, is_guaranteed_to_transfer_execution_to_successor,
    is_guaranteed_to_transfer_execution_to_successor_block,
};
use llrm_mir::valuetracking::{AssumptionCache, Query};

use crate::lcssa::{arms, from_arms};
use crate::local::{is_instruction_trivially_dead, may_have_side_effects};

/// `DefaultBBDupThreshold`, the size gate of a copied block.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L88>
const DEFAULT_BB_DUP_THRESHOLD: u32 = 6;

/// `BBDuplicateThreshold` of a function with `minsize`
/// ([L310](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L310)).
const MIN_SIZE_BB_DUP_THRESHOLD: u32 = 3;

/// `PhiDuplicateThreshold`: a block with more phis is not copied.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L99>
const PHI_DUPLICATE_THRESHOLD: u32 = 76;

pub struct JumpThreading;

impl FunctionPass for JumpThreading {
    fn name(&self) -> &'static str {
        "jumpthreading"
    }

    /// `JumpThreadingPass::run`
    /// ([L240](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L240)).
    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if llrm_support::env_flag!("LLRM_NO_JUMPTHREADING") {
            return PreservedAnalyses::all();
        }
        let outer = Rc::clone(analyses.outer());
        let globals = Rc::clone(&outer.globals);
        let tti = crate::looprotate::target_tti(outer.target());
        let assumptions = AssumptionCache::of(unit.context, &globals, unit.metadata, unit.function);
        let mut pass = State {
            unit,
            globals,
            callees: outer.callees(),
            tti: &*tti,
            assumptions,
            lvi: LazyValueInfo::new(),
            loop_headers: IdSet::new(),
            unreachable: IdSet::new(),
            bb_dup_threshold: DEFAULT_BB_DUP_THRESHOLD,
        };
        if pass.run_impl() { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// A value known in a predecessor: `PredValueInfoTy`.
type PredValues = Vec<(ConstantId, BlockId)>;

/// What `JumpThreadingPass` holds: the function and its unit, `LVI`,
/// `LoopHeaders`, `Unreachable` and `BBDupThreshold`.
struct State<'a, 'u> {
    unit: &'a mut Unit<'u>,
    globals: Rc<Vec<GlobalValue>>,
    callees: &'a llrm_mir::memory::Callees,
    tti: &'a dyn Tti,
    assumptions: AssumptionCache,
    lvi: LazyValueInfo,
    loop_headers: IdSet<BlockId>,
    unreachable: IdSet<BlockId>,
    bb_dup_threshold: u32,
}

impl State<'_, '_> {
    // ---- reading the function ----

    fn function(&self) -> &Function {
        &*self.unit.function
    }

    fn query(&self) -> Query<'_> {
        Query {
            context: &*self.unit.context,
            globals: &self.globals,
            metadata: self.unit.metadata,
            function: &*self.unit.function,
            layout: self.unit.layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    fn op(
        &self,
        inst: InstId,
        index: usize,
    ) -> Operand {
        self.function().instruction(inst).operands[index]
    }

    fn opcode(
        &self,
        inst: InstId,
    ) -> Opcode {
        self.function().instruction(inst).opcode.clone()
    }

    fn result(
        &self,
        inst: InstId,
    ) -> Operand {
        Operand::Value(self.function().instruction(inst).result.expect("a value"))
    }

    /// `dyn_cast<Instruction>(value)`.
    fn instruction_of(
        &self,
        value: Operand,
    ) -> Option<InstId> {
        let Operand::Value(id) = value else { return None };
        match self.function().value(id).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    fn ty_of(
        &self,
        value: Operand,
    ) -> llrm_mir::types::TypeId {
        self.function().operand_type(&*self.unit.context, value).expect("a typed operand")
    }

    fn is_undef(
        &self,
        constant: ConstantId,
    ) -> bool {
        matches!(
            self.unit.context.get(constant).kind,
            ConstantKind::Undef | ConstantKind::Poison
        )
    }

    fn is_undef_operand(
        &self,
        value: Operand,
    ) -> bool {
        matches!(value, Operand::Constant(c) if self.is_undef(c))
    }

    fn int_bits(
        &self,
        constant: ConstantId,
    ) -> Option<u128> {
        match self.unit.context.get(constant).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        }
    }

    fn bool_constant(
        &self,
        value: bool,
    ) -> ConstantId {
        let ty = self.unit.context.types.int(1);
        self.unit.context.int(ty, i128::from(value))
    }

    /// `getKnownConstant`
    /// ([L534](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L534)),
    /// the integer preference: an undef or a `ConstantInt`.
    fn known_constant(
        &self,
        value: Operand,
    ) -> Option<ConstantId> {
        let Operand::Constant(c) = value else { return None };
        (self.is_undef(c) || self.int_bits(c).is_some()).then_some(c)
    }

    /// The edges into `block`, one per terminator operand naming it, in
    /// use-list order: LLVM's `predecessors(BB)`.
    fn pred_edges(
        &self,
        block: BlockId,
    ) -> Vec<BlockId> {
        let function = self.function();
        function
            .block_users(block)
            .iter()
            .filter(|one| function.instruction(one.user).opcode.is_terminator())
            .filter_map(|one| function.parent(one.user))
            .collect()
    }

    /// `getSinglePredecessor`.
    fn single_predecessor(
        &self,
        block: BlockId,
    ) -> Option<BlockId> {
        let edges = self.pred_edges(block);
        let first = *edges.first()?;
        edges.iter().all(|&one| one == first).then_some(first)
    }

    /// `successors(BB)`: one per edge, in operand order.
    fn successor_edges(
        &self,
        block: BlockId,
    ) -> Vec<BlockId> {
        let term = self.function().terminator(block).expect("a terminated block");
        self.function()
            .instruction(term)
            .operands
            .iter()
            .filter_map(|one| if let Operand::Block(to) = one { Some(*to) } else { None })
            .collect()
    }

    // ---- LVI, asked of the function as it is now ----

    /// `LVI->getConstantOnEdge` as a `getKnownConstant`.
    fn lvi_constant_on_edge(
        &mut self,
        value: Operand,
        from: BlockId,
        to: BlockId,
        cxt: Option<InstId>,
    ) -> Option<ConstantId> {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        let found = self.lvi.get_constant_on_edge(&mut env, value, from, to, cxt)?;
        self.known_from_lattice(found)
    }

    /// `LVI->getConstant`.
    fn lvi_constant(
        &mut self,
        value: Operand,
        at: InstId,
    ) -> Option<ConstantId> {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        let found = self.lvi.get_constant(&mut env, value, at)?;
        self.known_from_lattice(found)
    }

    /// `LVI->getPredicateOnEdge` as an `i1`.
    fn lvi_predicate_on_edge(
        &mut self,
        pred: IntPredicate,
        value: Operand,
        constant: ConstantId,
        from: BlockId,
        to: BlockId,
        cxt: Option<InstId>,
    ) -> Option<ConstantId> {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        let known = self.lvi.get_predicate_on_edge(&mut env, pred, value, constant, from, to, cxt)?;
        Some(self.bool_constant(known))
    }

    /// `LVI->getPredicateAt`.
    fn lvi_predicate_at(
        &mut self,
        pred: IntPredicate,
        value: Operand,
        constant: ConstantId,
        at: InstId,
    ) -> Option<ConstantId> {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        let known = self.lvi.get_predicate_at(&mut env, pred, value, constant, at, false)?;
        Some(self.bool_constant(known))
    }

    /// `LVI->getConstantRangeOnEdge`.
    fn lvi_range_on_edge(
        &mut self,
        value: Operand,
        from: BlockId,
        to: BlockId,
        cxt: Option<InstId>,
    ) -> ConstantRange {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        self.lvi.get_constant_range_on_edge(&mut env, value, from, to, cxt)
    }

    /// `LVI->threadEdge`.
    fn lvi_thread_edge(
        &mut self,
        pred: BlockId,
        old: BlockId,
        new: BlockId,
    ) {
        let env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        self.lvi.thread_edge(&env, pred, old, new);
    }

    /// What `getKnownConstant` takes of an LVI answer: an integer.
    fn known_from_lattice(
        &self,
        found: LatticeConstant,
    ) -> Option<ConstantId> {
        match found {
            LatticeConstant::Int { width, bits } => {
                let ty = self.unit.context.types.int(width);
                Some(self.unit.context.constant(Constant { ty, kind: ConstantKind::Int(bits) }))
            }
            _ => None,
        }
    }

    // ---- editing ----

    /// `I->eraseFromParent()`, told to LVI as its value handle is.
    fn erase(
        &mut self,
        inst: InstId,
    ) {
        if let Some(result) = self.function().instruction(inst).result {
            self.lvi.forget_value(Operand::Value(result));
        }
        self.unit.function.erase(inst).expect("an instruction nothing uses");
    }

    /// `Succ->removePredecessor(BB, true)`.
    fn remove_predecessor_keeping_phis(
        &mut self,
        block: BlockId,
        pred: BlockId,
    ) {
        let phis: Vec<(InstId, Operand)> =
            crate::edges::phis(self.function(), block).into_iter().map(|phi| (phi, self.result(phi))).collect();
        self.unit.function.predecessor_removed(self.unit.context, block, pred, true);
        for (phi, result) in phis {
            if self.function().is_erased(phi) {
                self.lvi.forget_value(result);
            }
        }
    }

    // ---- JumpThreading.cpp ----

    /// `runImpl`
    /// ([L284](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L284)).
    fn run_impl(&mut self) -> bool {
        self.bb_dup_threshold =
            if self.function().attrs.iter().any(|attr| matches!(attr, Attribute::Flag(flag) if flag == "minsize")) {
                MIN_SIZE_BB_DUP_THRESHOLD
            } else {
                DEFAULT_BB_DUP_THRESHOLD
            };
        let Some(entry) = self.function().entry() else { return false };

        self.unreachable.clear();
        let reached = self.reachable_from(entry);
        for block in self.function().layout().to_vec() {
            if !reached.contains(&block) {
                self.unreachable.insert(block);
            }
        }
        self.find_loop_headers();

        let mut ever_changed = false;
        loop {
            let mut changed = false;
            for block in self.function().layout().to_vec() {
                if self.unreachable.contains(&block) || !self.function().is_placed(block) {
                    continue;
                }
                while self.process_block(block) {
                    changed = true;
                }

                // Stop processing BB if it's the entry or is now deleted.
                if !self.function().is_placed(block) || Some(block) == self.function().entry() {
                    continue;
                }
                if self.pred_edges(block).is_empty() {
                    // When processBlock makes BB unreachable it doesn't bother
                    // to fix up the instructions in it. We must remove BB to
                    // prevent invalid IR.
                    self.loop_headers.remove(&block);
                    self.lvi.erase_block(block);
                    crate::simplifycfg::dead_blocks_deleted(self.unit.context, self.unit.function, &[block]);
                    changed = true;
                    continue;
                }

                // processBlock doesn't thread BBs with unconditional TIs.
                // However, if BB is "almost empty", we attempt to merge BB
                // with its sole successor.
                let term = self.function().terminator(block).expect("a terminated block");
                let instruction = self.function().instruction(term);
                if instruction.opcode == Opcode::Br && instruction.operands.len() == 1 {
                    let Operand::Block(succ) = instruction.operands[0] else { unreachable!("a jump's target") };
                    let only_terminator = self
                        .function()
                        .iter_block(block)
                        .find(|&inst| self.function().instruction(inst).opcode != Opcode::Phi)
                        == Some(term);
                    if only_terminator
                        && !self.loop_headers.contains(&block)
                        && !self.loop_headers.contains(&succ)
                        && crate::simplifycfg::empty_block_removed(self.unit.context, self.unit.function, block, succ)
                    {
                        self.lvi.erase_block(block);
                        changed = true;
                    }
                }
            }
            ever_changed |= changed;
            if !changed {
                break;
            }
        }
        self.loop_headers.clear();
        ever_changed
    }

    /// The blocks reachable from `entry`.
    fn reachable_from(
        &self,
        entry: BlockId,
    ) -> IdSet<BlockId> {
        let mut seen = IdSet::new();
        let mut work = vec![entry];
        seen.insert(entry);
        while let Some(block) = work.pop() {
            for succ in self.function().successors(block) {
                if seen.insert(succ) {
                    work.push(succ);
                }
            }
        }
        seen
    }

    /// `findLoopHeaders`
    /// ([L527](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L527)):
    /// the targets of the back edges of a walk from the entry
    /// (`FindFunctionBackedges`).
    fn find_loop_headers(&mut self) {
        self.loop_headers = crate::simplifycfg::loop_headers(self.function());
    }

    /// `replaceNonLocalUsesWith`
    /// ([Local.cpp:3258](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3258)).
    fn replace_non_local_uses_with(
        &mut self,
        from: InstId,
        to: Operand,
    ) -> bool {
        let block = self.function().parent(from);
        let result = self.function().instruction(from).result.expect("a value");
        let uses: Vec<Use> = self.function().users(result).iter().copied().collect();
        let mut changed = false;
        for one in uses {
            if self.function().parent(one.user) == block {
                continue;
            }
            self.unit.function.set_operand(one.user, one.index as usize, to);
            changed = true;
        }
        changed
    }

    /// `replaceFoldableUses`
    /// ([L388](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L388)).
    fn replace_foldable_uses(
        &mut self,
        cond: InstId,
        to_val: Operand,
        known_at_end_of_bb: BlockId,
    ) -> bool {
        let mut changed = false;
        let cond_value = self.result(cond);
        // We can unconditionally replace all uses in non-local blocks (i.e.
        // uses strictly dominated by BB), since LVI information is true from
        // the terminator of BB.
        if self.function().parent(cond) == Some(known_at_end_of_bb) {
            changed |= self.replace_non_local_uses_with(cond, to_val);
        }
        let reversed: Vec<InstId> = self.function().iter_block_rev(known_at_end_of_bb).collect();
        for inst in reversed {
            // Reached the Cond whose uses we are trying to replace, so there
            // are no more uses.
            if inst == cond {
                break;
            }
            // We only replace uses in instructions that are guaranteed to
            // reach the end of BB, where we know Cond is ToVal.
            if !is_guaranteed_to_transfer_execution_to_successor(&self.query(), inst) {
                break;
            }
            let operands = self.function().instruction(inst).operands.clone();
            if operands.contains(&cond_value) {
                let replaced = operands.iter().map(|&one| if one == cond_value { to_val } else { one }).collect();
                self.unit.function.set_operands(inst, replaced);
                changed = true;
            }
        }
        let result = self.function().instruction(cond).result.expect("a value");
        if self.function().users(result).is_empty()
            && !may_have_side_effects(&*self.unit.context, self.callees, &self.globals, self.function(), cond)
        {
            self.erase(cond);
            changed = true;
        }
        changed
    }

    /// `getJumpThreadDuplicationCost`
    /// ([L451](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L451)):
    /// what copying `bb` up to `stop_at` costs, `u32::MAX` if it cannot be.
    fn duplication_cost(
        &self,
        bb: BlockId,
        stop_at: InstId,
        threshold: u32,
    ) -> u32 {
        let function = self.function();
        // Do not duplicate the BB if it has a lot of PHI nodes.
        let mut phi_count = 0;
        let mut first_non_phi = None;
        for inst in function.iter_block(bb) {
            if function.instruction(inst).opcode != Opcode::Phi {
                first_non_phi = Some(inst);
                break;
            }
            phi_count += 1;
            if phi_count > PHI_DUPLICATE_THRESHOLD {
                return u32::MAX;
            }
        }
        let Some(first) = first_non_phi else { return 0 };

        let mut bonus = 0;
        if function.terminator(bb) == Some(stop_at) {
            // Threading through a switch statement is particularly
            // profitable.
            if function.instruction(stop_at).opcode == Opcode::Switch {
                bonus = 6;
            }
        }
        let threshold = threshold + bonus;

        // Sum up the cost of each instruction until we get to the terminator.
        let mut size = 0u32;
        for inst in function.iter_from(first) {
            if inst == stop_at {
                break;
            }
            // Stop scanning the block if we've reached the threshold.
            if size > threshold {
                return size;
            }
            // Blocks with NoDuplicate are modelled as having infinite cost.
            if let Opcode::Call(call) = &function.instruction(inst).opcode
                && call.attrs.iter().any(|attr| {
                    matches!(
                        attr,
                        Attribute::Flag(flag) if flag == "noduplicate" || flag == "convergent"
                    )
                })
            {
                return u32::MAX;
            }
            let cost = instruction_cost(
                self.tti,
                &*self.unit.context,
                self.unit.layout,
                &self.globals,
                function,
                inst,
                CostKind::SizeAndLatency,
            );
            if cost == Cost::FREE {
                continue;
            }
            // All other instructions count for at least one unit.
            size += 1;
            // Calls are more expensive.
            if let Opcode::Call(_) = function.instruction(inst).opcode {
                if self.is_intrinsic_call(inst) {
                    size += 1;
                } else {
                    size += 3;
                }
            }
        }
        size.saturating_sub(bonus)
    }

    /// `isa<IntrinsicInst>(call)`.
    fn is_intrinsic_call(
        &self,
        inst: InstId,
    ) -> bool {
        llrm_mir::memory::callee(&*self.unit.context, self.function(), inst)
            .and_then(|id| self.globals.get(id.0 as usize))
            .is_some_and(|callee| callee.name.as_deref().is_some_and(|name| name.starts_with("llvm.")))
    }

    /// `computeValueKnownInPredecessors`, the entry of the recursion.
    fn compute_value_known_in_predecessors(
        &mut self,
        value: Operand,
        bb: BlockId,
        result: &mut PredValues,
        cxt: InstId,
    ) -> bool {
        let mut recursion = HashSet::default();
        self.compute_value_known_in_predecessors_impl(value, bb, result, &mut recursion, cxt)
    }

    /// `computeValueKnownInPredecessorsImpl`
    /// ([L558](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L558)),
    /// the integer preference.
    fn compute_value_known_in_predecessors_impl(
        &mut self,
        value: Operand,
        bb: BlockId,
        result: &mut PredValues,
        recursion: &mut HashSet<Operand>,
        cxt: InstId,
    ) -> bool {
        // This method walks up use-def chains recursively. Because of this, we
        // could get into an infinite loop going around loops in the use-def
        // chain. To prevent this, keep track of what (value, block) pairs
        // we've already visited and terminate the search if we loop back to
        // them.
        if !recursion.insert(value) {
            return false;
        }

        // If V is a constant, then it is known in all predecessors.
        if let Some(known) = self.known_constant(value) {
            for pred in self.pred_edges(bb) {
                result.push((known, pred));
            }
            return !result.is_empty();
        }

        // If V is a non-instruction value, or an instruction in a different
        // block, then it can't be derived from a PHI.
        let inst = self.instruction_of(value).filter(|&inst| self.function().parent(inst) == Some(bb));
        let Some(inst) = inst else {
            // Okay, if this is a live-in value, see if it has a known value at
            // the any edge from our predecessors.
            for pred in self.pred_edges(bb) {
                // If the value is known by LazyValueInfo to be a constant in a
                // predecessor, use that information to try to thread this
                // block.
                let mut known = self.lvi_constant_on_edge(value, pred, bb, Some(cxt));
                // If I is a non-local compare-with-constant instruction, use
                // more-rich 'getPredicateOnEdge' method.
                if known.is_none()
                    && let Some(def) = self.instruction_of(value)
                    && let Opcode::ICmp(predicate) = self.opcode(def)
                    && let Operand::Constant(cst) = self.op(def, 1)
                {
                    let lhs = self.op(def, 0);
                    known = self.lvi_predicate_on_edge(predicate, lhs, cst, pred, bb, Some(cxt));
                }
                if let Some(known) = known.and_then(|c| self.known_constant(Operand::Constant(c))) {
                    result.push((known, pred));
                }
            }
            return !result.is_empty();
        };

        match self.opcode(inst) {
            // If I is a PHI node, then we know the incoming values for any
            // constants.
            Opcode::Phi => {
                let incoming = arms(self.function(), inst);
                for (in_val, in_block) in incoming {
                    if let Some(known) = self.known_constant(in_val) {
                        result.push((known, in_block));
                    } else if let Some(known) = self
                        .lvi_constant_on_edge(in_val, in_block, bb, Some(cxt))
                        .and_then(|c| self.known_constant(Operand::Constant(c)))
                    {
                        result.push((known, in_block));
                    }
                }
                return !result.is_empty();
            }
            // Handle Cast instructions.
            Opcode::Cast(op) => {
                let source = self.op(inst, 0);
                let mut vals = PredValues::new();
                self.compute_value_known_in_predecessors_impl(source, bb, &mut vals, recursion, cxt);
                if vals.is_empty() {
                    return false;
                }
                // Convert the known values.
                let ty = self.function().instruction(inst).ty;
                for (known, pred) in vals {
                    if let Some(folded) = constantfolding::constant_fold_cast_operand(
                        &*self.unit.context,
                        self.unit.layout,
                        op,
                        known,
                        ty,
                    ) {
                        result.push((folded, pred));
                    }
                }
                return !result.is_empty();
            }
            Opcode::Freeze => {
                let source = self.op(inst, 0);
                self.compute_value_known_in_predecessors_impl(source, bb, result, recursion, cxt);
                let query = self.query();
                let kept: PredValues = result
                    .iter()
                    .copied()
                    .filter(|&(known, _)| is_guaranteed_not_to_be_undef_or_poison(&query, Operand::Constant(known), 0))
                    .collect();
                *result = kept;
                return !result.is_empty();
            }
            _ => {}
        }

        // Handle some boolean conditions.
        let ty = self.function().instruction(inst).ty;
        if self.unit.context.types.int_bits(ty) == Some(1) {
            // X | true -> true, X & false -> false
            let logical = self.logical_operands(inst);
            if let Some((is_or, op0, op1)) = logical {
                let mut lhs_vals = PredValues::new();
                let mut rhs_vals = PredValues::new();
                self.compute_value_known_in_predecessors_impl(op0, bb, &mut lhs_vals, recursion, cxt);
                self.compute_value_known_in_predecessors_impl(op1, bb, &mut rhs_vals, recursion, cxt);
                if lhs_vals.is_empty() && rhs_vals.is_empty() {
                    return false;
                }
                let interesting = self.bool_constant(is_or);
                let mut lhs_known_bbs = HashSet::default();
                // Scan for the sentinel. If we find an undef, force it to the
                // interesting value: x|undef -> true and x&undef -> false.
                for &(known, pred) in &lhs_vals {
                    if known == interesting || self.is_undef(known) {
                        result.push((interesting, pred));
                        lhs_known_bbs.insert(pred);
                    }
                }
                for &(known, pred) in &rhs_vals {
                    if (known == interesting || self.is_undef(known)) && !lhs_known_bbs.contains(&pred) {
                        result.push((interesting, pred));
                    }
                }
                return !result.is_empty();
            }

            // Handle the NOT form of XOR.
            if self.opcode(inst) == Opcode::Binary(BinaryOp::Xor)
                && matches!(
                    self.op(inst, 1),
                    Operand::Constant(c) if self.int_bits(c) == Some(1)
                )
            {
                let source = self.op(inst, 0);
                self.compute_value_known_in_predecessors_impl(source, bb, result, recursion, cxt);
                if result.is_empty() {
                    return false;
                }
                // Invert the known values.
                for entry in result.iter_mut() {
                    entry.0 = match self.int_bits(entry.0) {
                        Some(bits) => self.bool_constant(bits == 0),
                        None => entry.0,
                    };
                }
                return true;
            }
        } else if let Opcode::Binary(op) = self.opcode(inst) {
            // Try to simplify some other binary operator values.
            if let Operand::Constant(rhs) = self.op(inst, 1)
                && self.int_bits(rhs).is_some()
            {
                let mut lhs_vals = PredValues::new();
                let lhs = self.op(inst, 0);
                self.compute_value_known_in_predecessors_impl(lhs, bb, &mut lhs_vals, recursion, cxt);
                // Try to use constant folding to simplify the binary operator.
                for (known, pred) in lhs_vals {
                    let folded = constfold::binary(&*self.unit.context, op, known, rhs);
                    if let Some(known) = folded.and_then(|c| self.known_constant(Operand::Constant(c))) {
                        result.push((known, pred));
                    }
                }
            }
            return !result.is_empty();
        }

        // Handle compare with phi operand, where the PHI is defined in this
        // block.
        if let Opcode::ICmp(pred) = self.opcode(inst) {
            let cmp_lhs = self.op(inst, 0);
            let cmp_rhs = self.op(inst, 1);
            let lhs_phi = self.instruction_of(cmp_lhs).filter(|&def| self.opcode(def) == Opcode::Phi);
            let phi = lhs_phi.or_else(|| self.instruction_of(cmp_rhs).filter(|&def| self.opcode(def) == Opcode::Phi));
            // Do not perform phi translation across a loop header phi, because
            // this may result in comparison of values from two different loop
            // iterations.
            if let Some(phi) = phi
                && self.function().parent(phi) == Some(bb)
                && !self.loop_headers.contains(&bb)
            {
                let incoming = arms(self.function(), phi);
                for (in_val, pred_bb) in incoming {
                    let (lhs, rhs) = if lhs_phi == Some(phi) {
                        (in_val, self.phi_translation(cmp_rhs, bb, pred_bb))
                    } else {
                        (self.phi_translation(cmp_lhs, bb, pred_bb), in_val)
                    };
                    let mut res =
                        simplify_icmp_operands(&*self.unit.context, self.unit.layout, self.function(), pred, lhs, rhs);
                    if res.is_none() {
                        let Operand::Constant(rhs_const) = rhs else { continue };
                        // getPredicateOnEdge call will make no sense if LHS is
                        // defined in BB.
                        if self.instruction_of(lhs).is_some_and(|def| self.function().parent(def) == Some(bb)) {
                            continue;
                        }
                        res = self
                            .lvi_predicate_on_edge(pred, lhs, rhs_const, pred_bb, bb, Some(cxt))
                            .map(Operand::Constant);
                    }
                    if let Some(known) = res.and_then(|one| self.known_constant(one)) {
                        result.push((known, pred_bb));
                    }
                }
                return !result.is_empty();
            }

            // If comparing a live-in value against a constant, see if we know
            // the live-in value on any predecessors.
            if let Operand::Constant(cmp_const) = cmp_rhs {
                let lhs_local = self.instruction_of(cmp_lhs).filter(|&def| self.function().parent(def) == Some(bb));
                if lhs_local.is_none() {
                    for pred_bb in self.pred_edges(bb) {
                        let res = self.lvi_predicate_on_edge(pred, cmp_lhs, cmp_const, pred_bb, bb, Some(cxt));
                        if let Some(known) = res.and_then(|c| self.known_constant(Operand::Constant(c))) {
                            result.push((known, pred_bb));
                        }
                    }
                    return !result.is_empty();
                }

                // InstCombine can fold some forms of constant range checks
                // into (icmp (add (x, C1)), C2). See if we have we have such a
                // thing with x as a live-in.
                if let Some(cmp_bits) = self.int_bits(cmp_const)
                    && let Some(add) = lhs_local.filter(|&def| self.opcode(def) == Opcode::Binary(BinaryOp::Add))
                    && let Operand::Constant(add_const) = self.op(add, 1)
                    && let Some(add_bits) = self.int_bits(add_const)
                {
                    let add_lhs = self.op(add, 0);
                    if self.instruction_of(add_lhs).is_none_or(|def| self.function().parent(def) != Some(bb)) {
                        let width = self.unit.context.types.int_bits(self.ty_of(add_lhs)).expect("an integer");
                        for pred_bb in self.pred_edges(bb) {
                            // If the value is known by LazyValueInfo to be a
                            // ConstantRange in a predecessor, use that
                            // information to try to thread this block.
                            let range = self.lvi_range_on_edge(add_lhs, pred_bb, bb, Some(cxt));
                            // Propagate the range through the addition.
                            let range = range.add(&ConstantRange::single(width, add_bits));
                            // Get the range where the compare returns true.
                            let cmp_range = ConstantRange::make_exact_icmp_region(pred, width, cmp_bits);
                            let res = if cmp_range.contains_range(&range) {
                                self.bool_constant(true)
                            } else if cmp_range.inverse().contains_range(&range) {
                                self.bool_constant(false)
                            } else {
                                continue;
                            };
                            result.push((res, pred_bb));
                        }
                        return !result.is_empty();
                    }
                }

                // Try to find a constant value for the LHS of a comparison,
                // and evaluate it statically if we can.
                let mut lhs_vals = PredValues::new();
                self.compute_value_known_in_predecessors_impl(cmp_lhs, bb, &mut lhs_vals, recursion, cxt);
                for (known, pred_bb) in lhs_vals {
                    let folded = constfold::compare(&*self.unit.context, pred, known, cmp_const);
                    if let Some(known) = folded.and_then(|c| self.known_constant(Operand::Constant(c))) {
                        result.push((known, pred_bb));
                    }
                }
                return !result.is_empty();
            }
        }

        if self.opcode(inst) == Opcode::Select {
            // Handle select instructions where at least one operand is a known
            // constant and we can figure out the condition value for any
            // predecessor block.
            let true_val = self.known_constant(self.op(inst, 1));
            let false_val = self.known_constant(self.op(inst, 2));
            let mut conds = PredValues::new();
            let condition = self.op(inst, 0);
            if (true_val.is_some() || false_val.is_some())
                && self.compute_value_known_in_predecessors_impl(condition, bb, &mut conds, recursion, cxt)
            {
                for (cond, pred) in conds {
                    // Figure out what value to use for the condition.
                    let known_cond = match self.int_bits(cond) {
                        // A known boolean.
                        Some(bits) => bits == 1,
                        // Either operand will do, so be sure to pick the one
                        // that's a known constant.
                        None => true_val.is_some(),
                    };
                    // See if the select has a known constant value for this
                    // predecessor.
                    if let Some(val) = if known_cond { true_val } else { false_val } {
                        result.push((val, pred));
                    }
                }
                return !result.is_empty();
            }
        }

        // If all else fails, see if LVI can figure out a constant value for us.
        if let Some(known) = self.lvi_constant(value, cxt).and_then(|c| self.known_constant(Operand::Constant(c))) {
            for pred in self.pred_edges(bb) {
                result.push((known, pred));
            }
        }
        !result.is_empty()
    }

    /// `m_LogicalOr` / `m_LogicalAnd` of an `i1`: `or`/`and`, or a `select`
    /// with a constant arm. Whether it is an or, and the two operands.
    fn logical_operands(
        &self,
        inst: InstId,
    ) -> Option<(bool, Operand, Operand)> {
        let is_bool = |value: Operand, want: bool| {
            matches!(
                value,
                Operand::Constant(c) if self.int_bits(c) == Some(u128::from(want))
            )
        };
        match self.opcode(inst) {
            Opcode::Binary(BinaryOp::Or) => Some((true, self.op(inst, 0), self.op(inst, 1))),
            Opcode::Binary(BinaryOp::And) => Some((false, self.op(inst, 0), self.op(inst, 1))),
            Opcode::Select => {
                let (cond, a, b) = (self.op(inst, 0), self.op(inst, 1), self.op(inst, 2));
                if is_bool(a, true) {
                    Some((true, cond, b))
                } else if is_bool(b, false) {
                    Some((false, cond, a))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// `Value::DoPHITranslation(CurBB, PredBB)`: a phi of `bb` is its input
    /// from `pred`, anything else itself.
    fn phi_translation(
        &self,
        value: Operand,
        bb: BlockId,
        pred: BlockId,
    ) -> Operand {
        match self.instruction_of(value) {
            Some(def) if self.opcode(def) == Opcode::Phi && self.function().parent(def) == Some(bb) => {
                arms(self.function(), def).into_iter().find(|&(_, from)| from == pred).map_or(value, |(v, _)| v)
            }
            _ => value,
        }
    }

    /// `getBestDestForJumpOnUndef`
    /// ([L929](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L929)):
    /// the index of the successor with the fewest predecessors.
    fn best_dest_for_jump_on_undef(
        &self,
        bb: BlockId,
    ) -> usize {
        let successors = self.successor_edges(bb);
        let mut min_succ = 0;
        let mut min_num_preds = self.pred_edges(successors[0]).len();
        for (index, &succ) in successors.iter().enumerate().skip(1) {
            let num_preds = self.pred_edges(succ).len();
            if num_preds < min_num_preds {
                min_succ = index;
                min_num_preds = num_preds;
            }
        }
        min_succ
    }

    /// `processBlock`
    /// ([L948](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L948)).
    fn process_block(
        &mut self,
        bb: BlockId,
    ) -> bool {
        // If the block is trivially dead, just return and let the caller nuke
        // it. This simplifies other transformations.
        if !self.function().is_placed(bb) || (self.pred_edges(bb).is_empty() && Some(bb) != self.function().entry()) {
            return false;
        }

        // If this block has a single predecessor, and if that pred has a
        // single successor, merge the blocks. This encourages recursive jump
        // threading because now the condition in this block can be threaded
        // through predecessors of our predecessor block.
        if self.maybe_merge_basic_block_into_only_pred(bb) {
            return true;
        }

        // Not yet ported: tryToUnfoldSelectInCurrBB (S2), processGuards (J2).

        // Look to see if the terminator is a conditional branch or switch, if
        // not we can't thread it.
        let terminator = self.function().terminator(bb).expect("a terminated block");
        let mut condition = match self.opcode(terminator) {
            Opcode::Br if self.function().instruction(terminator).operands.len() == 3 => self.op(terminator, 0),
            Opcode::Switch => self.op(terminator, 0),
            _ => return false,
        };

        // Keep track if we constant folded the condition in this invocation.
        let mut constant_folded = false;

        // Run constant folding to see if we can reduce the condition to a
        // simple constant.
        if let Some(inst) = self.instruction_of(condition) {
            let env = FoldEnv { globals: &self.globals, layout: self.unit.layout };
            let folded = constantfolding::constant_fold_instruction(&*self.unit.context, env, self.function(), inst);
            if let Some(simple) = folded {
                let result = self.function().instruction(inst).result.expect("a value");
                self.unit.function.replace_value(result, Operand::Constant(simple));
                if is_instruction_trivially_dead(
                    &*self.unit.context,
                    self.callees,
                    &self.globals,
                    self.function(),
                    inst,
                ) {
                    self.erase(inst);
                }
                condition = Operand::Constant(simple);
                constant_folded = true;
            }
        }

        // If the terminator is branching on an undef or freeze undef, we can
        // pick any of the successors to branch to. Let
        // getBestDestForJumpOnUndef decide.
        let freeze = self
            .instruction_of(condition)
            .filter(|&inst| self.opcode(inst) == Opcode::Freeze)
            .filter(|&inst| self.is_undef_operand(self.op(inst, 0)) && self.single_use(inst));
        if self.is_undef_operand(condition) || freeze.is_some() {
            let best = self.best_dest_for_jump_on_undef(bb);
            let successors = self.successor_edges(bb);
            // Fold the branch/switch.
            for (index, &succ) in successors.iter().enumerate() {
                if index != best {
                    self.remove_predecessor_keeping_phis(succ, bb);
                }
            }
            let term = self.function().terminator(bb).expect("a terminator");
            self.replace_with_jump(term, successors[best]);
            if let Some(freeze) = freeze {
                self.erase(freeze);
            }
            return true;
        }

        // If the terminator of this block is branching on a constant, simplify
        // the terminator to an unconditional branch. This can occur due to
        // threading in other blocks.
        if self.known_constant(condition).is_some() {
            crate::simplifycfg::terminator_folded(self.unit.context, self.callees, self.unit.function, bb, true);
            return true;
        }

        let Some(cond_inst) = self.instruction_of(condition) else {
            // FIXME: Unify this with code below.
            if self.process_threadable_edges(condition, bb, terminator) {
                return true;
            }
            return constant_folded;
        };

        // Some of the following optimization can safely work on the unfrozen
        // cond.
        let cond_without_freeze =
            if self.opcode(cond_inst) == Opcode::Freeze { self.op(cond_inst, 0) } else { condition };

        if let Some(cmp) = self.instruction_of(cond_without_freeze)
            && let Opcode::ICmp(pred) = self.opcode(cmp)
            && let Operand::Constant(cond_const) = self.op(cmp, 1)
        {
            // If we're branching on a conditional, LVI might be able to
            // determine it's value at the branch instruction. We only handle
            // comparisons against a constant at this time.
            let lhs = self.op(cmp, 0);
            let terminator_now = self.function().terminator(bb).expect("a terminator");
            if let Some(res) = self.lvi_predicate_at(pred, lhs, cond_const, terminator_now) {
                // We can safely replace *some* uses of the CondInst if it has
                // exactly one value as returned by LVI. RAUW is incorrect in
                // the presence of guards and assumes, that have the `Cond` as
                // the use.
                if self.replace_foldable_uses(cmp, Operand::Constant(res), bb) {
                    return true;
                }
            }
            // Not yet ported: tryToUnfoldSelect (S2).
        }

        // Not yet ported: tryToUnfoldSelect of a switch (S2), the load of
        // simplifyPartiallyRedundantLoad (S4), updatePredecessorProfileMetadata
        // (J3).

        // Handle a variety of cases where we are branching on something
        // derived from a PHI node in the current block. If we can prove that
        // any predecessors compute a predictable value based on a PHI node,
        // thread those predecessors.
        if self.process_threadable_edges(
            Operand::Value(self.function().instruction(cond_inst).result.expect("a value")),
            bb,
            terminator,
        ) {
            return true;
        }

        // Not yet ported: processBranchOnPHI, processBranchOnXOR (S2),
        // processImpliedCondition (S3).
        false
    }

    /// `hasOneUse()`.
    fn single_use(
        &self,
        inst: InstId,
    ) -> bool {
        let result = self.function().instruction(inst).result.expect("a value");
        self.function().users(result).has_one_use()
    }

    /// `UncondBrInst::Create(target, term)` then `term->eraseFromParent()`.
    fn replace_with_jump(
        &mut self,
        term: InstId,
        target: BlockId,
    ) {
        let void = self.function().instruction(term).ty;
        let jump = self
            .unit
            .function
            .create_instruction(Opcode::Br, void, vec![Operand::Block(target)], Flags::default(), None);
        self.unit.function.insert(jump, Position::Before(term)).expect("a placed terminator");
        self.unit.function.erase(term).expect("a terminator nothing uses");
    }

    /// `processThreadableEdges`
    /// ([L1572](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1572)).
    fn process_threadable_edges(
        &mut self,
        cond: Operand,
        bb: BlockId,
        cxt: InstId,
    ) -> bool {
        // If threading this would thread across a loop header, don't even try
        // to thread the edge.
        if self.loop_headers.contains(&bb) {
            return false;
        }

        let mut pred_values = PredValues::new();
        if !self.compute_value_known_in_predecessors(cond, bb, &mut pred_values, cxt) {
            // Not yet ported: maybethreadThroughTwoBasicBlocks (S2).
            return false;
        }

        // Decide what we want to thread through. Convert our list of known
        // values to a list of known destinations for each pred. This also
        // discards duplicate predecessors and keeps track of the undefined
        // inputs (which are represented as a null dest in the PredToDestList).
        let mut seen_preds = HashSet::default();
        let mut pred_to_dest: Vec<(BlockId, Option<BlockId>)> = Vec::new();
        let term = self.function().terminator(bb).expect("a terminator");
        let term_operands = self.function().instruction(term).operands.clone();

        // `None` is no destination yet; `Some(dest)` with `dest` `None` is an
        // undef's. `multiple_dest` is LLVM's MultipleDestSentinel.
        let mut only_dest: Option<Option<BlockId>> = None;
        let mut multiple_dest = false;
        let mut only_val: Option<ConstantId> = None;
        let mut multiple_val = false;

        for &(val, pred) in &pred_values {
            if !seen_preds.insert(pred) {
                continue; // Duplicate predecessor entry.
            }
            let dest_bb = if self.is_undef(val) {
                None
            } else if self.function().instruction(term).opcode == Opcode::Br {
                let Operand::Block(dest) = term_operands[if self.int_bits(val) == Some(0) { 2 } else { 1 }] else {
                    unreachable!("a branch's target")
                };
                Some(dest)
            } else {
                // `SI->findCaseValue(CI)->getCaseSuccessor()`: the default
                // when no case matches.
                let Operand::Block(default) = term_operands[1] else { unreachable!("a switch's default") };
                let found = term_operands[2..]
                    .chunks(2)
                    .find_map(
                        |pair| match (pair[0], pair[1]) {
                            (Operand::Constant(case), Operand::Block(dest)) if case == val => Some(dest),
                            _ => None,
                        },
                    );
                Some(found.unwrap_or(default))
            };

            // If we have exactly one destination, remember it for efficiency
            // below.
            if pred_to_dest.is_empty() {
                only_dest = Some(dest_bb);
                only_val = Some(val);
            } else {
                if only_dest != Some(dest_bb) {
                    multiple_dest = true;
                }
                // It possible we have same destination, but different value,
                // e.g. default case in switchinst.
                if only_val != Some(val) {
                    multiple_val = true;
                }
            }
            pred_to_dest.push((pred, dest_bb));
        }

        // If all edges were unthreadable, we fail.
        if pred_to_dest.is_empty() {
            return false;
        }

        // If all the predecessors go to a single known successor, we want to
        // fold, not thread. By doing so, we do not need to duplicate the
        // current block and also miss potential opportunities in case we
        // dont/cant duplicate.
        if let (false, Some(Some(only))) = (multiple_dest, only_dest)
            && self.pred_edges(bb).len() == pred_to_dest.len()
        {
            let mut seen_first_branch_to_only_dest = false;
            for succ in self.successor_edges(bb) {
                if succ == only && !seen_first_branch_to_only_dest {
                    // Don't modify the first branch.
                    seen_first_branch_to_only_dest = true;
                } else {
                    // This is unreachable successor.
                    self.remove_predecessor_keeping_phis(succ, bb);
                }
            }

            // Finally update the terminator.
            let term = self.function().terminator(bb).expect("a terminator");
            self.replace_with_jump(term, only);

            // If the condition is now dead due to the removal of the old
            // terminator, erase it.
            if let Some(cond_inst) = self.instruction_of(cond) {
                let result = self.function().instruction(cond_inst).result.expect("a value");
                if self.function().users(result).is_empty()
                    && !may_have_side_effects(
                        &*self.unit.context,
                        self.callees,
                        &self.globals,
                        self.function(),
                        cond_inst,
                    )
                {
                    self.erase(cond_inst);
                } else if let Some(val) = only_val.filter(|_| !multiple_val) {
                    // We can safely replace *some* uses of the CondInst if it
                    // has exactly one value as returned by LVI.
                    self.replace_foldable_uses(cond_inst, Operand::Constant(val), bb);
                }
            }
            return true;
        }

        // Determine which is the most common successor. If we have many inputs
        // and this block is a switch, we want to start by threading the batch
        // that goes to the most popular destination first. If we only know
        // about one threadable destination (the common case) we can avoid
        // this.
        let mut most_popular_dest = only_dest.flatten();
        let mut most_popular_is_undef = matches!(only_dest, Some(None));
        if multiple_dest {
            // Remove any loop headers from the Dest list, threadEdge
            // conservatively won't process them, but we might have other
            // destination that are eligible and we still want to process.
            pred_to_dest.retain(|&(_, dest)| !dest.is_some_and(|dest| self.loop_headers.contains(&dest)));
            if pred_to_dest.is_empty() {
                return false;
            }
            most_popular_dest = self.find_most_popular_dest(bb, &pred_to_dest);
            most_popular_is_undef = most_popular_dest.is_none();
        }

        // Now that we know what the most popular destination is, factor all
        // predecessors that will jump to it into a single predecessor.
        let mut preds_to_factor: Vec<BlockId> = Vec::new();
        for &(pred, dest) in &pred_to_dest {
            if dest == most_popular_dest {
                // This predecessor may be a switch or something else that has
                // multiple edges to the block. Factor each of these edges by
                // listing them according to # occurrences in PredsToFactor.
                for succ in self.successor_edges(pred) {
                    if succ == bb {
                        preds_to_factor.push(pred);
                    }
                }
            }
        }

        // If the threadable edges are branching on an undefined value, we get
        // to pick the destination that these predecessors should get to.
        if most_popular_is_undef {
            let index = self.best_dest_for_jump_on_undef(bb);
            most_popular_dest = Some(self.successor_edges(bb)[index]);
        }

        // Ok, try to thread it!
        self.try_thread_edge(bb, &preds_to_factor, most_popular_dest.expect("a destination"))
    }

    /// `findMostPopularDest`
    /// ([L1483](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1483)):
    /// the destination most predecessors go to, `None` when only undefs do.
    fn find_most_popular_dest(
        &self,
        bb: BlockId,
        pred_to_dest: &[(BlockId, Option<BlockId>)],
    ) -> Option<BlockId> {
        // Populate DestPopularity with the successors in the order they appear
        // in the successor list. We map nullptr to 0 so that we can return
        // nullptr when PredToDestList contains nullptr only.
        let mut popularity: Vec<(Option<BlockId>, u32)> = vec![(None, 0)];
        for succ in self.successor_edges(bb) {
            if !popularity.iter().any(|&(dest, _)| dest == Some(succ)) {
                popularity.push((Some(succ), 0));
            }
        }
        for &(_, dest) in pred_to_dest {
            if let Some(dest) = dest {
                if let Some(entry) = popularity.iter_mut().find(|entry| entry.0 == Some(dest)) {
                    entry.1 += 1;
                } else {
                    popularity.push((Some(dest), 1));
                }
            }
        }
        // `llvm::max_element` with `less_second` keeps the first of equals.
        let mut best = popularity[0];
        for &entry in &popularity[1..] {
            if entry.1 > best.1 {
                best = entry;
            }
        }
        best.0
    }

    /// `tryThreadEdge`
    /// ([L2368](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2368)).
    fn try_thread_edge(
        &mut self,
        bb: BlockId,
        pred_bbs: &[BlockId],
        succ_bb: BlockId,
    ) -> bool {
        // If threading to the same block as we come from, we would infinite
        // loop.
        if succ_bb == bb {
            return false;
        }
        // If threading this would thread across a loop header, don't thread
        // the edge. See the comments above findLoopHeaders for justifications
        // and caveats.
        if self.loop_headers.contains(&bb) || self.loop_headers.contains(&succ_bb) {
            return false;
        }
        let term = self.function().terminator(bb).expect("a terminator");
        let cost = self.duplication_cost(bb, term, self.bb_dup_threshold);
        if cost > self.bb_dup_threshold {
            return false;
        }
        self.thread_edge(bb, pred_bbs, succ_bb);
        true
    }

    /// `threadEdge`
    /// ([L2407](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2407)):
    /// the blocks in `pred_bbs` are factored into one predecessor, which jumps
    /// to a copy of `bb` that goes on to `succ_bb`.
    fn thread_edge(
        &mut self,
        bb: BlockId,
        pred_bbs: &[BlockId],
        succ_bb: BlockId,
    ) {
        // And finally, do it! Start by factoring the predecessors if needed.
        let pred_bb = if pred_bbs.len() == 1 { pred_bbs[0] } else { self.split_block_preds(bb, pred_bbs) };

        self.lvi_thread_edge(pred_bb, bb, succ_bb);

        let name = self.function().block(bb).name.clone().map(|name| format!("{name}.thread"));
        let new_bb = self.unit.function.create_block(name.as_deref());
        self.unit.function.insert_block(new_bb, Some(pred_bb)).expect("a new block");

        // Copy all the instructions from BB to NewBB except the terminator.
        let mut value_mapping = ValueMap::default();
        self.clone_instructions(&mut value_mapping, bb, new_bb, pred_bb);

        // We didn't copy the terminator from BB over to NewBB, because there
        // is now an unconditional jump to SuccBB. Insert the unconditional
        // jump.
        let void = self.function().instruction(self.function().terminator(bb).expect("a terminator")).ty;
        let jump = self
            .unit
            .function
            .create_instruction(Opcode::Br, void, vec![Operand::Block(succ_bb)], Flags::default(), None);
        self.unit.function.insert(jump, Position::End(new_bb)).expect("a placed block");

        // Check to see if SuccBB has PHI nodes. If so, we need to add entries
        // to the PHI nodes for NewBB now.
        self.add_phi_node_entries_for_mapped_block(succ_bb, bb, new_bb, &value_mapping);

        // Update the terminator of PredBB to jump to NewBB instead of BB. This
        // eliminates predecessors from BB, which requires us to simplify any
        // PHI nodes in BB.
        let pred_term = self.function().terminator(pred_bb).expect("a terminator");
        let operands = self.function().instruction(pred_term).operands.clone();
        for (index, operand) in operands.into_iter().enumerate() {
            if operand == Operand::Block(bb) {
                self.remove_predecessor_keeping_phis(bb, pred_bb);
                self.unit.function.set_operand(pred_term, index, Operand::Block(new_bb));
            }
        }

        self.update_ssa(bb, new_bb, &value_mapping);

        // At this point, the IR is fully up to date and consistent. Do a quick
        // scan over the new instructions and zap any that are constants or
        // dead. This frequently happens because of phi translation.
        self.simplify_instructions_in_block(new_bb);
    }

    /// `splitBlockPreds`
    /// ([L2497](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2497)),
    /// with `SplitBlockPredecessors` and the suffix `.thr_comm`.
    fn split_block_preds(
        &mut self,
        bb: BlockId,
        preds: &[BlockId],
    ) -> BlockId {
        crate::edges::split_block_predecessors(self.unit.function, bb, preds)
    }

    /// `cloneInstructions`
    /// ([L2061](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L2061)):
    /// the phis of `bb` as single-input phis from `pred_bb`, then every
    /// non-phi instruction but the terminator, operands remapped.
    fn clone_instructions(
        &mut self,
        value_mapping: &mut ValueMap,
        bb: BlockId,
        new_bb: BlockId,
        pred_bb: BlockId,
    ) {
        let insts: Vec<InstId> = self.function().iter_block(bb).collect();
        let terminator = *insts.last().expect("a terminator");
        // Clone the phi nodes of the source basic block into NewBB. The
        // resulting phi nodes are trivial since NewBB only has one
        // predecessor, but SSAUpdater might need to rewrite the operand of the
        // cloned phi.
        for &inst in &insts {
            if self.opcode(inst) != Opcode::Phi {
                break;
            }
            let incoming = arms(self.function(), inst)
                .into_iter()
                .find(|&(_, from)| from == pred_bb)
                .expect("an input from the predecessor");
            let ty = self.function().instruction(inst).ty;
            let name = self.value_name(inst);
            let new = self
                .unit
                .function
                .create_instruction(Opcode::Phi, ty, from_arms(&[incoming]), Flags::default(), name.as_deref());
            self.unit.function.insert(new, Position::End(new_bb)).expect("a placed block");
            let (from, to) = (self.function().instruction(inst).result, self.function().instruction(new).result);
            value_mapping.values.insert(from.expect("a value"), Operand::Value(to.expect("a value")));
        }

        // Clone the non-phi instructions of the source basic block into NewBB,
        // keeping track of the mapping and using it to remap operands in the
        // cloned instructions.
        for &inst in &insts {
            if inst == terminator || self.opcode(inst) == Opcode::Phi {
                continue;
            }
            let new = self.unit.function.clone_instruction(inst);
            self.unit.function.insert(new, Position::End(new_bb)).expect("a placed block");
            if let (Some(from), Some(to)) =
                (self.function().instruction(inst).result, self.function().instruction(new).result)
            {
                if let Some(name) = self.function().value(from).name.clone() {
                    self.unit.function.set_name(to, &name);
                }
                value_mapping.values.insert(from, Operand::Value(to));
            }
            // Remap operands to patch up intra-block references.
            self.unit.function.remap_instruction(new, value_mapping);
        }
    }

    fn value_name(
        &self,
        inst: InstId,
    ) -> Option<String> {
        let result = self.function().instruction(inst).result?;
        self.function().value(result).name.clone()
    }

    /// `addPHINodeEntriesForMappedBlock`
    /// ([L1898](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1898)).
    fn add_phi_node_entries_for_mapped_block(
        &mut self,
        phi_bb: BlockId,
        old_pred: BlockId,
        new_pred: BlockId,
        value_map: &ValueMap,
    ) {
        for phi in crate::edges::phis(self.function(), phi_bb) {
            let mut incoming = arms(self.function(), phi);
            // Ok, we have a PHI node. Figure out what the incoming value was
            // for the DestBlock.
            let mut iv = incoming
                .iter()
                .find(|&&(_, from)| from == old_pred)
                .map(|&(value, _)| value)
                .expect("an input from the old predecessor");
            // Remap the value if necessary.
            if let Operand::Value(value) = iv
                && let Some(&mapped) = value_map.values.get(&value)
            {
                iv = mapped;
            }
            incoming.push((iv, new_pred));
            self.unit.function.set_operands(phi, from_arms(&incoming));
        }
    }

    /// `updateSSA`
    /// ([L1982](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1982)):
    /// uses of what `bb` defines outside it read the original, the copy, or a
    /// phi of the two.
    fn update_ssa(
        &mut self,
        bb: BlockId,
        new_bb: BlockId,
        value_mapping: &ValueMap,
    ) {
        for inst in self.function().iter_block(bb).collect::<Vec<_>>() {
            let Some(result) = self.function().instruction(inst).result else { continue };
            // Scan all uses of this instruction to see if it is used outside
            // of its block, and if so, record them in UsesToRename.
            let mut uses_to_rename: Vec<Use> = Vec::new();
            let mut lifetime_markers: Vec<InstId> = Vec::new();
            for one in self.function().users(result).iter() {
                let user = self.function().instruction(one.user);
                if self.is_lifetime_marker(one.user) {
                    lifetime_markers.push(one.user);
                } else if user.opcode == Opcode::Phi {
                    let Operand::Block(from) = user.operands[one.index as usize + 1] else {
                        unreachable!("a phi's block operand")
                    };
                    if from == bb {
                        continue;
                    }
                } else if self.function().parent(one.user) == Some(bb) {
                    continue;
                }
                uses_to_rename.push(*one);
            }
            // If there are no uses outside the block, we're done with this
            // instruction.
            if uses_to_rename.is_empty() {
                continue;
            }
            // We found a use of I outside of BB. Rename all uses of I that are
            // outside its block to be uses of the appropriate PHI node etc.
            let ty = self.function().instruction(inst).ty;
            let name = self.value_name(inst);
            let mut updater = SsaUpdater::new(ty, name.as_deref());
            updater.add_available_value(bb, Operand::Value(result));
            updater.add_available_value(new_bb, *value_mapping.values.get(&result).expect("a copy of every value"));
            while let Some(one) = uses_to_rename.pop() {
                updater.rewrite_use(self.unit.context, self.unit.layout, self.unit.function, one);
            }
            // Lifetime markers cannot be rewritten through PHIs. If threading
            // leaves one of them pointing at a PHI, drop the whole set.
            let has_phi_arg = lifetime_markers
                .iter()
                .any(
                    |&marker| !self.function().is_erased(marker)
                        && matches!(
                            self.op(marker, 1),
                            Operand::Value(value) if self.instruction_of(Operand::Value(value)).is_some_and(|def| self.opcode(def) == Opcode::Phi)
                        ),
                );
            if has_phi_arg {
                for marker in lifetime_markers {
                    if !self.function().is_erased(marker) {
                        self.erase(marker);
                    }
                }
            }
        }
    }

    /// `isLifetimeStartOrEnd`.
    fn is_lifetime_marker(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(self.function().instruction(inst).opcode, Opcode::Call(_))
            && llrm_mir::memory::callee(&*self.unit.context, self.function(), inst)
                .and_then(|id| self.globals.get(id.0 as usize))
                .is_some_and(|callee| {
                    callee
                        .name
                        .as_deref()
                        .is_some_and(
                            |name| name.starts_with("llvm.lifetime.start") || name.starts_with("llvm.lifetime.end"),
                        )
                })
    }

    /// `SimplifyInstructionsInBlock`
    /// ([Local.cpp:715](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L715)).
    fn simplify_instructions_in_block(
        &mut self,
        bb: BlockId,
    ) -> bool {
        let mut made_change = false;
        let mut work_list: Vec<InstId> = Vec::new();
        let mut in_work_list: IdSet<InstId> = IdSet::new();
        let insts: Vec<InstId> = self.function().iter_block(bb).collect();
        let terminator = insts.last().copied();
        for inst in insts {
            if Some(inst) == terminator {
                break;
            }
            if self.function().is_erased(inst) || in_work_list.contains(&inst) {
                continue;
            }
            made_change |= self.simplify_and_dce_instruction(inst, &mut work_list, &mut in_work_list);
        }
        while let Some(inst) = work_list.pop() {
            in_work_list.remove(&inst);
            if self.function().is_erased(inst) {
                continue;
            }
            made_change |= self.simplify_and_dce_instruction(inst, &mut work_list, &mut in_work_list);
        }
        made_change
    }

    /// `simplifyAndDCEInstruction`
    /// ([Local.cpp:657](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L657)).
    fn simplify_and_dce_instruction(
        &mut self,
        inst: InstId,
        work_list: &mut Vec<InstId>,
        in_work_list: &mut IdSet<InstId>,
    ) -> bool {
        let dead = |this: &Self, inst: InstId| {
            is_instruction_trivially_dead(&*this.unit.context, this.callees, &this.globals, this.function(), inst)
        };
        if dead(self, inst) {
            // Null out all of the instruction's operands to see if any operand
            // becomes dead as we go.
            let operands = self.function().instruction(inst).operands.clone();
            self.unit.function.set_operands(inst, Vec::new());
            for operand in operands {
                let Some(def) = self.instruction_of(operand) else { continue };
                if def == inst
                    || !self.function().users(self.function().instruction(def).result.expect("a value")).is_empty()
                {
                    continue;
                }
                // If the operand is an instruction that became dead as we
                // nulled out the operand, and if it is 'trivially' dead,
                // delete it in a future loop iteration.
                if dead(self, def) && in_work_list.insert(def) {
                    work_list.push(def);
                }
            }
            self.erase(inst);
            return true;
        }

        if let Some(simple) = simplify_instruction(&*self.unit.context, self.unit.layout, self.function(), inst) {
            let result = self.function().instruction(inst).result.expect("a value");
            // Add the users to the worklist. CAREFUL: an instruction can use
            // itself, in the case of a phi node.
            for one in self.function().users(result).iter() {
                if one.user != inst && in_work_list.insert(one.user) {
                    work_list.push(one.user);
                }
            }
            // Replace the instruction with its simplified value.
            let mut changed = false;
            if !self.function().users(result).is_empty() {
                self.unit.function.replace_value(result, simple);
                changed = true;
            }
            if dead(self, inst) {
                self.erase(inst);
                changed = true;
            }
            return changed;
        }
        false
    }

    /// `maybeMergeBasicBlockIntoOnlyPred`
    /// ([L1919](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L1919)).
    fn maybe_merge_basic_block_into_only_pred(
        &mut self,
        bb: BlockId,
    ) -> bool {
        let Some(single_pred) = self.single_predecessor(bb) else { return false };
        let term = self.function().terminator(single_pred).expect("a terminator");
        let special = matches!(self.opcode(term), Opcode::Invoke(_) | Opcode::Resume);
        if special || self.successor_edges(single_pred).len() != 1 || single_pred == bb {
            return false;
        }
        // MergeBasicBlockIntoOnlyPred may delete SinglePred, we need to avoid
        // deleting a BB pointer from Unreachable.
        if self.unreachable.contains(&single_pred) {
            return false;
        }

        // If SinglePred was a loop header, BB becomes one.
        if self.loop_headers.remove(&single_pred) {
            self.loop_headers.insert(bb);
        }

        self.lvi.erase_block(single_pred);
        self.merge_basic_block_into_only_pred(bb);

        // Now that BB is merged into SinglePred (i.e. SinglePred code followed
        // by BB code within one basic block `BB`), we need to invalidate the
        // LVI information associated with BB, because the LVI information need
        // not be true for all of BB after the merge. Invalidate LVI
        // information for BB if the LVI is not provably true for all of BB.
        if !is_guaranteed_to_transfer_execution_to_successor_block(&self.query(), bb) {
            self.lvi.erase_block(bb);
        }
        true
    }

    /// `MergeBasicBlockIntoOnlyPred`
    /// ([Local.cpp:755](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L755)):
    /// the only predecessor's instructions move to the front of `dest`, which
    /// takes its place.
    fn merge_basic_block_into_only_pred(
        &mut self,
        dest: BlockId,
    ) {
        // If BB has single-entry PHI nodes, fold them.
        while let Some(&phi) = crate::edges::phis(self.function(), dest).first() {
            let result = self.function().instruction(phi).result.expect("a value");
            let mut new_val = self.op(phi, 0);
            // Replace self referencing PHI with poison, it must be dead.
            if new_val == Operand::Value(result) {
                let ty = self.function().instruction(phi).ty;
                new_val = Operand::Constant(self.unit.context.constant(Constant { ty, kind: ConstantKind::Poison }));
            }
            self.unit.function.replace_value(result, new_val);
            self.unit.function.set_operands(phi, Vec::new());
            self.erase(phi);
        }

        let pred = self.single_predecessor(dest).expect("a block with a single predecessor");
        let replace_entry_bb = self.function().entry() == Some(pred);

        // Anything that branched to PredBB now branches to DestBB.
        self.unit.function.replace_block_uses_with(pred, dest);

        // Splice all the instructions from PredBB to DestBB.
        let term = self.function().terminator(pred).expect("a terminator");
        self.unit.function.erase(term).expect("a terminator nothing uses");
        let moved: Vec<InstId> = self.function().iter_block(pred).collect();
        if !moved.is_empty() {
            let first = self.function().block(dest).first().expect("a terminated block");
            self.unit.function.splice(&moved, Position::Before(first)).expect("instructions to move");
        }

        // If the PredBB is the entry block of the function, move DestBB up to
        // become the entry block after we erase PredBB.
        if replace_entry_bb {
            self.unit.function.move_block_after(dest, pred).expect("placed blocks");
        }
        self.unit.function.erase_block(pred).expect("an emptied block nothing names");
    }
}
