---
architecture-index: pass lists against clang
architecture-order: 200
---
# Pass-list parity with clang

For contributors choosing which passes run at which level. Clang 20's pipelines are the reference (the pass lists; the costs below are the distro clang 20.1.8's and are void as a cost reference to the pinned 24.0.0git, whose `d_faces` -O2 is 415.5 M. It compiles d_faces in 520/670/672/567 M instructions at -O1/-O2/-O3/-Os, gcc 13.4 in 636/1137/1413/944, llrm in 3,040/5,521/5,930/5,671); gcc's passes are listed beside them and taken only where a measured row says gcc does better.

TL;DR: the pass lists differ only at -O1, where llrm runs `jumpthread`, `dse` and a full `gvn` that clang's -O1 does not; dropping the first two is free on the 71 programs and d_faces (-164 M, -5.5%), once a slot's stores go with its last read. At -O2, -O3 and -Os llrm's list is a subset of clang's; the 8x gap is each pass costing 4x to 23x its clang counterpart, and `rounds` (clang runs each instance once, 8 interleaved InstCombines and SimplifyCFGs) is a step that cost 50% more instructions when removed at -O1.

## Sources

Behaviour only. Clang: `clang -O<n> -mllvm -print-pipeline-passes` of clang 20.1.8, built from [`PassBuilderPipelines.cpp`](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Passes/PassBuilderPipelines.cpp#L409) (`buildO1FunctionSimplificationPipeline` L409, `buildFunctionSimplificationPipeline` L549, `buildModuleOptimizationPipeline` L1414). Per-pass time: `clang -ftime-report` on `render/d_faces.c`, a pass's share of the whole compile applied to the instructions of that compile (a timer, not a counter: read a row within about 15%). gcc: `gcc -O<n> -fdump-passes` of gcc 13.4 ([`passes.def`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def)), the passes marked ON, by family. llrm: `LLRM_DEBUG=time LLRM_TIME_TOP=400`, runs and Minstr (own and inclusive of the analyses a pass asks for) on main 4532a46f1, d_faces -m32, the same file and flags.

## Per level

`llrm/clang` is llrm's inclusive cost over clang's estimated cost of the passes that do the same work. Rows pair what one clang pass does with what several of ours do (InstCombine is `fold` and `algebraic`; LICM is `hoist` and `loopmotion`).


### -O1 (d_faces: clang 520 M, gcc 636 M, llrm 3040 M on main 4532a46f1)

| llrm pass | runs | own M | incl M | clang passes (instances at this level) | clang est. M | llrm/clang | gcc families (instances) |
|---|---|---|---|---|---|---|---|
| sroa + promote | 33 | 48 | 107 | sroa×4, mem2reg×1, loop-load-elim×1 | 13.5 | 7.9x | tree-esra×1, tree-sra×1, tree-ssa×1 |
| fold + algebraic | 46 | 92 | 324 | instcombine×8, instsimplify×1, sccp×1, reassociate×1, aggressive-instcombine×0, vector-combine×1 | 77.3 | 4.2x | tree-ccp×5, tree-forwprop×4, tree-copyprop×5, tree-reassoc×2, tree-widening_mul×0 |
| decide | 34 | 79 | 144 | simplifycfg×8, correlated-propagation×0, constraint-elimination×0 | 19.9 | 7.2x | tree-dom×3, tree-cfg×1, tree-vrp×0, tree-evrp×0 |
| jumpthread | 11 | 62 | 131 | jump-threading×0 | 0.0 | — | tree-thread×0, tree-threadfull×0 |
| hoist + loopmotion | 45 | 208 | 293 | licm×3 | 12.8 | 23.0x | tree-lim×4 |
| gvn | 34 | 133 | 172 | early-cse×2, gvn×0, mldst-motion×0 | 11.4 | 15.2x | tree-fre×3, tree-pre×0 |
| dse | 33 | 45 | 59 | dse×0 | 0.0 | — | tree-dse×5 |
| dead | 43 | 10 | 10 | adce×1, bdce×1 | 4.3 | 2.4x | tree-dce×8, tree-cddce×3 |
| indvars | 44 | 87 | 106 | indvars×1, loop-deletion×2 | 28.4 | 3.7x | tree-ivcanon×1, tree-sccp×1 |
| lsr | 11 | 127 | 193 |  | 0.0 | — | tree-ivopts×1 |
| rotate | 22 | 3 | 18 | loop-rotate×2 | 9.9 | 1.8x | tree-ch×2 |
| fill + merge | 24 | 23 | 25 | loop-idiom×1, loop-distribute×1, memcpyopt×1 | 12.1 | 2.0x | tree-ldist×0, tree-store-merging×0 |
| unroll | 24 | 5 | 16 | loop-unroll-full×1, loop-unroll×1 | 0.7 | 22.5x | tree-cunroll×1, tree-cunrolli×0 |
| sink | 11 | 7 | 19 | loop-sink×1 | 0.0 | — | tree-sink×2 |
| interprocedural | 1 | 56 | 83 | inline×1, ipsccp×1, globalopt×2, function-attrs×2 | 17.0 | 4.9x | ipa-inline×1, ipa-pure-const×1, ipa-cp×0 |
| lcssa + loopsimplify | 45 | 33 | 36 | (lcssa)×0, (loop-simplify)×0 | 4.3 | 8.4x |  |
| **sum of these rows** | | | 1736 | | 211 | 8.2x | |
| mir pipeline (all of them and what they ask for) | 12 | 77 | 2009 | clang optimizer (all IR passes) | 233 | 8.6x | |


### -O2 (d_faces: clang 670 M, gcc 1137 M, llrm 5521 M on main 4532a46f1)

| llrm pass | runs | own M | incl M | clang passes (instances at this level) | clang est. M | llrm/clang | gcc families (instances) |
|---|---|---|---|---|---|---|---|
| sroa + promote | 35 | 68 | 150 | sroa×4, mem2reg×1, loop-load-elim×1 | 12.8 | 11.7x | tree-esra×1, tree-sra×1, tree-ssa×1 |
| fold + algebraic | 47 | 101 | 362 | instcombine×8, instsimplify×1, sccp×1, reassociate×1, aggressive-instcombine×1, vector-combine×2 | 87.1 | 4.2x | tree-ccp×5, tree-forwprop×4, tree-copyprop×5, tree-reassoc×2, tree-widening_mul×1 |
| decide | 35 | 81 | 153 | simplifycfg×8, correlated-propagation×2, constraint-elimination×1 | 33.5 | 4.6x | tree-dom×3, tree-cfg×1, tree-vrp×2, tree-evrp×1 |
| jumpthread | 11 | 62 | 137 | jump-threading×2 | 10.0 | 13.8x | tree-thread×2, tree-threadfull×2 |
| hoist + loopmotion | 46 | 214 | 306 | licm×4 | 17.1 | 17.9x | tree-lim×4 |
| gvn | 35 | 357 | 405 | early-cse×2, gvn×1, mldst-motion×1 | 66.4 | 6.1x | tree-fre×5, tree-pre×1 |
| dse | 34 | 46 | 60 | dse×1 | 3.6 | 16.9x | tree-dse×5 |
| dead | 45 | 13 | 13 | adce×1, bdce×1 | 4.3 | 2.9x | tree-dce×8, tree-cddce×3 |
| indvars | 46 | 101 | 113 | indvars×1, loop-deletion×2 | 37.8 | 3.0x | tree-ivcanon×1, tree-sccp×1 |
| lsr | 11 | 149 | 225 |  | 0.0 | — | tree-ivopts×1 |
| rotate | 22 | 3 | 19 | loop-rotate×2 | 10.0 | 1.9x | tree-ch×2 |
| fill + merge | 26 | 34 | 36 | loop-idiom×1, loop-distribute×1, memcpyopt×1 | 10.7 | 3.3x | tree-ldist×1, tree-store-merging×1 |
| unroll | 26 | 7 | 21 | loop-unroll-full×1, loop-unroll×1 | 12.1 | 1.7x | tree-cunroll×1, tree-cunrolli×1 |
| sink | 11 | 7 | 22 | loop-sink×1 | 0.0 | — | tree-sink×2 |
| interprocedural | 1 | 121 | 159 | inline×1, ipsccp×1, globalopt×2, function-attrs×2 | 18.6 | 8.6x | ipa-inline×1, ipa-pure-const×1, ipa-cp×1 |
| lcssa + loopsimplify | 46 | 34 | 37 | (lcssa)×0, (loop-simplify)×0 | 4.3 | 8.6x |  |
| **sum of these rows** | | | 2218 | | 328 | 6.8x | |
| mir pipeline (all of them and what they ask for) | 12 | 86 | 2502 | clang optimizer (all IR passes) | 300 | 8.3x | |


### -O3 (d_faces: clang 672 M, gcc 1413 M, llrm 5930 M on main 4532a46f1)

| llrm pass | runs | own M | incl M | clang passes (instances at this level) | clang est. M | llrm/clang | gcc families (instances) |
|---|---|---|---|---|---|---|---|
| sroa + promote | 41 | 84 | 187 | sroa×4, mem2reg×1, loop-load-elim×1 | 12.1 | 15.4x | tree-esra×1, tree-sra×1, tree-ssa×1 |
| fold + algebraic | 56 | 126 | 412 | instcombine×8, instsimplify×1, sccp×1, reassociate×1, aggressive-instcombine×1, vector-combine×2 | 83.5 | 4.9x | tree-ccp×5, tree-forwprop×4, tree-copyprop×5, tree-reassoc×2, tree-widening_mul×1 |
| decide | 43 | 104 | 178 | simplifycfg×8, correlated-propagation×2, constraint-elimination×1 | 33.5 | 5.3x | tree-dom×3, tree-cfg×1, tree-vrp×2, tree-evrp×1 |
| jumpthread | 12 | 62 | 138 | jump-threading×2 | 9.3 | 14.9x | tree-thread×2, tree-threadfull×2 |
| hoist + loopmotion | 55 | 225 | 320 | licm×4 | 17.8 | 17.9x | tree-lim×4 |
| gvn | 43 | 386 | 438 | early-cse×2, gvn×1, mldst-motion×1 | 66.3 | 6.6x | tree-fre×5, tree-pre×1 |
| dse | 42 | 55 | 71 | dse×1 | 3.6 | 20.0x | tree-dse×5 |
| dead | 54 | 16 | 16 | adce×1, bdce×1 | 4.3 | 3.7x | tree-dce×8, tree-cddce×3 |
| indvars | 55 | 116 | 129 | indvars×1, loop-deletion×2 | 37.1 | 3.5x | tree-ivcanon×1, tree-sccp×1 |
| lsr | 12 | 150 | 227 |  | 0.0 | — | tree-ivopts×1 |
| rotate | 24 | 3 | 18 | loop-rotate×2 | 10.7 | 1.7x | tree-ch×2 |
| fill + merge | 30 | 35 | 36 | loop-idiom×1, loop-distribute×1, memcpyopt×1 | 10.7 | 3.4x | tree-ldist×1, tree-store-merging×1 |
| unroll | 30 | 8 | 22 | loop-unroll-full×1, loop-unroll×1 | 12.8 | 1.7x | tree-cunroll×1, tree-cunrolli×1 |
| sink | 12 | 8 | 22 | loop-sink×1 | 0.0 | — | tree-sink×2 |
| interprocedural | 1 | 157 | 408 | inline×1, ipsccp×1, globalopt×2, function-attrs×2 | 17.1 | 23.8x | ipa-inline×1, ipa-pure-const×1, ipa-cp×1 |
| lcssa + loopsimplify | 55 | 42 | 44 | (lcssa)×0, (loop-simplify)×0 | 4.3 | 10.4x |  |
| **sum of these rows** | | | 2665 | | 323 | 8.2x | |
| mir pipeline (all of them and what they ask for) | 12 | 89 | 2852 | clang optimizer (all IR passes) | 301 | 9.5x | |


### -Os (d_faces: clang 567 M, gcc 944 M, llrm 5671 M on main 4532a46f1)

| llrm pass | runs | own M | incl M | clang passes (instances at this level) | clang est. M | llrm/clang | gcc families (instances) |
|---|---|---|---|---|---|---|---|
| sroa + promote | 35 | 68 | 150 | sroa×4, mem2reg×1, loop-load-elim×1 | 10.4 | 14.4x | tree-esra×1, tree-sra×1, tree-ssa×1 |
| fold + algebraic | 36 | 80 | 280 | instcombine×8, instsimplify×1, sccp×1, reassociate×1, aggressive-instcombine×1, vector-combine×2 | 71.2 | 3.9x | tree-ccp×5, tree-forwprop×4, tree-copyprop×5, tree-reassoc×2, tree-widening_mul×1 |
| decide | 35 | 81 | 153 | simplifycfg×8, correlated-propagation×2, constraint-elimination×1 | 26.6 | 5.7x | tree-dom×3, tree-cfg×1, tree-vrp×2, tree-evrp×1 |
| jumpthread | 11 | 49 | 112 | jump-threading×2 | 6.9 | 16.1x | tree-thread×2, tree-threadfull×2 |
| hoist + loopmotion | 46 | 436 | 533 | licm×4 | 10.4 | 51.1x | tree-lim×4 |
| gvn | 35 | 357 | 406 | early-cse×2, gvn×1, mldst-motion×1 | 40.0 | 10.2x | tree-fre×5, tree-pre×1 |
| dse | 34 | 46 | 60 | dse×1 | 2.9 | 20.8x | tree-dse×5 |
| dead | 34 | 10 | 10 | adce×1, bdce×1 | 3.5 | 2.9x | tree-dce×8, tree-cddce×3 |
| indvars | 35 | 88 | 97 | indvars×1, loop-deletion×2 | 23.7 | 4.1x | tree-ivcanon×1, tree-sccp×1 |
| lsr | 11 | 154 | 481 |  | 0.0 | — | tree-ivopts×1 |
| rotate | 11 | 0 | 1 | loop-rotate×2 | 6.9 | 0.2x | tree-ch×2 |
| fill + merge | 26 | 35 | 36 | loop-idiom×1, loop-distribute×1, memcpyopt×1 | 9.3 | 3.9x | tree-ldist×1, tree-store-merging×1 |
| unroll | 26 | 7 | 21 | loop-unroll-full×1, loop-unroll×1 | 9.3 | 2.2x | tree-cunroll×1, tree-cunrolli×1 |
| sink | 11 | 4 | 15 | loop-sink×1 | 0.0 | — | tree-sink×2 |
| interprocedural | 1 | 118 | 157 | inline×1, ipsccp×1, globalopt×2, function-attrs×2 | 13.9 | 11.3x | ipa-inline×1, ipa-pure-const×1, ipa-cp×1 |
| lcssa + loopsimplify | 35 | 27 | 29 | (lcssa)×0, (loop-simplify)×0 | 3.5 | 8.2x |  |
| **sum of these rows** | | | 2541 | | 239 | 10.6x | |
| mir pipeline (all of them and what they ask for) | 12 | 82 | 2824 | clang optimizer (all IR passes) | 254 | 11.1x | |


## What the list changes

Experiments on the 71 vsgcc programs (executed instructions, 486 clocks, code bytes) at -O1, llrm against itself, and against min(gcc, clang) per row; "falls" is a row where llrm was at or below min(gcc, clang) and is above it after.

| change at -O1 | d_faces compile | 71 programs, geomean ins / clocks / code | rows that fall (ins / clocks / code) |
|---|---|---|---|
| no `jumpthread` (clang's -O1 has none) | 3,041 -> 2,932 M | 1.001 / 1.002 / 1.002 | 0 / 0 / 0 |
| no `decide` | 3,041 -> 3,031 M | 1.000 / 1.001 / 1.001 | 0 / 1 / 0 |
| no `gvn` (clang has EarlyCSE x2, no GVN) | not measured | 1.127 / 1.133 / 1.020 | 3 / 6 / 6 |
| no `dse` (clang has none) | not measured | 1.355 / 1.277 / 1.667 | 22 / 23 / 54 |
| no `dse`, with a slot's stores going with its last read (`slots`) | 2,976 -> 2,922 M | 1.000 / 1.000 / 1.001 | 0 / 0 / 0 |
| no `dse` and no `jumpthread`, with `slots` | 2,976 -> 2,812 M | 1.001 / 1.002 / 1.003 | 0 / 0 / 0 |
| `rounds` = 1 at every level | 3,084 -> 2,682 M at -O1 | 1.511 / 1.402 / 1.467 | 25 / 32 / 51 |

`dse` is load-bearing today because `promote` leaves every store it replaces a load of and `dse` is the pass that removes them; clang's SROA and mem2reg remove the stores and the alloca themselves. d_faces is identical at -O1 with and without `dse` once the slot's stores go with its last read. With `jumpthread` off d_faces runs 0.17% more clocks and 3.5% fewer code bytes. `gvn` stays at -O1. EarlyCSE's way, one walk down the dominator tree with scoped tables, was built (`avail::forwardable_scoped`, branch `alim/perf/early-cse-gvn`) and does not pay at the walker's quality. d_faces -O1 (main 3,041 M, 424,865 instructions, 191,021 memory operands): tables reset at every join, `gvn` 172 -> 111 M and the compile -39 M, but memory operands +5.1%, instructions +1.7%, clocks +0.38%; the entries of the dominator carried across a join and killed eagerly by what the paths between write, output identical to main and the compile +107 M (120 M in `regions::may_alias`: each indexed write against each indexed entry); the same kills made lazily, when a load hits an entry, output identical, `gvn` 217 M against 172 M. Clang's EarlyCSE is cheap because a write kills everything, which is the first row's quality; llrm's MemorySSA walker is cheaper than the precise pairwise test because it shares its structure with every other client and memoizes its alias answers. The cost to cut is the walker's (`frontier_for` 56 M, `walked_from` 55 M, MemorySSA structure 57 M), not the walk's shape.

At -O2, -O3 and -Os clang runs every pass llrm does (llrm's list is a subset of clang's, including its `jump-threading` x2, `gvn`, `dse`, `correlated-propagation` x2), so no pass leaves a list there; what the table shows at those levels is cost per pass.

## Where the gap is

The mid-end is 8.6x clang's optimizer at -O1 (2,009 M against an estimated 233 M). The widest rows at -O1: `hoist` + `loopmotion` 23x (LICM), `gvn` 15x (EarlyCSE), `decide` 7x (SimplifyCFG), `sroa` + `promote` 8x, `fold` + `algebraic` 4x (InstCombine, which is also the largest clang row at 31%). `lsr` (193 M) has no IR counterpart: clang's LoopStrengthReduce runs in code generation at 8% of the compile.

## The alias ladder

`regions::overlapping` answers the walker's pairwise queries cheapest first already: on d_faces -O1 it is asked 99,086 times (104,866 at -O2); `typed_apart` answers 43,666, canonical references' displacement 2,731, an unescaped object against a pointer 7,378, `foreign_apart` none (111 K calls, 6 M), and 45,311 (46%) reach the precise `may_alias`. Two wastes: `may_alias` runs `typed_apart` and `foreign_apart` again (45 K calls, ~7 M), and it is asked 45,311 times for 6,411 distinct pairs, 73% of them with no interval facts (a pure function of the two references and the target). The whole of `overlapping` costs 54 M (1.8% of the compile); removing both wastes would save about 21 M (0.7%). Measured and not done: a gain under 1% is not worth a change.

## Compile cost against clang and gcc

`tools/compile-gap.py` (needs `QCPORT`) builds llrm-cc with the `dist` profile (fat LTO, one codegen unit: the shipped binary; a `release` build reads 11% to 20% more instructions, so compile-time claims quote `dist` and gates stay on `release`), compiles d_faces and the bench programs `-S` with `-fno-inline-functions` at each level, and prints user-space instructions of llrm, clang and gcc (`perf stat`, best of three). For d_faces it then prints llrm's phases in **own** instructions (`LLRM_DEBUG=time`'s `[instr]` table: a step's analyses are listed under their own names, so no instruction counts twice) beside clang's `-ftime-report` shares applied to its instruction count. Wall-clock columns of the same report move with the load of the host (a loaded run read `annotated` at 5x); the instruction columns do not. Main 254855801 with #1912 (one allocation per function below -Omax), d_faces, M instructions:

| level | llrm | clang | gcc | llrm own: mir / analysis / regalloc / lir / isel | clang optimizer / codegen |
|---|---|---|---|---|---|
| -O1 | 2,423 | 481 | 662 | 820 / 556 / 445 / 339 / 51 | 189 / 233 |
| -O2 | 3,147 | 557 | 1,162 | 1,342 / 701 / 479 / 366 / 55 | 272 / 221 |
| -O3 | 3,471 | 558 | 1,445 | 1,561 / 772 / 490 / 382 / 62 | 277 / 218 |
| -Os | 3,416 | 553 | 968 | 1,385 / 914 / 500 / 325 / 52 | 254 / 232 |

llrm compiles d_faces in 5.0x to 6.2x clang's instructions. Per-program gaps are small except where a loop nest is fully unrolled: at -O3 matmul 781/251/708, nbody 192/93/157, quicksort 119/79/108 (llrm/clang/gcc); every other program and level is within 0.8x to 1.5x of clang. matmul -O3's output is the size of clang's (486 against 432 lines); its cost is the 6.4x larger body (894 to 1,422 instructions) the cleanup rounds run over, 39% of the pipeline's work in steps that found nothing.

Levers at -O2, own instructions against the clang counterpart's share of its optimizer or codegen (an upper bound: no pass reaches zero):

| lever | llrm own | clang's | ceiling |
|---|---|---|---|
| `gvn` | 325 | ~47 (GVN, EarlyCSE) | 278 (8.8%) |
| the analyses (`bounded` 99, `through-memory` 69, `annotated` 67, `pointer-values` 62, others) | 701 | inside its passes | up to 701 (22%); `annotated` alone is 67 (2.1%) |
| loop passes: `lsr` 128, `hoist` 123, `indvars` 62 | 313 | ~53 (LSR, LICM, IndVars) | ~260 (8.3%) |
| `interprocedural` 108, `jumpthread` 57, `decide` 73 | 238 | ~35 | ~200 (6.4%) |
| allocation (`regalloc`, `split`, `spill`, `facts`) | 479 | ~60 | ~420 (13%) |
| `lir` (`peephole` 106, `jumps` 64, ...) | 366 | ~35 | ~330 (10.5%) |
| module analyses with no timer of their own (`mir pipeline`: `Summaries`, `GlobalsAA`) | 81 | none | 81 (2.6%) |

The ceilings of the rows above the analyses sum to 1,569 M of 3,147: reaching every one leaves 1,578 M, 2.8x clang's 557 (distro; 3.8x the pinned clang 24's 415.5 M: derived, not measured, as llrm's 1,578 M is not re-taken on the pinned build), and taking half of the analyses' 701 as well leaves 1,228 M, 2.2x (3.0x the pinned: derived, not measured). The rest of the gap is not in a pass of the list: it is the 385 M of the other `mir` steps (`fold`, `algebraic`, `dse`, `promote`, ...), the 100 M of summaries and `memoryssa`, and the cost of each visit of a step that finds nothing; the per-pass levers alone do not reach clang.

An earlier version of this table read inclusive times (`annotated` 37 ms is its 8 ms plus the analyses it asks for) as if own; the rows above do not overlap.

## Not measured

m16 (the Open Watcom rows of `tools/bench`), clang's limits per level (inline threshold, unroll thresholds, `devirt<4>`), and trunk's pipeline against the measured release.
