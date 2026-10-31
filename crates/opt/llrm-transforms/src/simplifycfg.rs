//! A terminator on a constant becomes a jump, the blocks nothing reaches go,
//! and what that leaves is merged: LLVM's `SimplifyCFGPass` and the
//! `ConstantFoldTerminator`, `removeUnreachableBlocks` and
//! `BasicBlock::removePredecessor` it is made of.
//!
//! # How it works
//! `simplified` is `simplifyFunctionCFGImpl`:
//!
//! 1. `unreachable_removed` walks the body from the entry. Each block it
//!    reaches has its terminator folded first (`terminator_folded`), so only
//!    the successor that still runs is followed; the blocks it did not reach
//!    are deleted, with their edges out (`removePredecessor`) and every use of
//!    what they defined replaced by poison.
//! 2. `iteratively_simplified` visits the blocks in layout order until a visit
//!    changes none (`iterativelySimplifyCFG`). A visit (`block_simplified`,
//!    LLVM's `simplifyOnce`) deletes a block with no predecessor (or only
//!    itself), folds its terminator, removes the duplicate phis of the block,
//!    merges it into its one predecessor and, if it holds only phis and a jump,
//!    removes it (`empty_block_removed`): its predecessors go to its successor,
//!    which takes over its phis.
//! 3. When step 2 changed anything, unreachable blocks are removed again and,
//!    if that removed any, step 2 and the removal alternate until neither does.
//!
//! `terminator_folded` replaces
//!
//! - `br i1 %c, X, X` by `br X`, taking one phi input of `X` away;
//! - `br i1 true/false, A, B` by a jump to the arm that runs;
//! - a `switch` on a constant by a jump to the case it takes (or the default),
//!   a `switch` whose cases all go to one block by a jump, and a case that goes
//!   where the default goes is dropped; a switch left with one case is a `br`
//!   on an `icmp eq`.
//!
//! The condition that nothing reads afterwards is deleted with what only it
//! read. The new `br` carries `!llvm.loop`.
//!
//! ```text
//!   b0: br i1 true, b1, b2         b0: br b1
//!   b1: ...                  ==>   b1: ...        (b2, and its phi inputs
//!   b2: %p = phi [%x, b0], ..                      from b0, are gone)
//! ```
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   br i1 true, label %b1, label %b2
//!
//! b1:
//!   ret i16 %x
//!
//! b2:
//!   ret i16 0
//! }
//! ```
//! After:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   ret i16 %x
//! }
//! ```
//! `b2` is no longer reached, so it goes; `b1` has one predecessor, so it is
//! merged into `b0`.
//!
//! From unit test `test_a_branch_on_true_is_a_jump_and_the_dead_arm_goes` in
//! `simplifycfg_tests.rs`.
//!
//! # LLVM structure
//! [`SimplifyCFGPass`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L272)
//! with the options of the early function pipeline slot
//! ([`PassBuilderPipelines.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L1178):
//! the defaults). `MIRROR` unless a line below says `DEVIATE`.
//!
//! | LLVM | here | |
//! |---|---|---|
//! | [`simplifyFunctionCFGImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L272) | `simplified` | MIRROR |
//! | [`iterativelySimplifyCFG`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L229) | `iteratively_simplified` | MIRROR; D1 |
//! | [`SimplifyCFGOpt::simplifyOnce`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9303) | `block_simplified` | partly: the steps up to `simplifyUncondBranch`'s first half; the rest is not ported yet (D2) |
//! | [`simplifyUncondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8800) | `jump_simplified` | the empty-block half; D2 |
//! | [`TryToSimplifyUncondBranchFromEmptyBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1147) | `empty_block_removed` | MIRROR; D6 |
//! | [`SimplifyCFGOptions::NeedCanonicalLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/SimplifyCFGOptions.h#L29), read at [`simplifyUncondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8812) | `Options::need_canonical_loop` | MIRROR: true by default and at the early slot, cleared at the one after vectorization ([`PassBuilderPipelines.cpp#L1443`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L1443)); D7 |
//! | [`hoistCommonCodeFromSuccessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1873), called from `simplifyCondBranch` ([L9016](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9016)) and `simplifySwitch` ([L8647](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8647)) | `branch_hoisted`, `hoistcommon::hoisted`, `Options::hoist_common_insts` | MIRROR for the calls; see that module |
//! | [`foldTwoEntryPHINode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3834), called from `simplifyOnce` ([L9351](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9351)) under `SpeculateBlocks` | `twoentryphi::folded`, `Options::speculate_blocks` | MIRROR; see that module |
//! | [`simplifyBranchOnICmpChain`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5469), called from `simplifyCondBranch` ([L8985](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8985)) | `block_simplified`, `icmpchain::folded` | MIRROR for the call; see that module |
//! | [`speculativelyExecuteBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3244), called from `simplifyCondBranch` ([L9060](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9060)) | `branch_speculated`, `speculate::executed` | MIRROR for the call; see that module |
//! | [`foldCondBranchOnValueKnownInPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3814), called from `simplifyCondBranch` ([L9068](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9068)) | `block_simplified`, `knownpred::folded` | MIRROR for the call; see that module |
//! | [`tryToSimplifyUncondBranchWithICmpSelectInIt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5328) and its `ICmpInIt` form, called from `simplifyUncondBranch` ([L8827](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8827)) | `block_simplified`, `uncondicmp::folded` | MIRROR for the call; see that module |
//! | [`tailMergeBlocksWithSimilarFunctionTerminators`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L142), called from `simplifyFunctionCFGImpl` after the first `removeUnreachableBlocks` | `tailmerge::merged` | MIRROR; see that module |
//! | [`sinkCommonCodeFromPredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2425) and [`simplifyDuplicatePredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8573), called from `simplifyOnce` ([L9336](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9336)) under `SinkCommonInsts` | `sinkcommon::sunk`, `sinkcommon::duplicate_predecessors_merged`, `Options::sink_common_insts` | MIRROR for the calls; see that module |
//! | [`foldBranchToCommonDest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4231), called from `simplifyCondBranch` ([L9005](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9005)) under `SpeculateBlocks` | `branchtocommon::folded` | MIRROR for the call; see that module |
//! | [`mergeConditionalStores`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4604), called from `simplifyCondBranch` ([L9083](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9083)) and `SimplifyCondBranchToCondBranch` ([L4805](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4805)) | `stores_merged` (the diamond), `condtocond::simplified` (the `MergeCondStores` step) | MIRROR for the calls |
//! | [`SimplifyCondBranchToCondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4773), called from `simplifyCondBranch` ([L9075](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9075)) | `cond_to_cond_simplified`, `condtocond::simplified` | MIRROR; D1-D2 there |
//! | [`removeUndefIntroducingPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9230), called from `simplifyOnce` ([L9327](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9327)) | `undefpred::removed` | MIRROR; D1 there |
//! | [`FindFunctionBackedges`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L236) | `loop_headers` | MIRROR |
//! | [`removeUnreachableBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L2912) and `markAliveBlocks` | `unreachable_removed` | the terminator folding and the deletion; D3 |
//! | [`ConstantFoldTerminator`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L133) | `terminator_folded` | MIRROR; D4 |
//! | [`BasicBlock::removePredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L446) | `Function::predecessor_removed` (`edit.rs`) | MIRROR (one phi input an edge) |
//! | [`DeleteDeadBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L94) | `dead_blocks_deleted` | MIRROR |
//! | [`EliminateDuplicatePHINodes`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1527) | `transform::duplicate_phis_in` | MIRROR |
//! | [`MergeBlockIntoPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L237) | `cfg::merged_into_predecessor` | MIRROR |
//!
//! - D1. `LoopHeaders` is found once per `iteratively_simplified` and keeps the
//!   blocks that go; LLVM's `WeakVH`s drop them. Nothing reads a deleted
//!   block's entry.
//! - D2. Not ported yet, each a later change of this series: the terminator
//!   simplifications `simplifyUncondBranch` beyond the empty block
//!   (`tryToSimplifyUncondBranchWithICmpInIt`, landing pads: none in the MIR),
//!   `simplifyCondBranch`, `simplifySwitch`, `simplifyUnreachable`.
//! - D3. `markAliveBlocks` also turns what follows a call of a `noreturn`
//!   function, `llvm.assume(false)` and a store to `undef` into `unreachable`
//!   (`FoldInstsToUnreachable`): not ported yet. It reads the target's
//!   `NullPointerIsDefined`, which the MIR has no flag for. `invoke`,
//!   `catchswitch` and `indirectbr` do not exist in the MIR.
//! - D4. No branch weights: the MIR has none, so the case weights
//!   `ConstantFoldTerminator` merges and the `!prof` it sets are not kept.
//!   `!make.implicit` is not a MIR kind. There is no `DomTreeUpdater`; as at
//!   LLVM's default (`simplifycfg-require-and-preserve-domtree` off, where the
//!   pass is given no `DominatorTree`) the analyses are invalidated when
//!   anything changed: that is a MIRROR.
//! - D5. Temporary, driven by rows. After a terminator was folded,
//!   `transform::_trivial_phis` runs over the whole body, as `decide` did after
//!   the branches it decided. SimplifyCFG has no such construct: its phi
//!   cleanup is
//!   [`BasicBlock::removePredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L446)
//!   (`hasConstantValue`, L468), of the phis of the successor whose edge went
//!   (here `Function::predecessor_removed`). What `_trivial_phis` does is
//!   InstSimplify's
//!   [`simplifyPHINode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/InstructionSimplify.cpp#L5620),
//!   which InstCombine and EarlyCSE run over the whole function. Whole body,
//!   not the touched successors, because the phis are not there: gvn leaves
//!   `phi [%x, entry], [%self, latch]` in loop headers (premise:
//!   `gvn_tests::test_forwarding_a_loops_cell_to_its_phi_leaves_a_phi_of_itself`),
//!   which `decide` removed whenever it decided something. With the touched
//!   successors only, `bintree` -O3 was 7 instructions longer (twice; LSR
//!   declines the build loop with the phis in), seen failing in
//!   `simplifycfg_tests::test_a_fold_takes_a_phi_of_itself_and_one_value_out_of_the_body`.
//!   It runs once in a pass run that folded, not once per fold; compile
//!   instructions on the largest programs, base to here: `jpeg_single` (C)
//!   -2.0%, `jpeg_fixed` (Nib) -0.3%, `qcport/mdl_ai` -0.4%, `stft_single` (C)
//!   +0.08%, `jpeg_single` (BASIC, flat) +0.06%. Exit: gvn leaves no phi of
//!   itself (#2186) and `hoist` no longer lifts into an unguarded entry
//!   (#2194; a rule in `algebraic` that removed the phis made `x_hanoi2` -O2
//!   22.8% slower through it); then this goes.
//!
//! - D6. A block keeps no `takeName` (the MIR has no rename); an `llvm.loop` on
//!   the removed jump is any `llvm.loop` (the MIR has no debug-loc-only ones);
//!   `undef` is `poison`, the MIR's one. `decide`'s `_threaded` is not removed
//!   yet: it also bypasses jumps in the loop-simplify shapes this keeps; its
//!   removal needs the corpus run. Exit: the CI-gate phase PR removes it.
//! - D7. `needCanonicalLoops` (the table row) keeps only the jump block that
//!   would send two or more edges to a loop header, or leave one. Here a jump
//!   alone in a block that holds a loop's simple form up (its preheader, its
//!   one latch, a dedicated exit: `decide::loop_form_blocks`) is also kept
//!   while `Options::loop_form_kept` is on, because rounds repeat until nothing
//!   changes and `loopsimplify` runs in them: LLVM's passes run once, and
//!   without this the two undid each other each round ("cycle after 3 rounds"
//!   on the `deedlines` demo). The LLVM-order pipeline turns it off at every
//!   slot.
//!
//! # Runs
//! Once per round of the function fixed point, before `decide`: the branches
//! on a literal that `fold` just made are jumps before `decide` asks for the
//! facts of the body. Nothing runs at -O0.
//!
//! # Measured
//! The C corpus (`identical.py`: 395 sources, -O1/-O2/-Os/-Omax, 1,580
//! compiles): `decide` decides 646 branches instead of 3,002; the ones on a
//! literal or a constant `switch` go from 2,464 to 153 (the rest are made by
//! the old `jumpthread`'s own cleanup, after this pass), and it runs 32,640
//! times instead of 33,932. Of the 1,583 objects 40 differ in bytes, 36 only in
//! label numbers; of the rest, `bintree` -O3 is 7 instructions longer (twice)
//! and `switch_empty_cases` 2 shorter (twice). The 140 bench rows at -O2 and
//! -Os of the 14 programs that differ have every counter unchanged.
//!
//! # Keeps
//! Blocks and edges change: every analysis is invalidated when anything
//! changed.
//!
//! # See also
//! [`crate::cfg`], [`crate::decide`].

