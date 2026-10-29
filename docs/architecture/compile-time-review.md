---
architecture-index: compile time against gcc and clang
architecture-order: 110
---
# llrm compile time against gcc 13.4 and clang 20: review and design

> **Reference note.** The clang figures and the "x clang" ratios in this review are from the distro clang 20.1.8; they are void as a cost reference to the pinned 24.0.0git `d1106deb71cc` (`d_faces` -O2 415.5 M) and are re-derived in the plan's scoreboard.

Input `qbq-llrm/src/render/d_faces.c`, `-m32`, flags of `compile_time.sh`. llrm at [0b0770d37](https://github.com/ali-mosavian/llrm/commit/0b0770d37a113eb051460385d7b3bba98306ba8b), release profile. Instructions are `perf stat -e instructions:u`; step costs are `LLRM_DEBUG=time` own work (its total, 8583 M, is 4% above perf's 8280 M from the timer itself). How to reproduce: appendix C.

gcc and LLVM figures come from runs on the same input and flags: dumps (`-fdump-rtl-ira-details`, `-fdump-rtl-reload-details`, `-fdump-statistics`, `-print-after`, `-fdebug-pass-manager`), gdb hit counts on cc1 and on libLLVM with its matching debug-symbol package, and perf call graphs (23.6 k samples gcc, 36.6 k clang). Appendices A and B hold the full traces with a source link per step. "File" figures are for all of d_faces.c; "fn" figures are for `D_DRAW_FACES`/`d_draw_faces` alone. llrm's single allocation is measured with `-fno-allocation-search -fno-allocation-routes`.

| level | llrm M | gcc M | clang M | llrm/gcc | llrm/clang |
|---|---|---|---|---|---|
| -O0 | 512 | 231 | 146 | 2.2 | 3.5 |
| -O1 | 8280 | 636 | 519 | 13.0 | 15.9 |
| -O2 | 8901 | 1136 | 669 | 7.8 | 13.3 |
| -O3 | 9571 | 1412 | 672 | 6.8 | 14.2 |
| -Os | 8638 | 944 | 566 | 9.1 | 15.2 |

One function is the compile: `D_DRAW_FACES` takes 573 of the 632 ms charged to functions (91%). Its output is 1445 instructions in llrm, 1371 in gcc and 2023 in clang, so the gap is not input size: llrm spends about 5 M instructions per emitted instruction, gcc and clang about 0.4.

## 1. TL;DR

d_faces -O1: llrm 8.28 G instructions, gcc 0.64 G, clang 0.52 G (13x, 16x). All three handle the same units of work on the hot function: ~450 values, ~200-250 blocks, ~100 spills. LLVM even runs three times as many passes on it. llrm pays far more per unit:

| unit | llrm | gcc | LLVM |
|---|---|---|---|
| allocations of a function | up to 6 (21 on d_faces) | 1 (IRA + LRA) | 1 |
| spill or split event | ~4 M | spills in one LRA pass | ~0.2 M |
| split candidate costed | ~200 k | | 23 k |
| MIR pass run | ~28 M | ~4 M | ~0.4 M |
| analysis computations per pass run | 2.8 | | 0.53 |
| post-selection clean-up | 0.98 G | ~82 M | ~14 M |

The five structural causes, as shares of 8.28 G. They overlap, so they do not add up:
1. **Up to six allocations per function, each rerunning the LIR pipeline: 36%** (3.1).
2. **Analyses thrown away on any edit and recomputed whole: 23%.** Points-to alone is 0.82 G, against gcc's 16 M (section 4).
3. **Each allocator round re-derives function-wide state: 12% per allocation** (3.2).
4. **LIR clean-up rebuilds the whole body per sub-pass: 12%** (section 7).
5. **The optimizer reruns its pass list to a fixed point and prices by cloning the function.** Rounds 3-5 are 7.8%; pass runs that changed nothing are 10.6% (section 6).

The cause is not representation. IPC equals gcc's, and the cycle ratio equals the instruction ratio (section 8).

The same holds at every optimizing level (2.2). At -O1, -O2, -O3 and -Os, allocation multiplicity is 30-37% of the compile, and the second route never changes the listing. llrm is 13-16x clang throughout; the ratio to gcc falls from 13x to 7x only because gcc grows. At -O2/-O3, llrm's extra cost is the optimizer: its middle end grows by 1.33 G where clang's grows by 116 M. -O0 has no repeated work and is 2.2x gcc from constant per-pass cost.

The design (section 10) follows one rule: every fact has one owner, is computed once per function, and is updated from the edit that changed it. It takes LLVM's allocator and incremental live ranges, gcc's points-to on SSA names and its dirty-block dataflow, and llrm's change log, executed-work cost model and recompute-and-compare checks. The projection is 8.3 G → ~1.9 G (section 11). About 2.6 G of the gain keeps today's output. Two of the decisions trade output and are the user's: dropping the second route (1.93 G, same listing on d_faces) and replacing the trials (1.05 G more, +1.2% static instructions in `D_DRAW_FACES`).

## 2. Where the time goes

| llrm step (own) | % of 8583 M | gcc pass (exclusive) | % of 636 M | clang pass (inclusive) | % of 552 M |
|---|---|---|---|---|---|
| split placed | 5.2 | reload (LRA) | 10.1 | InstCombine (73 runs) | 10.2 |
| mir hoist | 4.3 | ira | 10.0 | X86 isel (SelectionDAG) | 9.7 |
| regalloc trial | 4.2 | dom | 8.1 | RAGreedy | 5.0 |
| lir peephole | 4.0 | execute_todo (all) | 5.5 | IndVarSimplify | 3.5 |
| regalloc base | 3.6 | combine | 4.7 | LoopStrengthReduce | 3.1 |
| split carving | 3.1 | ccp | 4.6 | SimplifyCFG | 2.8 |
| analysis globals-aa | 2.5 | dse (RTL) | 3.4 | LICM | 2.2 |
| analysis pointer-values | 2.4 | ivopts | 2.8 | MachineScheduler | 1.7 |
| analysis points-to | 2.3 | fre | 2.2 | EarlyCSE | 1.5 |
| regalloc candidates | 2.3 | fwprop1/2 | 2.1 | SROA | 1.5 |
| mir gvn | 2.3 | postreload | 2.1 | LoopRotate | 1.3 |
| analysis call-effects | 2.1 | loop2_invariant | 2.0 | CodeGenPrepare | 1.2 |
| intervals walk | 2.0 | expand | 2.0 | LoopIdiomRecognize | 1.1 |
| facts intervals | 2.0 | cse1 | 1.9 | RegisterCoalescer | 1.1 |
| analysis through-memory | 2.0 | forwprop | 1.8 | MachineLICM | 1.0 |

Self symbols: llrm memmove 3.2%, mimalloc 5.7% in its top three symbols, `intervals::changes` 2.5% (the per-round block-by-block compare), `alias::points_to` 1.6%. gcc's top symbols are `bitmap_set_bit` 2.8%, `bitmap_clear_bit` 1.4% and `bitmap_bit_p` 1.3%: sparse bitmaps for df, IRA and alias. clang's are the dynamic loader at 9.5% across three symbols (12.3% with its callees), `_int_malloc` 1.5% and `hash_short` 1.2%; past the loader no LLVM symbol is above 1.3%. All three profiles are flat. What separates them is the step counts below.

### 2.1 Cost per unit of work

Per emitted instruction of the hot function (llrm 1445, gcc 1371, clang 2023 instructions):

| job | llrm | gcc | LLVM |
|---|---|---|---|
| register allocation (file / fn insns) | 3.71 G: 2.6 M | ~128 M: 93 k | ~39 M: 19 k |
| alias, memory SSA, call effects | ~1.3 G: 0.9 M | ~27 M: 20 k | ~16 M: 8 k |
| post-selection clean-up (llrm LIR passes; gcc combine, cse1, fwprop, postreload, cprop, bbro, stack; LLVM peephole, MachineCSE, copy propagation, BranchFolder, MachineLICM) | 0.98 G: 0.68 M | ~82 M: 60 k | ~14 M: 7 k |

The work units are about the same: values, spills, splits, blocks. What differs is the cost of handling one unit. Spill or split event: ~4 M against LLVM's ~0.2 M (RAGreedy as a whole over 99 spills, 31 region splits and 55 evictions), and 28 k for `InlineSpiller::spill` itself. Split candidate: 200 k against 23 k. Analyses per pass run: 2.8 against 0.53. Liveness per edit: the whole function against the dirty blocks.

### 2.2 Across optimization levels

The same input and flags at -O0, -O1, -O2, -O3 and -Os. llrm figures are `LLRM_DEBUG=time` own work, plus `perf stat` for whole compiles. gcc and clang figures are inclusive shares from perf call graphs, averaged over three runs per level (the min-max range across runs is within ±2 points).

**llrm by component (M, own work):**

| level | total | front end | MIR passes | analyses | isel | LIR passes | allocator | one allocation, total |
|---|---|---|---|---|---|---|---|---|
| -O0 | 446 | 84 | 12 | 0 | 97 | 120 | 113 | 446 |
| -O1 | 8524 | 84 | 1706 | 1945 | 65 | 983 | 3713 | 5320 |
| -O2 | 9167 | 84 | 2048 | 2151 | 66 | 998 | 3791 | 5892 |
| -O3 | 9891 | 84 | 2584 | 2395 | 70 | 1038 | 3691 | 6701 |
| -Os | 8952 | 84 | 2139 | 1755 | 60 | 862 | 4007 | 5461 |

**Allocation multiplicity at every level (perf, M; `D_DRAW_FACES` instructions/memory operands):**

| level | default | `-fno-allocation-routes` | `-fno-allocation-search` | both | listing, default → both |
|---|---|---|---|---|---|
| -O0 | 512 | 512 | 512 | 512 | 2725/1287, unchanged |
| -O1 | 8279 | 6350 (-23%) | 6469 (-22%) | 5301 (-36%) | 1445/661 → 1462/657 |
| -O2 | 8901 | 6923 (-22%) | 7033 (-21%) | 5867 (-34%) | 1447/664 → 1467/657 |
| -O3 | 9570 | 7718 (-19%) | 7845 (-18%) | 6660 (-30%) | 1443/664 → 1463/657 |
| -Os | 8638 | 6522 (-24%) | 6789 (-21%) | 5449 (-37%) | 1370/647 → 1404/675 |

Without the second route the whole listing is identical to the default at every level. Without the trials, `D_DRAW_FACES` grows by 1.2-1.4% at -O1 to -O3. At -Os it grows by 2.5% and gains 4.3% more memory operands, so the trials are worth most at the size level.

**The same job in the three compilers (M):**

| job | compiler | -O0 | -O1 | -O2 | -O3 | -Os |
|---|---|---|---|---|---|---|
| whole compile | llrm / gcc / clang | 512 / 231 / 146 | 8279 / 636 / 521 | 8901 / 1137 / 671 | 9570 / 1413 / 672 | 8638 / 944 / 567 |
| register allocation | llrm allocator | 113 | 3713 | 3791 | 3691 | 4007 |
| | gcc IRA + LRA | 54 | 105 | 129 | 148 | 78 |
| | clang greedy | (fast RA) | 25 | 26 | 28 | 25 |
| middle end | llrm MIR passes + analyses | 12 | 3651 | 4199 | 4979 | 3894 |
| | clang IR pipeline | 0 | 169 | 275 | 285 | 207 |
| points-to | llrm pointer family | 0 | 819 | 845 | 922 | 678 |
| | gcc `compute_may_aliases` | 0 | 12 | 27 | 20 | 22 |
| machine dataflow | gcc `df_analyze` | 2 | 51 | 125 | 114 | 99 |
| instruction selection | llrm isel / clang isel | 97 / 32 | 65 / 51 | 66 / 55 | 70 / 56 | 60 / 54 |
| process start | clang dynamic linking | 48 | 64 | 66 | 63 | 62 |

**Optimizer iteration:**

| level | llrm pass runs (in the fixed point + after it) | max rounds | rounds 3-5, M | idle runs, M | llrm analysis computations per pass run | clang per pass run |
|---|---|---|---|---|---|---|
| -O1 | 686 + 209 | 5 | 641 | 882 | 2508 / 895 = 2.8 | 505 / 951 = 0.53 |
| -O2 | 743 + 220 | 5 | 699 | 913 | 2666 / 963 = 2.8 | 575 / 1258 = 0.46 |
| -O3 | 889 + 220 | 5 | 699 | 1549 | 3283 / 1109 = 3.0 | 575 / 1286 = 0.45 |
| -Os | 743 + 143 | 5 | 987 | 1214 | 2302 / 886 = 2.6 | 579 / 1143 = 0.51 |

What changes with the level:
- **-O1 to -O3 and -Os have the same structure.** Allocation multiplicity is 30-37% at every optimizing level, analyses 20-24%, the allocator 37-45% and LIR clean-up 10-12%. The ratio to gcc falls from 13x to 7x only because gcc grows (636 → 1413 M). llrm grows too (8.3 → 9.6 G), and the ratio to clang stays at 13-16x.
- **What grows in llrm at -O2/-O3 is the optimizer.** MIR passes rise from 1.71 to 2.58 G and analyses from 1.95 to 2.40 G. gvn with PRE pricing goes from 193 to 423-485 M (two numberings, frequencies and pricing per run), and `bounded` from 117 to 264-286 M. clang's IR pipeline grows by 116 M over the same range; llrm's middle end grows by 1.33 G.
- **-Os is the most expensive allocation.** The allocator is 4.0 G, with 285 splits and 546 spills on `D_DRAW_FACES` against 199 and 415 at -O1, and last-chance recolouring costs 381 M (4.3%). hoist reaches 604 M. gcc's -Os allocation is its cheapest (78 M, one IRA region).
- **-O0 is a different problem.** llrm is 2.2x gcc and 3.5x clang (5.2x clang without clang's 48 M of dynamic linking). Nothing at -O0 iterates or repeats. The cost is constant per pass: isel 97 M against clang's 32 M; floatassign 62 M (14% of the compile) for x87 stack assignment; the HIR verifier 20 M at every level; the front end 84 M against gcc's 19 M.

## 3. Register allocation

### 3.1 Up to six allocations

**(a) llrm.** `directed` [backend/assemble.rs:608-678](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/assemble.rs#L608-L678) runs the allocator alone, then the spiller route when frame traffic is ≥0.5% of the work (`SPILLER_TRAFFIC`, [backend/assemble.rs:589](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/assemble.rs#L589)). Each route clones `Staged` and runs the whole machine pipeline `flow::machine_with` [flow.rs:66-139](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/flow.rs#L66-L139): FloatAssign, Coalescer, RegAlloc, Peephole and ControlFlow, all of which see identical input on both routes up to SsaSpill. Inside RegAlloc, `RegAlloc::transform` runs the base [backend/allocate.rs:3818](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L3818), builds trial shapes ("regalloc candidates", [backend/allocate.rs:3834-3870](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L3834-L3870)) and runs two trials [backend/allocate.rs:3893](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L3893). Each run starts from `Facts::of(None)` [backend/allocate.rs:1981](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L1981): numbering, intervals, masks, widths, hints and classes from scratch, 21 times on d_faces.

| run | instr M | listing (`D_DRAW_FACES` insns / mem operands) |
|---|---|---|
| default | 8280 | 1445 / 661 |
| `-fno-allocation-routes` | 6350 (-23%) | 1445 / 661 (identical file) |
| `-fno-allocation-search` | 6469 (-22%) | 1462 / 657 |
| both | 5301 (-36%) | 1462 / 657 |

Steps: "lir regalloc" 13x 3573 M, "regalloc trial" 8x 1826 M, "regalloc base" 13x 1468 M, "regalloc candidates" 4x 200 M own (shape building: `splitkit::loop_bases` walks the whole body per loop and per candidate value, [backend/splitkit.rs:106-155](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/splitkit.rs#L106-L155)).

**(b) gcc and clang.** gcc runs IRA once ([ira.cc:5540](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira.cc#L5540), regional over the loop tree, [ira-build.cc:328](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L328)) and LRA once, iterating constraints only on what is left in memory ([lra.cc:2375](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra.cc#L2375)). IRA+LRA are about 20% of gcc's 631 M (`ira_build` 5.6%, `ira_color` 2.6%, `lra` 9.1%), ~120 M. LLVM runs `RAGreedy` once ([RegAllocGreedy.cpp:2723](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2723)); `RegAllocBase::allocatePhysRegs` is 5.1% of clang's 552 M, ~28 M. Neither compares whole allocations.

**(c) Why slower.** Up to 3x the full LIR pipeline (routes) times up to 3x allocation (base + 2 trials), each from cold facts. The trials share 0-4 of their first 119-189 events with the base ([allocation-algorithm note](../optimizations/allocation-algorithm.md)), so nothing is reused.

**(d) Instead.** Drop the second route by default and make the trials decisions inside one run; the decision points are those of the [single-allocation note](../optimizations/single-allocation-decisions.md):
- the split is priced with its border copies after hinting, as `calculateRegionSplitCost` does ([RegAllocGreedy.cpp:1086](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1086)) — replaces `Whole`;
- a loop-local piece weighted by the loop's frequency, protected through `tryEvict` ([RegAllocGreedy.cpp:580](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L580)) — replaces `Scoped`;
- spill weight charges the unfoldable address use ([CalcSpillWeights.cpp:160](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/CalcSpillWeights.cpp#L160)) — replaces `Retainable`;
- fold or unfold per use in the spiller's rewrite — replaces `Addressed`/`Unfolded`.
Owner: the allocator run alone; the shapes and `directed`'s second route go. Until the corpus gate holds, the cheap step is to run the LIR passes before RegAlloc once and share them between routes (they see identical input).

**(e) Gain and risk.** Measured: -2.98 G (-36%) on d_faces with +1.2% static instructions in `D_DRAW_FACES`. Routes alone: -1.93 G with no change on this file; over the corpus route B wins 30% of functions (mean 1.8%) and loses 39%. The trial wins (19-26% of spilling functions, median 1.7-4.9%) are lost until the in-run decisions above replace them. The user decides.

### 3.2 Each round re-derives function-wide state

**(a) llrm.** One allocation of `D_DRAW_FACES` makes ~117 splits and ~124 spills ("regalloc split" 117x, "regalloc spill" 124x, "regalloc facts" 205x in the one-allocation run). With one allocation the allocator steps are 0.98 G: split placed 165, regalloc own 326, split carving 85, facts 150, intervals 87. Per round, these walk the whole function:
- `Indexes::patch` compares every block and recomputes `bases` over all blocks [analysis/intervals.rs:230-370](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/analysis/intervals.rs#L230-L370); the index is an `IndexMap<insn id, slot>` [analysis/intervals.rs:105](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/analysis/intervals.rs#L105).
- "intervals walk" walks every instruction after a full liveness dataflow even when few values are asked [analysis/intervals.rs:1576-1655](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/analysis/intervals.rs#L1576-L1655), [backend/allocate.rs:833](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L833); it is the fallback when a rewrite touched more than a third of the values [backend/allocate.rs:1678](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L1678).
- `_wanted` is a BFS over every hint [backend/allocate.rs:1463](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L1463); `gone` scans every placed value [backend/allocate.rs:2485](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L2485); `reclassed` compares every value's class [backend/allocate.rs:1798-1806](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L1798-L1806).
- Every `postings::following` call compares all blocks by pointer [backend/postings.rs:618-637](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/postings.rs#L618-L637); cached CFG facts take a lock and check the shape in O(B) per call [analysis/facts.rs:54-77](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/analysis/facts.rs#L54-L77).
- `split placed` [backend/splitkit.rs:1514](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/splitkit.rs#L1514) (1.2-1.4 M per call) builds a frequency map over every block [backend/spillplacement.rs:247](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/spillplacement.rs#L247) and, per candidate register, gathers and sorts every segment of that register's holders [backend/splitkit.rs:1081-1103](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/splitkit.rs#L1081-L1103).
- `split carving` rebuilds every covered block and the source block of each bridge [backend/splitkit.rs:615-709](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/splitkit.rs#L615-L709); a bridge block changes the shape, so the next round renumbers and rebuilds bundles, graph and depths.
- Last-chance recolouring: 531 calls, 114 M own over all 21 allocations, releasing and retaking `LiveUnion` holders per register tried [backend/allocate.rs:3530-3567](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L3530-L3567).

**(b) LLVM.** `selectOrSplitImpl` ([RegAllocGreedy.cpp:2422](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2422)) splits with `SplitEditor`, which creates the new registers' intervals from the instructions it rewrote ([SplitKit.cpp:1502](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SplitKit.cpp#L1502), `LiveRangeEdit::createFrom` [LiveRangeEdit.cpp:55](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRangeEdit.cpp#L55)); `SlotIndexes` give a new instruction an index in place ([SlotIndexes.h:531](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/CodeGen/SlotIndexes.h#L531)) and renumber only a local run when a gap is exhausted ([SlotIndexes.cpp:172](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SlotIndexes.cpp#L172)). Interference is `LiveRegMatrix`/`LiveIntervalUnion` queried per block ([LiveRegMatrix.cpp:111](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRegMatrix.cpp#L111)) through `InterferenceCache` ([InterferenceCache.cpp:131](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InterferenceCache.cpp#L131)); split placement solves on edge bundles only where the value is live ([SpillPlacement.cpp:366](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SpillPlacement.cpp#L366)). Recolouring is bounded by depth and interference ([RegAllocGreedy.cpp:95](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L95), [RegAllocGreedy.cpp:1957](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1957)). gcc's IRA colours the loop tree once ([ira-color.cc:3594](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-color.cc#L3594)); LRA rebuilds live ranges per constraint iteration ([lra.cc:2375](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra.cc#L2375)), a handful of times, not per spill.

**(c) Why slower.** Per round the cost is O(function), not O(edited ranges): ~4 M instructions per round on 1900 instructions, ~2000 instructions per LIR instruction per round. With rounds ∝ N, the allocator is N². The [allocation-algorithm note](../optimizations/allocation-algorithm.md)'s *body* class (59% of the allocator at `cells` 896) is this.

**(d) Instead.** Stable slot numbers stored on the instruction (not a hash map), and an edit API that returns the blocks and values it touched; every fact in `Facts` updated from that set. Per round:
- `Indexes`: patch only changed blocks' prefix sums; a bridge block gets slots without a fresh epoch.
- Postings: updated by the editor that changed a block, so `following` is O(1).
- CFG facts (frequency map, bundles, depths): kept per body and updated when a bridge block is added, not recomputed on shape change.
- `split placed`: per-block interference queries against the union (LLVM's `InterferenceCache`), no gather-and-sort.
- Recolouring: ask whether every holder has another free register before moving anything ([allocation-algorithm note](../optimizations/allocation-algorithm.md), part 4).

**(e) Gain and risk.** From ~4 M to at most 0.5 M per event over 241 events: ~0.85 G per allocation (16% of the one-allocation compile, about 3x that today). No output change; the risk is a stale fact, guarded by `LLRM_CHECK_FACTS`/`LLRM_CHECK_INTERVALS`.

### 3.3 Traced against gcc and LLVM

Same function, comparable input: llrm gets 450 values in 248 blocks into RegAlloc (1635 insns), gcc 442 pseudos in 206 blocks, LLVM 507 vregs in 258 blocks after coalescing. All three spill about 100 values.

| step | llrm, one allocation | llrm today (-O1) | gcc | LLVM |
|---|---|---|---|---|
| liveness / intervals built | `Facts::of` fresh 1, updated 204; full intervals walks 39 | fresh 21, updated 779, full walks 86 | IRA live ranges once (1282 ranges); LRA live ranges 6 builds | `LiveIntervals::analyze` 1; each interval calculated once (1196 = 1068 + 128 new) |
| allocation passes over the function | 1 | 21 (13 base + 8 trials), 2 routes of the LIR pipeline | IRA: 17 `color_pass`, one per loop region; LRA: 4 constraint passes over 1254, 163, 41 and 326 insns | 1 queue: 941 dequeues for 507 vregs |
| splits | 117 split steps, 59 splits made | 380 split steps | IRA region borders: 89 new pseudos; LRA inheritance 88, splits 0 | 99 `trySplit`, 31 region splits, 645 candidates costed |
| spills | 124 spill steps (103 spills), each a rewrite plus a facts update | 522 spill steps | 79 pseudos into 42 slots in **one** `lra_spill` | 99 `InlineSpiller::spill`, 2.8 M in all (28 k each) |
| eviction / recolouring | recolour 126 calls, 20 M | 531 calls, 114 M | none (IRA colours top-down per region) | 203 `tryEvict`, 55 evictions; last-chance recolouring 0 |
| new values | 372 | | 507 (419 reload + 88 inheritance) | 128 |
| cost (whole file) | 0.98 G | 3.71 G | IRA+LRA ~128 M | RAGreedy 27.5 M, LiveIntervals 5.5 M, coalescer 6.0 M |

Where the work differs, step by step:
- **A spill rewrites the body and re-derives facts.** In llrm each spill runs `spiller::spilled_apart` over the body ([backend/spiller.rs:308](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/spiller.rs#L308)), then `ranges::changes` compares every block ([analysis/intervals.rs:635](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/analysis/intervals.rs#L635)), then `Facts::of` patches indexes over all blocks, rebuilds masks, widths, hints and classes, and `_wanted` and `gone` scan every value ([backend/allocate.rs:2397-2582](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L2397-L2582)). That is ~4 M instructions per event. LLVM's spill touches only the uses of the spilled register: `InlineSpiller::spill` inserts the loads and stores, `LiveRangeEdit` creates the new registers' intervals, and `SlotIndexes` slots the new instructions into gaps (439 inserts, 28 local renumbers, 0 `repairIntervalsInRange`). That is 28 k instructions per spill, 140x less. gcc does not spill during colouring at all: IRA only marks an allocno as memory in its region, and LRA rewrites every spilled pseudo in one pass at the end.
- **Split pricing scans where LLVM caches.** LLVM costs 645 candidate registers for 15.1 M (23 k each). Its `InterferenceCache` keeps first and last interference per (physreg, block) and reuses it across candidates: 645 gets, 3510 block updates, 29 resets. llrm's `split placed` costs ~7 candidates per call at 1.4 M per call (~200 k per candidate, 9x). Each candidate gathers and sorts every segment of the register's holders ([backend/splitkit.rs:1081-1103](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/splitkit.rs#L1081-L1103)), and each call builds a frequency map over all blocks ([backend/spillplacement.rs:247](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/spillplacement.rs#L247)).
- **A carve renumbers the next round.** A bridge block changes llrm's CFG shape, so the next round starts a fresh slot epoch and rebuilds bundles, graph and depths ([backend/splitkit.rs:656-709](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/splitkit.rs#L656-L709), [analysis/intervals.rs:393](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/analysis/intervals.rs#L393)). LLVM's `SplitEditor` inserts copies into existing blocks and creates none. gcc's IRA adds 4 jumps at region borders in `ira_emit`, once.
- **Stage monotonicity is there in both; the retry is not.** llrm's queue also has Assign → Split → Spill stages and cascade numbers ([backend/allocate.rs:2043-2393](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L2043-L2393)). The extra work is around them: 2 trials and 2 routes run the whole allocator again from cold facts.

## 4. Analyses: alias, points-to, memory SSA, invalidation

**(a) llrm.** The cache is one `HashMap<TypeId, Box<dyn Cached>>` per function [passes.rs:863](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/passes.rs#L863), [passes.rs:966-1088](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/passes.rs#L966-L1088). Almost every pass preserves only `Dominators` and `Loops` (hoist [hoist.rs:134](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/hoist.rs#L134), gvn [gvn.rs:198-199](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/gvn.rs#L198-L199), fold [fold.rs:139](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/fold.rs#L139), decide [decide.rs:138](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/decide.rs#L138)). What survives an edit:
- `Pointers` (points-to) is reused unchanged or solved again; it has no `update` [manager.rs:1097-1123](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/manager.rs#L1097-L1123). Any block created or erased, any moved load or GEP, any erased load or store forces the solve [depends.rs:46-91](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/depends.rs#L46-L91).
- `Annotated` is neither SKIPS nor INCREMENTAL [manager.rs:1126-1161](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/manager.rs#L1126-L1161). Memory SSA's "moves only" update still recomputes `Annotated` and deep-compares it [memoryssa.rs:280-295](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/memoryssa.rs#L280-L295).
- Passes invalidate everything mid-run and ask again: fold [fold.rs:175-177](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/fold.rs#L175-L177) then floatfold re-asks FloatFacts [floatfold.rs:132-134](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/floatfold.rs#L132-L134); decide [decide.rs:161-164](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/decide.rs#L161-L164); jumpthread runs fold, algebraic, decide and dead, each after a full invalidation [jumpthread.rs:235-243](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/jumpthread.rs#L235-L243).
- Every segment between program passes gets a new `ModuleAnalyses` [passes.rs:1882-1885](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/passes.rs#L1882-L1885); Stamp and Interprocedural build their own [interprocedural.rs:351-367](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/interprocedural.rs#L351-L367). Function caches and the GlobalsAA/Summaries memos are lost each time.
- The same solve runs outside the manager: GlobalsAA does its own points-to per body [globalsaa.rs:403](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/globalsaa.rs#L403), Summaries its own point-values per body [manager.rs:629](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/manager.rs#L629) and two escape solves per procedure visit [alias.rs:2211-2213](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/alias.rs#L2211-L2213); decide runs its own `known_walked` [decide.rs:182](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/decide.rs#L182); gvn reruns `consts::known` [gvn.rs:267](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/gvn.rs#L267); `Shape::of` is called directly from 30+ passes.

Measured (own M, runs, cache misses from the `time` lookups):

| analysis | own M | runs | misses / hits |
|---|---|---|---|
| globals-aa | 212 | 12 | 12 / 42 |
| pointer-values | 207 | 178 | 107 / 74 |
| points-to | 200 | 180 | 127 / 281 |
| call-effects | 178 | 114 | 125 / 29 |
| through-memory | 168 | 63 | 63 / 33 |
| summaries (direct, values, points-to) | 322 | 180 | 12 / 30 |
| bounded | 117 | 44 | 39 / 27 |
| float-facts | 103 | 48 | 48 / 0 |
| annotated | 89 | 125 | 125 / 264 |
| dominated-edges | 78 | 196 | 151 / 49 |
| registers | 66 | 204 | 204 / 1028 |
| loops | 15 | 168 | 168 / 0 |

All analysis and summaries steps: 1945 M own (23%). Stamp (214 M) is one cold GlobalsAA plus one cold Summaries; "mir outer analyses" is 406 M over 19 function passes.

**(b) gcc and LLVM.** gcc: points-to runs twice per function, `pass_build_ealias` and `pass_build_alias` ([passes.def:90](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L90), [passes.def:213](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L213); [tree-ssa-structalias.cc:8008](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L8008)), and its result lives on the SSA names (`ptr_info_def`, [tree-ssanames.h:25](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssanames.h#L25)) for every later pass: `compute_may_aliases` is 2.6% of gcc, 16 M. Memory SSA is the IR's virtual operands, kept current by `update_ssa` ([tree-into-ssa.cc:3392](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-into-ssa.cc#L3392)). Dominators are kept until the CFG changes ([dominance.cc:731](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/dominance.cc#L731)); loops are kept across passes under `PROP_loops` ([tree-pass.h:218](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-pass.h#L218)) and repaired by `fix_loop_structure` ([loop-init.cc:200](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/loop-init.cc#L200)). What a pass breaks it declares through TODO flags executed once after it ([passes.cc:2052](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2052)).

LLVM: a pass returns `PreservedAnalyses` ([Analysis.h:111](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/Analysis.h#L111)); the manager invalidates only what is not preserved, and an analysis's own `invalidate` decides from its dependencies ([PassManagerImpl.h:172](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/PassManagerImpl.h#L172), `LoopInfo::invalidate` [LoopInfo.cpp:878](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/LoopInfo.cpp#L878)). There is no function-wide points-to: `BasicAAResult::alias` answers per query ([BasicAliasAnalysis.cpp:885](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L885)), memoised in `AAQueryInfo` ([AliasAnalysis.h:238](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/Analysis/AliasAnalysis.h#L238)); BasicAA is 1.6% of clang, ~9 M. MemorySSA is updated, not rebuilt ([MemorySSAUpdater.cpp:307](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSAUpdater.cpp#L307)), and its walker stops at `memssa-check-limit` 100 ([MemorySSA.cpp:75](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSA.cpp#L75)). GlobalsAA is computed twice per module (`require<globals-aa>`, `recompute-globalsaa` in the -O1 pipeline; [GlobalsModRef.cpp:495](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/GlobalsModRef.cpp#L495)).

**(c) Why slower.** 16 full points-to solves per body where gcc does 2 and LLVM none; a full `Annotated` after every changing pass; module facts rebuilt per segment. Each is O(function) or O(module) per edit instead of O(edit).

**(d) Instead.**
1. One `ModuleAnalyses` for the whole pipeline, passed through segments; program passes report the bodies they changed (the `edit_log` already exists, [passes.rs:1326](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/passes.rs#L1326)) and only those bodies' entries go.
2. Dependency-aware invalidation: an analysis declares what it reads (as `Shape::preserved` already does, [cfg.rs:554-556](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/cfg.rs#L554-L556)); `Annotated` survives when `Pointers`, `Registers`, `Counted` and `DominatedEdges` survive and the change log touched no load or store.
3. `Pointers::update` from the change log: a moved instruction keeps its facts; an erased load or store drops its own entry; only values reached through def-use from a changed pointer are re-solved (the `reached_by` worklist used by `DominatedEdges`, [manager.rs:1452-1513](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/manager.rs#L1452-L1513)).
4. One owner for points-to: GlobalsAA and Summaries ask the function cache for `Pointers`/`PointerValues` instead of solving their own.
5. No pass invalidates everything mid-run; fold, decide and jumpthread return what they changed and let the manager update.
6. decide and gvn ask `ThroughMemory`/`Registers`; `Shape::of` outside the manager goes.
Owner table: section 5.2.

**(e) Gain and risk.** Bringing points-to from ~16 solves per body to ~2 and keeping `Annotated` across non-memory edits removes most of the 0.82 G pointer family and about half of `annotated`/`accesses`/`call-effects`; one `ModuleAnalyses` removes most of the 0.82 G of cold module rebuilds (overlapping). Estimate 1.0-1.3 G (12-16%). No output change if updates are exact; the risk is a stale fact, so each update needs the `LLRM_CHECK_*` recompute-and-compare check the tree already uses.

### 4.1 Traced against gcc and LLVM

| | llrm (file) | gcc (fn) | LLVM (file) |
|---|---|---|---|
| whole-function points-to solves | `Pointers` 127, `PointerValues` 107, plus GlobalsAA's per-body and Summaries' per-body solves | 2 (`ealias`, `alias`), 3 iterations each; 840 of 1285 and 593 of 1027 variables unified offline | 0 |
| per-query alias | inside `Annotated` (125 rebuilds of every load's and store's `MemRef`) | `stmt_may_clobber_ref_p_1` 14,275, `walk_non_aliased_vuses` 832 (limit 1000 each), DSE `ref_maybe_used_by_stmt_p` 914 | `BasicAAResult::alias` 2904, 36% answered by `AAQueryInfo` or trivially; 5.7 M |
| memory SSA | `Accesses` 98 rebuilds; updated only when every change is a move | virtual operands in the IR; `update_ssa` 40 calls (4 only-virtuals) | `MemorySSA` 20 builds (4 on the fn); walker 245 calls; updater: 72 removes, 21 moves, 19 batched updates |
| cost | 0.82 G pointer family, 466 M accesses (incl.), 472 M annotated (incl.) | `compute_may_aliases` 2.5% (16 M) | BasicAA 5.7 M, walker 6.6 M, MemorySSA builds 4.1 M |

The difference is when facts are produced, not just who owns them. gcc solves points-to once per function and stores the result on each SSA name, so every later pass reads it there. A transform that makes a new name copies or derives the info: `duplicate_ssa_name_ptr_info` ([tree-ssanames.cc:725](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssanames.cc#L725)). A pass whose rewrite makes flow-sensitive info stale drops only that part: `reset_flow_sensitive_info` ([tree-ssanames.cc:803](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssanames.cc#L803)). LLVM never solves: it answers each query from the two pointers' def chains and caches within a batch. llrm solves the whole function again after any edit that moves or erases a pointer-modelled instruction ([depends.rs:46-91](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/depends.rs#L46-L91)), then rebuilds `Annotated`, which no edit preserves.

## 5. Liveness, dataflow and fact ownership

### 5.1 Traced against gcc and LLVM

| | llrm | gcc (fn) | LLVM (fn) |
|---|---|---|---|
| CFG dominators | 187 computations, 208 hits (file) | 152 calls: 74 cached, 33 renumber only, 45 full | IR DomTree built 9 times for 348 pass runs; 118 incremental batches (file) |
| loops | 168 computations, 0 hits (file) | `flow_loops_find` 32, `fix_loop_structure` 24 | LoopInfo 5 |
| machine liveness | allocator: 1 fresh + 204 updates per allocation plus 39 full walks; peephole lane liveness rerun in full after any change ([backend/liveness.rs:331-410](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/liveness.rs#L331-L410)); floatassign 2 full liveness per run | `df_analyze` 27 calls, 79 worklist solves; 1.64 visits per block; 720 dirty blocks recomputed (~13%); 4458 `df_insn_rescan`, 1552 of them no-ops, 283 deferred into 80 batch flushes; 7.5% of the file | LiveVariables 1, LiveIntervals 1, `handleMove` 81, `shrinkToUses` 103 |

gcc keeps each block's local LR sets and recomputes them only for blocks marked dirty (`df_set_bb_dirty`, [df-core.cc:1601](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1601); local recompute over `out_of_date_transfer_functions`, [df-problems.cc:968](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L968)). The global solve then starts from the previous solution. llrm's LIR liveness has three owners (allocator `live_rows`, peephole lanes, floatassign's `allocate::live`), and each recomputes the whole function.

### 5.2 Who owns each fact

Counts are d_faces -O1 (11 bodies), from the `time` lookups (misses = computations).

| fact | llrm owner, computations on d_faces | gcc | LLVM |
|---|---|---|---|
| dominators | `Dominators`, incremental subtree update [dominators.rs:44-76](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/dominators.rs#L44-L76); 187 misses, 208 hits; also inside 30+ direct `Shape::of` | kept until the CFG changes [dominance.cc:720](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/dominance.cc#L720) | `DominatorTreeAnalysis`, preserved; `DomTreeUpdater` [DomTreeUpdater.h:28](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/Analysis/DomTreeUpdater.h#L28) |
| loops | `Loops`, never incremental; 168 misses, 0 hits | loop tree kept across passes (`PROP_loops`), `fix_loop_structure` | `LoopInfo`, preserved by loop passes |
| liveness (MIR) | not cached; rebuilt per spill forecast [spill.rs:1351-1356](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/spill.rs#L1351-L1356) | df problems, solved on dirty blocks | not kept on IR; computed in codegen |
| liveness (LIR) / intervals | `Facts::of` from scratch 21x, updated 779x; full walks 86x; peephole lanes per change | IRA live ranges once [ira-build.cc:3469](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L3469); LRA per constraint iteration | `LiveIntervals` once, edited by `LiveRangeEdit`/`handleMove` [LiveIntervals.cpp:1559](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L1559) |
| points-to | `Pointers` 127 full solves + `PointerValues` 107 + GlobalsAA and Summaries' own; 0.82 G | 2 solves per function, kept on SSA names; 16 M | none; BasicAA per query with `AAQueryInfo`; ~9 M |
| memory SSA | `Accesses`, 98 rebuilds (only moves update) [memoryssa.rs:237-297](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/memoryssa.rs#L237-L297) | virtual operands in the IR, `update_ssa` | `MemorySSA`, `MemorySSAUpdater` |
| per-access refs | `Annotated`, 125 rebuilds, never kept | `ao_ref` per query | `MemoryLocation` per query |
| call mod/ref | `CallEffects` 125, `GlobalsAA` 12, `Summaries` 12 per module | ipa-modref, ipa-pure-const once; local-pure-const 2 | GlobalsAA 2 per module; function-attrs once per SCC |
| SCEV-like | `Counted` 146 (per-loop update), `Followers` 94 | scev cache per loop pass; niter on the loop | `ScalarEvolution` lazy per query, `forgetLoop` [ScalarEvolution.cpp:8494](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/ScalarEvolution.cpp#L8494) |
| constants / ranges | `Registers` 204 (update), `ThroughMemory` 63 + decide's own, `Bounded` 39, `FloatFacts` 48 | ranger on demand, cached | `LazyValueInfo`, known bits per query |

Where llrm recomputes, clones or rewalks per query or per round: points-to and `Annotated` after any memory edit (section 4); module facts per segment (section 4); `Shape::of` from passes; MIR liveness per forecast; GVN and hoist clones (section 6); allocator facts per allocation and function walks per round (section 3.2).

## 6. Optimizer pipeline

**(a) llrm.** `Fixed::fixed` [pipeline.rs:948-1009](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/pipeline.rs#L948-L1009) reruns 19 passes per round until a round changes nothing, at most `max(16, blocks+insts+1)` rounds. Only five passes narrow `reads()` (passes.rs default ALL, [passes.rs:1664-1666](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/passes.rs#L1664-L1666)), so fold, hoist and gvn rerun after any edit. `D_DRAW_FACES` takes 5 rounds and 101 pass runs. Over all bodies (LLRM_DEBUG=runs):

| round | runs | changed | work with analyses M |
|---|---|---|---|
| 1 | 231 | 46 | 855 |
| 2 | 220 | 32 | 407 |
| 3 | 196 | 5 | 276 |
| 4 | 21 | 2 | 203 |
| 5 | 18 | 0 | 163 |

Runs that changed nothing: 0.88 G with their analyses. Inside passes, pricing is done by copying the function: hoist clones the whole function per loop and shrink step and runs weighted cost, full liveness and a spill forecast [hoist.rs:248-332](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/hoist.rs#L248-L332); its invariant set grows by rescanning the loop to a fixed point [hoist.rs:536-573](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/hoist.rs#L536-L573) (hoist 366 M own, 46 runs); gvn forks the function every run and assigns the fork back [gvn.rs:381-424](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/gvn.rs#L381-L424) (193 M own); the candidate passes clone and run a nested `Fixed` on the copy [candidate.rs:139-160](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/candidate.rs#L139-L160).

**(b) gcc and LLVM.** gcc's `passes.def` order runs once per function at -O1 (fre 3 times, ccp 5, dse 5, lim 4; [passes.def:185](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L185)); passes that iterate do so internally (FRE `may_iterate`, [passes.def:215](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L215)). LLVM's -O1 function pipeline ([PassBuilderPipelines.cpp:409](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Passes/PassBuilderPipelines.cpp#L409)) runs InstCombine 8 times with `max-iterations=1` and an internal worklist ([InstructionCombining.cpp:5126](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5126)); loop passes run per loop through `FunctionToLoopPassAdaptor` ([LoopPassManager.cpp:197](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LoopPassManager.cpp#L197)). LICM decides hoisting from MemorySSA without copying the function ([LICM.cpp:296](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LICM.cpp#L296)); it is 3.8% of clang, ~21 M.

**(c) Why slower.** Each round reruns every pass over the whole function; rounds 3+ change 7 things for 0.64 G. Copy-and-price makes a pass O(loops x function) instead of O(loop).

**(d) Instead.** As the [pipeline-design note](pipeline-design.md): round 1 in order, round 2 only over the blocks and loops round 1's change log reaches (per-pass worklist from `changes_since`), nothing after. Hoist and gvn price a move from the move itself: a register-pressure delta over the loop's blocks from the cached liveness, not a cloned function's forecast. gvn edits in place and keeps the change log (`fork` loses marks taken before it, [module.rs:613-632](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/module.rs#L613-L632)).

**(e) Gain and risk.** Rounds 3+ -0.64 G (7.8%); a worklist round 2 removes most of the 0.88 G idle work (overlapping with section 4). A hard 2-round cap made 116 bench rows worse; a worklist round 2 keeps the same rewrites as round 2 today, and rounds 3+ changed 7 things on d_faces. The risk is the cases a later round fixes (sieve +15% in the [pipeline-design note](pipeline-design.md)'s cap); they must be named before the cap.

### 6.1 Traced against gcc and LLVM

| | llrm (fn) | gcc (fn) | LLVM (fn) |
|---|---|---|---|
| pass runs | 101 in `Fixed` (5 rounds) + 19 later = ~120 | 154 executed (289 invocations) | 348 new-PM runs + 198 codegen |
| analysis computations | 2508 for the file (~900 pass runs: 2.8 per run) | TODOs: 24 cleanup_cfg, 31 update_ssa, 2 rebuild_alias | 92 for the fn (0.26 per run), 60 invalidations; 505 for 951 runs in the file (0.53) |
| skipped as unchanged | 1 "settled", 0 "unreached" on the fn | gate per pass | InstCombine `LastRunTracking`: 27 of 73 runs skipped |
| iteration inside a pass | per pass, then the whole list again | FRE once per block at -O1; `ssa_propagate` worklists (7 runs) | InstCombine 1 iteration per run with a worklist; SimplifyCFG per-block local loop |
| cost per pass run | ~3.3 G (91% of the 3.64 G MIR stage) / ~120 = **~28 M** | ~4 M (all of cc1 / 154) | ~0.4 M (83% of the 176 M IR pipeline / 348) |

The number of pass runs is not the difference: LLVM runs three times as many passes on this function. The cost of a run is the difference, at ~65x LLVM's. Two things drive it: the analyses each run recomputes (2.8 computations per run against LLVM's 0.53), and passes that walk or clone the whole function per loop or per candidate (section 6).

### 6.2 Pass list at -O1

Runs on d_faces (11 bodies; llrm counts include the passes run once after `Fixed`). gcc counts are per function from `-fdump-passes`; clang's from `-print-pipeline-passes` and `-debug-pass=Structure`.

| job | llrm runs (own M) | gcc per fn | clang per fn |
|---|---|---|---|
| CSE / value numbering | gvn 35 (193) | fre 3, dom 3 | early-cse 2; no GVN at -O1 |
| folding / combining | fold 47 (22, 491 with analyses), algebraic 46 (109) | ccp 5, forwprop 4 | instcombine 8 x 1 iteration |
| branch decisions / threading | decide 35 (165), jumpthread 11 (90) | ethread, dom threading, vrp none at -O1 | none at -O1 |
| invariant motion | hoist 46 (366), loopmotion 35 (9) | lim 4 | licm 3 |
| DSE | dse 35 (67) | dse 5 | none at -O1 |
| SROA / promotion | sroa 11, promote 35 (55) | esra, sra | sroa 4, mem2reg 1 |
| induction | indvars 46 (96), lcssa 46 (35) | ivcanon 1, ivopts 1 | indvars 1 |
| strength reduction | lsr 11 (87) | ivopts 1 | LSR 1 (codegen) |
| IPA | stamp 1 (118), interprocedural 1 (127), globals-aa 12 | einline, inline, modref, pure-const once | inliner per SCC, function-attrs |
| instruction selection | isel 11 (66) | expand + combine 1 | SelectionDAG 1 (10.5% of clang) |
| register allocation | 21 allocations (13 base + 8 trials) | IRA 1, LRA 1 | greedy 1 |
| LIR clean-up | peephole 13 (344), jumps 13 (166), floatassign 13 (137), coalesce 13 (92) | cse2, combine, ce, bbro once | peephole-opt, machine-cse, branch-folder once |

No counterpart and above 1% of the compile: `decide` (2.0%, branch outcomes from SCCP-style propagation and ranges per round), `mir stamp` (2.6% total, cold module facts), `floatassign` (1.7%, the x87 stack assignment; gcc's `reg-stack` is one pass), `regalloc candidates` (2.4%, trial shapes), the second route (23%).

## 7. Selection and LIR clean-up

**(a) llrm.** Per run (13 runs on d_faces): peephole 344 M, jumps 166 M, floatassign 137 M, coalesce 92 M, phielim 63 M (lir total 981 M).
- Peephole is ~40 whole-body sub-passes in a fixed order [backend/peephole.rs:310-389](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/peephole.rs#L310-L389); each generated group rebuilds every block and the body's maps whether or not a rule matched [backend/peep/walk.rs:507-516](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/peep/walk.rs#L507-L516); lane liveness reruns in full after any change [backend/liveness.rs:331-410](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/liveness.rs#L331-L410).
- Jumps loops merge and DCE to a fixed point; each round clones the body, hashes a tail key that clones `Semantics` per instruction, and deep-compares the body [backend/jumps.rs:199-207](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/jumps.rs#L199-L207), [backend/jumps.rs:1556-1583](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/jumps.rs#L1556-L1583); `threaded` restarts from block 0 [backend/jumps.rs:1183-1189](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/jumps.rs#L1183-L1189).
- FloatAssign clones every instruction's `Semantics` and rebuilds every block twice before its no-float exit [backend/floatassign.rs:154-306](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/floatassign.rs#L154-L306), which also breaks block identity for identity-keyed caches downstream; with floats it runs full liveness twice [backend/floatassign.rs:1935-1946](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/floatassign.rs#L1935-L1946) and walks the whole body once per spilled float [backend/floatassign.rs:2005-2045](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/floatassign.rs#L2005-L2045).

**(b) gcc and LLVM.** gcc's RTL passes edit insns in place in a doubly linked chain and update dataflow per insn (`df_insn_rescan`, [df-scan.cc:1013](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-scan.cc#L1013); `df_analyze` solves only dirty blocks, [df-core.cc:1265](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1265)). LLVM's `PeepholeOptimizer`, `MachineCSE` and `BranchFolder` run once, edit `MachineInstr`s in an ilist ([MachineBasicBlock.h:144](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/CodeGen/MachineBasicBlock.h#L144)) and find uses through `MachineRegisterInfo` use lists ([MachineRegisterInfo.cpp:273](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/MachineRegisterInfo.cpp#L273)).

**(c) Why slower.** O(N) rebuild per sub-pass regardless of matches, and full liveness per change: ~40 x O(N) per run, times 13 runs.

**(d) Instead.** Blocks rebuilt only when a rule fired (return the input `Arc` unchanged otherwise); a dirty-block set carried through the peephole sequence so a sub-pass visits only blocks a previous one changed after its first run; lane liveness updated per changed block; jumps compares by a change flag, not `PartialEq`; floatassign tests the opcode before cloning and exits before rebuilding when there are no floats.

**(e) Gain and risk.** About half of the 0.98 G (0.5 G, 6%); a third of it disappears anyway with one allocation (3.1). No output change.

### 7.1 Traced against gcc and LLVM

| | llrm | gcc | LLVM |
|---|---|---|---|
| selection | isel 11 runs, 66 M (file) | expand 2.0%, vregs 1153 recog | SelectionDAG 156 DAGs, 417 combine runs, 13,276 node visits; 53.7 M (file) |
| combine / peephole | peephole 13 runs x ~40 sub-passes, each rebuilding every block: 344 M | combine: 796 tries, 64 combined, 1356 recog: 4.7% (30 M) | PeepholeOptimizer 1 run 1.8 M, MachineCSE 1.9 M, MachineCopyPropagation 3.2 M |
| branch clean-up | jumps 13 runs, a merge/DCE fixed point with a full-body clone and deep compare per round: 166 M | bbro 0.6%, cfg cleanup 14 calls | BranchFolder 2 runs, 965 block visits: 1.7 M |
| x87 | floatassign 13 runs, 137 M | reg-stack 0.75% (5 M) | in isel/FP stackifier |

Selection costs the same in all three (llrm 66 M against clang's 54 M). The clean-up after it is where llrm is 10-50x: work proportional to the whole body per sub-pass, run once per route.

## 8. Representation

| | llrm | gcc | clang |
|---|---|---|---|
| instructions M | 8240 | 631 | 552 |
| cycles M | 3655 | 287 | 288 |
| IPC | 2.25 | 2.20 | 1.92 |
| LLC misses per k instr | 6.6 | 3.7 | 5.0 |
| L1d miss rate | 3.8% | 2.8% | 1.9% |
| branch misses per k instr | 4.0 | 5.4 | 5.3 |
| page faults | 791 | 3617 | 6030 |
| malloc/free, self samples | 10.0% (mimalloc) | ~5% | ~4% |

llrm's self samples by container: BTree 9.6%, hash maps 10.8% (FxHash 6.1%, SipHash 0.8% in `llrm-mir`'s std maps, [hash.rs:6-12](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/hash.rs#L6-L12)), memmove 3.2%, clone 2.6%, drop 2.5%, sort 2.0%. Text rendering for the cycle check ([pipeline.rs:964](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/pipeline.rs#L964), [pipeline.rs:1004](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/pipeline.rs#L1004)) is 0.5%.

What the representation costs:
- MIR: plain `Vec` arenas with ids and maintained use lists ([module.rs:263-306](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/module.rs#L263-L306)), close to gcc's and LLVM's. Block instruction lists are `Vec<InstId>`, so insert and erase are O(block) ([edit.rs:186-265](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/edit.rs#L186-L265)) against LLVM's ilist ([BasicBlock.h:63](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/BasicBlock.h#L63)) and gcc's linked `gimple` ([gimple.h:286](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple.h#L286)). `replace_all_uses_with` is O(U²) through `retain` per use ([edit.rs:61-72](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/edit.rs#L61-L72), [edit.rs:474-483](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/edit.rs#L474-L483)); LLVM's `doRAUW` is O(U) ([Value.cpp:503](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/IR/Value.cpp#L503)).
- LIR: immutable `Arc<[Arc<Insn>]>` blocks, edited by copying the block ([model/lir.rs:640-653](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/model/lir.rs#L640-L653)); a fat `Insn` with `Semantics`, `BTreeSet` clobbers and `Vec` operands ([model/lir.rs:113-146](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/model/lir.rs#L113-L146)); no use lists, so occurrences come from the side index `Postings` or from scans. This is what makes "touched only" hard in 3.2 and 7: a pass cannot edit one instruction without rebuilding its block, and cannot find a value's uses without the side index.
- Orderedness: 170 `BTreeMap/BTreeSet` keyed by ids in `crates/opt` against 96 dense `IdMap/IdSet`.

IPC and the cycle ratio say this is a constant factor on the work (allocation, trees and hashes are ~25% of samples), not the 13x. Changing containers alone would move the compile by less than that 25%, which is what the micro-optimization PRs have shown.

## 9. How they stay fast

| job | best | why, measured |
|---|---|---|
| register allocation | LLVM greedy | 27.5 M for ~100 spills; each live interval is computed once; spills are local edits |
| live ranges under edits | LLVM `SlotIndexes` + `LiveRangeEdit` | 439 inserts, 28 local renumbers, 0 repairs |
| split costing | LLVM `InterferenceCache` | 23 k per candidate, against llrm's 200 k |
| points-to | gcc, solved on SSA names | 2 solves per function, 16 M, kept by every later pass |
| machine dataflow | gcc df | dirty blocks only (13%), 1.64 visits per block |
| memory SSA | LLVM `MemorySSAUpdater` | 20 builds for the file; edits update it |
| analysis invalidation | LLVM result-level `invalidate` | 0.53 computations per pass run |
| pass iteration | LLVM InstCombine and gcc FRE | one iteration with a worklist inside; reruns on unchanged IR skipped (27 of 73) |
| what changed | **llrm change log** (`changes_since`, `footprint::of`) | finer than `PreservedAnalyses` or TODO flags: it names the instructions |
| choosing between alternatives | **llrm executed-work cost model** (`_emitted`, `profit`) | prices output in instructions and memory operands; gcc and LLVM use heuristics |
| catching stale facts | **llrm `LLRM_CHECK_*`** recompute-and-compare | already exists for facts, intervals, postings and summaries |

llrm's own strengths are the edit log and the exact cost model. It uses them wrongly. The log only decides whether to skip a pass, not how to update a fact. The cost model is applied by running whole alternatives (six allocations, cloned functions) instead of pricing a decision where it is made.

Each entry gives the mechanism, the invariant that keeps it correct, where it is, what it did on d_faces, and llrm's version of it.

**9.1 Lazy analyses with result-level invalidation (LLVM).**
- Mechanism: an analysis runs on the first `getResult`. After a pass, each cached result is asked whether the pass's `PreservedAnalyses` and the results it depends on still cover it.
- Invariant: a result is valid until a pass that does not preserve it, or one of its inputs, changes the unit.
- Where: [PassManagerImpl.h:137](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/PassManagerImpl.h#L137), [PassManagerImpl.h:172](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/PassManagerImpl.h#L172), [Analysis.h:111](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/Analysis.h#L111).
- d_faces: 92 analysis runs for 348 pass runs on the fn.
- llrm: the cache exists ([passes.rs:966-1088](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/passes.rs#L966-L1088)); what is missing is per-result invalidation. Add `fn invalidate(&self, preserved: &PreservedAnalyses, changes: &[Change]) -> bool` to `Analysis`, defaulting to today's rule. `Annotated` implements it as "invalid iff Pointers, Registers, Counted or DominatedEdges is invalid, or a load or store changed". Owner: the manager in `passes.rs`. Protocol: passes keep returning what they preserve, and results decide for themselves.

**9.2 Pass properties and TODO flags (gcc).**
- Mechanism: a pass declares the properties it requires, provides and destroys, plus TODO flags such as `TODO_update_ssa_only_virtuals` and `TODO_cleanup_cfg`. The fix-ups run once after the pass, only when flagged.
- Invariant: a property holds until a pass that destroys it runs.
- Where: [passes.cc:2052](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2052), [passes.cc:2580](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2580), [tree-pass.h:218](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-pass.h#L218).
- d_faces: 344 `execute_todo` calls, 24 cfg cleanups, 2 alias rebuilds, 5.5% of the file.
- llrm: the change log already carries the facts the TODOs encode (`footprint::of`, [footprint.rs:64-88](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/footprint.rs#L64-L88)). Use it to invalidate (9.1), not just to skip passes.

**9.3 Skip a pass on its own unchanged output (LLVM `LastRunTracking`).**
- Mechanism: InstCombine records the IR state it left and skips a rerun on the same state.
- Invariant: the pass is idempotent on its own fixpoint.
- Where: [LastRunTrackingAnalysis.h:46](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/Analysis/LastRunTrackingAnalysis.h#L46), [InstructionCombining.cpp:5609](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5609).
- d_faces: 27 of 73 runs skipped.
- llrm: `settled` is this ([pipeline.rs:966-983](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/pipeline.rs#L966-L983)), but fold, hoist and gvn read `Footprint::ALL` ([passes.rs:1664-1666](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/ir/llrm-mir/src/passes.rs#L1664-L1666)), so any edit wakes them. Give each a measured `reads()` footprint. Idle runs cost 0.88 G with their analyses.

**9.4 One outer iteration, worklists inside (LLVM InstCombine, gcc FRE/ccp).**
- Mechanism: a pass seeds a worklist and re-adds the users of what it changed. There is no outer loop over the pass list.
- Invariant: local fixpoint through def-use within the pass.
- Where: [InstructionCombining.cpp:5126](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5126), [tree-ssa-propagate.cc:434](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-propagate.cc#L434), [tree-ssa-sccvn.cc:8753](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-sccvn.cc#L8753).
- d_faces: 1 iteration per InstCombine run; FRE visits each block once.
- llrm: round 2 of `Fixed` runs each pass only on the blocks and loops reached by round 1's `changes_since`; there is no round 3. Rounds 3-5 cost 0.64 G.

**9.5 Points-to stored on SSA names, solved once (gcc).**
- Mechanism: one constraint solve per function, with offline variable unification. Results live in `ptr_info` on each SSA name. New names copy or derive it; a rewrite that breaks flow-sensitive bits resets only those.
- Invariant: an SSA name's targets are a property of its definition, valid at every use.
- Where: [tree-ssa-structalias.cc:8008](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L8008), [tree-ssanames.h:25](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssanames.h#L25), [tree-ssanames.cc:725](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssanames.cc#L725), [tree-ssanames.cc:803](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssanames.cc#L803).
- d_faces: 2 solves, 3 iterations each, 16 M.
- llrm: split `PointerValues` into a per-value table keyed by `ValueId`, updated by def-use. A new value takes its transfer function from its operands (`_direct`); an erased value is dropped; a phi or select whose arms changed requeues its users. The flow-sensitive parts (spill-slot cells, `EscapedBefore`) become lazy per-query answers with a cache, as LLVM's `EarliestEscapeAnalysis` does ([BasicAliasAnalysis.cpp:213](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L213)), invalidated per instruction ([BasicAliasAnalysis.cpp:246](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L246)). Owner: `Pointers`. GlobalsAA and Summaries read it instead of solving.

**9.6 Memory SSA updated, walker cached and bounded (LLVM; gcc virtual operands).**
- Mechanism: edits call the updater: `removeMemoryAccess` reconnects a removed def's users to its defining access, `moveToPlace` moves, `insertDef` renames below. A use caches its clobber, and walks stop at 100 alias checks.
- Invariant: every access's defining access is correct after each update.
- Where: [MemorySSAUpdater.cpp:1290](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSAUpdater.cpp#L1290), [MemorySSAUpdater.cpp:1185](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSAUpdater.cpp#L1185), [MemorySSAUpdater.cpp:307](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSAUpdater.cpp#L307), [MemorySSA.cpp:75](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSA.cpp#L75).
- d_faces: 20 builds for the file, 245 walker calls.
- llrm: `Accesses::update` handles erased loads (drop the use), erased stores (users take its defining access), new stores (insertDef) and moves. It stops recomputing and comparing `Annotated` ([memoryssa.rs:280-295](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-analysis/src/memoryssa.rs#L280-L295)). 98 rebuilds become updates.

**9.7 Dataflow with per-block local sets, dirty blocks and deferred rescans (gcc df).**
- Mechanism: each problem keeps per-block gen/kill. An edited insn marks its block dirty; `df_analyze` recomputes the local sets of dirty blocks only and re-solves from the old solution with an RPO worklist. Rescans may be deferred and batched, and a no-op rescan returns early.
- Invariant: a block's transfer function changes only when one of its insns changed.
- Where: [df-core.cc:1265](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1265), [df-core.cc:1601](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1601), [df-problems.cc:968](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L968), [df-scan.cc:1013](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-scan.cc#L1013), [df-scan.cc:1210](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-scan.cc#L1210).
- d_faces: 27 analyses, 13% of blocks dirty, 1.64 visits per block.
- llrm: one LIR liveness owner keyed by block identity: per-block use/def and live-in/out, cached on `LirBlock` (blocks are already `Arc` and replaced on edit, so a new `Arc` is the dirty mark). The allocator, peephole lanes and floatassign ask it. Re-solving starts from the blocks whose `Arc` changed.

**9.8 SlotIndexes with gaps, intervals edited, never rebuilt (LLVM).**
- Mechanism: an index lives on the instruction, spaced so that an insert takes a midpoint, with local renumbering when a gap runs out. `LiveRangeEdit` computes the intervals of new registers only; `handleMove` and `shrinkToUses` patch existing ones.
- Invariant: index order is instruction order, and every edit updates the intervals it affects.
- Where: [SlotIndexes.h:531](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/CodeGen/SlotIndexes.h#L531), [SlotIndexes.cpp:172](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SlotIndexes.cpp#L172), [LiveRangeEdit.cpp:55](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRangeEdit.cpp#L55), [LiveIntervals.cpp:1559](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L1559), [LiveIntervalCalc.cpp:41](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervalCalc.cpp#L41).
- d_faces: 1 analyze, 1196 interval calculations (each once), 28 renumbers, 0 repairs.
- llrm: gapped slots exist ([analysis/intervals.rs:132](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/analysis/intervals.rs#L132)), but they live in an `IndexMap` ([analysis/intervals.rs:105](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/analysis/intervals.rs#L105)) and are patched by comparing every block. Store the slot on `Insn`, have the editor (spiller, splitkit) return the changed blocks and new values, and compute intervals for exactly those. Drop the ">1/3 touched" full walk ([backend/allocate.rs:1678](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/allocate.rs#L1678)). A bridge block takes slots inside its neighbours' gap, so no fresh epoch.

**9.9 InterferenceCache and per-block interference (LLVM).**
- Mechanism: first and last interference per (physreg, block), filled lazily and validated by the union's tag. Region-split costing reads it across all candidates.
- Invariant: an entry is used only while its reg units' union tags are unchanged.
- Where: [InterferenceCache.cpp:62](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InterferenceCache.cpp#L62), [InterferenceCache.cpp:131](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InterferenceCache.cpp#L131), [RegAllocGreedy.cpp:1063](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1063).
- d_faces: 645 candidates for 15.1 M.
- llrm: `LiveUnion` gets a per-register version counter. `split placed` asks a cache for `(first, last)` interference in a block per register, instead of gathering and sorting holders' segments ([backend/splitkit.rs:1081-1103](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/backend/llrm-core/src/backend/splitkit.rs#L1081-L1103)). The block frequency map moves out of `Placement::new` into a per-body fact.

**9.10 Local spill code (LLVM InlineSpiller; gcc LRA).**
- Mechanism: a spill inserts loads and stores at the spilled register's uses and defs and folds where it can. The new short ranges go back on the queue at `RS_Done` and are never split again. LRA spills all memory pseudos in one pass, and its constraint loop revisits only the insns pushed onto `lra_constraint_insn_stack`.
- Invariant: the new ranges are local to one instruction, so no other range's interference changes except at those points.
- Where: [InlineSpiller.cpp:1285](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InlineSpiller.cpp#L1285), [RegAllocGreedy.cpp:2516](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2516), [lra-spills.cc:604](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-spills.cc#L604), [lra.cc:1775](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra.cc#L1775), [lra.cc:2371](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra.cc#L2371).
- d_faces: LLVM 99 spills for 2.8 M; LRA's constraint passes shrink 1254 → 163 → 41 insns.
- llrm: `spilled_apart` returns the edit (blocks, removed and added points, new values). `Facts` updates only those values; the body-class steps (spill cleanup, `_wanted`, `gone`, `reclassed`) run on the edit's values. Reload values are queued as unsplittable with their intervals computed from their one or two points.

**9.11 Regional allocation and cost classes (gcc IRA).**
- Mechanism: one allocno per pseudo per loop region, coloured top-down with caps carrying inner info up. Costs run once, over each pseudo's cost class only. Conflicts are bit vectors over min/max conflict ids, plus hard-reg sets.
- Invariant: a region's colouring only needs its own allocnos and the caps of its children.
- Where: [ira-build.cc:3469](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L3469), [ira-color.cc:3594](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-color.cc#L3594), [ira-costs.cc:1757](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-costs.cc#L1757), [ira-conflicts.cc:91](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-conflicts.cc#L91).
- d_faces: 17 regions, 2041 allocnos, 11,347 conflicts in 336 KB, 1 cost pass.
- llrm: this is what the `Scoped` trial approximates by running a whole second allocation. The in-run form is a loop-local piece at the Split stage ([single-allocation note](../optimizations/single-allocation-decisions.md), step 2), without a second allocation.

**9.12 Batched dominator updates (LLVM) and cached dominance (gcc).**
- Mechanism: CFG edits queue edge updates, applied in a batch before the next query; gcc keeps `DOM_OK` until a CFG change.
- Where: [DomTreeUpdater.h:28](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/Analysis/DomTreeUpdater.h#L28), [dominance.cc:724](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/dominance.cc#L724).
- d_faces: 118 incremental batches against 100 rebuilds; gcc 74 of 152 calls cached.
- llrm: dominators are already incremental. Loops are not: 168 computations, 0 hits, but only 15 M. Low priority. The 30+ direct `Shape::of` calls should ask the manager.

**9.13 Scoped tables over the dominator tree, no copies (LLVM EarlyCSE, gcc dom).**
- Mechanism: one dominator-tree DFS with a scoped hash table, edited in place.
- Where: [EarlyCSE.cpp:654](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L654), [domwalk.cc:273](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/domwalk.cc#L273).
- d_faces: EarlyCSE 8.2 M for 22 runs.
- llrm: gvn forks the whole function every run and assigns the fork back ([gvn.rs:381-424](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/gvn.rs#L381-L424)): 193 M own for 35 runs, and the fork drops marks taken before it. At -O1 one numbering is made, so it can edit `unit.function` in place.

**9.14 Price without copying (LLVM LICM, gcc lim).**
- Mechanism: hoisting is decided per instruction from MemorySSA and alias queries; register pressure is not forecast by copying the function.
- Where: [LICM.cpp:296](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LICM.cpp#L296).
- d_faces: LICM 12.0 M for 77 loop visits.
- llrm: hoist clones the function per loop and per shrink step and runs full liveness and a forecast on the copy ([hoist.rs:248-332](https://github.com/ali-mosavian/llrm/blob/0b0770d37a113eb051460385d7b3bba98306ba8b/crates/opt/llrm-transforms/src/hoist.rs#L248-L332)): 366 M own. Price a move from a pressure delta over the loop's blocks, using the cached liveness of 9.7.

## 10. The design

### 10.1 The rule, and the protocol that enforces it

**Edit → owners → facts.** Every mutation of MIR or LIR goes through an editor that appends to the change log (MIR does this today). An analysis declares:
- `depends`: the analyses it reads;
- `update(&self, changes) -> Option<Self>`: patch from the changes, or `None` to recompute;
- `invalidate(changes, preserved) -> bool`: LLVM's result-level check, driven by the log instead of a pass's word.

The manager calls `update` on the next `get`, never eagerly. A recompute is legal but counted, and a function where computations per pass run rise above 1 is a failure the gate reports.

**One owner per fact, no private solves.** GlobalsAA, Summaries, decide, gvn and the 30+ direct `Shape::of` callers ask the manager. A pass that changes the IR mid-run reports the edit and does not call `invalidate(none)`.

**One cache for the pipeline.** A single `ModuleAnalyses` lives through all segments. A program pass reports the bodies it changed (`edit_log` exists), and only their entries go.

**Checked, not trusted.** Every `update` has an `LLRM_CHECK_<fact>` mode that recomputes and compares, on in CI for the corpus. This is how llrm keeps its exactness while becoming incremental.

### 10.2 MIR: facts and their update rules

| fact | model | update from the log |
|---|---|---|
| dominators | llrm's incremental subtree (exists) | as today; loops repaired from it (gcc `fix_loop_structure`), not rebuilt |
| pointer values | gcc: a set per SSA value, flow-insensitive | new value: transfer from its operands; erased value: drop it; changed phi/select: requeue its users |
| escapes, pointers in spill cells (flow-sensitive) | LLVM `EarliestEscapeAnalysis` | lazy per query, cached, invalidated per instruction |
| memory SSA | LLVM `MemorySSAUpdater` | erased load: drop its use; erased store: users take its defining access; new store: `insertDef`; move: `moveToPlace` |
| `Annotated` (MemRefs) | kept per access | a load's or store's entry is recomputed when it or its pointer changed |
| registers, counted, bounded | llrm incremental (exists) | as today |
| call effects, summaries, GlobalsAA | gcc IPA: once per program, stored on the call graph | a body's entry is recomputed when that body's memory footprint changed |

**Pipeline.** A fixed pass order, run once (gcc `passes.def`, LLVM's -O1 list). Each pass iterates internally with a worklist (InstCombine, FRE). A second round runs only over what round 1's log reaches, and a pass whose `reads()` footprint no edit met is skipped (LLVM `LastRunTracking`, llrm `settled` with honest footprints). There is no round 3. The convergence check becomes a debug mode.

**Pricing without copies.** hoist and gvn price a decision from a pressure delta over the affected blocks, using cached liveness and the executed-work model. They do not clone the function and forecast. gvn edits in place with a scoped table over the dominator tree (EarlyCSE, gcc dom).

### 10.3 Backend: one pipeline, one allocation, local edits

**One LIR pipeline per function.** Selection, then the LIR passes once. The spiller route is not a second pipeline. Its spill choices become an input to the allocator's weights ([single-allocation note](../optimizations/single-allocation-decisions.md), step 5).

**LIR representation** (the change that makes the rest possible):
- the slot index lives on `Insn` (LLVM `SlotIndexes`, gaps as today);
- the editor is the only way to change a block, and it returns `Edit { blocks, values_added, values_removed, points }`;
- `Postings` (def/use per value) is maintained by the editor, so finding uses is O(uses), not an O(B) check (LLVM `MachineRegisterInfo` use lists);
- block `Arc` identity is the dirty mark for dataflow (gcc `df_set_bb_dirty`).

**LIR liveness: one owner, gcc df.** Per-block use/def and live-in/out cached on the block. A changed block's local sets are recomputed and the worklist is seeded from them. The allocator, peephole lanes and floatassign ask this owner.

**Allocator: LLVM greedy, with llrm's decisions in it.**
- Queue, stages Assign → Split → Spill → Done with no return, eviction cascades (llrm has these).
- Live intervals computed once per function. A spill or split updates only the values in its `Edit` (`LiveRangeEdit`); reload ranges are queued at Done from their one or two points.
- Split costing through an interference cache per (register, block) with version tags (LLVM `InterferenceCache`); block frequencies and edge bundles are per-body facts, updated when a bridge block is added.
- No trials. The trials' wins become priced decisions at the point they arise, using llrm's executed-work model:
  - split pay priced with border copies after hinting (replaces `Whole`);
  - a loop-local protected piece, gcc IRA's region allocation done as a split candidate (replaces `Scoped`);
  - the unfoldable address use charged in spill weight (replaces `Retainable`);
  - fold or unfold chosen per use at spill time (replaces `Addressed` and `Unfolded`).
- Recolouring only after asking whether every holder has another free register.

**Clean-up.** Peephole sub-passes visit only the blocks changed since their last visit and return the input `Arc` when no rule fired. jumps tracks a change flag instead of deep-comparing. floatassign exits before it rebuilds anything when the body has no floats.

### 10.4 Budgets the gate enforces

Unit costs, measured on d_faces and checked per PR by `LLRM_DEBUG=time` (instructions, not milliseconds):

| unit | llrm today | target | reference |
|---|---|---|---|
| spill or split event | ~4 M | ≤ 0.3 M | LLVM 0.2 M |
| split candidate costed | ~200 k | ≤ 30 k | LLVM 23 k |
| MIR pass run | ~28 M | ≤ 2 M | gcc ~4 M, LLVM 0.4 M |
| analysis computations per pass run | 2.8 | ≤ 0.5 | LLVM 0.53 |
| allocations per function | up to 6 | 1 | gcc, LLVM 1 |
| LIR pipeline runs per function | up to 3 | 1 | 1 |
| full LIR liveness per edit | whole function | dirty blocks | gcc 13% |

## 11. Plan

Gains are for d_faces -O1. "Measured" means a run on this tree. A projection names the measured quantity and the rate it is projected to.

| # | change | evidence | gain | output |
|---|---|---|---|---|
| 1 | Spill and split as local edits: `Edit` record from `spilled_apart`/`carved_moving`, slot on `Insn`, `Facts` updated for the edit's values, interference cache for split costing (9.8-9.10) | ~4 M per event against LLVM's 0.2 M; 241 events per allocation; split candidate 200 k against 23 k | ~0.85 G per allocation; ~2-3 G at today's 21 allocations | same |
| 2 | Analysis protocol: `update`/`invalidate` per result, one `ModuleAnalyses`, no invalidate-all mid-pass, one owner for points-to (9.1, 9.2) | 1.95 G of analyses at 2.8 computations per pass run against LLVM's 0.53 | ~0.9 G | same |
| 3 | Points-to per SSA value, maintained from the log; escapes per query (9.5) | 234 solves plus GlobalsAA's and Summaries' own, against gcc's 2 per function | ~0.6 G of the 0.82 G family | same (precision kept by the check mode) |
| 4 | Memory SSA updated; `Annotated` invalid only on memory edits (9.6) | 98 and 125 rebuilds, 466 M and 472 M inclusive | ~0.4 G, partly inside 2 | same |
| 5 | One LIR liveness owner with dirty blocks; peephole visits changed blocks; floatassign early exit (9.7) | clean-up 0.98 G against ~82 M and ~14 M | ~0.5 G | same |
| 6 | Optimizer: one round plus a worklist round over `changes_since`; footprints for fold, hoist and gvn; gvn in place; hoist priced from a pressure delta (9.3, 9.4, 9.13, 9.14) | rounds 3-5: 0.64 G; idle runs: 0.88 G; hoist + gvn own: 0.56 G | ~0.8 G | changes; bench gate, with the cases a later round fixes named first |
| 7 | Drop the second route (3.1) | `-fno-allocation-routes`: -1.93 G at -O1 (-19% to -24% at every optimizing level), identical listing at every level | 1.93 G (measured) | changes in the corpus: route B wins 30% of functions (mean 1.8%) and loses 39%. **User's decision** |
| 8 | Trials become in-run decisions (3.1 d, 9.11) | both flags: -2.98 G | 1.05 G beyond 7 (measured) | +1.2% static instructions in `D_DRAW_FACES` at -O1 (-O2/-O3 +1.4%, -Os +2.5% and +4.3% memory operands); trial wins lost until replaced. **User's decision**, behind `-fallocation-single` |

Projected path, without double counting:

| after | d_faces -O1 | output |
|---|---|---|
| today | 8.3 G | |
| 7 + 8 | 5.3 G (measured) | changes |
| 1 | ~4.5 G | same |
| 2, 3, 4 | ~3.2 G | same |
| 6 | ~2.4 G | changes |
| 5 | ~1.9 G | same |

That is ~3x gcc (0.64 G). Order of work: 1, 2-4 and 5 first, since they keep today's output and are judged by the budgets in 10.4. Then 6 behind the bench gate. 7 and 8 land when the user decides. What remains after all eight is the passes' own work per run and the container constant factor (BTree and hash maps are ~20% of samples).

What not to do:
- Swapping containers or hashers as the main lever: IPC equals gcc's, and the cycle ratio equals the instruction ratio.
- A hard cap of 2 optimizer rounds without a worklist: 116 bench rows got worse.
- Removing the cycle-check text render for speed: it is 0.5%.
- Making trial allocations faster: they share nothing with the base, so they cannot be cut short. Remove them or keep them.
- Per-shape gates on route B: no fact of the base run predicts its wins ([single-allocation note](../optimizations/single-allocation-decisions.md)).
- Counting pass runs as the problem: LLVM runs more of them. The cost per run is the problem.

## Appendix A. Trace: gcc 13.4 on d_draw_faces

Input and flags as specified. cc1 alone: 635.8 M instructions:u (`perf stat`); 1 % = 6.36 M.

Instruments:
- RA dumps: `-fdump-rtl-ira-details`, `-fdump-rtl-reload-details` (d_draw_faces section only).
- All passes: `-fdump-{tree,rtl}-all-details`, `-fdump-statistics`, `-fdump-tree-{e,}alias-details-stats`.
- gdb hit counts: breakpoint commands print every hit. Each `execute_one_pass` hit prints the pass name and `function_name(cfun)`, so the other hits are attributed to d_draw_faces and to a pass (log1.txt, ana.py).
- perf: 20 runs × `perf record -e instructions:u -c 30000 --call-graph dwarf` give 23,588 samples. One run is throttled to about 1,460 samples, which is why there are 20.
- Per-pass split: a sample is charged to the innermost `execute_one_pass` frame. The callee frame is mapped to the pass whose `opt_pass::execute` vtable slot (slot 5, read by gdb) contains it, or to a static that execute tail-jumps to (perpass2.py).

Instrument caveats (checked, not guessed):
- cc1 is stripped except for its dynamic symbols. Statics carry 45 % of self samples as `[unknown]`; their inclusive cost is still attributed to the exported caller.
- `df_analyze()` incl is 0.28 % but `df_analyze_problem` is 7.53 %, because `df_analyze` tail-calls the static `df_analyze_1`. The same applies to `recog` (0.03 % incl over 4803 calls) and `lra_inheritance` (0.01 %). Tail calls hide them, so I use their call counts, not their perf %.
- `refs_may_alias_p_1` gets only 127 hits. Internal callers use the static `refs_may_alias_p_2` [tree-ssa-alias.cc:2359](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-alias.cc#L2359), so I count the exported entry points instead.
- `et_new_tree` is inlined into `calculate_dominance_info`, so it cannot detect computes. Instead I set address breakpoints after the `DOM_OK` early return (0x1228fe8) and on the `DOM_NONE` full-compute path (0x1228ff4), both read from objdump.
- Cross-check: gdb gives 79 `df_worklist_dataflow` calls, and the dumps give 79 `df_worklist_dataflow_doublequeue` lines. They agree.

### A.1 Register allocation

#### IRA (pass `ira`: 10.01 % exclusive of other passes; `ira()` [ira.cc:5540](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira.cc#L5540))

| Step | Location | Measured on d_draw_faces | perf incl (file) |
|---|---|---|---|
| region choice | [toplev.cc:1313](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/toplev.cc#L1313) | IRA_REGION_MIXED at -O1. 17 loops found → **17 regions**, 206 blocks, 725 program points | |
| ira_build | [ira-build.cc:3469](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L3469) | 1 call | 3.68 % |
| create_loop_tree_nodes / form_loop_tree | [ira-build.cc:125](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L125), [ira-build.cc:328](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L328) | 17 loop-tree nodes | |
| create_allocnos → ira_create_allocno | [ira-build.cc:2106](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L2106), [ira-build.cc:476](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L476) | **2041 allocnos** (gdb: 2041 calls). 1114 regular + 927 caps, for **442 pseudos** | |
| ira_costs → find_costs_and_classes | [ira-costs.cc:2383](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-costs.cc#L2383), [ira-costs.cc:1757](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-costs.cc#L1757) | **1 cost pass** (only "Pass 0" in the dump; no -fexpensive-optimizations at -O1) | 1.03 % |
| live ranges + compress | [ira-lives.cc:1725](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-lives.cc#L1725), [ira-lives.cc:1748](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-lives.cc#L1748) | 1282 ranges | |
| propagate_allocno_info / create_caps | [ira-build.cc:2033](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L2033), [ira-build.cc:3002](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L3002) | runs (more than 1 region) | |
| ira_build_conflicts | [ira-conflicts.cc:798](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-conflicts.cc#L798) | **11,347 conflicts**, 593 copies. Bit table 336,120 B (uncompressed 522,496), built over min/max conflict-id ranges [ira-conflicts.cc:91](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-conflicts.cc#L91). Hard-reg conflicts are HARD_REG_SETs ORed per object [ira-conflicts.cc:854](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-conflicts.cc#L854) | 1.01 % |
| ira_color → color_pass per region | [ira-color.cc:5289](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-color.cc#L5289), [ira-color.cc:3594](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-color.cc#L3594), traversal [ira-color.cc:3770](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-color.cc#L3770) | 1 call, **17 color_pass** (one per region), 1440 pushes. 568 "Spill aN" (allocno marked memory in some region), 38 improve_allocation re-spills [ira-color.cc:3147](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-color.cc#L3147). move_spill_restore [ira-color.cc:3786](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-color.cc#L3786): 2 rounds, 2 moves hoisted | 3.42 % |
| ira_traverse_loop_tree | [ira-build.cc:1771](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L1771) | used by build and color | 5.23 % |
| result | | 673 of 1114 allocnos in memory. **89 pseudos** in memory in at least one region, **77** in all. Cost 973,893 (reg 265,198, mem 708,695) | |
| ira_emit | [ira-emit.cc:1254](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-emit.cc#L1254), change_loop [ira-emit.cc:561](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-emit.cc#L561) | RTL changed for 16 loops, **89 new pseudos** at region borders, border moves ld 1194 / st 540 / move 6654 (cost units), 4 new jumps, 160 insn rescans | 0.60 % |
| flattening | [ira-build.cc:3125](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L3125) | **not called**. With LRA, IRA only renames allocnos to their emit regs [ira.cc:5816](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira.cc#L5816). Then 1 df_analyze [ira.cc:5842](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira.cc#L5842) | 0 |
| df inside ira | | 4 df_analyze, 4 worklist solves (831 blocks, 1624 visits) | |

#### LRA (pass `reload`: 10.08 %; `lra()` 9.38 % incl; [lra.cc:2293](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra.cc#L2293), loop [lra.cc:2371](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra.cc#L2371))

| Iter | Step (location) | d_draw_faces |
|---|---|---|
| 1 | Local #1, lra_constraints [lra-constraints.cc:5119](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-constraints.cc#L5119) → curr_insn_transform [lra-constraints.cc:3998](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-constraints.cc#L3998) → process_alt_operands [lra-constraints.cc:2027](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-constraints.cc#L2027) | 1254 insns get an alternative chosen. 379 reload pseudos, 428 reload insns, 129 pseudo→equiv substitutions, 17 equiv init insns removed. 18 pseudos that IRA put in %ebp are spilled (bp not eliminable) |
| 1 | Inheritance #1, lra_inheritance [lra-constraints.cc:7050](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-constraints.cc#L7050) / inherit_in_ebb [lra-constraints.cc:6593](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-constraints.cc#L6593) | 78 inheritances, 4 rejected, 0 splits (290 EBB headers over both rounds) |
| 1 | live ranges, lra_create_live_ranges [lra-lives.cc:1485](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-lives.cc#L1485) | builds #1 and #2 (rebuilt after removing dead pseudos), 2307→1649 points |
| 1 | Assignment #1, lra_assign [lra-assigns.cc:1605](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-assigns.cc#L1605) | 455 assignments, 3 spills by assign_by_spills [lra-assigns.cc:1382](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-assigns.cc#L1382), 1 reassign round |
| 1 | Undo inheritance #1 [lra-constraints.cc:7511](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-constraints.cc#L7511), then build #3, coalesce [lra-coalesce.cc:222](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-coalesce.cc#L222) | 0 coalesced |
| 2 | Local #2 / Inheritance #2 / builds #4, #5 / Assignment #2 / Undo #2 / build #6 | 163 insns, 40 reload pseudos, 42 reload insns. 10 inheritances. 50 assignments |
| 3 | Local #3, then lra_spill [lra-spills.cc:604](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-spills.cc#L604) ([lra.cc:2494](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra.cc#L2494)) | 41 insns. **79 pseudos into 42 stack slots** |
| 4 | Local #4 after elimination [lra-eliminations.cc:1360](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-eliminations.cc#L1360) | 326 insns, no new reloads → done |

gdb call counts: lra 1, lra_constraints 4, lra_inheritance 2, lra_create_live_ranges 4 (6 builds in the dump), lra_assign 2, lra_undo_inheritance 2, lra_coalesce 1, lra_spill 1. Remat: 0.
Totals: 507 new pseudos (419 reload + 88 inheritance), 470 reload insns, 1 df_analyze, 3 worklist solves (639 blocks).
perf incl: lra_create_live_ranges 2.87 %, lra_constraints 1.89 %, lra_assign 1.09 %, lra_undo_inheritance 0.30 %, lra_spill 0.27 %, lra_eliminate 0.25 %.
Target init outside the pass: ira_init 1.13 %, ira_init_register_move_cost 0.61 %.

### A.2 Alias, points-to, memory SSA

| Item | Location | d_draw_faces |
|---|---|---|
| compute_may_aliases | [tree-ssa-structalias.cc:8008](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L8008) | **2 runs** (ealias, alias), both from TODO_rebuild_alias [passes.cc:2069](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2069). File: 2.47 % incl over 20 runs |
| ealias: constraints → solve | find_func_aliases [tree-ssa-structalias.cc:5068](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L5068), perform_var_substitution [tree-ssa-structalias.cc:2341](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L2341), solve_graph [tree-ssa-structalias.cc:2711](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L2711) | 2388 constraints, 1285 vars, 840 statically unified, **3 solve iterations**, 886 edges, 1774 implicit edges |
| alias | same | 2094 constraints, 1027 vars, 593 unified, **3 iterations**, 871 edges, 1522 implicit edges |
| stmt_may_clobber_ref_p_1 | [tree-ssa-alias.cc:3230](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-alias.cc#L3230) | **14,275 calls**: dce 7919, cddce 4007, fre 1242, dom 621, local-fnsummary 474, expand 12 (1.18 % incl) |
| walk_non_aliased_vuses | [tree-ssa-alias.cc:3849](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-alias.cc#L3849) | **832 calls**: fre 551, dom 281. Each starts with limit 1000 (param [params.opt:924](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/params.opt#L924), set [tree-ssa-sccvn.cc:3891](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-sccvn.cc#L3891)) (0.56 %) |
| ref_maybe_used_by_stmt_p | [tree-ssa-alias.cc:2993](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-alias.cc#L2993) | 914, all in dse (budget param [params.opt:121](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/params.opt#L121), [tree-ssa-dse.cc:882](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-dse.cc#L882)) |
| refs_may_alias_p_1 (exported entry only) | [tree-ssa-alias.cc:2534](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-alias.cc#L2534) | 127: fre 80, lim 47 |
| walk_aliased_vdefs | [tree-ssa-alias.cc:4007](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-alias.cc#L4007) | 0.15 % incl |
| FRE | [tree-ssa-sccvn.cc:8753](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-sccvn.cc#L8753), do_rpo_vn_1 [tree-ssa-sccvn.cc:7565](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-sccvn.cc#L7565) | non-iterating at -O1 (`iterate_p = may_iterate && optimize > 1`). fre1: 181 blocks each visited once, 238 eliminated. fre3: 157 blocks once, 2 eliminated ("RPO iterations" histogram = 10, i.e. 1.0 visits per block) |
| update_ssa | [tree-into-ssa.cc:3392](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-into-ssa.cc#L3392) | **40 calls**: TODO_update_ssa 35, **TODO_update_ssa_only_virtuals 4**, no_phi 1 (1.04 % incl) |

### A.3 Dataflow, dominance, loops

| Item | Location | d_draw_faces |
|---|---|---|
| problems added | df_lr [df-problems.cc:1275](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L1275), live [df-problems.cc:1803](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L1803), note [df-problems.cc:3782](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L3782), chain [df-problems.cc:2740](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L2740), RD [df-problems.cc:707](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L707), MD [df-problems.cc:4795](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L4795) | LR 1 (dfinit; stays live), LIVE 5 (ce1, loop2_invariant, init-regs, ce2, ce3), NOTE 8, CHAIN 16 (all loop2_invariant), **RD 0, MD 0**, word_lr 0 |
| df_analyze | [df-core.cc:1265](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1265) → df_analyze_1 [df-core.cc:1212](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1212) | **27 calls**, plus df_analyze_loop [df-core.cc:1439](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1439) in loop2_invariant |
| df_worklist_dataflow | [df-core.cc:1094](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1094), doublequeue [df-core.cc:1016](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1016) | **79 solves**: 7676 blocks in scope, out of 16,715 function blocks summed over the solves; 12,622 block visits (1.64 per block). loop2_invariant: 48 solves over 1281 of 10,320 blocks |
| dirty blocks | df_set_bb_dirty [df-core.cc:1601](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1601); LR local recompute only for out-of-date blocks [df-problems.cc:968](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L968) | 2232 calls. **720 distinct dirty blocks over 27 df_analyze** (about 13 % of 27 × ~200) |
| rescans | df_insn_rescan [df-scan.cc:1013](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-scan.cc#L1013), defer [df-scan.cc:1051](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-scan.cc#L1051), process [df-scan.cc:1210](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-scan.cc#L1210) | 4458 calls. The dumps show 1552 "no change", 907 immediate rescans, 391 new insns, **283 deferred**, 79 deferred deletions. **80** process_deferred_rescans calls |
| perf | | df_analyze_problem 7.53 %, df_worklist_dataflow 2.97 % (self 0.59 %), df_insn_rescan 0.93 %, deferred processing 0.09 %, regstat_compute_ri 0.71 % |
| calculate_dominance_info | [dominance.cc:720](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/dominance.cc#L720), cached return [dominance.cc:724](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/dominance.cc#L724) | **152 calls: 74 returned DOM_OK (cached), 33 fast-query renumber only, 45 full computes** (1.10 % incl) |
| loops | loop_optimizer_init [loop-init.cc:86](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/loop-init.cc#L86), flow_loops_find [cfgloop.cc:423](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/cfgloop.cc#L423), fix_loop_structure [loop-init.cc:200](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/loop-init.cc#L200) | 25 / 32 / 24 calls (0.72 / 0.47 / 0.27 %) |

### A.4 Pipeline and worklists

- execute_one_pass [passes.cc:2580](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2580): 289 calls on d_draw_faces, gate at [passes.cc:2597](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2597). **154 passes executed** (they wrote a dump): 106 GIMPLE/IPA, 48 RTL.
- execute_todo [passes.cc:2142](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2142) / execute_function_todo [passes.cc:2052](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2052): 344 calls, 302 with flags. **5.47 % of the file** (its callees included).

  | TODO bit | Count on d_draw_faces |
  |---|---|
  | cleanup_cfg | 24 |
  | update_ssa | 31 |
  | update_ssa_only_virtuals | 2 |
  | remove_unused_locals | 4 |
  | df_finish | 20 |
  | df_verify | 3 |
  | rebuild_alias | 2 |
  | update_address_taken | 7 |
  | mark_first_instance | 130 |
  | verify_il | 172 (checking build only) |

- cleanup_tree_cfg [tree-cfgcleanup.cc:1212](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-cfgcleanup.cc#L1212): 26 calls, 2.40 %. RTL cleanup_cfg [cfgcleanup.cc:3110](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/cfgcleanup.cc#L3110): 14 calls, 0.81 %.
- Propagation engine ssa_propagate [tree-ssa-propagate.cc:434](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-propagate.cc#L434): 7 runs (ccp 4, copyprop 3), 4.64 %.
- dom_walker::walk [domwalk.cc:273](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/domwalk.cc#L273): 23 walks, 9.46 % incl; dom pass alone 8.08 %.
- ccp: 38 copies propagated, 97 statements deleted.
- Ranger inside dom/ccp: gimple_ranger::range_of_expr [gimple-range.cc:74](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-range.cc#L74), 3.28 %.
- Off at -O1: pre, vrp, cse2, gcse2, peephole2, sched1/2, rnreg, ree, web, hoist.

### A.5 Selection, peephole, clean-up

| Pass | Location | d_draw_faces | perf % (pass) |
|---|---|---|---|
| expand | [cfgexpand.cc:6637](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/cfgexpand.cc#L6637) | 141 rescans, 1 cleanup_cfg | 1.97 |
| vregs | | 1153 recog calls | small |
| cse1 | [cse.cc:6647](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/cse.cc#L6647) | 1 df solve, 125 rescans | 1.91 |
| fwprop1/2 | [fwprop.cc:960](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/fwprop.cc#L960) | 1 df_analyze each | 2.12 |
| ce1 / ce2 / ce3 | [ifcvt.cc:5830](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ifcvt.cc#L5830) | 5 / 2 / 2 df solves | 1.50 / 1.73 (ce2+ce3) |
| loop2_invariant | [loop-invariant.cc:2276](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/loop-invariant.cc#L2276) | 48 region solves, 16 chain problems | 1.99 |
| combine | [combine.cc:1117](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/combine.cc#L1117), try_combine [combine.cc:2520](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/combine.cc#L2520) | **796 tries** (536 two-insn, 260 three-insn), **64 combined** (62 two, 1 three, 1 with note), 1356 recog calls. File total: 1059 attempts, 84 successes | 4.72 |
| split1..4 | [recog.cc:3433](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/recog.cc#L3433) | 3 split_all_insns calls | 0.28 |
| postreload | [postreload.cc:62](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/postreload.cc#L62) | 30 rescans | 2.06 |
| cprop_hardreg | [regcprop.cc:757](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/regcprop.cc#L757) | 132 deferred rescans | 0.74 |
| bbro | [bb-reorder.cc:2558](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/bb-reorder.cc#L2558) | 484 recog, 242 rescans | 0.64 |
| stack (x87) | [reg-stack.cc:3344](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/reg-stack.cc#L3344) | 208 rescans, 180 dirty blocks | 0.75 |
| shorten | [final.cc:823](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/final.cc#L823) | 1 call | 0.37 |
| final | [final.cc:4231](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/final.cc#L4231) | | 0.95 |

recog (via recog_memoized [recog.h:270](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/recog.h#L270)): 4803 calls on d_draw_faces (combine 1356, vregs 1153, reload 598, bbro 484, ira 250, ce1 184, loop2_invariant 156, shorten 102, cse1 101).

### A.6 Whole-file profile (23,588 samples)

Top self (named; 45.25 % sits in unnamed statics):

| % | Symbol |
|---|---|
| 2.79 | bitmap_set_bit |
| 1.39 | bitmap_clear_bit |
| 1.34 | bitmap_bit_p |
| 1.26 | get_ref_base_and_extent |
| 0.84 | ggc_internal_alloc |
| 0.78 | _int_malloc |
| 0.70 | bitmap_ior_into |
| 0.68 | wi::mul_internal |
| 0.62 | bitmap_ior_and_compl |
| 0.59 | df_worklist_dataflow |
| 0.56 | operand_equal_p |
| 0.50 | side_effects_p |
| 0.46 | do_lookup_x (ld.so) |
| 0.45 | volatile_refs_p |
| 0.45 | regstat_compute_ri |

Top passes (exclusive of nested passes; TODOs counted separately):

| % | Pass | % | Pass |
|---|---|---|---|
| 10.08 | reload (LRA) | 2.12 | fwprop1/2 |
| 10.01 | ira | 2.06 | postreload |
| 8.08 | dom | 1.99 | loop2_invariant |
| 5.47 | execute_todo (all passes) | 1.97 | expand |
| 4.72 | combine | 1.91 | cse1 |
| 4.60 | ccp | 1.76 | forwprop |
| 3.41 | dse (RTL dse1/dse2) | 1.73 | ce2/ce3 |
| 2.83 | ivopts | 2.20 | fre |

Outside passes: callgraph/IPA driver 4.82 %, front end (c_parse_file) 2.32 %, toplev init 2.37 %, dynamic loader 1.40 %.

### A.7 How gcc stays fast

| Mechanism | Invariant | Where | Evidence on d_draw_faces |
|---|---|---|---|
| df problems registered on demand; LR persists, others added and dropped per pass | A problem is valid until df_finish_pass removes it. Passes ask only for what they read | [df-core.cc:415](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L415), [df-core.cc:621](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L621) | RD 0 and MD 0 times. LIVE 5, NOTE 8, CHAIN 16 (loop2_invariant only) |
| Dirty-block local recompute | A block's transfer function changes only if one of its insns changed; df_set_bb_dirty marks it | [df-core.cc:1601](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1601), [df-problems.cc:968](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L968) | 27 df_analyze, 720 dirty blocks recomputed (about 13 % of blocks × calls) |
| Worklist solve with RPO doublequeue | Monotone problem; a block is requeued only when a predecessor's out changes | [df-core.cc:1016](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1016) | 1.64 visits per block (12,622 / 7676) |
| Region solving | df_analyze_loop restricts the problem to a loop's blocks | [df-core.cc:1439](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1439) | loop2_invariant: 1281 of 10,320 blocks |
| Deferred rescans | While DF_DEFER_INSN_RESCAN is set, refs go stale until one batch rescan; an unchanged insn returns early | [df-scan.cc:1051](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-scan.cc#L1051), [df-scan.cc:1071](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-scan.cc#L1071), [df-scan.cc:1210](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-scan.cc#L1210) | 4458 rescan calls: 1552 found no change, 283 deferred, 80 batch flushes. 0.93 % + 0.09 % |
| IRA allocno model, regional | One allocno per pseudo per region. Caps carry inner-region info up. Coloring runs top-down per region | [ira-build.cc:3469](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-build.cc#L3469), [ira-color.cc:3594](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-color.cc#L3594) | 17 regions, 2041 allocnos for 442 pseudos, 17 color_pass. Only 89 border pseudos emitted |
| Cost classes, single cost pass | Each pseudo is costed only over its cost-class subset of register classes; one pass at -O1 | [ira-costs.cc:224](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-costs.cc#L224), [ira-costs.cc:1757](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-costs.cc#L1757) | 1 cost pass, 1.03 % |
| Conflicts as id-ranged bit vectors + HARD_REG_SET | An allocno conflicts only within [min,max] conflict ids. Hard regs are a fixed-width set | [ira-conflicts.cc:91](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-conflicts.cc#L91), [ira-conflicts.cc:854](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ira-conflicts.cc#L854) | 11,347 conflicts in 336 KB, against 522 KB uncompressed. 1.01 % |
| LRA local-only iterations | Each constraint pass touches only insns whose operands changed. Inheritance and splits are EBB-local and undone if unassigned | [lra.cc:2371](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra.cc#L2371), [lra-constraints.cc:6593](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-constraints.cc#L6593), [lra-constraints.cc:7511](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-constraints.cc#L7511) | 4 constraint passes (1254 → 163 → 41 → 326 insns), 2 inheritance, 6 live builds, 1 spill |
| Live-range point compression | Points with no range start or end are merged | [lra-lives.cc:1485](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/lra-lives.cc#L1485) | 2307 → 1649 points (71 %) per build |
| PROP_* and TODO_* as invalidation | A pass declares what it needs, provides and destroys. Expensive fixups run only when the flag is set | [passes.cc:2644](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2644), [passes.cc:2697](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2697), [passes.cc:2052](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.cc#L2052) | 344 execute_todo: only 24 cleanup_cfg, 31 update_ssa, 2 rebuild_alias. 5.47 % in total |
| Dominance cache | dom_computed[dir] == DOM_OK until a CFG change frees it | [dominance.cc:724](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/dominance.cc#L724) | 152 calls: 74 cached, 33 renumber only, 45 full |
| Loop-structure fixup | Loops persist across passes; LOOPS_NEED_FIXUP triggers repair, not a rebuild | [loop-init.cc:100](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/loop-init.cc#L100), [loop-init.cc:200](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/loop-init.cc#L200) | 25 loop_optimizer_init, 24 fixups |
| Points-to on SSA names, offline unification | Equal pointer/location classes merge before solving; the graph is solved once per function | [tree-ssa-structalias.cc:2341](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L2341), [tree-ssa-structalias.cc:2711](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L2711) | 2 runs per function. 840/1285 and 593/1027 vars unified statically, 3 iterations each, 2.47 % file |
| Virtual operands (one memory SSA web) | All memory state is one .MEM chain; queries walk only vdefs | [tree-ssa-alias.cc:3849](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-alias.cc#L3849), [tree-ssa-alias.cc:4007](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-alias.cc#L4007) | 832 FRE/DOM walks, each limited to 1000 queries; 4 only-virtuals update_ssa |
| Non-iterating FRE at -O1 | RPO VN without backedge iteration is still sound (it is only less precise) | [tree-ssa-sccvn.cc:8753](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-sccvn.cc#L8753) | Each block visited exactly once (181 and 157) |
| Incremental SSA update | Only marked names and blocks are renamed | [tree-into-ssa.cc:3392](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-into-ssa.cc#L3392) | 40 update_ssa, 1.04 % |

## Appendix B. Trace: clang/LLVM 20.1.8 on d_draw_faces

Input/flags as specified. `d_draw_faces`: 2742 IR instructions at entry (helpers `clip_w`, `ifloor`, `cam_plane_dist`, `sc_*` inlined into it), 1248 at the end of the IR pipeline; 98.5% of the IR that reaches codegen.

### B.1 Instruments

| Instrument | What it gave | Caveat |
|---|---|---|
| `perf stat` | driver run 552.0 M instr:u; cc1 run alone 520.6 M | 31 M is driver work outside cc1 |
| `perf record -c 20000 -g` (frame pointers), 30 cc1 runs | 36 603 samples, 14.2 k instr/sample | kernel caps at 20 kHz, hence 30 runs; libLLVM symbolized with `libllvm20-dbgsym_20.1.8-0ubuntu4` (build-id 468b536f… matches); libclang-cpp has no symbols |
| gdb hit counts, python `gdb.Breakpoint` + `.debug` file | per-function call counts; a snapshot at each `SelectionDAGISel::runOnMachineFunction` isolates `d_draw_faces` (last of 6 machine functions) | debug info is line tables only: no locals, so no stage histogram |
| `-fdebug-pass-manager`, `-debug-pass=Executions` | pass and analysis runs/invalidations per function | — |
| `-print-after=<pass> -filter-print-funcs=d_draw_faces` | MIR sizes, vregs, stack refs | — |

All M figures are instructions measured in the cc1 profile. % columns: of 552 M, and of 552 M minus dynamic linking (484.1 M).

Instrument faults found and fixed during the run (no counts below come from these):
- Exported `RegAllocBase::enqueue` showed 5 hits; greedy calls its own `RAGreedy::enqueue`, which has 940.
- `DomTreeBuilder::Calculate<>` showed 18 IR builds, because the wrapper is inlined. `SemiNCAInfo::CalculateFromScratch` shows 100.
- Line breakpoints at RegAllocGreedy.cpp:2479/1039/1050/1325 landed on lines 2485/1045/1059/1328 (optimized code), so they were dropped.

Dynamic linking (`_dl_start` → relocation + symbol lookup) costs 67.9 M (13.0% of cc1, 12.3% of 552 M). Static constructors (`_dl_init`) cost 13.4 M (2.4%).

### B.2 Whole file: top 15 self symbols

| Symbol (DSO) | M | % 552 M | % no-dynlink |
|---|---|---|---|
| `_dl_relocate_object_no_relro` (ld.so) | 29.1 | 5.3 | 6.0 |
| `do_lookup_x` (ld.so) | 11.8 | 2.1 | 2.4 |
| `_dl_lookup_symbol_x` (ld.so) | 11.6 | 2.1 | 2.4 |
| unsymbolized (libclang-cpp) | 11.2 | 2.0 | 2.3 |
| unsymbolized (kernel entry) | 9.6 | 1.7 | 2.0 |
| `_int_malloc` (libc) | 8.5 | 1.5 | 1.7 |
| `hashing::detail::hash_short` | 6.8 | 1.2 | 1.4 |
| `__cxa_atexit` (libc) | 5.8 | 1.1 | 1.2 |
| `memmove` (libc) | 5.3 | 1.0 | 1.1 |
| `SmallPtrSetImplBase::insert_imp_big` | 5.1 | 0.9 | 1.1 |
| `computeKnownBitsFromOperator` | 4.3 | 0.8 | 0.9 |
| `strcmp` (ld.so) | 3.1 | 0.6 | 0.6 |
| `SelectionDAGISel::SelectCodeCommon` | 2.7 | 0.5 | 0.6 |
| `InstCombinerImpl::run` (self) | 2.5 | 0.5 | 0.5 |
| `SmallPtrSetImplBase::doFind` | 2.5 | 0.4 | 0.5 |

The profile is flat: past the loader, no LLVM symbol reaches 1.3% self.

### B.3 Whole file: top 15 passes (inclusive)

| Pass | M | % 552 M | % no-dynlink |
|---|---|---|---|
| InstCombinePass (73 invocations) | 56.5 | 10.2 | 11.7 |
| X86 DAG→DAG isel (SelectionDAGISel) | 53.7 | 9.7 | 11.1 |
| RAGreedy | 27.5 | 5.0 | 5.7 |
| IndVarSimplifyPass | 19.3 | 3.5 | 4.0 |
| LoopStrengthReduce (codegen IR) | 17.0 | 3.1 | 3.5 |
| SimplifyCFGPass | 15.6 | 2.8 | 3.2 |
| LICMPass | 12.0 | 2.2 | 2.5 |
| MachineScheduler | 9.3 | 1.7 | 1.9 |
| EarlyCSEPass | 8.2 | 1.5 | 1.7 |
| SROAPass | 8.2 | 1.5 | 1.7 |
| LoopRotatePass | 7.0 | 1.3 | 1.4 |
| CodeGenPrepare | 6.7 | 1.2 | 1.4 |
| LoopIdiomRecognizePass | 6.2 | 1.1 | 1.3 |
| RegisterCoalescer | 6.0 | 1.1 | 1.2 |
| MachineLICM (early + late) | 5.5 | 1.0 | 1.1 |

Next in the list are LiveIntervals 5.5, IPSCCP 5.0, AsmPrinter 5.0 and Inliner 4.9. The backend (`HandleTranslationUnit`) costs 386.8 M (70.1%); parse + Sema + IRGen cost the remaining ~35 M of `ParseAST`'s 422 M.

### B.4 Register allocation on d_draw_faces

MIR shape (from the `-print-after` dumps):

| Point | MBBs | instrs | vregs | COPY | %stack refs |
|---|---|---|---|---|---|
| after finalize-isel | 214 | 1705 | 1069 | 246 | 49 |
| after twoaddress | 258 | 2171 | 1068 | 844 | 49 |
| after register-coalescer | 258 | 1507 | 507 | 143 | 49 |
| after greedy | 258 | 1742 | 616 | 155 | 329 |
| after virtregrewriter | 258 | 1678 | 0 | 53 | 329 |

Spill code is +280 stack references: stores +121 (MOV32mr +50, ST_Fp32m +37, MOV32mi +31, MOV16mr +3), reloads +110 (MOV32rm +67, LD_Fp32m +36, MOV16rm +7), and ~49 folded memory operands (PUSH32rmm, MOVSX32rm16, CMP16mi, SUB32rm…). The 74 spill slots are fi#23..96.

Call sequence on `d_draw_faces` (gdb deltas after its isel snapshot):

| Step | Count | M (incl.) | Source |
|---|---|---|---|
| LiveVariables::analyze | 1 | 2.3 | [LiveVariables.cpp:628](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveVariables.cpp#L628) |
| SlotIndexes::analyze | 2 (stack-coloring, then RA) | 2.4 | — |
| LiveIntervals::analyze / computeVirtRegs | 1 / 1 | 5.5 | [LiveIntervals.cpp:161](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L161), [LiveIntervals.cpp:237](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L237) |
| LiveIntervalCalc::calculate | 1196 = 1068 initial + 128 LRE vregs | 4.9 | [LiveIntervalCalc.cpp:41](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervalCalc.cpp#L41) |
| RegisterCoalescer::runOnMachineFunction | 1 | 6.0 | [RegisterCoalescer.cpp:4244](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegisterCoalescer.cpp#L4244) |
| copyCoalesceWorkList / joinCopy | 265 / 834 | 5.8 / 5.6 | [RegisterCoalescer.cpp:4046](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegisterCoalescer.cpp#L4046), [RegisterCoalescer.cpp:2021](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegisterCoalescer.cpp#L2021) |
| LiveRange::join / updateRegDefsUses / reMaterializeTrivialDef | 556 / 597 / 278 | — | — |
| MachineScheduler: ScheduleDAGMILive::schedule (regions) | 149 | 8.6 | [MachineScheduler.cpp:1455](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/MachineScheduler.cpp#L1455) |
| LiveIntervals::handleMove (scheduler moves) | 81 | 0.9 | [LiveIntervals.cpp:1559](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L1559) |
| calculateSpillWeightsAndHints / weightCalcHelper | 1 / 784 | 1.0 | — |
| RAGreedy::runOnMachineFunction | 2 (AMX tile pass no-op + real) | 27.5 | [RegAllocGreedy.cpp:2723](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2723) |
| RAGreedy::enqueue / dequeue | 940 / 941 | — | [RegAllocGreedy.cpp:281](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L281), [RegAllocGreedy.cpp:387](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L387) |
| selectOrSplitImpl = tryAssign calls | 926 | 23.3 | [RegAllocGreedy.cpp:2422](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2422), [RegAllocGreedy.cpp:400](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L400) |
| LiveRegMatrix::checkInterference / query / collectInterferingVRegs | 5088 / 1106 / 5833 | 2.2 | [LiveRegMatrix.cpp:194](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRegMatrix.cpp#L194) |
| LiveRegMatrix::assign / unassign | 679 / 63 | — | [LiveRegMatrix.cpp:111](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRegMatrix.cpp#L111), [LiveRegMatrix.cpp:128](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRegMatrix.cpp#L128) |
| tryEvict (stage ≠ RS_Split) | 203 | 0.9 | [RegAllocGreedy.cpp:580](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L580), [RegAllocGreedy.cpp:2458](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2458) |
| tryFindEvictionCandidate / canEvictInterferenceBasedOnCost | 203 / 1383 | 0.9 | [RegAllocEvictionAdvisor.cpp:276](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocEvictionAdvisor.cpp#L276), [RegAllocEvictionAdvisor.cpp:186](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocEvictionAdvisor.cpp#L186) |
| evictInterference (successful evictions, 63 ranges) | 55 | <0.1 | [RegAllocGreedy.cpp:484](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L484) |
| trySplit | 99 | 17.0 | [RegAllocGreedy.cpp:1801](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1801) |
| tryRegionSplit / calculateRegionSplitCost | 94 / 94 | 15.1 / 11.1 | [RegAllocGreedy.cpp:1063](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1063), [RegAllocGreedy.cpp:1174](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1174) |
| calculateRegionSplitCostAroundReg (candidate physregs) | 645 | 11.2 | — |
| addSplitConstraints / addThroughConstraints | 722 / 2247 | 2.2 / 5.3 | — |
| growRegion / calcCompactRegion / calcGlobalSplitCost | 396 / 94 / 284 | 7.9 / — / — | [RegAllocGreedy.cpp:730](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L730) |
| SpillPlacement prepare / iterate / finish | 722 / 2324 / 396 | 0.1 / 1.3 / 0.4 | [SpillPlacement.cpp:385](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SpillPlacement.cpp#L385), [SpillPlacement.cpp:366](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SpillPlacement.cpp#L366) |
| InterferenceCache get / Entry::reset / Entry::update | 645 / 29 / 3510 | 0.2 / — / 3.4 | [InterferenceCache.cpp:62](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InterferenceCache.cpp#L62), [InterferenceCache.cpp:131](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InterferenceCache.cpp#L131) |
| doRegionSplit = splitAroundRegion (region splits done) | 31 | 3.6 | [RegAllocGreedy.cpp:925](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L925) |
| tryBlockSplit / tryInstructionSplit / tryLocalSplit | 63 / 3 / 5 | 1.2 / — / — | [RegAllocGreedy.cpp:1296](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1296), [RegAllocGreedy.cpp:1570](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1570) |
| SplitAnalysis::analyze / SplitEditor::finish | 106 / 50 | 0.2 / 3.9 | [SplitKit.cpp:347](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SplitKit.cpp#L347), [SplitKit.cpp:1502](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SplitKit.cpp#L1502) |
| splitLiveThroughBlock / RegIn / RegOut / SingleBlock | 580 / 28 / 59 / 34 | — | [SplitKit.cpp:1633](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SplitKit.cpp#L1633) |
| InlineSpiller::spill | 99 | 2.8 | [InlineSpiller.cpp:1285](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InlineSpiller.cpp#L1285), [RegAllocGreedy.cpp:2515](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2515) |
| setStage(range, RS_Done) after spill (register-probed) | 99 | — | [RegAllocGreedy.cpp:2516](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2516) |
| InlineSpiller foldMemoryOperand / rematerializeAt / eliminateRedundantSpills | 330 / 83 / 66 | 0.8 / 0.1 / — | — |
| VirtRegMap::assignVirt2StackSlot (spill slots) | 74 | — | — |
| tryLastChanceRecoloring | 0 | 0 | [RegAllocGreedy.cpp:1957](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1957) |
| tryHintRecoloring | 17 | — | [RegAllocGreedy.cpp:2789](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2789) |
| LiveRangeEdit::createFrom / eliminateDeadDefs | 128 / 121 | — / 1.0 | [LiveRangeEdit.cpp:55](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRangeEdit.cpp#L55), [LiveRangeEdit.cpp:438](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRangeEdit.cpp#L438) |
| LiveIntervals::splitSeparateComponents | 134 | — | [LiveIntervals.cpp:1802](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L1802) |
| LiveIntervals::repairIntervalsInRange | **0** | 0 | [LiveIntervals.cpp:1700](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L1700) |
| LiveIntervals::shrinkToUses (+ coalescer's own) | 61 (+42) | 3.3 | [LiveIntervals.cpp:485](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L485) |
| SlotIndexes insertMachineInstrInMaps / renumberIndexes / repairIndexesInRange | 439 / 28 / 0 | — | [SlotIndexes.cpp:172](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SlotIndexes.cpp#L172) |
| SlotIndexes::removeMachineInstrFromMaps | 855 | — | — |
| VirtRegRewriter | 1 | 0.9 | [VirtRegMap.cpp:281](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/VirtRegMap.cpp#L281) |

Stage flow, from the counts:
- 926 selections.
- 203 ranges that fail `tryAssign` and are not in RS_Split try eviction; 55 succeed and evict 63 ranges.
- 99 trySplit calls (RS_Split/Split2 ranges): 94 region-split attempts, 31 region splits done, 63 block-split attempts.
- 99 spills. No last-chance recoloring.
- How many ranges get deferred (`setStage(RS_Split)`, line 2479) is not measured. The call is inlined, and the line breakpoint relocates.

Time split (`-time-passes` "Register Allocation" group, existing data): Global Splitting 78%, Spiller 15%, Evict 5%, Seed 1%, Local Splitting 0.3%.

### B.5 Alias analysis / MemorySSA (whole file)

| Item | Count | M | Source |
|---|---|---|---|
| BasicAAResult::alias = aliasCheck | 2904 (36 in codegen of d_draw_faces) | 5.7 | [BasicAliasAnalysis.cpp:1546](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L1546) |
| aliasCheckRecursive (AAQI cache miss, non-trivial) | 1850 | 3.2 | [BasicAliasAnalysis.cpp:1701](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L1701) |
| aliasGEP / aliasPHI | 1246 / 0 | 3.1 | — |
| AAResults::alias (aggregate entry) / getModRefInfo(I, Loc) | 1645 / 2889 | 2.9 / 5.6 | — |
| TypeBasedAA::alias / GlobalsAA::alias | 1310 / 1081 | — | — |
| GlobalsAA analysis | 1 run (RequireAnalysisPass) + RecomputeGlobalsAA; `analyzeModule` symbol 0 hits (inlined) | — | — |
| MemorySSA built (function) | 20 = MemorySSAAnalysis run 20 / invalidated 20; d_draw_faces 4 | 4.1 | [MemorySSA.cpp:1233](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSA.cpp#L1233) |
| Walker getClobberingMemoryAccessBase | 245 | 6.6 | [MemorySSA.cpp:2529](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSA.cpp#L2529) |
| MemorySSAUpdater insertUse / insertDef / moveToPlace / removeMemoryAccess / applyUpdates | 21 / 2 / 21 / 72 / 19 | 2.0 (applyUpdates) | — |
| LICM: runOnLoop = hoistRegion = sinkRegion | 77 | 12.0 | [LICM.cpp:189](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LICM.cpp#L189) |
| LICM canSinkOrHoistInst | 1847 | 5.9 | — |
| EarlyCSE getMatchingValue / isSameMemGeneration (MSSA query) | 531 / 97 | — | — |

### B.6 Liveness, dominators, loops

Machine level (d_draw_faces):
- LiveVariables 1×, LiveIntervals computed 1× and then kept by incremental updates through coalescer, scheduler, RA and rewriter: handleMove 81, shrinkToUses 61+42, repairIntervalsInRange 0.
- MachineDominatorTree built 8× from scratch (no incremental updates: ApplyUpdates/InsertEdge/DeleteEdge 0); post-dom 5×; MachineLoopInfo 5×; MachineBlockFrequency 5 executions.

IR level, new PM, d_draw_faces (`-fdebug-pass-manager`):

| Analysis | run | invalidated |
|---|---|---|
| DominatorTreeAnalysis | 9 | 9 |
| LoopAnalysis | 5 | 5 |
| ScalarEvolutionAnalysis | 5 | 5 |
| MemorySSAAnalysis | 4 | 4 |
| PostDominatorTree | 3 | 3 |
| BasicAA / AAManager | 8 / 8 | 8 / 8 |
| all analyses on d_draw_faces | 92 | 60 |

Whole file:
- DomTree from scratch: 100 IR forward (new PM DominatorTreeAnalysis 58; the rest legacy codegen and internal recalcs), 39 IR post-dom.
- Incremental DomTree: SemiNCA ApplyUpdates 118 batches (InsertEdge 151, DeleteEdge 124); DomTreeUpdater applyUpdates 44 calls, applyUpdatesImpl 283 + 293 flush points.
- LoopInfo analyze: 67 IR, 30 machine.
- SCEV:

| ScalarEvolution item | Count |
|---|---|
| constructed | 24 |
| getSCEV | 9891 |
| createSCEV | 1333 (87% of `getSCEV` calls are cache hits) |
| forgetLoop | 24 |
| forgetValue | 111 |
| getBackedgeTakenInfo | 1919 |

SCEV inclusive cost: getSCEV 12.9 M, getRangeRef 10.7 M, getBackedgeTakenInfo 10.0 M.

### B.7 Pipeline iteration

d_draw_faces:
- 348 new-PM pass runs.
- Per-loop passes run once per visit over 22 loops: LICM 65, LoopRotate 43, LoopDeletion 43, IndVars/LoopIdiom/LoopInstSimplify/SimpleLoopUnswitch 22 each.
- Function-level passes: InstCombine 8 (IR size 1146–1313), SimplifyCFG 8, SROA 4, EarlyCSE 2.
- Legacy codegen: 198 pass executions.

Whole file:

| Item | Count | Note |
|---|---|---|
| new-PM pass runs | 951 | analyses run 505 / invalidated 312 |
| InstCombinePass::run | 73 | — |
| combineInstructionsOverFunction | 46 | 27 runs skipped by LastRunTracking (IR unchanged since the last run) |
| InstCombinerImpl::run iterations | 46 | exactly 1 per run (max-iterations=1) |
| SimplifyCFGPass | 78 | iterativelySimplifyCFG 78; simplifyCFG (per block) 3448 |
| EarlyCSE | 22 | — |
| DevirtSCCRepeatedPass / Inliner | 11 / 11 | one iteration per SCC; no devirt repeat |

InstCombine worklist size is not readable: the debug info has no locals. The IR size per run (above) is the initial worklist size, since prepareICWorklist adds every reachable instruction.

### B.8 Instruction selection and clean-up (d_draw_faces)

| Item | Count | M | Source |
|---|---|---|---|
| SelectionDAGs (CodeGenAndEmitDAG, one per IR block) | 156 | 45.3 (8.7% incl. of cc1) | [SelectionDAGISel.cpp:917](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L917) |
| SelectionDAG::Combine runs | 417 = 2×156 + 105 after type legalization | 21.0 | [SelectionDAGISel.cpp:958](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L958), [SelectionDAGISel.cpp:1004](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L1004) |
| DAGCombiner::combine (node visits) | 13 276 | 16.0 | [DAGCombiner.cpp:2019](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2019) |
| LegalizeTypes / Legalize / ScheduleDAGSDNodes::Run | 156 each | 2.6 / 3.0 / 5.0 | — |
| X86DAGToDAGISel::Select | — | 7.7 | — |
| MachineCSE | 1 run | 1.9 | [MachineCSE.cpp:941](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/MachineCSE.cpp#L941) |
| PeepholeOptimizer | 1 run | 1.8 | [PeepholeOptimizer.cpp:1676](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/PeepholeOptimizer.cpp#L1676) |
| MachineLICM | 2 runs (early + late), 3810 Hoist attempts | 5.5 | [MachineLICM.cpp:373](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/MachineLICM.cpp#L373) |
| BranchFolder OptimizeFunction / TailMergeBlocks / OptimizeBranches / OptimizeBlock | 2 / 5 / 4 / 965 | 1.7 | [BranchFolding.cpp:180](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/BranchFolding.cpp#L180) |
| MachineScheduler regions | 149 | 9.3 | [MachineScheduler.cpp:1455](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/MachineScheduler.cpp#L1455) |
| MachineCopyPropagation | 2 runs | 3.2 | — |

### B.9 How LLVM stays fast

**PreservedAnalyses + lazy AnalysisManager**
- Mechanism: an analysis runs only on `getResult`. After each pass, the manager drops only the results that the pass's PreservedAnalyses does not cover.
- Invariant: a cached result is valid until a pass that does not preserve it modifies the IR unit.
- Source: [PassManagerImpl.h:85](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/PassManagerImpl.h#L85), [PassManagerImpl.h:137](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/PassManagerImpl.h#L137), [PassManager.cpp:129](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/IR/PassManager.cpp#L129)
- Evidence: d_draw_faces had 348 pass runs, 92 analysis runs and 60 invalidations. DomTree was computed 9× for 348 passes; SCEV and LoopInfo 5× each.

**LastRunTracking (InstCombine)**
- Mechanism: InstCombine records the IR state it left the function in. A rerun on an untouched function is skipped.
- Invariant: InstCombine on its own fixpoint output is a no-op.
- Source: [InstructionCombining.cpp:5609](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5609)
- Evidence: 27 of 73 invocations skipped.

**InstCombine worklist, max-iterations=1**
- Mechanism: one pass over a worklist seeded with all instructions; changed users are re-added; no outer fixpoint loop.
- Invariant: users of a changed instruction go back on the worklist, so local fixpoint holds within one iteration.
- Source: [InstructionCombining.cpp:5127](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5127), [InstructionCombining.cpp:5550](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5550)
- Evidence: InstCombinerImpl::run ran 46 times in 46 combine calls (1.0 iterations each). Still the most expensive pass, at 56.5 M.

**SlotIndexes with gaps**
- Mechanism: indexes are spaced `InstrDist` apart. An inserted instruction takes the midpoint, and renumbering happens only when the gap is exhausted.
- Invariant: index order equals instruction order; live ranges stay valid without renumbering.
- Source: [SlotIndexes.h:555](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/CodeGen/SlotIndexes.h#L555), [SlotIndexes.h:564](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/CodeGen/SlotIndexes.h#L564), [SlotIndexes.cpp:172](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SlotIndexes.cpp#L172)
- Evidence: 439 inserts, 28 local renumbers, 0 repairIndexesInRange.

**LiveIntervals kept incrementally**
- Mechanism: computed once per function. The coalescer joins ranges, the scheduler calls handleMove, and RA edits through LiveRangeEdit (createFrom, eliminateDeadDefs, shrinkToUses).
- Invariant: each edit updates the affected intervals in place, so a full recompute is never needed.
- Source: [LiveIntervals.cpp:161](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L161), [LiveIntervals.cpp:1559](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervals.cpp#L1559), [LiveRangeEdit.cpp:55](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRangeEdit.cpp#L55)
- Evidence: LiveIntervals::analyze 1×; LiveIntervalCalc::calculate 1196 = 1068 + 128 new vregs, i.e. each interval computed once; repairIntervalsInRange 0; handleMove 81; shrinkToUses 103.

**Greedy priority queue + stage monotonicity**
- Mechanism: ranges are dequeued by priority. A failed range advances New → Assign → Split → Split2 → Spill → Done and never returns to an earlier stage. Split products inherit a later stage.
- Invariant: stage only increases and each stage has finitely many outcomes, so the loop terminates without a global retry.
- Source: [RegAllocEvictionAdvisor.h:46](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocEvictionAdvisor.h#L46), [RegAllocGreedy.cpp:2479](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2479), [RegAllocGreedy.cpp:1805](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1805), [RegAllocGreedy.cpp:2516](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2516), [RegAllocBase.cpp:90](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocBase.cpp#L90)
- Evidence: 941 dequeues for ~507 live vregs after coalescing (~1.9 per vreg); 99 trySplit, 99 spills, 0 last-chance recolorings.

**Eviction cascade + cost bound**
- Mechanism: each eviction assigns a cascade number. A range can evict only ranges with a lower cascade, and only when the summed spill weight is below the current best. The interference scan stops at `EvictInterferenceCutoff`.
- Invariant: cascades strictly increase along an eviction chain, so evictions cannot cycle.
- Source: [RegAllocGreedy.cpp:490](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L490), [RegAllocEvictionAdvisor.cpp:239](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocEvictionAdvisor.cpp#L239), [RegAllocEvictionAdvisor.cpp:208](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocEvictionAdvisor.cpp#L208)
- Evidence: 203 tryEvict calls, 1383 cost checks, 55 evictions (63 ranges); 0.9 M total.

**InterferenceCache**
- Mechanism: per-physreg, per-block first/last interference, filled lazily (`Entry::update`) and reused across split candidates. Entries are validated by LiveIntervalUnion tags.
- Invariant: an entry is used only while every reg unit's union tag is unchanged.
- Source: [InterferenceCache.cpp:62](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InterferenceCache.cpp#L62), [InterferenceCache.cpp:118](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InterferenceCache.cpp#L118), [InterferenceCache.cpp:131](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InterferenceCache.cpp#L131)
- Evidence: 645 get, 29 resets, 3510 block updates; 3.4 M.

**SplitAnalysis / SplitEditor + SpillPlacement**
- Mechanism: split cost is solved per candidate on the edge-bundle graph (Hopfield-style `iterate`); only the best candidate is materialized.
- Invariant: SplitEditor rewrites uses via the value map and recomputes only the new intervals.
- Source: [RegAllocGreedy.cpp:1063](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1063), [SpillPlacement.cpp:366](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SpillPlacement.cpp#L366), [SplitKit.cpp:1502](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SplitKit.cpp#L1502)
- Evidence: 645 candidates costed, 722 prepare, 2324 iterate, but only 31 region splits done. This search is RA's largest cost: tryRegionSplit 15.1 of 27.5 M.

**DomTreeUpdater batching**
- Mechanism: CFG edits queue updates, which are applied in batches through SemiNCA incremental update instead of rebuilds.
- Invariant: updates are applied before the next tree query (flush).
- Source: [GenericDomTreeUpdater.h:150](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/Analysis/GenericDomTreeUpdater.h#L150), [GenericDomTreeUpdater.h:202](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/Analysis/GenericDomTreeUpdater.h#L202)
- Evidence: 118 incremental batches (275 edge updates) against 100 IR rebuilds. Machine level has 0 incremental updates and 8 rebuilds on d_draw_faces.

**MemorySSA walker cache + check limit**
- Mechanism: a use's clobber is stored as its optimized access. Walks stop at `memssa-check-limit` = 100 alias checks, and LICM/EarlyCSE caps their own MSSA use (100/500).
- Invariant: the cached clobber stays valid until an update resets it.
- Source: [MemorySSA.cpp:2555](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSA.cpp#L2555), [MemorySSA.cpp:75](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSA.cpp#L75), [LICM.cpp:157](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LICM.cpp#L157), [EarlyCSE.cpp:76](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L76)
- Evidence: 20 MemorySSA builds, 245 walker calls (6.6 M).

**AAQueryInfo / BatchAA cache**
- Mechanism: (loc, loc) results are memoized per query batch.
- Invariant: the IR does not change during a batch.
- Source: [AliasAnalysis.h:266](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/Analysis/AliasAnalysis.h#L266), [BasicAliasAnalysis.cpp:1701](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L1701)
- Evidence: 2904 aliasCheck calls; 1850 reach aliasCheckRecursive, so 36% end in the cache or trivial checks. BasicAA total 5.7 M.

**SCEV memoization**
- Mechanism: ValueExprMap caches SCEV per value. forgetLoop/forgetValue drop only the affected entries.
- Invariant: every map entry is consistent with the current IR, because any pass that changes a value forgets it.
- Source: [ScalarEvolution.cpp:4547](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/ScalarEvolution.cpp#L4547), [ScalarEvolution.cpp:8494](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/ScalarEvolution.cpp#L8494)
- Evidence: getSCEV 9891, createSCEV 1333 (87% hits), forgetLoop 24, forgetValue 111. SCEV is still large: IndVars 19.3 M plus LSR 17.0 M.

**EarlyCSE scoped hash table**
- Mechanism: one dominator-tree DFS; available values are pushed and popped per scope.
- Invariant: a value is visible only in blocks its definition dominates.
- Source: [EarlyCSE.cpp:654](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L654)
- Evidence: 22 runs (2 on d_draw_faces), 8.2 M total.

**SimplifyCFG local iteration**
- Mechanism: iterates per block until no local change, with no whole-pipeline repeat.
- Source: [SimplifyCFGPass.cpp:248](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L248), [SimplifyCFG.cpp:8469](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8469)
- Evidence: 78 runs, 3448 simplifyCFG block visits, 15.6 M.

**CGSCC devirt cap**
- Mechanism: DevirtSCCRepeatedPass repeats only when a call was devirtualized, up to MaxIterations.
- Source: [CGSCCPassManager.cpp:358](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/CGSCCPassManager.cpp#L358), [CGSCCPassManager.cpp:481](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/CGSCCPassManager.cpp#L481)
- Evidence: 11 SCCs, 11 runs, no repeats.

**Biggest non-algorithmic cost**
- Process start: dynamic linking of libLLVM/libclang-cpp is 67.9 M (12.3%) and static initializers 13.4 M (2.4%). Together that is more than RAGreedy and isel combined for this file.

---

## Appendix C. How to reproduce

From `qbq-llrm/src`, with `F` the foreign flags of `compile_time.sh` (`-D` far/near keywords empty, `-march=i486 -fno-pic -fno-stack-protector -fcf-protection=none`) and `I="-I . -I render -I game -I host -I qgl"`:

| what | command |
|---|---|
| whole compile | `perf stat -e instructions:u llrm-cc -m32 -O1 $I -S -o /dev/null render/d_faces.c`; the same with `gcc`/`clang` and `$F` |
| llrm steps, nesting, cache lookups | `LLRM_DEBUG=time LLRM_TIME_TOP=500` (add `timefunc` for per-function totals) |
| optimizer rounds | `LLRM_DEBUG=runs` |
| allocation events | `LLRM_DEBUG=regalloc,split,spill` |
| one allocation | `-fno-allocation-search -fno-allocation-routes` |
| LIR per machine phase | `ISEL_DUMP=1` |
| gcc allocation and passes | `-fdump-rtl-ira-details -fdump-rtl-reload-details -fdump-statistics-stats -fdump-passes` |
| clang passes and analyses | `-Xclang -fdebug-pass-manager`, `-mllvm -print-pipeline-passes`, `-mllvm -debug-pass=Structure`, `-mllvm -print-after=greedy -mllvm -filter-print-funcs=d_draw_faces` |
| per-pass shares | `perf record -e instructions:u --call-graph dwarf`, then `perf report --children`; average several runs (single runs vary by up to 2 points) |
| internal call counts | gdb breakpoint hit counts on cc1 and on libLLVM (with Ubuntu's `libllvm20-dbgsym` for internal symbols) |
