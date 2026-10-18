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