use llrm_analysis::occurrence::arms;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::footprint::Footprint;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, PreservedAnalyses, Unit};

use crate::twoentryphi::{self, Speculation};
use crate::{
    branchtocommon, condstores, condtocond, hoistcommon, icmpchain, knownpred, sinkcommon, speculate, tailmerge,
    uncondicmp, undefpred,
};
use crate::{cfg, dead, edges, transform};

/// LLVM's `SimplifyCFGPass` at one slot's `SimplifyCFGOptions`
/// (`PassBuilderPipelines.cpp`); by default those of the early slot.
#[derive(Default)]
pub struct SimplifyCfg {
    pub options: Options,
}

impl FunctionPass for SimplifyCfg {
    fn name(&self) -> &'static str {
        "simplifycfg"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("simplifycfg")
    }

    fn reads(&self) -> Footprint {
        Footprint::CONTROL | Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let tti = crate::looprotate::target_tti(outer.target());
        let spec = Speculation { tti: &*tti, layout: unit.layout, outer: &outer };
        let known = KnownLoops {
            info: analyses.get::<llrm_mir::passes::Loops>(&*unit.context, unit.layout, &*unit.function),
            cfg_epoch: unit.function.epochs().cfg,
        };
        if simplified_with_known(&self.options, unit.context, &spec, unit.declared, unit.function, Some(&known)) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// The options of one slot: LLVM's `SimplifyCFGOptions`, as far as ported.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// `needCanonicalLoops`: true by default and at the early slot, false at
    /// the one after vectorization.
    pub need_canonical_loop: bool,
    /// D7: keep the jump alone in a block that holds a loop's simple form up,
    /// which the pin removes (`TryToSimplifyUncondBranchFromEmptyBlock` at
    /// L8820 asks only the loop headers). On for the rounds that repeat
    /// with `loopsimplify`; off in the LLVM-order pipeline, whose passes run
    /// once.
    pub loop_form_kept: bool,
    /// `SpeculateBlocks`: true by default and at every slot.
    pub speculate_blocks: bool,
    /// `HoistCommonInsts`: hoist the code the arms of a branch share even
    /// where the arms differ; off, only arms that are the same entirely.
    pub hoist_common_insts: bool,
    /// `SinkCommonInsts`: sink the code every jump into a block shares.
    pub sink_common_insts: bool,
    /// `ForwardSwitchCondToPhi`.
    pub forward_switch_cond_to_phi: bool,
    /// `ConvertSwitchRangeToICmp`.
    pub convert_switch_range_to_icmp: bool,
    /// `ConvertSwitchToArithmetic`.
    pub convert_switch_to_arithmetic: bool,
    /// `ConvertSwitchToLookupTable`.
    pub convert_switch_to_lookup_table: bool,
    /// `-fno-jump-tables`: what clang writes on every function as the
    /// "no-jump-tables" attribute, which stops a lookup table.
    pub no_jump_tables: bool,
}

