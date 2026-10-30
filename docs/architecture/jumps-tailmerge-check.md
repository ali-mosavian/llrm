# Jumps: TailMergeBlocks, faithful-translation check

`backend/jumps/tailmerge.rs` at llrm `f7fb445c355c58d4d6a8a0d543755c57a14b5278` against `BranchFolding.cpp` at llvm-project `d1106deb71cc81016406a1917d0508d2df296266`. MIRROR means the same condition and order; each DEVIATE has a number and its fact.

Source of every LLVM link: `https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp`

## Function by function

- `TailMergeBlocks` ([L1109](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1109)) is `Folder::run` (`tailmerge.rs` `run`). MIRROR: no-successor blocks first, then each block after the entry with two or more predecessors; `TailMergeThreshold` 150 and `TriedMerging`; the loop-header and same-loop tests after placement. DEVIATE T1, T2, T3, T4.
- `TryTailMergeBlocks` ([L981](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L981)) is `try_merge`. MIRROR: sort by hash, `ComputeSameTails`, the two-block layout-successor case, entry block never the tail, `PredBB` preferred, split when none is the whole tail. DEVIATE T5.
- `ComputeSameTails` ([L743](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L743)) is `same_tails`. MIRROR, line for line.
- `ProfitableToMerge` ([L633](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L633)) is `profitable_because`. MIRROR: PredBB rule, unreachable pair, layout successor, `AfterPlacement` both-fall-through rule, the effective length, the size rule. DEVIATE T6, T7.
- `ComputeCommonTailLength` ([L417](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L417)) is `common` with `identical`. DEVIATE T8.
- `HashEndOfMBB` ([L363](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L363)) is `hash_of`: the last non-meta instruction, hashed by operation, frame effect and register fields (not Debug text).
- `CreateCommonTailOnlyBlock` ([L799](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L799)) and `EstimateRuntime` ([L530](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L530)) are the split choice in `try_merge` and `estimate`: PredBB first, else least estimated time (`<=`), call 10, memory 2, other 1. MIRROR.
- `SplitMBBAt` ([L485](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L485)) is `split`: the new block takes the successors, is laid out next, is in the same loop and has the same frequency. MIRROR (`isLegalToSplitMBBAt` is true on x86).
- `setCommonTailEdgeWeights` ([L1259](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1259)) is the first half of `merge`. DEVIATE T9.
- `OptimizeFunction` ([L237](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L237)) is `merged` plus the loop in `jumps::optimized`. DEVIATE T10, T11.

## Deviations

- T1 `mergeable`: a block whose last instruction is a barrier, data, owns source bytes or names a symbol is never a candidate; LLVM hashes any last instruction. Fact: inherited from the old `merged` (same exclusion); not measured with the exclusion lifted. Open.
- T2 candidates are visited in layout order; LLVM walks `predecessors()` in edge order. Fact: the order only changes which block is first in `MergePotentials`, and the sort that follows makes the walk order irrelevant except among equal hashes (T5).
- T3 distinct predecessors and `succ_size` count distinct successors; LLVM counts edges. Fact: duplicate edges arise only from a branch whose two ways reach one block, which `threaded` folds (`_converged`) before the merge.
- T4 no EH pad, inline-asm-br or phi tests. Fact: LIR has no landing pads and no phis after allocation.
- T5 equal-hash ties are ordered by block address; LLVM's `array_pod_sort` is `qsort`, whose tie order is unspecified. Fact: deterministic output; no LLVM order to copy.
- T6 "next block" for the layout-successor and fall-through-predecessor tests skips blocks with no code (`after`, `before`); LLVM uses the literal neighbour. Fact: `_explicit` leaves code-less blocks that LLVM's `OptimizeBlock` already removed; without the skip `x_primes` was +9.2% code (measured, fixed in `before`). The two-block case in `try_merge` uses the literal neighbour, as LLVM.
- T7 minimum common tail 3 at -O1/-O2/-O3 and 2 at -Os. LLVM passes `TailDupSize + 1` ([MachineBlockPlacement.cpp L3624](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L3624)): 3, 5 at -O3, 2 at -Os. Fact: at -O3 the minimum 4 and 5 gave identical code and clocks on the 284 rows.
- T8 instructions are equal on operation, clobbers, frame effect and register fields, not on value names (the allocator's names), which `merge` renames to the kept tail's after. LLVM compares physical registers only. Fact: LIR carries SSA value names next to registers.
- T9 the kept tail's edge odds are set as LLVM does, but its block frequency is not stored: the table forgets the tail and the frequency is worked out from its predecessors. Fact: LIR frequencies derive from the odds; the sum is the same by construction.
- T10 no `OptimizeBranches` between rounds; `jumps::optimized` runs `threaded` and `machinedce::eliminated` between calls to `merged`. Fact: `_explicit` leaves fall-throughs as jumps, and a stale one made the merge see the wrong layout (found on `x_primes`).
- T11 no re-placement after a change; the rotation and the `returns` copies that follow are gcc's, the translation of `MachineBlockPlacement` being steps 2 and 3. Tail duplication runs before the merge, as LLVM places with it. Fact: merge first gave `x_cordic` -O3 +5.0% clocks (the join could no longer be copied into an arm); moving duplication and rotation both before the merge (`LLRM_TM_LATE`, reverted) gave `hanoi` +21.9%.
- T12 anchors (an inserted `Nothing` with a virtual definition and no bytes) do not count as instructions of a tail. Fact: LLVM has none after allocation (`ExpandPostRAPseudos`); counted, three of them made a "common tail" of no code and split an empty join block (`x_cordic` -O3, +4.25% clocks of the 5.0%).
- T13 anchors just before the tail leave with it. Fact: left, the arm that now only jumps keeps a non-threadable block (`_passage` refuses around an anchor).
- T14 `threaded` also takes the branch of `Bcc T; B F` through a block that only jumps, as `OptimizeBlock` does for TBB; before, only the last instruction was. Fact: `recpow` -O2 ran a `jmp` after every taken `jne` (+0.53% clocks, +25 bytes).

- D1 (temporary) at -O1/-O2/-O3, a block on a cycle that ends in a branch is not a merge candidate (issue 2351). Fact: the middle end leaves the 5-instruction latch inline in each arm of `x_switch`; the faithful merge shares it and every trip pays one more jump, -O2/-O3 clocks 43,048 to 43,462 (+0.96%). With the guard `x_switch` -O2/-O3 equals base. Cost: `x_primes` -O1/-O2/-O3 code 119 to 130, not above min(gcc,pin) (130 at -O1). Exit: LLVM's shape from the middle end and placement (steps 2 and 3), then delete the guard.

## What is left

`x_switch` -Os is the one row worse in clocks, a priced trade at size: 120,827 to 121,435 (+0.50%) for code 228 to 188 (-17.5%). The size mode takes any merge that saves bytes.

Removed from the old pass with no LLVM home: the hotness guard (`preferred`, plan D1). No row ends above min(gcc, pin) without it.
