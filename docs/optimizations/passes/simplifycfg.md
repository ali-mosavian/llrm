> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/simplifycfg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/simplifycfg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/simplifycfg.rs sha256:ea9eb328fab50d729bbeffeb0188f5bb65719b87cd9a11ee2aa5019b550e3db5 -->

# simplifycfg

A terminator on a constant becomes a jump, the blocks nothing reaches go,
and what that leaves is merged: LLVM's `SimplifyCFGPass` and the
`ConstantFoldTerminator`, `removeUnreachableBlocks` and
`BasicBlock::removePredecessor` it is made of.

## How it works
`simplified` is `simplifyFunctionCFGImpl`:

1. `unreachable_removed` walks the body from the entry. Each block it
   reaches has its terminator folded first (`terminator_folded`), so only
   the successor that still runs is followed; the blocks it did not reach
   are deleted, with their edges out (`removePredecessor`) and every use of
   what they defined replaced by poison.
2. `iteratively_simplified` visits the blocks in layout order until a visit
   changes none (`iterativelySimplifyCFG`). A visit (`block_simplified`,
   LLVM's `simplifyOnce`) deletes a block with no predecessor (or only
   itself), folds its terminator, removes the duplicate phis of the block,
   merges it into its one predecessor and, if it holds only phis and a jump,
   removes it (`empty_block_removed`): its predecessors go to its successor,
   which takes over its phis.
3. When step 2 changed anything, unreachable blocks are removed again and,
   if that removed any, step 2 and the removal alternate until neither does.

`terminator_folded` replaces

- `br i1 %c, X, X` by `br X`, taking one phi input of `X` away;
- `br i1 true/false, A, B` by a jump to the arm that runs;
- a `switch` on a constant by a jump to the case it takes (or the default),
  a `switch` whose cases all go to one block by a jump, and a case that goes
  where the default goes is dropped; a switch left with one case is a `br`
  on an `icmp eq`.

The condition that nothing reads afterwards is deleted with what only it
read. The new `br` carries `!llvm.loop`.

```text
  b0: br i1 true, b1, b2         b0: br b1
  b1: ...                  ==>   b1: ...        (b2, and its phi inputs
  b2: %p = phi [%x, b0], ..                      from b0, are gone)
```

## Example
Before:
```mir
define i16 @f(i16 %x) {
b0:
  br i1 true, label %b1, label %b2

b1:
  ret i16 %x

b2:
  ret i16 0
}
```
After:
```mir
define i16 @f(i16 %x) {
b0:
  ret i16 %x
}
```
`b2` is no longer reached, so it goes; `b1` has one predecessor, so it is
merged into `b0`.

From unit test `test_a_branch_on_true_is_a_jump_and_the_dead_arm_goes` in
`simplifycfg_tests.rs`.