impl Options {
    /// The slot at
    /// [L1438](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L1438):
    /// the extra sinking can create larger blocks, so loops need not stay
    /// canonical.
    pub const L1438: Options = Options {
        need_canonical_loop: false,
        loop_form_kept: true,
        speculate_blocks: true,
        hoist_common_insts: true,
        sink_common_insts: true,
        forward_switch_cond_to_phi: true,
        convert_switch_range_to_icmp: true,
        convert_switch_to_arithmetic: true,
        convert_switch_to_lookup_table: true,
        no_jump_tables: false,
    };

    /// [`AArch64PassConfig::addIRPasses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64TargetMachine.cpp#L666):
    /// the pass the target adds after AtomicExpand.
    pub const ATOMIC_TIDY: Options = Options {
        convert_switch_to_arithmetic: false,
        no_jump_tables: false,
        ..Options::L1438
    };

    /// The slot at
    /// [L1682](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L1682),
    /// after LoopSink: the pin sets the switch options,
    /// `speculateUnpredictables` and `hoistLoadsStoresWithCondFaulting`; the
    /// last two are not translated.
    pub const L1682: Options = Options {
        need_canonical_loop: true,
        loop_form_kept: true,
        speculate_blocks: true,
        hoist_common_insts: false,
        sink_common_insts: false,
        forward_switch_cond_to_phi: false,
        convert_switch_range_to_icmp: true,
        convert_switch_to_arithmetic: true,
        convert_switch_to_lookup_table: false,
        no_jump_tables: false,
    };
}

impl Default for Options {
    fn default() -> Self {
        Options {
            need_canonical_loop: true,
            loop_form_kept: true,
            speculate_blocks: true,
            hoist_common_insts: false,
            sink_common_insts: false,
            forward_switch_cond_to_phi: false,
            convert_switch_range_to_icmp: true,
            convert_switch_to_arithmetic: false,
            convert_switch_to_lookup_table: false,
            no_jump_tables: false,
        }
    }
}

/// `function` simplified with the early slot's options: LLVM's
/// `simplifyFunctionCFGImpl`. Whether anything changed.
pub fn simplified(
    context: &mut Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
) -> bool {
    simplified_with(&Options::default(), context, spec, declared, function)
}

/// The loops the manager held when the pass began, and the CFG epoch they were
/// held at: read for the blocks that keep a loop's form while the CFG is as it
/// was (`NeedCanonicalLoop`).
pub struct KnownLoops {
    pub info: std::rc::Rc<llrm_mir::loopinfo::LoopInfo>,
    pub cfg_epoch: u64,
}

/// `simplified` under `options`.
pub fn simplified_with(
    options: &Options,
    context: &mut Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
) -> bool {
    simplified_with_known(options, context, spec, declared, function, None)
}

/// `simplified_with`, given the loops the manager held.
pub fn simplified_with_known(
    options: &Options,
    context: &mut Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
    known: Option<&KnownLoops>,
) -> bool {
    let callees = spec.outer.callees();
    let mut folded = false;
    let mut ever = unreachable_removed(context, callees, function, &mut folded);
    ever |= tailmerge::merged(context, function);
    ever |= iteratively_simplified(options, context, spec, declared, function, &mut folded, known);
    // `iterativelySimplifyCFG` can make a loop dead; the removal and the
    // visits then alternate until neither does anything.
    if !ever {
        return false;
    }
    if unreachable_removed(context, callees, function, &mut folded) {
        while iteratively_simplified(options, context, spec, declared, function, &mut folded, known)
            | unreachable_removed(context, callees, function, &mut folded)
        {}
    }
    if folded {
        // D5.
        transform::_trivial_phis(function).expect("a phi nothing reads is erased");
    }
    true
}

