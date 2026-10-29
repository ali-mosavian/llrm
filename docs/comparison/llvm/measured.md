---
architecture-index: LLVM measured
architecture-order: 310
---
# LLVM measured: what bounds its optimizer's work

TL;DR: on d_faces `-O2`, LLVM runs about as many passes as llrm (856 against 746) and spends 7x to 17x fewer instructions per run (0.21 M against 1.4 M with analyses; 11 M against 0.63 M on `d_draw_faces`). The number of runs is not the gap; the cost of a run is. It is small because each pass is one worklist walk over a function that SROA and EarlyCSE have already cut to 42% of its size, because analyses are computed a handful of times and thrown away only after a pass changes the IR, and because every query-driven walk has a cap.

Measured with an LLVM built from source (Release, assertions off, X86 only, statically linked, `-g1`) and `perf stat -e instructions:u`, quickest of three; one pinned commit for the structure, one tag for the scoreboard column. Nothing here claims a one-to-one pass equivalence: llrm's MIR and LLVM's IR differ, so rows compare phases and structures.

## Setup

| what | value |
|---|---|
| LLVM commit (structure, callgrind) | `d1106deb71cc81016406a1917d0508d2df296266`, clang 24.0.0git |
| LLVM tag (scoreboard column) | `llvmorg-20.1.8`, `87f0227cb60147a26a1eeb4fb06e3b505e9c7261` |
| build | `cmake -G Ninja llvm -DCMAKE_BUILD_TYPE=Release -DLLVM_ENABLE_ASSERTIONS=OFF -DLLVM_TARGETS_TO_BUILD=X86 -DLLVM_ENABLE_PROJECTS=clang -DCMAKE_CXX_FLAGS=-g1 -DCMAKE_C_COMPILER=clang-20 -DCMAKE_CXX_COMPILER=clang++-20 -DLLVM_USE_LINKER=lld -DLLVM_INCLUDE_TESTS=OFF`, then `ninja clang opt llc llvm-extract` |
| input | `render/d_faces.c` of qbq-llrm, `-w -m32 -march=i486 -fno-inline-functions`, the Borland keywords defined away, as `tools/compile-gap.py` |
| llrm | `llrm-cc` built `--profile dist` from main `954bc48fa`, `tools/compile-gap.py` and `LLRM_DEBUG=runs,time` |
| d_faces IR | 11 functions, 3,295 IR instructions before optimization (counted from the `.ll`): `d_draw_faces` 2,742, `clip_w` 346, `sc_mipfloor_c` 70, `sc_shift_c` 34, `cam_plane_dist` 33, `qglFaceFetch` 27, `ifloor` 23, `d_faces_layout_ok` 14, three of 2 |

Commands: IR `clang <flags> -O2 -Xclang -disable-llvm-passes -emit-llvm -S`; pass counts `opt -passes='default<O2>' -debug-pass-manager -print-changed -disable-output` (`-Os` is `default<O2>` on the `optsize` IR, which LLVM requires); phases `valgrind --tool=callgrind clang -cc1 <the driver's cc1 line>` then `callgrind_annotate --inclusive=yes`.

## Total instructions (M, user space)

| program | level | llrm | clang 20 distro | clang 20.1.8 static | clang 24 static |
|---|---|---:|---:|---:|---:|
| d_faces | -O1 | 2,263 | 478 | 396 | 348 |
| d_faces | -O2 | 2,830 | 553 | 462 | 413 |
| d_faces | -O3 | 3,133 | 556 | 464 | 415 |
| d_faces | -Os | 2,896 | 550 | 459 | 407 |
| lru | -O2 | 114 | 76 | 43 | 39 |
| bintree | -O2 | 91 | 75 | 42 | 37 |
| queens | -O2 | 94 | 71 | 39 | 34 |
| sieve | -O2 | 61 | 68 | 36 | 32 |

