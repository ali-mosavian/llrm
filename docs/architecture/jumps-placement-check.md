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
- Triangle chains (`precomputeTriangleChains` L1333) run only when tail duplication in placement is allowed: with stage 4.
