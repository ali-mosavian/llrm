# Jumps: MachineBlockPlacement, translation plan

`MachineBlockPlacement.cpp` at llvm-project `d1106deb71cc81016406a1917d0508d2df296266` (3933 lines) against `backend/jumps.rs` (`_placed`, `duplicated_tails`, `rotated`, `duplicated`). Steps 2 and 3 of parity slot 62 (step 1: [jumps-tailmerge-check.md](jumps-tailmerge-check.md)). Each stage is one PR to `feat/parity`, ON by default, with what it replaces switched off; its function-by-function check is added here when it lands.

Source: `https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp`

## Stages

1. **Chains** (`buildCFGChains` [L2799](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L2799), `buildChain` [L1920](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L1920), `selectBestSuccessor` [L1665](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L1665), `selectBestCandidateBlock` L1778, `collectViableSuccessors` L762, `markChainSuccessors` L713, `getFirstUnplacedBlock` L1846/1873, `fillWorkLists` L1888, `hasBetterLayoutPredecessor` L1479). Replaces `_placed`'s traces. Without loops, tail duplication, triangle/trellis chains.
2. **Loops** (`buildLoopChains` L2706, `collectLoopBlockSet` L2653, `findBestLoopTop` L2240, `findBestLoopExit` L2269, `rotateLoop` L2423, `hasViableTopFallthrough` L2390, `canMoveBottomBlockToTop` L2010, `FallThroughGains` L2083). Replaces `_tests` and `rotated`'s gcc rule (D4).
3. **Triangles and trellises** (`precomputeTriangleChains` L1333, `isTrellis` L1023, `getBestTrellisSuccessor` L1123, `getBestNonConflictingEdges` L1079).
4. **Tail duplication in placement** (`maybeTailDuplicateBlock` L3213, `repeatedlyTailDuplicateBlock` L3155, `shouldTailDuplicate` L840, `isProfitableToTailDup` L869, `canTailDuplicateUnplacedPreds` L1210, `findDuplicateCandidates` L3376, `isBestSuccessor` L3338, `initTailDupThreshold` L3486; 2 instructions, 4 at -O3). Replaces `duplicated_tails` and `duplicated` (D3).
5. **After placement** (`optimizeBranches` L2964, `alignBlocks` L3003, `run` L3568: merge, then re-place only if it changed the blocks).

Not translated: `ExtTsp` placement (`applyExtTsp` L3680): off by default at the pin without a profile.

## Deviations to state before the first stage

- D2 gcc traces and not `selectBestSuccessor`'s probabilities: every row re-measured.
- D3 tail duplication by bytes against LLVM's instruction counts.
- D4 `_tests` loop headers after latches against `findBestLoopTop`.
- D5 the merge runs before placing and again after (T11 of the tail-merge check).
- D6 block frequency and probabilities come from llrm's `Frequency` and `branchprob`, not `MachineBlockFrequencyInfo`. Fact to measure: do they agree on the corpus.

## Stages 1 and 2: chains and loops (`jumps/placement.rs`)

Pin links as above. MIRROR: same condition and order; each DEVIATE has a fact.

- `buildCFGChains` L2799 is `Placer::build`: each block a chain, loops innermost first, `fillWorkLists`, `buildChain` from the entry. MIRROR. Result: the function chain, entry first.
- `buildChain` L1920, `markChainSuccessors` L713, `fillWorkLists` L1888, `getFirstUnplacedBlock` L1846/1873, `selectBestCandidateBlock` L1778: MIRROR (`build_chain`, `mark_chain_successors`, `fill_work_lists`, `best_candidate`).
- `collectViableSuccessors` L762, `selectBestSuccessor` L1665, `hasBetterLayoutPredecessor` L1479: MIRROR without trellis, `ComputedEdges` kept, tail-duplication candidates (stages 3 and 4). `getLayoutSuccessorProbThreshold` is `LIKELY` (no profile data in llrm).
- `buildLoopChains` L2706, `collectLoopBlockSet` L2653 (no profile: all of the loop), `findBestLoopTop` L2240, `findBestLoopExit` L2269, `rotateLoop` L2423, `hasViableTopFallthrough` L2390, `canMoveBottomBlockToTop` L2010, `TopFallThroughFreq`, `FallThroughGains` L2083: MIRROR. `rotateLoopWithProfile` is not translated (needs profile data).
- Replaces `_placed`'s traces, `_onward`, `_tests` and `rotated` (gcc `rotate_loop`): D2, D4 end here.

Deviations:

- D7 frequencies and probabilities are `f64` from `Frequency` and `BlockOdds`; LLVM's are fixed point (`BlockFrequency`, `BranchProbability`). Fact: ties LLVM sees as equal can differ in the last bit; not measured on the corpus.
- D8 `findBestLoopExit` never raises `BestExitLoopDepth` in LLVM, so any exit to a block in a loop wins; copied as is.
- D9 the loop's blocks and the blocks of a loop are visited in layout order, header first; LLVM's `MachineLoop` order is its DFS order. Sibling loops are visited innermost-first by size. Fact: only ties differ.
- D10 `cold` (isel's unreachable-only blocks) no longer places a block last; only the odds do, as in LLVM.
- D11 `-Os`/`-Oz`: `findBestLoopTop` is skipped, as `shouldOptimizeForSize`; no size-specific rule remains.
- D12 a loop headed by the entry keeps its top (LLVM's entry has no predecessors).

Tests that encoded gcc's rules, each re-checked with `llc -O2 -mtriple=i386 -tail-dup-placement=false` (pin build) on the same CFG:

- diamond, arm 10 taken 0.9 / 0.7 / 0.5 / 0.375 / 0.1: llc `[1,10,30,20]`, `[1,10,20,30]`, `[1,20,10,30]`, `[1,20,10,30]`, `[1,20,30,10]`: `test_a_diamond_is_placed_as_machine_block_placement_does` (replaces the two `LIKELY` diamond tests).
- size diamond: `test_for_size_a_diamond_is_placed_as_for_speed` (llc `-Os` equals `-O2` here).
- while loop entered by falling into its test, and with an exit block after: llc `[e,h,body,exit]`, test stays on top: the two loop tests renamed.
- loop nothing falls into: llc `[e,other,latch,h,exit]`, body first and test last: `test_a_loop_nothing_falls_into_is_rotated_to_end_at_its_exit_test`.
- `cmp; je 4` with two returns: llc `[e,r4,r9]`, `jne 9`: `test_shared_machine_pipeline_threads_the_final_branch_pair`.
- isel listings that follow from the same rules, five tests in `isel_tests.rs`: a two-way branch to two returns puts its first successor next (`jge L3; L2`; llc `[e,r4,r9]`); the counted loop `sum` keeps `[entry,test,body,done]` (llc the same; the test copy at the bottom is `duplicated`, stage 4); the `switch` chain's last compare falls into `one` (llc ends its chain `JCC other; one`). The switch lowering itself differs from LLVM's tree, so only the last block's order is compared.

## Stage 3a: trellises

- `isTrellis` L1023, `getBestNonConflictingEdges` L1079, `getBestTrellisSuccessor` L1123: MIRROR (`is_trellis`, `best_trellis_successor`), called from `selectBestSuccessor` after the `ComputedEdges` lookup. The tail-duplication branch of `getBestTrellisSuccessor` comes with stage 4.
- Test: `test_a_trellis_places_the_heaviest_pair_of_edges_as_fall_throughs` (llc at the pin breaks the source order the same way; failed with the call disabled).
- Triangle chains (`precomputeTriangleChains` L1333): stage 4.

## Stage 4: tail duplication in placement (`jumps/placement/taildup.rs`)

- `maybeTailDuplicateBlock` L3213, `repeatedlyTailDuplicateBlock` L3155 with `TailDuplicator::tailDuplicateAndUpdate` L165 and `tailDuplicate` L859 (layout mode, after allocation: no phis, no copies): MIRROR (`maybe_tail_duplicate`, `repeatedly_tail_duplicate`, `copy_into`, `remove`). The block last in the chain takes the tail whole when it is all that is left to reach it; the removed block leaves its chain, work list and loop set.
- `shouldTailDuplicate` L840 and `TailDuplicator::shouldTailDuplicate` L562, `canTailDuplicate` L825, `canTailDuplicateUnplacedPreds` L1210, `isProfitableToTailDup` L869, `greaterWithBias`, `precomputeTriangleChains` L1333, the duplicate candidates in `selectBestSuccessor` and the tail-duplication branch of `getBestTrellisSuccessor`: MIRROR. `isSimpleBB`, `findDuplicateCandidates`, `isBestSuccessor` and `scaleThreshold` need profile data: not translated. `MachinePostDominatorTree` is `placement/postdom.rs`, found once as LLVM's is.
- Replaces `duplicated_tails`, `duplicated` and `rotated`'s copies (D3 ends); `duplicated_returns` (gcc's `copy_bb_p`, in `assemble`) stays until it is looked at.

Deviations:

- D13 a block is counted without the jump to the block laid out next, and `jcc next; jmp other` as one `jncc`: LLVM's form after branch folding, which the explicit jumps here do not have yet.
- D14 `TailDupSize` is 2 (1 for size, 4 at -O3 via `cpu::Profile::aggressive`). Bench: the -O3 threshold loses the -O3 size gain (m32 -595 B to +26 B, m16 -354 to -94; clocks .8972 vs .8964); kept, as the pin does.
- D15 not copied: a block with a phi, a call's record, a group, a symbol, spread bytes, data, a barrier, a call or an indirect jump; the entry is never copied or removed (it may have predecessors here). Copies claim none of the source bytes.
- D16 the old guard against a copy that makes a loop irreducible is gone: LLVM has none. Fact to measure: how many bodies become irreducible on the corpus, and what `Frequency` makes of them.
- D17 the odds of a pred after a copy are the tail's (`BlockOdds::rerouted`); block frequencies are not updated, as `MBFI` is not.

Tests: `test_a_join_of_one_instruction_is_copied_into_both_arms` (llc at the pin: same orders, join gone), `test_for_size_a_join_of_two_instructions_is_not_copied`, `test_a_join_is_not_copied_into_a_block_that_branches_to_it`, `test_a_copied_test_keeps_its_loops_odds`, the loop tests (llc: guard plus test copied into the latch), post-dominator unit test; all seen failing with the threshold at 0. Removed with the code they tested: seven `duplicated_tails` tests and the copy-limit test; `test_a_small_join_is_copied_into_its_arm_before_the_arms_are_merged` guarded the merge-before-copy order, which no longer exists (the merge runs after placement, as LLVM's).

## Stage 5: after placement (`jumps.rs`: `inverted`, `aligned`)

- `optimizeBranches` L2964 is the second half of `inverted`: a two-way branch with neither way next gets the likelier way as its conditional jump. MIRROR, and skipped for size as `shouldOptimizeForSize` does (before, size flipped it too). `jcc next; jmp M` becoming `j!cc M` is `analyzeBranch(AllowModify)`.
- `alignBlocks` L3003 is `aligned`, run on the final layout after the merge: every block but the first that is in a loop, not cold against the entry or its loop header (20%), and entered by no layout-predecessor edge, or one under 20% of its frequency, gets the target's loop alignment. MIRROR; `getPrefLoopAlignment` is `Target::pref_loop_alignment` (m32 16 bytes, m16 none: E2-1), `getMaxPermittedBytesForAlignment` is the default 0 (no limit). Not for size (`hasOptSize` without `alignLoopsWithOptSize`).
- D19 (a fact, not a deviation) m32 code grows 4% at -O1..-O3 (+6.5/+7.5/+8.4 KB on the rows corpus) from the nop padding: the pin sets `setPrefLoopAlignment(Align(16))` for every CPU ([X86ISelLowering.cpp L2950](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L2950)), and `llc -O2 -mcpu=i486` marks the loop blocks `(align 16)`. The 486 clock model does not see alignment, so no clock gain shows. Exit: none while m32 follows the pin; m16 aligns nothing.
- D18 the loop of a block is the nearest natural loop of the final layout, with no `llvm.loop.align` metadata.
- Tests: `test_for_size_a_two_way_branch_keeps_its_condition` (failed before the skip), `test_a_loop_block_nothing_falls_into_is_aligned`.
- Not translated, as off by default at the pin: `ExtTsp` placement, `rotateLoopWithProfile`, profile-driven duplicate candidates.

## Branch folding (OptimizeBranches)

`jumps/branchfold.rs` replaces `_step` and the bookkeeping it needed (`Places`, `_stranded`, `_converged`). Output is unchanged on 41 kernels and d_faces at -Os/-O1/-O2/-O3, m32 and m16 (assembly compared against the build before).

- D20 `OptimizeBlock`'s rules that move a block (`moveAfter`, `moveBefore`) are not translated: placement has laid the blocks out, and LLVM redoes its layout when this pass changes anything after placement, which we do not.
- D21 The next block is the next that emits something, and a block's own jump is read against it, not the previous block's against this one: the explicit form leaves blocks of only anchors in between.
- Not yet translated: merging a block into its only predecessor, the identical-conditional single-branch block, the tail-call fold (no unconditional tail calls reach this pass).
- D22 Block frequencies are `f64`, LLVM's are fixed-point `BlockFrequency`. Where the odds make two frequencies equal (a loop of 31 in 32 leaves as often as it is entered), fixed point keeps them equal and `f64` does not; `at_least` and `above` compare within 1e-9 relative. The comparisons are `rotateLoop`'s `FallThrough2Top >= ExitFreq` and `findBestLoopExit`'s edge frequencies. Checked with llc on a 3-deep loop nest (`test_a_loop_is_not_rotated_where_its_exit_only_ties_the_top_fall_through`).