/// Every block visited, in layout order, until a round changes none:
/// `iterativelySimplifyCFG`.
fn iteratively_simplified(
    options: &Options,
    context: &mut Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
    folded: &mut bool,
    known: Option<&KnownLoops>,
) -> bool {
    let (mut changed, mut local) = (false, true);
    let headers = loop_headers(function);
    // D7: with the option, a function with a loop only; found at the first
    // jump alone in a block, which most functions never have.
    let form = LoopForm {
        wanted: !headers.is_empty() && options.need_canonical_loop && options.loop_form_kept,
        known,
        cell: std::cell::OnceCell::new(),
    };
    let headers = if options.need_canonical_loop { headers } else { Default::default() };
    while local {
        local = false;
        for block in function.layout().to_vec() {
            // `SimplifyCFGOpt::run`: a visit that asked to be repeated is.
            let mut resimplify = true;
            while resimplify && function.is_placed(block) {
                resimplify = false;
                if block_simplified(
                    options,
                    context,
                    spec,
                    declared,
                    function,
                    block,
                    folded,
                    &headers,
                    &form,
                    &mut resimplify,
                ) {
                    local = true;
                }
            }
        }
        changed |= local;
    }
    changed
}

/// The blocks that hold a loop's simple form up (D7), found when first asked.
struct LoopForm<'a> {
    wanted: bool,
    known: Option<&'a KnownLoops>,
    cell: std::cell::OnceCell<std::collections::BTreeSet<i64>>,
}

impl LoopForm<'_> {
    fn contains(
        &self,
        function: &Function,
        block: i64,
    ) -> bool {
        self.wanted
            && self
                .cell
                .get_or_init(|| {
                    // The manager's loops, where the CFG is as they were found.
                    let held = self.known.filter(|one| one.cfg_epoch == function.epochs().cfg);
                    crate::decide::loop_form_blocks(
                        function,
                        held.map(|one| llrm_analysis::cfg::natural(function, &one.info)),
                    )
                })
                .contains(&block)
    }
}

/// `block` holds phis and a jump only, and is not the entry: the blocks
/// `jump_simplified` can remove.
fn only_phis_and_jump(
    function: &Function,
    block: BlockId,
) -> bool {
    function.entry() != Some(block) && function.block(block).len() == edges::phis(function, block).len() + 1
}

/// One visit of `block`: LLVM's `SimplifyCFGOpt::simplifyOnce`, as far as it
/// is ported (D2).
fn block_simplified(
    options: &Options,
    context: &mut Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
    block: BlockId,
    folded: &mut bool,
    headers: &IdSet<BlockId>,
    form: &LoopForm,
    resimplify: &mut bool,
) -> bool {
    let callees = spec.outer.callees();
    // No predecessor but not the entry, or only itself: unreachable.
    let reach = reached_from(function, block);
    if (reach == Reach::Nothing && function.entry() != Some(block)) || reach == Reach::Itself {
        dead_blocks_deleted(context, function, &[block]);
        return true;
    }
    let mut changed = terminator_folded(context, callees, function, block, true);
    *folded |= changed;
    changed |= transform::duplicate_phis_in(function, block).expect("a phi nothing reads is erased");
    if undefpred::removed(context, spec, function, block) {
        return true;
    }
    changed |= cfg::merged_into_predecessor(function, block);
    if options.sink_common_insts && function.is_placed(block) {
        if sinkcommon::sunk(context, spec, function, block) {
            return true;
        }
        if !(options.need_canonical_loop && headers.contains(&block))
            && sinkcommon::duplicate_predecessors_merged(function, block)
        {
            return true;
        }
    }
    if options.speculate_blocks && function.is_placed(block) && twoentryphi::folded(context, spec, function, block) {
        return true;
    }
    // `simplifyOnce`'s switch on the terminator (SimplifyCFG.cpp#L9368): each
    // arm runs only its own kind's steps, in the pin's order.
    let acted = match terminator_kind(function, block) {
        // Each fold of `simplifyCondBranch` returns `requestResimplify()`.
        Terminator::Conditional => {
            let acted = cond_branch_simplified(options, context, spec, function, block, folded, headers);
            *resimplify |= acted;
            acted
        }
        // `simplifySwitch`: the shared hoist, then the switch steps.
        Terminator::Switch => {
            function.is_placed(block)
                && (branch_hoisted(options, context, spec, function, block)
                    || crate::switchopt::simplified(options, context, spec, declared, function, block))
        }
        // `simplifyUncondBranch`, then the rest of it where the empty block
        // did not go.
        Terminator::Jump => {
            function.is_placed(block)
                && ((only_phis_and_jump(function, block)
                    && !form.contains(function, llrm_analysis::cfg::id(block))
                    && jump_simplified(context, function, block, headers))
                    || (function.is_placed(block) && uncondicmp::folded(context, spec.layout, function, block)))
        }
        Terminator::Other => false,
    };
    changed | acted
}

#[cfg(test)]
thread_local! {
    /// The predecessor lists `predecessor_list` built.
    pub(crate) static PREDECESSOR_LISTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// The blocks `cond_branch_simplified` visited.
    pub(crate) static CONDITIONAL_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The kind of terminator ending a block, as `simplifyOnce` dispatches on it.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Terminator {
    Conditional,
    Switch,
    Jump,
    Other,
}

fn terminator_kind(
    function: &Function,
    block: BlockId,
) -> Terminator {
    let Some(last) = function.terminator(block) else { return Terminator::Other };
    let one = function.instruction(last);
    match (&one.opcode, one.operands.len()) {
        (Opcode::Br, 3) => Terminator::Conditional,
        (Opcode::Br, 1) => Terminator::Jump,
        (Opcode::Switch, _) => Terminator::Switch,
        _ => Terminator::Other,
    }
}

/// `simplifyCondBranch`: the value comparisons (SimplifyCFG.cpp:8963), the
/// compare chain, and the folds that follow, in the pin's order.
fn cond_branch_simplified(
    options: &Options,
    context: &mut Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    folded: &mut bool,
    headers: &IdSet<BlockId>,
) -> bool {
    #[cfg(test)]
    CONDITIONAL_VISITS.with(|one| one.set(one.get() + 1));
    if function.is_placed(block) && crate::switchvalue::folded(options, context, spec, function, block) {
        return true;
    }
    if function.is_placed(block) && icmpchain::folded(context, spec, function, block) {
        return true;
    }
    if function.is_placed(block) && dominating_condition_folded(context, spec, function, block) {
        return true;
    }
    if options.speculate_blocks && function.is_placed(block) && branchtocommon::folded(context, spec, function, block) {
        return true;
    }
    if function.is_placed(block) && branch_hoisted(options, context, spec, function, block) {
        return true;
    }
    if options.speculate_blocks && function.is_placed(block) && branch_speculated(context, spec, function, block) {
        return true;
    }
    if function.is_placed(block) && knownpred::folded(context, spec, function, block, headers) {
        return true;
    }
    if function.is_placed(block) && cond_to_cond_simplified(context, spec, function, block) {
        // The block it left with one predecessor holds phis of one input (D5).
        *folded = true;
        return true;
    }
    function.is_placed(block) && stores_merged(context, spec, function, block)
}

/// The branch ending `block` made one on a constant when the condition of its
/// dominating predecessor implies it: `simplifyCondBranch`'s
/// `isImpliedByDomCondition` step, before `foldBranchToCommonDest`. The
/// constant branch is folded by the next visit.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8989>
fn dominating_condition_folded(
    context: &mut Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let &[condition, Operand::Block(..), Operand::Block(..)] = &function.instruction(last).operands[..] else {
        return false;
    };
    if function.instruction(last).opcode != Opcode::Br || integer(context, condition).is_some() {
        return false;
    }
    let query = llrm_mir::valuetracking::Query::bare(context, function, spec.layout);
    let Some(implied) = llrm_mir::valuetracking::context::is_implied_by_dom_condition_cond(&query, condition, last)
    else {
        return false;
    };
    let i1 = context.types.int(1);
    let known = Operand::Constant(context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(implied)) }));
    function.set_operand(last, 0, known);
    condition_deleted(context, spec.outer.callees(), function, condition, true);
    true
}

