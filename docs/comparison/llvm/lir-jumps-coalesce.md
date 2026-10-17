# LIR jumps and coalescing against BranchFolding, block placement and RegisterCoalescer

TL;DR: on `d_faces` `-O2` (11 functions, 2,250 LIR instructions) `jumps` costs 57.9 M and the coalescer 55.2 M, against 5.9 M and 10.1 M for LLVM's counterparts on 1,594 machine instructions: 6.9x and 3.9x per instruction. Both run once per function; the extra is structure, not repeats. The coalescer builds a whole-function interference graph and every interval up front (29.7 M of its 55.2 M) where LLVM joins live ranges pairwise; `jumps` spends 33 M on duplication, threading and clock pricing that LLVM's branch folding does not do, and only 10 M on the merge-and-DCE loop. Peephole and x87 are on [their own page](lir-cleanup.md); today's numbers for them are in the table.

## Measured

`d_faces.c` `-m32 -march=i486 -O2`, callgrind inclusive, `CARGO_INCREMENTAL=0` build of main at `98921a80e`; calls are per-function runs (11 functions). LLVM: `llc -O2` on the same file's optimized IR, 6 functions, 1,594 machine instructions at greedy's input ([compile-cost tracker](https://github.com/ali-mosavian/llrm/issues/1994)).

| llrm stage | M | runs | K per LIR insn | LLVM counterpart | M | K per MI |
|---|--:|--:|--:|---|--:|--:|
| peephole | 97.1 | 11 | 43.2 | X86 peephole + MachineCSE + CopyProp + DeadMI + MachineSink + FixupBW | 12.6 | 7.9 |
| jumps | 57.9 | 11 | 25.7 | BranchFolding + MachineBlockPlacement | 5.9 | 3.7 |
| coalesce | 55.2 | 11 | 24.5 | RegisterCoalescer + PHIElimination + TwoAddress | 10.1 | 6.3 |
| floatassign + floatalloc | 43.8 + 21.8 | 4 (the float functions) | | X86FPStackifier | 1.6 | 1.0 |

The per-instruction ratio is 5.5x, 6.9x, 3.9x. The body is 1.41x larger (2,250 against 1,594), so about 4x of it is cost per instruction. The mapping is by purpose, not by identical work.

## Coalescing

**LLVM.** `RegisterCoalescer` visits the copies in an order (loop depth, then split edges, then CFG degree) ([`RegisterCoalescer.cpp#L4058`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L4058)) and for each asks `LiveIntervals` whether the two live ranges can merge, by merging them value by value ([`JoinVals`, #L3154-L3222](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3154-L3222)). There is no interference graph. Copies deferred as not yet joinable are retried until nothing changes ([#L4286-L4321](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L4286-L4321)); `LargeIntervalSizeThreshold` bounds the cost ([#L111](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L111)). `LiveIntervals` is built once for the allocator and kept.

**llrm.** `coalesce::joined` ([`coalesce.rs#L173`](https://github.com/ali-mosavian/llrm/blob/98921a80e4e6eeaba9c701b4d9b0f3f5fafad7ab/crates/backend/llrm-core/src/backend/coalesce.rs#L173)) runs once per function, no retry. In callgrind: `_interference_among` 15.8 M and `intervals` 13.9 M, built before the first copy is looked at; the per-copy loop is the rest (palette intersection, `_merged`, `_clobbered` per register run before the cheap Briggs test: [#L249-L260](https://github.com/ali-mosavian/llrm/blob/98921a80e4e6eeaba9c701b4d9b0f3f5fafad7ab/crates/backend/llrm-core/src/backend/coalesce.rs#L249-L260)); `Webs::george` 1.4 M over 816 calls. The graph is also built by `twoaddr` and the spiller's sibling pricing.

**Mirror or deviate.** LLVM structure: interval-pair joins in a priority order, over `LiveIntervals`; ours: a whole-function interference graph with Briggs/George counts; deviate only if the graph's 54% buys joins the interval method misses, which is the POC's question (below).

## Jumps

**LLVM.** `BranchFolding` repeats tail merge, branch optimization and hoisting until nothing changes ([`BranchFolding.cpp#L277-L287`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L277-L287)); tail merging hashes only a block's end and considers at most 150 candidates ([#L97-L99](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L97-L99)); the common tail is at least 3 instructions. `MachineBlockPlacement` builds chains by edge probability, tail-duplicates at most 2 instructions (4 aggressively), and redoes the layout only if branch folding changed something ([`MachineBlockPlacement.cpp#L3629-L3644`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L3629-L3644)).

**llrm.** `jumps::optimized` ([`jumps.rs#L186-L212`](https://github.com/ali-mosavian/llrm/blob/98921a80e4e6eeaba9c701b4d9b0f3f5fafad7ab/crates/backend/llrm-core/src/backend/jumps.rs#L186-L212)), once per function. Callgrind split: `duplicated` (tails, then returns) 12.8 M over 22 calls, `threaded` 11.7 M over 14, `_work` (static and dynamic pricing of each choice) 8.4 M, `merged` 7.0 M, `_placed` 5.4 M, frequency 3.9 M, `machinedce::eliminated` 3.4 M, `_hoisted` 3.3 M. The merge-and-DCE fixed point ([#L199-L207](https://github.com/ali-mosavian/llrm/blob/98921a80e4e6eeaba9c701b4d9b0f3f5fafad7ab/crates/backend/llrm-core/src/backend/jumps.rs#L199-L207), which clones the body and compares it per round) is the loop that reads as the cost, and it is 10.4 M of 57.9 (18%).

**Mirror or deviate.** LLVM does not price duplication and threading in the clock model; ours does (`_work`, rotation), which is a named extra and the 33 M. Keep or remove is a rows question (the choices change code), not a speed one.

## x87

[lir-cleanup.md](lir-cleanup.md) has the peephole and stackifier comparison. For the allocation half: llrm's `floatassign` is a register allocator for the x87 stack (allocnos, spill placement, Belady victim rounds: [`floatassign.rs#L2059-L2101`](https://github.com/ali-mosavian/llrm/blob/98921a80e4e6eeaba9c701b4d9b0f3f5fafad7ab/crates/backend/llrm-core/src/backend/floatassign.rs#L2059-L2101)) run before `floatalloc`, the stackifier; LLVM's stackifier is one forward walk per block over registers greedy already assigned ([`X86FloatingPoint.cpp#L347-L394`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FloatingPoint.cpp#L347-L394)). Callgrind: `_allocnos` 16.8 M and `allocate::live` 11.0 M of the 43.8 M. Four functions of the eleven use it.

## What is not measured

- The cost per instruction of each stage over its own input (the table divides by the allocator's input size, not each stage's).
- Which copies the interval join would miss that the graph finds (the POC).
- The reading of agent-found details above that I did not time: `Webs::join` neighbour updates, the pinned Briggs scans.

## Proposed POC

An LLVM-shaped coalescer run in shadow mode on the live LIR of each function (as the register worklist for `fold`): copies in LLVM's order, joined by interval merge with no graph; count the joins made, the copies ours joins that it does not (classified by why: pinned, clobber, width), and its instructions. Kill if it misses more than 5% of today's joins on `d_faces` -O1/-O2/-Os and the benches.