The distro clang loads `libLLVM.so`: an empty `-c` compile is 43 M against 13 M static, and d_faces reads 20% above the same source built static. Small programs are hit hardest (lru 76 against 43), so llrm's ratio on them is 2.7x, not 1.5x, against a static build.

Phases of clang 24, one `cc1` process under callgrind (M; the rest of the total is process start):

| level | total | frontend | optimizer | codegen | of codegen: isel, greedy RA |
|---|---:|---:|---:|---:|---|
| -O1 | 349.8 | 33.1 | 121.4 | 186.4 | 49.1, 34.4 |
| -O2 | 415.4 | 33.0 | 177.7 | 195.7 | 50.7, 39.5 |
| -O3 | 416.9 | 33.1 | 179.0 | 195.9 | 50.6, 39.4 |
| -Os | 409.8 | 33.1 | 176.6 | 191.2 | 48.8, 39.9 |

llrm `-O2` by own work: `mir` 1,111, `analysis` 651, `regalloc` 456, `lir` 365, `isel` 55.

## Pass runs, d_faces `-O2`

| | LLVM 24 | llrm |
|---|---:|---:|
| function and loop pass runs, whole file | 856 | 746 |
| runs that change the IR | 182 (21%) | 85 (11%) |
| optimizer instructions (M) | 178 | 1,817 (`mir pipeline`, inclusive) |
| per run (M) | 0.21 | 2.4 (1.4 counting only a run's own and its analyses) |
| `d_draw_faces`: runs | 77 function + 207 loop (10 loops) | 86 (4 rounds) |
| `d_draw_faces`: runs that change | 78 of 284 | 21 of 86 |
| `d_draw_faces`: M per run | 0.63 | 11 |

By family (not a pass equivalence; llrm M counts the analyses a pass asked for first):

| LLVM pass | runs | M | llrm pass | runs | M |
|---|---:|---:|---|---:|---:|
| GVN | 11 | 32.1 | gvn | 35 | 237 |
| InstCombine | 88 (56 executed) | 39.5 | fold | 36 | 175 |
| LICM | 70 (loops) | 7.3 | hoist, loopmotion | 35, 35 | 90, 25 |
| SROA | 44 | 9.8 | promote, sroa | 35, 12 | 93, 37 |
| IndVarSimplify | 19 | 11.5 | indvars | 35 | 60 |
| SimplifyCFG | 88 | 8.4 | decide | 35 | 131 |

Analyses over the same runs (LLVM: `Running analysis` lines; llrm: `[instr]` table):

| analysis | LLVM computed, whole file | `d_draw_faces` |
|---|---:|---:|
| DominatorTree | 55 | 8 |
| LoopInfo | 37 | 6 |
| ScalarEvolution | 35 | 6 |
| MemorySSA | 27 | 4 |
| AAManager | 44 | 7 |
| BlockFrequency | 1 | 0 |

llrm: 2,953 analysis runs (651 M) and 88 `memoryssa structure` builds (42 M).

Top 25 by inclusive instructions, clang 24 `-O2` (M, share of 415 M; a pass includes the analyses it asked for):

| pass | M | % | pass | M | % |
|---|---:|---:|---|---:|---:|
| SelectionDAG isel | 50.7 | 12.2 | EarlyCSE | 5.7 | 1.4 |
| InstCombine | 39.5 | 9.5 | JumpThreading | 4.5 | 1.1 |
| Greedy RA | 39.5 | 9.5 | LiveVariables | 4.0 | 1.0 |
| GVN | 32.1 | 7.7 | MachineSinking | 3.8 | 0.9 |
| IndVarSimplify | 11.5 | 2.8 | LoopUnroll | 3.6 | 0.9 |
| SROA | 9.8 | 2.4 | MachineCopyPropagation | 3.2 | 0.8 |
| MachineScheduler | 9.6 | 2.3 | MachineBlockPlacement | 3.2 | 0.8 |
| MachineLICM | 8.5 | 2.1 | ReachingDefInfo | 3.1 | 0.7 |
| SimplifyCFG | 8.4 | 2.0 | LoopIdiomRecognize | 3.0 | 0.7 |
| LICM | 7.3 | 1.8 | LoopDeletion | 3.0 | 0.7 |
| CorrelatedValuePropagation | 6.7 | 1.6 | CodeGenPrepare | 2.9 | 0.7 |
| LiveIntervals | 6.7 | 1.6 | X86AsmPrinter | 2.8 | 0.7 |
| RegisterCoalescer | 6.5 | 1.6 | MachineCSE | 2.7 | 0.7 |
| LoopRotate | 5.9 | 1.4 | | | |

Scaling, one function extracted with `llvm-extract` (clang 24, `opt -O2` and `llc -O2` for i486, the cost of an empty module taken off):

| function | IR instrs | opt (M) | llc (M) | opt k/instr | llc k/instr |
|---|---:|---:|---:|---:|---:|
| qglFaceFetch | 27 | 4.0 | 2.7 | 148 | 100 |
| sc_mipfloor_c | 70 | 6.7 | 7.3 | 96 | 104 |
| scr_pal_blend | 125 | 6.1 | 9.9 | 48 | 79 |
| clip_w | 346 | 16.0 | 15.7 | 46 | 45 |
| r_portal_mark | 662 | 40.7 | 35.3 | 62 | 53 |
| sb_build | 723 | 33.0 | 30.5 | 46 | 42 |
| mdl_draw_tris | 1,424 | 106.5 | 82.1 | 75 | 58 |
| d_draw_faces | 2,742 | 179.7 | 149.3 | 66 | 55 |

## a. Runs per function

`d_draw_faces` gets 77 function-pass runs (45 distinct passes; SimplifyCFG and InstCombine 8 each, one GVN, one DSE) and 207 loop-pass runs over its 10 loops; the loop pipeline runs twice. llrm runs 86 on it in four rounds. A pass that changes nothing is cheap because the function it sees is small and the pass walks it once: On `d_draw_faces` SROA takes 2,663 instructions to 1,563 and EarlyCSE to 1,146 before the first InstCombine; later passes read about 1,000. InstCombine is skipped outright 32 times of 88 (`InstCombinePass::run` 88, `InstCombinerImpl::run` 56, callgrind), because the last run left nothing to do.

## b. Analyses

Each analysis is computed a few times per function: DominatorTree 55 times, LoopInfo 37, ScalarEvolution 35, MemorySSA 27, AAManager 44, BlockFrequency once, over 856 runs; on `d_draw_faces` 8, 6, 6, 4 and 7. SimplifyCFG is the main invalidator (DominatorTree 32 of 55, LoopInfo 18 of 37, MemorySSA 8 of 27); InstCombine and EarlyCSE appear 11 times each, once per function, which is the end of a function's pipeline, not an IR change. The attribution is the pass line nearest before each `Invalidating analysis` line, so it is approximate; reuse (a cached `getResult`) is not printed, so no reuse count is given. A pass that changes nothing returns `PreservedAnalyses::all()`; InstCombine, when it changes instructions only, keeps `CFGAnalyses`; the pass manager invalidates after every pass from that set.

## c. MemorySSA

27 builds cost 2.5 M (0.6%); the walker answers 286 `getClobberingMemoryAccess` queries (251 + 35) in 3.2 M (0.8%), 260 `findClobber` walks. GVN does not use MemorySSA at all; it uses MemoryDependence (23.5 M inclusive). The walk is capped at `memssa-check-limit` 100 and an access with more than 100 candidates is not optimized. llrm builds MemorySSA 88 times for 42 M (0.48 M a build against 0.09 M) and does not report walks in the table above; the 4,567 walks quoted elsewhere come from local instrumentation of `-Os` and are not repeated here.

## d. Where instructions go

Share of 415 M at `-O2`: isel 12.2%, greedy RA 9.5%, InstCombine 9.5%, GVN 7.7%, SROA 2.4%, SimplifyCFG 2.0%, LICM 1.8% (MachineLICM 2.1%), EarlyCSE 1.4%. The optimizer is 43%, codegen 47%, frontend 8%. Among optimizer passes InstCombine and GVN lead; llrm's `gvn` alone is 237 M, 8% of its compile.

## e. Scaling

Linear from about 100 instructions: optimizer plus codegen cost 88 to 133 k instructions per IR instruction from 346 to 2,742 instructions (log-log slope 0.9 for the optimizer from 70 to 2,742). Below 70 instructions fixed work (pipeline setup, analyses) dominates. The spread at one size (`sb_build` 46 k, `mdl_draw_tris` 75 k per instruction) is loops, not size.

## f. What bounds the work

| mechanism | where |
|---|---|
| InstCombine visits a worklist, one iteration by default (`InstCombineDefaultMaxIterations = 1`) | [`InstCombine.h#L29`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/InstCombine/InstCombine.h#L29), [`InstCombinerImpl::run#L5780`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5780), [`iteration limit#L6201`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L6201) |
| InstCombine is skipped when the last run found nothing and nothing changed since (`LastRunTrackingAnalysis`) | [`shouldSkip#L6262`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L6262), [`LastRunTrackingAnalysis.cpp#L30`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LastRunTrackingAnalysis.cpp#L30) |
| a pass that changes nothing returns `PreservedAnalyses::all()`; one that changes returns only what it kept | [`AM.invalidate after each pass#L80`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PassManagerImpl.h#L80), [`invalidate#L169`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PassManagerImpl.h#L169) |
| InstCombine keeps the CFG analyses | [`preserveSet<CFGAnalyses>#L6290`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L6288) |
| loop passes run per loop, in a loop pass manager with its own invalidation | [`LoopPassManager.cpp#L63`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopPassManager.cpp#L63) |
| the CGSCC pipeline reruns an SCC only when devirtualization or inlining changes it | [`DevirtSCCRepeatedPass#L367`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CGSCCPassManager.cpp#L367), [`CGSCCToFunctionPassAdaptor#L514`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CGSCCPassManager.cpp#L514) |
| MemorySSA walk limit 100 | [`MemorySSA.cpp#L77`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L77), [`MemorySSA.cpp#L1042`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1042) |
| LICM MemorySSA caps 100 and 250 | [`LICM.cpp#L150`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L150), [`LICM.cpp#L158`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L158) |
| EarlyCSE MemorySSA cap 500 | [`EarlyCSE.cpp#L77`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L77) |
| GVN gives up past 100 dependencies; MemoryDependence scans at most 100 instructions and 200 blocks | [`GVN.cpp#L129`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L129), [`GVN.cpp#L2073`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L2073), [`MemoryDependenceAnalysis.cpp#L75`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L75), [`MemoryDependenceAnalysis.cpp#L80`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L80) |
| ScalarEvolution arithmetic depth cap | [`ScalarEvolution.cpp#L198`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L198) |
| SROA works from a worklist of allocas | [`SROA.cpp#L188`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SROA.cpp#L188) |
| SimplifyCFG iterates only while it changes | [`SimplifyCFGPass.cpp#L229`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L229) |
| loop size thresholds: unroll, unswitch, jump-threading duplication | [`LoopUnrollPass.cpp#L83`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L83), [`SimpleLoopUnswitch.cpp#L92`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L92), [`JumpThreading.cpp#L88`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpThreading.cpp#L88) |
| the function simplification pipeline, one fixed order | [`PassBuilderPipelines.cpp#L630`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L630) |
| greedy RA takes live ranges from a priority queue | [`RegAllocGreedy.cpp#L418`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L418) |

Against llrm: the fixed order is the same idea as `pipeline-design.md`'s proposal, but LLVM's cheap runs come from the caps and the preserved-analysis contract, not from running fewer passes.