/// The code the two arms of the branch ending `block` share, run before it:
/// `simplifyCondBranch`'s call of `hoistCommonCodeFromSuccessors`, where both
/// arms have no other predecessor.
fn branch_hoisted(
    options: &Options,
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    if function.instruction(last).opcode == Opcode::Switch {
        return hoistcommon::hoisted(context, spec, function, last, !options.hoist_common_insts);
    }
    let &[_, Operand::Block(first), Operand::Block(second)] = &function.instruction(last).operands[..] else {
        return false;
    };
    function.sole_predecessor(first).is_some()
        && function.sole_predecessor(second).is_some()
        && hoistcommon::hoisted(context, spec, function, last, !options.hoist_common_insts)
}

/// What `predecessors(block)` is, to the one question asked of it at the head
/// of a visit: no block names it, only itself does, or another does.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Reach {
    Nothing,
    Itself,
    Other,
}

/// `predecessors(block)` as a `Reach`, without the list: the visit of every
/// block began by building it (9,955 builds, 1.7 M Ir in d_faces -O2).
fn reached_from(
    function: &Function,
    block: BlockId,
) -> Reach {
    let mut found = None;
    for one in function.block_users(block) {
        let user = one.user;
        if !function.instruction(user).opcode.is_terminator() {
            continue;
        }
        let Some(parent) = function.parent(user) else { continue };
        match found {
            None => found = Some(parent),
            Some(earlier) if earlier == parent => {}
            Some(_) => return Reach::Other,
        }
    }
    match found {
        None => Reach::Nothing,
        Some(parent) if parent == block => Reach::Itself,
        Some(_) => Reach::Other,
    }
}

/// `predecessors(block)`, counted in tests.
fn predecessor_list(
    function: &Function,
    block: BlockId,
) -> llrm_mir::blocklist::BlockList {
    #[cfg(test)]
    PREDECESSOR_LISTS.with(|count| count.set(count.get() + 1));
    function.predecessors(block)
}

/// The block `block`'s predecessors all have as their only predecessor:
/// `allPredecessorsComeFromSameSource`.
fn all_predecessors_come_from_same_source(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    // One predecessor, the common case: no list.
    if let Some(only) = function.sole_predecessor(block) {
        return function.sole_predecessor(only);
    }
    let mut source = None;
    for &pred in predecessor_list(function, block).iter() {
        let only = function.sole_predecessor(pred)?;
        if source.is_some_and(|one| one != only) {
            return None;
        }
        source = Some(only);
    }
    source
}

/// The stores to one address in the two diamonds ending at `block` and
/// before it, one after them: `simplifyCondBranch`'s diamond patterns.
fn stores_merged(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(qbi) = function.terminator(block).filter(|&last| {
        matches!(
            function.instruction(last).operands[..],
            [_, Operand::Block(_), Operand::Block(_)]
        )
            && function.instruction(last).opcode == Opcode::Br
    }) else {
        return false;
    };
    let conditional = |function: &Function, last: InstId| {
        matches!(
            function.instruction(last).operands[..],
            [_, Operand::Block(_), Operand::Block(_)]
        )
            && function.instruction(last).opcode == Opcode::Br
            && last != qbi
    };
    // The diamond patterns: the predecessors all come from one block.
    let Some(previous) = all_predecessors_come_from_same_source(function, block) else { return false };
    let Some(pbi) = function.terminator(previous).filter(|&last| conditional(function, last)) else {
        return false;
    };
    condstores::merged(context, spec, function, pbi, qbi)
}

/// `simplifyCondBranch`'s loop over the predecessors that end in a
/// conditional branch: `SimplifyCondBranchToCondBranch` with each.
fn cond_to_cond_simplified(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(bi) = function.terminator(block).filter(|&last| {
        function.instruction(last).opcode == Opcode::Br && function.instruction(last).operands.len() == 3
    }) else {
        return false;
    };
    for pred in predecessor_list(function, block) {
        if let Some(pbi) = function.terminator(pred).filter(|&last| {
            last != bi
                && function.instruction(last).opcode == Opcode::Br
                && function.instruction(last).operands.len() == 3
        }) && condtocond::simplified(context, spec, function, pbi, bi)
        {
            return true;
        }
    }
    false
}

/// The one-armed `if` ending `block` run before its branch: the speculation
/// of LLVM's `simplifyCondBranch`.
fn branch_speculated(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let &[_, Operand::Block(first), Operand::Block(second)] = &function.instruction(last).operands[..] else {
        return false;
    };
    let alone = |one: BlockId| function.sole_predecessor(one).is_some();
    let jumps_to = |one: BlockId, target: BlockId| {
        function.terminator(one).is_some_and(|jump| function.instruction(jump).operands[..] == [Operand::Block(target)])
    };
    let then = if alone(first) {
        (!alone(second) && jumps_to(first, second)).then_some(first)
    } else if alone(second) {
        jumps_to(second, first).then_some(second)
    } else {
        None
    };
    then.is_some_and(|then| speculate::executed(context, spec, function, block, then))
}