## LLVM structure
[`SimplifyCFGPass`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L272)
with the options of the early function pipeline slot
([`PassBuilderPipelines.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L1178):
the defaults). `MIRROR` unless a line below says `DEVIATE`.

| LLVM | here | |
|---|---|---|
| [`simplifyFunctionCFGImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L272) | `simplified` | MIRROR |
| [`iterativelySimplifyCFG`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L229) | `iteratively_simplified` | MIRROR; D1 |
| [`SimplifyCFGOpt::simplifyOnce`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9303) | `block_simplified` | partly: the steps up to `simplifyUncondBranch`'s first half; the rest is not ported yet (D2) |
| [`simplifyUncondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8800) | `jump_simplified` | the empty-block half; D2 |
| [`TryToSimplifyUncondBranchFromEmptyBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1147) | `empty_block_removed` | MIRROR; D6 |
| [`SimplifyCFGOptions::NeedCanonicalLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/SimplifyCFGOptions.h#L29), read at [`simplifyUncondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8812) | `Options::need_canonical_loop` | MIRROR: true by default and at the early slot, cleared at the one after vectorization ([`PassBuilderPipelines.cpp#L1443`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L1443)); D7 |
| [`hoistCommonCodeFromSuccessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1873), called from `simplifyCondBranch` ([L9016](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9016)) and `simplifySwitch` ([L8647](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8647)) | `branch_hoisted`, `hoistcommon::hoisted`, `Options::hoist_common_insts` | MIRROR for the calls; see that module |
| [`foldTwoEntryPHINode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3834), called from `simplifyOnce` ([L9351](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9351)) under `SpeculateBlocks` | `twoentryphi::folded`, `Options::speculate_blocks` | MIRROR; see that module |
| [`simplifyBranchOnICmpChain`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5469), called from `simplifyCondBranch` ([L8985](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8985)) | `block_simplified`, `icmpchain::folded` | MIRROR for the call; see that module |
| [`speculativelyExecuteBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3244), called from `simplifyCondBranch` ([L9060](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9060)) | `branch_speculated`, `speculate::executed` | MIRROR for the call; see that module |
| [`foldCondBranchOnValueKnownInPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3814), called from `simplifyCondBranch` ([L9068](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9068)) | `block_simplified`, `knownpred::folded` | MIRROR for the call; see that module |
| [`tryToSimplifyUncondBranchWithICmpSelectInIt`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5328) and its `ICmpInIt` form, called from `simplifyUncondBranch` ([L8827](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8827)) | `block_simplified`, `uncondicmp::folded` | MIRROR for the call; see that module |
| [`tailMergeBlocksWithSimilarFunctionTerminators`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L142), called from `simplifyFunctionCFGImpl` after the first `removeUnreachableBlocks` | `tailmerge::merged` | MIRROR; see that module |
| [`sinkCommonCodeFromPredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2425) and [`simplifyDuplicatePredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8573), called from `simplifyOnce` ([L9336](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9336)) under `SinkCommonInsts` | `sinkcommon::sunk`, `sinkcommon::duplicate_predecessors_merged`, `Options::sink_common_insts` | MIRROR for the calls; see that module |
| [`foldBranchToCommonDest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4231), called from `simplifyCondBranch` ([L9005](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9005)) under `SpeculateBlocks` | `branchtocommon::folded` | MIRROR for the call; see that module |
| [`mergeConditionalStores`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4604), called from `simplifyCondBranch` ([L9083](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9083)) and `SimplifyCondBranchToCondBranch` ([L4805](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4805)) | `stores_merged` (the diamond), `condtocond::simplified` (the `MergeCondStores` step) | MIRROR for the calls |
| [`SimplifyCondBranchToCondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4773), called from `simplifyCondBranch` ([L9075](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9075)) | `cond_to_cond_simplified`, `condtocond::simplified` | MIRROR; D1-D2 there |
| [`removeUndefIntroducingPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9230), called from `simplifyOnce` ([L9327](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9327)) | `undefpred::removed` | MIRROR; D1 there |
| [`FindFunctionBackedges`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L236) | `loop_headers` | MIRROR |
| [`removeUnreachableBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L2912) and `markAliveBlocks` | `unreachable_removed` | the terminator folding and the deletion; D3 |
| [`ConstantFoldTerminator`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L133) | `terminator_folded` | MIRROR; D4 |
| [`BasicBlock::removePredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L446) | `Function::predecessor_removed` (`edit.rs`) | MIRROR (one phi input an edge) |
| [`DeleteDeadBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L94) | `dead_blocks_deleted` | MIRROR |
| [`EliminateDuplicatePHINodes`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1527) | `transform::duplicate_phis_in` | MIRROR |
| [`MergeBlockIntoPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L237) | `cfg::merged_into_predecessor` | MIRROR |

- D1. `LoopHeaders` is found once per `iteratively_simplified` and keeps the
  blocks that go; LLVM's `WeakVH`s drop them. Nothing reads a deleted
  block's entry.
- D2. Not ported yet, each a later change of this series: the terminator
  simplifications `simplifyUncondBranch` beyond the empty block
  (`tryToSimplifyUncondBranchWithICmpInIt`, landing pads: none in the MIR),
  `simplifyCondBranch`, `simplifySwitch`, `simplifyUnreachable`.
- D3. `markAliveBlocks` also turns what follows a call of a `noreturn`
  function, `llvm.assume(false)` and a store to `undef` into `unreachable`
  (`FoldInstsToUnreachable`): not ported yet. It reads the target's
  `NullPointerIsDefined`, which the MIR has no flag for. `invoke`,
  `catchswitch` and `indirectbr` do not exist in the MIR.
- D4. No branch weights: the MIR has none, so the case weights
  `ConstantFoldTerminator` merges and the `!prof` it sets are not kept.
  `!make.implicit` is not a MIR kind. There is no `DomTreeUpdater`; as at
  LLVM's default (`simplifycfg-require-and-preserve-domtree` off, where the
  pass is given no `DominatorTree`) the analyses are invalidated when
  anything changed: that is a MIRROR.
- D5. Temporary, driven by rows. After a terminator was folded,
  `transform::_trivial_phis` runs over the whole body, as `decide` did after
  the branches it decided. SimplifyCFG has no such construct: its phi
  cleanup is
  [`BasicBlock::removePredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L446)
  (`hasConstantValue`, L468), of the phis of the successor whose edge went
  (here `Function::predecessor_removed`). What `_trivial_phis` does is
  InstSimplify's
  [`simplifyPHINode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/InstructionSimplify.cpp#L5620),
  which InstCombine and EarlyCSE run over the whole function. Whole body,
  not the touched successors, because the phis are not there: gvn leaves
  `phi [%x, entry], [%self, latch]` in loop headers (premise:
  `gvn_tests::test_forwarding_a_loops_cell_to_its_phi_leaves_a_phi_of_itself`),
  which `decide` removed whenever it decided something. With the touched
  successors only, `bintree` -O3 was 7 instructions longer (twice; LSR
  declines the build loop with the phis in), seen failing in
  `simplifycfg_tests::test_a_fold_takes_a_phi_of_itself_and_one_value_out_of_the_body`.
  It runs once in a pass run that folded, not once per fold; compile
  instructions on the largest programs, base to here: `jpeg_single` (C)
  -2.0%, `jpeg_fixed` (Nib) -0.3%, `qcport/mdl_ai` -0.4%, `stft_single` (C)
  +0.08%, `jpeg_single` (BASIC, flat) +0.06%. Exit: gvn leaves no phi of
  itself (#2186) and `hoist` no longer lifts into an unguarded entry
  (#2194; a rule in `algebraic` that removed the phis made `x_hanoi2` -O2
  22.8% slower through it); then this goes.

- D6. A block keeps no `takeName` (the MIR has no rename); an `llvm.loop` on
  the removed jump is any `llvm.loop` (the MIR has no debug-loc-only ones);
  `undef` is `poison`, the MIR's one. `decide`'s `_threaded` is not removed
  yet: it also bypasses jumps in the loop-simplify shapes this keeps; its
  removal needs the corpus run. Exit: the CI-gate phase PR removes it.
- D7. `needCanonicalLoops` (the table row) keeps only the jump block that
  would send two or more edges to a loop header, or leave one. Here a jump
  alone in a block that holds a loop's simple form up (its preheader, its
  one latch, a dedicated exit: `decide::loop_form_blocks`) is also kept
  while `Options::loop_form_kept` is on, because rounds repeat until nothing
  changes and `loopsimplify` runs in them: LLVM's passes run once, and
  without this the two undid each other each round ("cycle after 3 rounds"
  on the `deedlines` demo). The LLVM-order pipeline turns it
  off at every slot.

## Runs
Once per round of the function fixed point, before `decide`: the branches
on a literal that `fold` just made are jumps before `decide` asks for the
facts of the body. Nothing runs at -O0.

## Measured
The C corpus (`identical.py`: 395 sources, -O1/-O2/-Os/-Omax, 1,580
compiles): `decide` decides 646 branches instead of 3,002; the ones on a
literal or a constant `switch` go from 2,464 to 153 (the rest are made by
the old `jumpthread`'s own cleanup, after this pass), and it runs 32,640
times instead of 33,932. Of the 1,583 objects 40 differ in bytes, 36 only in
label numbers; of the rest, `bintree` -O3 is 7 instructions longer (twice)
and `switch_empty_cases` 2 shorter (twice). The 140 bench rows at -O2 and
-Os of the 14 programs that differ have every counter unchanged.

## Keeps
Blocks and edges change: every analysis is invalidated when anything
changed.

## See also
[`crate::cfg`](../analyses/cfg.md), [`crate::decide`](decide.md).