/// The back edges' targets, by one walk from the entry: LLVM's
/// `FindFunctionBackedges`, whose targets are `LoopHeaders`.
pub(crate) fn loop_headers(function: &Function) -> IdSet<BlockId> {
    let mut headers = IdSet::new();
    let Some(entry) = function.entry() else { return headers };
    // 0 unseen, 1 on the walk's path, 2 done.
    let mut state: IdMap<BlockId, u8> = IdMap::new();
    state.insert(entry, 1);
    let mut path = vec![(entry, function.successors(entry), 0usize)];
    while let Some((block, successors, next)) = path.last_mut() {
        let Some(&successor) = successors.get(*next) else {
            state.insert(*block, 2);
            path.pop();
            continue;
        };
        *next += 1;
        match state.get(&successor) {
            Some(1) => {
                headers.insert(successor);
            }
            Some(_) => {}
            None => {
                state.insert(successor, 1);
                path.push((successor, function.successors(successor), 0));
            }
        }
    }
    headers
}

/// `block` holds only phis and a jump: its jump removed, if `block` is not a
/// loop's header or entry in the way (`NeedCanonicalLoop`): the first part of
/// LLVM's `simplifyUncondBranch`.
fn jump_simplified(
    context: &Context,
    function: &mut Function,
    block: BlockId,
    loop_headers: &IdSet<BlockId>,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let (Opcode::Br, &[Operand::Block(successor)]) =
        (&function.instruction(last).opcode, &function.instruction(last).operands[..])
    else {
        return false;
    };
    if function.entry() == Some(block) || function.block(block).len() != edges::phis(function, block).len() + 1 {
        return false;
    }
    let canonical = edge_sources(function, block).len() >= 2
        && !loop_headers.is_empty()
        && (loop_headers.contains(&block) || loop_headers.contains(&successor));
    !canonical && empty_block_removed(context, function, block, successor)
}

/// The block of each edge into `block`, one per edge.
fn edge_sources(
    function: &Function,
    block: BlockId,
) -> Vec<BlockId> {
    function
        .block_users(block)
        .iter()
        .filter(|one| function.instruction(one.user).opcode.is_terminator())
        .filter_map(|one| function.parent(one.user))
        .collect()
}

/// The value `phi` takes from `from`.
fn value_from(
    function: &Function,
    phi: InstId,
    from: BlockId,
) -> Option<Operand> {
    arms(function, phi).into_iter().find(|&(_, at)| at == from).map(|(value, _)| value)
}

/// `isa<UndefValue>`: poison is one.
fn is_undef(
    context: &Context,
    value: Operand,
) -> bool {
    matches!(
        value,
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Poison | ConstantKind::Undef)
    )
}

/// `value` is a phi of `block`.
fn phi_of(
    function: &Function,
    value: Operand,
    block: BlockId,
) -> Option<InstId> {
    let Operand::Value(id) = value else { return None };
    let ValueDef::Instruction(inst) = function.value(id).def else { return None };
    (function.instruction(inst).opcode == Opcode::Phi && function.parent(inst) == Some(block)).then_some(inst)
}

/// `block`, holding only phis and a jump to `successor`, removed (or only its
/// predecessors sent on): LLVM's `TryToSimplifyUncondBranchFromEmptyBlock`.
/// MIRROR, except that a block keeps its name, and a loop's metadata check
/// is on any `llvm.loop`.
pub fn empty_block_removed(
    context: &Context,
    function: &mut Function,
    block: BlockId,
    successor: BlockId,
) -> bool {
    if block == successor {
        return false;
    }
    let sources = edge_sources(function, block);
    let predecessors: IdSet<BlockId> = sources.iter().copied().collect();
    let killable = predecessors_propagate(context, function, block, successor, &predecessors);
    let mut common = None;
    let mergeable = killable || redirectable(function, block, successor, &predecessors, &mut common);
    if (!killable && !mergeable) || too_many_phi_entries(function, block, successor, sources.len()) {
        return false;
    }
    let alone = function.sole_predecessor(successor).is_some();
    // A phi of `block` read elsewhere than by the successor's phis for this
    // edge keeps `block` (it dominates the successor).
    if !alone {
        for phi in edges::phis(function, block) {
            let result = function.instruction(phi).result.expect("a phi's value");
            if function.users(result).iter().any(|one| {
                let user = function.instruction(one.user);
                user.opcode != Opcode::Phi || user.operands[one.index as usize + 1] != Operand::Block(block)
            }) {
                return false;
            }
        }
    }
    let last = function.terminator(block).expect("a jump");
    let loop_node =
        function.instruction(last).metadata.iter().find(|(kind, _)| kind == "llvm.loop").map(|&(_, node)| node);
    if loop_node.is_some()
        && predecessors.iter().any(|one| {
            function
                .terminator(one)
                .is_some_and(|term| function.instruction(term).metadata.iter().any(|(kind, _)| kind == "llvm.loop"))
        })
    {
        return false;
    }

    for phi in edges::phis(function, successor) {
        values_redirected(context, function, block, &sources, phi, common);
    }

    if alone {
        function.erase(last).expect("a jump defines nothing");
        let anchor = function.iter_block(successor).find(|&one| function.instruction(one).opcode != Opcode::Phi);
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            function.move_to(inst, anchor.map_or(Position::End(successor), Position::Before)).expect("a placed phi");
        }
    } else {
        for phi in edges::phis(function, block) {
            function.set_operands(phi, Vec::new());
            function.erase(phi).expect("no use of an emptied phi is left");
        }
        if let Some(node) = loop_node {
            for one in predecessors.iter() {
                let term = function.terminator(one).expect("a predecessor's terminator");
                function.annotate(term, "llvm.loop", node);
            }
        }
    }

    if killable {
        function.replace_block_uses_with(block, successor);
        if let Some(last) = function.terminator(block) {
            function.erase(last).expect("a jump defines nothing");
        }
        function.erase_block(block).expect("a block nothing names");
    } else {
        // Everything but the common predecessor now goes to the successor.
        for one in function.block_users(block).snapshot() {
            let parent = function.parent(one.user);
            if function.instruction(one.user).opcode.is_terminator()
                && parent.is_some_and(|at| at != common.expect("a common predecessor") && predecessors.contains(&at))
            {
                function.set_operand(one.user, one.index as usize, Operand::Block(successor));
            }
        }
    }
    true
}

/// Whether the successor's phis and `block`'s agree where `block` and the
/// successor share a predecessor: `CanPropagatePredecessorsForPHIs`.
fn predecessors_propagate(
    context: &Context,
    function: &Function,
    block: BlockId,
    successor: BlockId,
    predecessors: &IdSet<BlockId>,
) -> bool {
    if function.sole_predecessor(successor).is_some() {
        return true;
    }
    let merge =
        |first: Operand, second: Operand| first == second || is_undef(context, first) || is_undef(context, second);
    for phi in edges::phis(function, successor) {
        let from_block = value_from(function, phi, block).expect("a phi entry for each edge");
        let inner = phi_of(function, from_block, block);
        for (value, at) in arms(function, phi) {
            if !predecessors.contains(&at) {
                continue;
            }
            let own = match inner {
                Some(inner) => value_from(function, inner, at).expect("an entry for each edge"),
                None => from_block,
            };
            if !merge(own, value) {
                return false;
            }
        }
    }
    true
}

/// The one predecessor `block` and its successor share, when `block` cannot
/// go but its other predecessors can be sent on:
/// `CanRedirectPredsOfEmptyBBToSucc`.
fn redirectable(
    function: &Function,
    block: BlockId,
    successor: BlockId,
    predecessors: &IdSet<BlockId>,
    common: &mut Option<BlockId>,
) -> bool {
    if edges::phis(function, block).is_empty()
        || edges::phis(function, successor).is_empty()
        || edge_sources(function, block).len() < 2
    {
        return false;
    }
    for one in function.predecessors(successor) {
        if predecessors.contains(&one) {
            if common.is_some() {
                return false;
            }
            *common = Some(one);
        }
    }
    true
}

/// Removing `block` would add more than 1000 entries to the successor's
/// phis: `introduceTooManyPhiEntries`.
fn too_many_phi_entries(
    function: &Function,
    block: BlockId,
    successor: BlockId,
    edges_in: usize,
) -> bool {
    if edges_in == 1 {
        return false;
    }
    let changed = edges::phis(function, successor)
        .into_iter()
        .filter(|&phi| phi_of(function, value_from(function, phi, block).expect("an entry"), block).is_none())
        .count();
    (edges_in - 1) * changed > 1000
}

/// `phi` of the successor takes, from each edge of `block`'s predecessors,
/// the value it took from `block`: `redirectValuesFromPredecessorsToPhi`
/// with its undef rules (poison is MIR's only undef).
fn values_redirected(
    context: &Context,
    function: &mut Function,
    block: BlockId,
    sources: &[BlockId],
    phi: InstId,
    common: Option<BlockId>,
) {
    let old = value_from(function, phi, block).expect("a phi entry for each edge");
    let mut operands = function.instruction(phi).operands.clone();
    let at = operands.chunks(2).position(|pair| pair[1] == Operand::Block(block)).expect("the entry");
    operands.drain(2 * at..2 * at + 2);

    let mut known: IdMap<BlockId, Option<Operand>> = sources.iter().map(|&one| (one, None)).collect();
    for pair in operands.chunks(2) {
        if let (value, Operand::Block(from)) = (pair[0], pair[1])
            && !is_undef(context, value)
            && let Some(slot) = known.get_mut(&from)
        {
            *slot = Some(value);
        }
    }
    let mut select = |value: Operand, from: BlockId| -> Operand {
        if !is_undef(context, value) {
            known.insert(from, Some(value));
            return value;
        }
        known.get(&from).copied().flatten().unwrap_or(value)
    };
    let mut added = Vec::new();
    if let Some(inner) = phi_of(function, old, block) {
        for (value, from) in arms(function, inner) {
            if Some(from) != common {
                added.push((select(value, from), from));
            }
        }
        if let Some(common) = common {
            added.push((value_from(function, inner, common).expect("an entry for the common predecessor"), block));
        }
    } else {
        for &from in sources {
            if Some(from) != common {
                added.push((select(old, from), from));
            }
        }
        if common.is_some() {
            added.push((old, block));
        }
    }
    for (value, from) in added {
        operands.extend([value, Operand::Block(from)]);
    }
    // An undef input takes the value another input gives that predecessor
    // (`replaceUndefValuesInPhi`); those left with none must match.
    let mut true_undef = Vec::new();
    for (at, pair) in operands.chunks_mut(2).enumerate() {
        if let (value, Operand::Block(from)) = (pair[0], pair[1])
            && is_undef(context, value)
            && known.contains_key(&from)
        {
            match known.get(&from) {
                Some(Some(one)) => pair[0] = *one,
                _ => true_undef.push(at),
            }
        }
    }
    // Both undef and poison incoming for one block is invalid: all become
    // undef.
    let poisons = true_undef
        .iter()
        .filter(|&&at| {
            matches!(
                operands[2 * at],
                Operand::Constant(id) if context.get(id).kind == ConstantKind::Poison
            )
        })
        .count();
    if poisons != 0 && poisons != true_undef.len() {
        let ty = function.instruction(phi).ty;
        let undef = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Undef }));
        for at in true_undef {
            operands[2 * at] = undef;
        }
    }
    function.set_operands(phi, operands);
}

/// The blocks the entry does not reach, deleted, after each block it reaches
/// has had its terminator folded (so a successor that no longer runs is not
/// followed): `removeUnreachableBlocks` over `markAliveBlocks`.
pub fn unreachable_removed(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    folded: &mut bool,
) -> bool {
    let Some(entry) = function.entry() else { return false };
    let mut reached = IdSet::new();
    reached.insert(entry);
    let (mut pending, mut changed) = (vec![entry], false);
    while let Some(block) = pending.pop() {
        let one = terminator_folded(context, callees, function, block, true);
        *folded |= one;
        changed |= one;
        for successor in function.successors(block) {
            if reached.insert(successor) {
                pending.push(successor);
            }
        }
    }
    let dead: Vec<BlockId> = function.layout().iter().copied().filter(|block| !reached.contains(block)).collect();
    if dead.is_empty() {
        return changed;
    }
    dead_blocks_deleted(context, function, &dead);
    true
}

/// `blocks` deleted together: each one's edges out told (`removePredecessor`,
/// keeping a one-input phi, since the block it names may go as well), what it
/// defined replaced by poison, its instructions and then the blocks erased.
/// LLVM's `DeleteDeadBlocks`.
pub fn dead_blocks_deleted(
    context: &Context,
    function: &mut Function,
    blocks: &[BlockId],
) {
    for &block in blocks {
        let operands = function.terminator(block).map(|last| function.instruction(last).operands.clone());
        for successor in edges_out_of(&operands.unwrap_or_default()) {
            function.predecessor_removed(context, successor, block, true);
        }
        for inst in function.iter_block(block).collect::<Vec<_>>().into_iter().rev() {
            if let Some(result) = function.instruction(inst).result {
                let poison = context.constant(Constant { ty: function.value(result).ty, kind: ConstantKind::Poison });
                function.replace_all_uses_with(result, Operand::Constant(poison));
            }
            function.erase(inst).expect("its uses were replaced");
        }
    }
    for &block in blocks {
        function.erase_block(block).expect("an emptied block nothing names");
    }
}

/// The integer `operand` is, if it is a constant one.
pub(crate) fn integer(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

/// Whether `block` holds nothing but an `unreachable`: LLVM's
/// `SwitchInst::defaultDestUnreachable`.
pub(crate) fn only_unreachable(
    function: &Function,
    block: BlockId,
) -> bool {
    function
        .iter_block(block)
        .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
        .is_some_and(|inst| function.instruction(inst).opcode == Opcode::Unreachable)
}

/// `last`, a terminator, replaced by `opcode` over `operands`; the new one
/// carries `last`'s `!llvm.loop`. Its id.
fn terminator_replaced(
    function: &mut Function,
    last: InstId,
    opcode: Opcode,
    operands: Vec<Operand>,
) -> InstId {
    let void = function.instruction(last).ty;
    let new = function.create_instruction(opcode, void, operands, Flags::default(), None);
    function.insert(new, Position::Before(last)).expect("a placed terminator");
    for (kind, node) in function.instruction(last).metadata.clone() {
        if kind == "llvm.loop" {
            function.annotate(new, &kind, node);
        }
    }
    new
}

/// `block`'s terminator folded if it is on a constant, or sends both arms the
/// same way: LLVM's `ConstantFoldTerminator`. `delete_dead_conditions`: the
/// condition nothing reads afterwards goes, with what only it read.
pub fn terminator_folded(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    block: BlockId,
    delete_dead_conditions: bool,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let (opcode, operands) = (function.instruction(last).opcode.clone(), function.instruction(last).operands.clone());
    match (opcode, &operands[..]) {
        (Opcode::Br, &[condition, Operand::Block(first), Operand::Block(second)]) => {
            let destination = if first == second {
                // `br i1 %c, X, X`: one of the two edges goes.
                function.predecessor_removed(context, first, block, false);
                first
            } else if let Some(value) = integer(context, condition) {
                let (kept, gone) = if value != 0 { (first, second) } else { (second, first) };
                function.predecessor_removed(context, gone, block, false);
                kept
            } else {
                return false;
            };
            terminator_replaced(function, last, Opcode::Br, vec![Operand::Block(destination)]);
            function.erase(last).expect("a terminator defines nothing");
            condition_deleted(context, callees, function, condition, delete_dead_conditions);
            true
        }
        (Opcode::Switch, &[condition, Operand::Block(default), ..]) => {
            switch_folded(context, callees, function, block, last, condition, default, delete_dead_conditions)
        }
        _ => false,
    }
}

/// `condition`, nothing reading it any more, deleted with what only it read.
pub(crate) fn condition_deleted(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    condition: Operand,
    wanted: bool,
) {
    if let (true, Operand::Value(value)) = (wanted, condition) {
        dead::dead_from(context, callees, function, [value]);
    }
}

/// The `switch` `last` of `block` folded: `ConstantFoldTerminator`'s switch
/// case.
#[allow(clippy::too_many_arguments)]
fn switch_folded(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    block: BlockId,
    last: InstId,
    condition: Operand,
    default: BlockId,
    delete_dead_conditions: bool,
) -> bool {
    let mut cases: Vec<(Operand, BlockId)> = function.instruction(last).operands[2..]
        .chunks(2)
        .map(|pair| match pair {
            [case, Operand::Block(target)] => (*case, *target),
            _ => unreachable!("a switch case is a value and a block"),
        })
        .collect();
    let value = integer(context, condition);
    // A default nothing runs is not a way out when a case is.
    let mut only = Some(match cases.first() {
        Some(&(_, first)) if only_unreachable(function, default) => first,
        _ => default,
    });
    let mut changed = false;
    let mut at = 0;
    while at < cases.len() {
        let (case, target) = cases[at];
        if value.is_some() && integer(context, case) == value {
            only = Some(target);
            break;
        }
        // A case that goes where the default goes is no explicit compare.
        if target == default {
            function.predecessor_removed(context, default, block, false);
            cases.remove(at);
            changed = true;
            continue;
        }
        if only != Some(target) {
            only = None;
        }
        at += 1;
    }
    if value.is_some() && only.is_none() {
        only = Some(default);
    }
    let rebuilt = |cases: &[(Operand, BlockId)]| {
        let mut operands = vec![condition, Operand::Block(default)];
        operands.extend(cases.iter().flat_map(|&(case, target)| [case, Operand::Block(target)]));
        operands
    };
    if let Some(destination) = only {
        // The first edge to `destination` stays the jump; every other edge of
        // the switch goes.
        let mut kept = false;
        for successor in edges_out_of(&rebuilt(&cases)) {
            if successor == destination && !kept {
                kept = true;
            } else {
                function.predecessor_removed(context, successor, block, false);
            }
        }
        terminator_replaced(function, last, Opcode::Br, vec![Operand::Block(destination)]);
        function.erase(last).expect("a terminator defines nothing");
        condition_deleted(context, callees, function, condition, delete_dead_conditions);
        return true;
    }
    if let [(case, target)] = cases[..] {
        // One case left: a `br` on an equality.
        let i1 = context.types.int(1);
        let compare = function.create_instruction(
            Opcode::ICmp(IntPredicate::Eq),
            i1,
            vec![condition, case],
            Flags::default(),
            Some("cond"),
        );
        function.insert(compare, Position::Before(last)).expect("a placed terminator");
        let result = function.instruction(compare).result.expect("a compare's value");
        terminator_replaced(
            function,
            last,
            Opcode::Br,
            vec![Operand::Value(result), Operand::Block(target), Operand::Block(default)],
        );
        function.erase(last).expect("a terminator defines nothing");
        return true;
    }
    if changed {
        function.set_operands(last, rebuilt(&cases));
    }
    changed
}

/// The blocks a terminator's `operands` name, once for each edge.
fn edges_out_of(operands: &[Operand]) -> Vec<BlockId> {
    operands
        .iter()
        .filter_map(|operand| if let Operand::Block(target) = operand { Some(*target) } else { None })
        .collect()
}

#[cfg(test)]
#[path = "simplifycfg_tests.rs"]
mod tests;
