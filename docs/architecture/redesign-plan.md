---
architecture-index: the plan to reach clang's compile time
architecture-order: 430
---
# Compile time at clang's level: the plan

**Scoreboard.** `d_faces` -O2 is 2,104 M instructions at main `9d50a350e` in a clean dist build, 5.1x clang 24's 413 M (section 2.0; the 2,253 M at `5e2e995ef` was not taken in the clean-build mode and is superseded; it was 2,903 M and 6.8x at `21f5c9fc8`); about 900 allocations per IR instruction against 19; 3.4 computes per pass run. Updated after each landing ([compile-gap-series]).

**Honest status.** The measured ceilings so far (keeps about 2%, updaters 0.5% to 3.5%, `ThroughMemory` cap 2.3%, `FloatFacts` 2% to 4%, algebraic 1% to 2%, allocator lanes about 8%) plus the queued PRs are well under half of the gap to 415 M. The rest is the architecture of 3.1/3.2 and the allocator rewrite, not constant factors.

**Target.** Parity is clang 24 at `d1106deb71cc` (415 M instructions on `d_faces` -O2) with rows held against min(gcc, clang); the static clang 20.1.8 (462 M) is the secondary column. Phase 3 (structural) starts now, in parallel with phases 1 and 2: each step is gated by its early probe and its design review, not by B1 or B2.

## Status

Merged since the scoreboard of section 2.0 (main `5eea0777a`; `d_faces` not re-taken since `cb2a78038`, so the scoreboard stands as measured): LoopInfo translated ([#2195]), InlineCost behind `LLRM_EXP_INLINEDECIDE` ([#2140]), the GVN and MemDep plan ([#2155]), poison float and address ([#2135]), two spiller fixes ([#2223]), call-effects renamed to a chain ([#2093]), `@noipa` on the Nib kernels ([#2210]), the pinned-clang row check (commit `5c49634a4`), three paid budget debts deleted. Open PRs are capped at 10 (oversight): green and correctness or instrument fixes stay in flight; red or stacked ones go to draft until green on main, order #2163, #2127, #2159, then #2164, #2172, #2160, #2137, #2156.

As of main `caf406d9b` (states checked with `gh`); `d_faces` -O2 is unchanged at 2,253 M since `5e2e995ef`, as no code landed in between (#2022, #2032, #2021, #2038 and #1958 are inside that baseline). Refreshed once per batch of merges by plan-upkeep-rows (the former llvm-compare). Sessions renamed as of 2026-10-17: the owner names in the tables below are the old ones. pass-list-parity and inline-storage-combine are now fold-poc; attrs-budgets, trace-recorder and sroa-promote-poc are llvm-order-poc (early FPM order and LLVM run counts, one experiment); ssa-spill-m16 and regalloc-quadratics are regalloc-core-poc; hoist-bounded and walker-caps are dse-poc (the MemorySSA and alias layer after); greedy-port, lsr-gvn-split and body-size-table are indvars-decide-poc; edit-rollback and edit-contract-replay are lir-cleanup-poc; objectref-identity and identity-s0-done are now cgscc-poc (the CGSCC inliner translation: bottom-up walk with simplify-before-inline, then `InlineCost.cpp`); the simplifycfg-poc name is retired, and SimplifyCFG keeps an owner (indvars-decide-poc, for the known-condition part) and a slot (LLVM runs it about 8 times). dse-poc is now memoryssa-port (`MemorySSA.cpp` and `MemorySSAUpdater.cpp` translated, #2108 and #2116; the LICM and DSE translations plug into it). fold-poc owns the single `InstructionSimplify` translation, used by the worklist and by EarlyCSE, and writes the `ValueTracking` API stub first. align-pad is now valuetracking-port (the translation of `ValueTracking.cpp`, `KnownBits` and `ConstantRange` at `d1106deb71cc`). llvm-order-poc: the EarlyCSE translation is done in shadow (54 of 58 identical to `opt`), `SROA.cpp` and `PromoteMemoryToRegister.cpp` next. gate-dist-build is now llvm-difftest (builds `tools/difftest`, the per-pass differential against LLVM `opt`; llvm-order-poc classifies; the EarlyCSE and GVN tables first). keep-analyses and facts-per-query are now basicaa-port (the translation of `BasicAliasAnalysis`, `CaptureTracking`, `AliasAnalysis` and `MemoryLocation` at `d1106deb71cc` as memoryssa-port's `AliasQuery`; the `Pointers` tower and its join rules are deleted when the clients move; first [#2125] and the main-order soundness search); identical-corpus is now simplifycfg-port (the `SimplifyCFG.cpp` translation, `ConstantFoldTerminator` and the early-FPM options first); scev-port ([#2150], `scev-port.md`): (1) LoopInfo first, which owns `Loops` and whose consumers are told first; (2) the SCEV tree translated on `ConstantRange`; (3) the Expander on the shared X86 TTI, which cgscc-poc owns; fill goes to LoopIdiom, owner loopidiom-port (renamed from loadpre-idioms); lcssa and stamp go to llvm-order-poc (stamp: it says what it is for first); emission is unowned and ranked last; the LIR clean-up (266 M gap) is lir-cleanup-poc, who writes a translation map. gvn-port ([#2155]: `GVN.cpp` and `MemoryDependenceAnalysis`) takes the gvn row's open owner; fold-poc also owns `ConstantFolding.cpp`; `AliasQuery` mirrors `AAResults` at the pin; the order stack lands OFF by default, with its conditions in its PR (oversight, 2026-10-18). Unchanged: plan-upkeep-rows.

Known gaps: the m16 rows were not re-run at `5e2e995ef` (run them off-peak when the queue is quiet). The rows at that SHA are the "never worse" baseline for every redesign PR: a per-row regression check, the worst row beside the geomean.

| step | owner | state | last measured |
|---|---|---|---|
| S1 fact placement | attrs-budgets, edit-rollback, keep-analyses | step 0 measured ([#2054]) | `ThroughMemory` enables 24.8% of `fold`'s edits, 0 of `decide`'s; `Annotated` 0 of `fold`'s, 52% to 75% of `hoist`, `gvn`, `loopmotion` |
| S2 fact-free run | pass-list-parity | measured: combine 0.30x to 1.03x of InstCombine at N=200 | exponent leaks: `dead` 1.43, `fold` on `mem` 1.41 (the `ThroughMemory` fact) |
| S4 allocator attribution | greedy-port, ssa-spill-m16 | step 0 measured ([#2049]); kill not met | 431 K fixed plus 309 per body instruction per round |
| S5 block-local dataflow | ssa-spill-m16 | KILLED ([#2043]) | -14.8 M but +0.36% bytes |
| S6 edit core | edit-rollback | measured; kill not met on `loop` only | `move_to` 925 M of 997.9 M primitives at N=1600 (4.32%) |
| S7 caps and pricing | hoist-bounded, greedy-port | running | forecast 2.6% / 4.9% |
| body-size factors per pass | indvars-decide-poc | measured ([#1994 comment](https://github.com/ali-mosavian/llrm/issues/1994#issuecomment-6237686443), [second](https://github.com/ali-mosavian/llrm/issues/1994#issuecomment-6237749851)) | gvn 26.3 K (llrm) against 28.6 K (LLVM) per instruction-run: 4.2x = runs 3.1 x body 1.47 x cost 0.98; regalloc 150 K against 25 K per instruction = body 1.41 x cost 4.3; table in 2.4 |
| fold numbers POC | fold-poc | measured; waits for the order | 19x to 10.1x of LLVM charged. The register-worklist fold matched 99.5% on `d_faces` and the benches (155 compiles) but missed 21% of 63,821 replacements over the full corpus (35,609 fold runs): front-end allocas are still in memory when fold runs on `tests/run/c`, qcport, nib and textfill. The PR waits for llvm-order-poc's order (promote, SROA, EarlyCSE before fold) and is re-judged by the corpus miss count. Lesson: a 155-compile set generalised wrongly |
| dse POC | dse-poc | measured | 13x LLVM per IR instruction; fact set plus walk step 3,440 instructions against a target of about 300 |
| lsr probe | indvars-decide-poc | KILLED | forecast kept; DEVIATE, 425 objects differ |
| mover-discount delete | regalloc-core-poc | queued, gated on rows | `unmeasured` |
| valuetracking-port | valuetracking-port | starting; fold-poc writes the API stub first | translation of `ValueTracking.cpp`, `KnownBits`, `ConstantRange`; consumers: InstSimplify gaps, EarlyCSE, InstCombine, SCCP, LVI, CVP, SCEV |
| basicaa-port | basicaa-port | starting | about 4,100 C++ lines at the core, about 6,400 llrm lines to delete when the clients have moved |
| oracle triage | basicaa-port | COLORS 634 of 1,323 ([#2111]), FRAME 21 ([#2112]), SCOPES 102 ([#2113]), STALE ([#2094]); soundness finds: DSE dropped a store through a phi with an unknown arm (cgscc-poc, fix PR coming), a join with an unfollowed arm published only the other arms' objects ([#2132], merged), kept points-to facts were not re-derived after in-place retargets ([#2125], merged), a body kept its memory-SSA structure after an edit changed it ([#2151], open), a volatile load is a Use in main's graph (issue #2129, no miscompile); oracle sweeps over C, BASIC, Nib and BQ at every level, m16 and m32, are a standing gate item (`identical.py`'s corpus includes the bas, nib and bq programs with every `LLRM_CHECK_*` on; llvm-difftest wires the gate); classes found on main by lir-cleanup-poc's sweep (`703d070b7` and #2152, 1,350 compiles, Nib only): an allocator crash on `jpeg_fixed.nib` -Omax (regalloc-core-poc), STALE frozen summaries at rotate, lcssa, window and lsr (cgscc-poc: soundness first, mirroring LLVM's rule that a pass must not add effects beyond the inferred memory attributes or must drop the attribute), REPLAY call-effects on Nib (basicaa-port, with #2158); [#2110] is held by the user (rows pending redo: its min(gcc, clang) sentence is void until redone on `main24d`, the pass-cost and object figures stand; no row trade; decision A: re-measure stacked on regalloc S1/S2 in shadow, the per-row table is part of S1's checkpoint) |
| 0.1 one baseline | llvm-compare | done at `5e2e995ef` except rows | `d_faces` -O2 2,253 M (5.5x clang 24), -O1 1,789, -O3 2,497, -Os 2,322; 2.98 M allocations, 601.7 MB |
| 0.2 counters as tests | attrs-budgets | draft [#1948] | counter cost 0.03% by callgrind own-function sum (bar 0.1% met by that number, not by perf stat, whose +0.13% total is inside the noise floor) |
| 0.3 `ObjectRef` identity (spike S0) | objectref-identity | answered: not a cause on main | `LLRM_CHECK_INTERN` scrambled ids: 0 of 1,583 differ; the 131 came from a loose share key; the context-free ordering cost +0.7% and was reverted |
| 1.0 inline storage | fold-poc | [#2051] (InlineVec) merged: instructions -0.277% (first taken -0.55%), cycles -0.14% against an A/A of 0.36, so inside noise (quiet window of 2026-10-18; first merged on instructions only, load about 90); [#2069] and [#2077] wait for a quiet-host cycles run | [#1956]: 259 K allocations |
| 1.1 slab | pass-list-parity | [#2022] merged | -4.2% instructions, -2.9% cycles (in the 5e2e995ef baseline) |
| 1.2 dense ids | attrs-budgets, ssa-spill-m16 | [#1971], [#2032] merged | -1.17%; -0.38% |
| 1.3 escape phase | keep-analyses | [#2035] open, [#2037] draft | -0.29%, -0.64% |
| 1.4 backend copies | ssa-spill-m16 | [#2027], [#2021], [#2038] merged | -0.56%, -0.52%; peak heap -15.4%, -13.9% (stacked peak: DHAT 18.7 MB at `5e2e995ef`) |
| 1.5 mask test | ssa-spill-m16 | [#2010] merged | diamond 800: 1,995 to 1,075 M; `d_faces` identical |
| 1.6 gvn, walker, lsr | greedy-port | [#1999], [#2017] merged; [#2012], [#2011], [#2020], [#2034], [#2008] open | `loop` N=1600 -65% stacked |
| 1.7 dse | edit-rollback | [#2015] merged; [#2013] open | 2.0x per doubling (`arith`), 2.17x to 2.33x (`mem`) |
| 1.8 deep nests | greedy-port | [#1977], [#1978] open | queens -O3 -23% |
| 1.9 exact ThroughMemory | keep-analyses | [#1915] merged | +1.0% (correctness) |
| 1.10 held PRs | greedy-port, attrs-budgets | [#1958] merged; [#1962], [#1965] drafts; [#1921] closed | per PR, stale |
| 2.1 point_values | keep-analyses | stopped; 2.1b killed ([#2053]) | asked 87% / 60% / 63% of solved; repeats 13.6 M |
| 2.2 Annotated per access | keep-analyses, edit-rollback | closed, not built ([#2072]) | 96.5% of reruns re-derive an equal answer, but a sound dependency from the change log invalidates 74.7% of entries (kill over 30%) |
| 2.3 persistent MemorySSA | hoist-bounded, keep-analyses | to do | `unmeasured` |
| 2.4 split path | greedy-port | B' killed; A' alone | B' -0.03% |
| 2.5 interval recompute | ssa-spill-m16 | closed | 0.25% realistic |
| 2.6 `Frequencies` dense | attrs-budgets | closed ([#2041]) | B-tree self cost at the sites 13.4 M, under the 15 M bar |
| 2.7 rollback | edit-rollback | draft [#1945] | +0.09% |
| 2.8 scaling shapes | hoist-bounded | to do | `unmeasured` |
| 2.9 Vec growth by site | unowned | to do | about 15 M |
| 3.1 facts from the change log | edit-rollback, keep-analyses | design, starts now | family 403 M own (12.8%) |
| 3.2 one pipeline pass | pass-list-parity, hoist-bounded | design, starts now | combine alone about 0; fold+algebraic 330 M is facts (290 of fold's 297 M: `FloatFacts` 136, `ThroughMemory` 72, `loop_exits` 30, alias tower 28, writes 23; rewriting about 7 M; 74 of algebraic's 101 M set-up) ([#2074]) |
| 3.3 allocator on the LIR core | greedy-port, ssa-spill-m16 | design page [#2042] (draft), starts now | `regalloc` 415.5 M |
| 3.4 LIR clean-up | ssa-spill-m16 | idle skip killed (10.4 M, [#2043]); solve-cost probe next | 290 M; exit about 250 M |
| 3.5 pricing (`hoist`, `lsr` forecast) | hoist-bounded, greedy-port | probe next | forecast 2.6% (-O2), 4.9% (-Os) |
| 3.6 code quality on pressure-heavy loops | greedy-port | closed at parity; Reassociate row low priority (pass-list-parity) | 90 / 24 against clang 82 / 23 |
| 3.7 llrm-codegen's machine layer target-generic | backend-two | M0 10-20 to FINAL 11-13; about 120 `llrm_x86*` references in about 40 core files | `llrm_x86*` references in core: about 120 |

## 1. TL;DR

`d_faces` -O2 compiles in 2,903 M instructions against LLVM's 415 M (6.8x; 4.8x to 6.4x in cycles) ([synthesis]). Bounded step by step, this plan does not close that gap. It removes about half of it, and it names the work that would close the rest:

| after | `d_faces` -O2, M instructions | over LLVM's 415 M | what that row counts |
|---|--:|--:|---|
| today | 2,903 (`21f5c9fc8`); 2,253 at main `5e2e995ef` | 6.8x; 5.4x | [synthesis]; the baseline of 0.1 ([compile-gap-series]) |
| phase 1 (the in-flight PRs that are ready) | about 2,660 | 6.4x | each PR's own measurement on its parent, multiplied: -8.3% |
| phase 1 plus the held PRs and drafts | about 2,510 | 6.0x | also [#1958] (an open ready PR, measured on an old main) and the drafts [#1921], [#1962], [#1965] at their old numbers: -13.6%. [#1921] is to be closed (see 1.10) and [#1962] is bundled with 2.6, so this row is an upper bound |
| phase 2 (open levers, each at its bound) | about 2,210 | 5.3x | point_values, Annotated, persistent MemorySSA, split path, interval recompute: -12% more |
| phase 3 (structural tracks, each at its bound) | about 1,460 | 3.5x | facts per query, one pipeline pass, allocator on the LIR core, LIR clean-up, pricing: -34% more |
| phase 4 (each pass at the spec's per-run budget) | 720 | 1.7x | [spec] section 1.3, a budget derived from gcc, not from clang |
| parity | 415 to 462 | 1.0x | needs budgets re-derived from clang's own phases (front end 33 M, optimizer 178 M, codegen 196 M, [readme]) |

The pools overlap, so each product overstates what the steps save together. The phase 2 and 3 rows are the plan's original bounds: 2.4 and 2.5 have since shrunk or closed (see their blocks), 3.1's saving is `unmeasured` until a family is built, and the "held drafts" row includes [#1921], which is to be closed. B1 re-derives the rows.

**Why the listed steps cannot close it.** The gap is not the number of pass runs (746 against 856). It is the cost of each run: a MIR pass run costs about 19 k instructions per IR instruction, against LLVM's several hundred (the "220" of [spec] 1.2 is not a per-pass floor: see 4.2). Every llrm pass sweeps the whole function with whole-function facts, recomputed 2.8 times per pass run; only 1.4% of the computes are on a body version already computed, so the ratio falls only by updating facts from the change log or asking them per query (step 3.1). An LLVM pass makes one worklist walk over a body that SROA and EarlyCSE have already shrunk. It asks per-query facts with caps, and 0.53 analysis computations per run ([spec] 1.2, [measured]). The in-flight set lowers constants: allocation, containers and quadratic loops. It does not change that shape.

**Architecture is necessary, not sufficient.** The repo's bounds for each architectural invariant take `d_faces` -O2 to about 1,500 to 2,000 M (3.6x to 4.8x); parity needs each pass at a per-run budget derived from clang (step 4.2). The study is [architecture-parity](architecture-parity.md); its spikes S0 to S7 run first (Phase A).

**What the plan puts on the critical path:**

0. Answered (spike S0): `ObjectRef` interning order is not a cause on main (`LLRM_CHECK_INTERN` scrambled ids: 0 of 1,583 differ); the 131 differing objects of the `point_values` share prototype came from a loose share key (the fingerprint equated bodies whose constants name different globals; `LLRM_CHECK_PVSHARED` fails on cached `Global(1)` against fresh `Global(0)`), which keep-analyses fixes in 2.1b.
1. Facts updated from the change log, taking analysis computations per pass run from 2.8 toward 0.5 for edit-rollback's family (step 3.1; edit-rollback with keep-analyses). The pointer families are small after the escape PRs; the family that carries the saving is edit-rollback's `Held`, `ThroughMemory` and decide facts (403 M own, 12.8%).
2. One pipeline pass with a combine worklist instead of rounds (step 3.2; pass-list-parity with hoist-bounded).
3. The allocator on the LIR core, 444 M against greedy's 39.5 M (distro-llc figure, not reproduced and being re-taken by regalloc-core-poc: do not quote; step 3.3; greedy-port with ssa-spill-m16).
4. Each pass held to a counted per-run budget (phase 4; each pass's owner).

## 2. Baseline snapshot

Main was `ef40f30a7` when this was read and is `c37be2a9c` as of this edit; PR states below were checked with `gh` on that day. `d_faces` flags are `-m32 -march=i486 -fno-inline-functions`, as in [synthesis]. Percentages are of `d_faces` -O2 instructions, measured on the PR's own parent unless stated.

### 2.0 Scoreboard re-taken (clean dist build)

**Phase table** (llvm-difftest; llrm dist at `cb2a78038`, `-m32 -mabi=sysv -march=i486 -O2 -fno-inline-functions`, `LLRM_DEBUG=counts,time` own work; clang is the pinned `clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)`, `-cc1` of the same flags under callgrind, inclusive M; input `d_faces.c` of the tree with hash `53880fe70961` (`d_faces.c` `1ab72e38cab4`); 11 functions each side, final machine instructions 1,816 (llrm) against 2,132 (clang): 1,195 against 195 instructions per final machine instruction). Whole: llrm 2,170.1 M, clang 415.5 M, 5.2x.

| phase | llrm M | clang M | ratio | clang counterpart | proposed owner |
|---|--:|--:|--:|---|---|
| front end (translate, hir to mir, data layout) | 49.4 | 44.8 | 1.1x | lex, parse, sema, IRGen | none (at parity) |
| optimizer: ten passes (gvn, fold, algebraic, sroa, promote, hoist, loopmotion, jumpthread, dse, indvars, unroll, rotate) | 498.7 | 116.6, with the analyses they use | | InstCombine, GVN, SROA, LICM, JumpThreading, DSE, IndVars, LoopUnroll, LoopRotate | the translation tracks of the per-pass table |
| optimizer: analyses | 535.5 | inside clang's pass rows | | nearest: through-memory ~ MemoryDependence 23.5 and MemorySSA ~6; pointer-values, points-to, globals-aa ~ BasicAA 19.5 and GlobalsAA 1.1; bounded, counted, demanded, float-facts ~ ValueTracking 12.5, LazyValueInfo 6.2, SCEV 5.6; call-effects ~ GlobalsModRef and FunctionAttrs (not separable); registers, pressure, exposed-frames, annotated, published, shape: none | memoryssa-port, basicaa-port, valuetracking-port; the "none" analyses below |
| optimizer: lsr | 89.5 | 10.0 (LSRInstance, inside the 177.5) | 8.9x | LSRInstance | loop-products-slope (the `LoopStrengthReduce.cpp` translation, see the decisions below) |
| optimizer: all other mir passes (interprocedural 62.7, decide 62.0, stamp 39.6, lcssa 21.1, fill 17.1) | 254.7 | 50.9 | 5.0x | SimplifyCFG 8.4, CVP 6.7, EarlyCSE 5.7, IPSCCP 4.1; stamp and fill: none | decide: simplifycfg-port; stamp: llvm-order-poc says what it is for first; fill: loopidiom-port; lcssa: llvm-order-poc |
| optimizer total | 1,378.4 | 177.5 | 7.8x | | |
| isel | 49.5 | 54.2 | 0.9x | SelectionDAGISel 51.3, CodeGenPrepare 2.9 | none (at parity) |
| LIR clean-up (peephole 82.2, jumps 56.6, coalesce 42.8, floatassign 38.8, floatalloc 20.5, shrink-wrap 11.2) | 315.5 | 49.6 | 6.4x | MachineCSE 2.7, Sink 3.8, CopyProp 3.2, PeepholeOpt 1.0, FPStackifier 1.4, BranchFolder 2.2, BlockPlacement 3.2, MachineLICM 8.6, DeadMI 1.5, MachineScheduler 9.6, TwoAddr 1.1, PHIElim 1.9, FixupBW 1.1, LateCleanup 1.1, LiveRangeShrink 1.0, PEI 1.1, ExecDomain 0.7, ReachingDef 3.1, MBFI 1.3 | lir-cleanup-poc (writes a translation map) |
| regalloc, split, spill, intervals, facts, classes, siblings | 354.3 | 58.9 | 6.0x | RAGreedy 39.5 (SplitKit inside; the distro-llc figure, being re-taken), RegisterCoalescer 6.5, LiveIntervals 6.6, LiveVariables 4.0, VirtRegRewriter 1.4, StackSlotColoring 0.9; facts, classes, siblings: none | regalloc-core-poc |
| emission (assemble, object write, omf) | 23.0 | 4.9 | 4.7x | AsmPrinter and MCObjectStreamer | unowned, ranked last |
| codegen pass manager and other, unattributed | 0.0 | 25.5 | | | |

Sum check: llrm 2,170.0 grouped plus 0.1 ungrouped is 2,170.1 whole; clang 44.8 + 177.5 + 54.2 + 49.6 + 58.9 + 4.9 + 25.5 is 415.4 (whole 415.5: front end 44.8, optimizer 177.5, codegen 193.2). Caveats: clang inclusive costs overlap (analyses sit inside the pass rows, while llrm's analyses are a separate row); the counterpart names for llrm's analyses are the nearest, not verified row by row; LSR is not in clang's pass table (a loop pass) and its 10.0 lies inside the 177.5; one run per column, with deterministic counts; the unit of the per-instruction figures is a final machine instruction. The LIR clean-up phases were re-taken on one instrument at main `703d070b7` ([#2134], lir-cleanup-poc: dist, `CARGO_INCREMENTAL=0`, callgrind inclusive of each phase's `transform_raising`, `d_faces` -O2, total 2,178.9 M): peephole 86.9, jumps 51.6, coalesce 54.5, floatassign 39.6 and floatalloc 21.0, phielim 9.9, parcopy 8.7, pressuresink 7.9, twoaddr 7.3, loopslots 6.1, `RegAlloc` 296.3 separate; shrink-wrap 11.2 was not re-taken. The two tables are at different commits (`cb2a78038` and `703d070b7`) and are not one table. Scripts and raw outputs: llvm-difftest's phase-table tools (no PR yet).

**Per-pass table** (the same run; llrm `tools/pass_budgets.py`, clang `tools/clang_pass_budgets.py` with the pinned `--clang`; cost per IR instruction per run, inclusive; the earlier table whose clang column was the distro clang 20.1.8 is void):

| llrm pass | clang pass | runs | IR instructions | llrm per unit | clang per unit | ratio | llrm M | clang M | proposed owner |
|---|---|---|---|--:|--:|--:|--:|--:|---|
| gvn | GVN | 35 / 11 | 7,979 / 1,250 | 23,863 | 25,680 | 0.93x | 190.4 | 32.1 | open (GVN.cpp's translation needs an owner other than fold-poc, whose queue is InstSimplify consumers, the worklist and builder folds; the gap is volume, 35 runs against 11, which llvm-order-poc's order reduces; GVN's natural dependency is the MemorySSA walker, on memoryssa-port's client turn-on list: LICM, DSE, GVN) |
| fold + algebraic | InstCombine | 92 / 88 | 18,212 / 9,719 | 13,612 | 4,064 | 3.35x | 247.9 | 39.5 | fold-poc |
| sroa + promote | SROA | 47 / 44 | 10,561 / 6,839 | 9,450 | 1,433 | 6.59x | 99.8 | 9.8 | llvm-order-poc |
| hoist + loopmotion | LICM | 81 / 70 | 18,145 / 9,472 | 8,129 | 771 | 10.55x | 147.5 | 7.3 | memoryssa-port (LICM translation plugs into it) |
| jumpthread | JumpThreading | 11 / 22 | 1,587 / 2,408 | 49,212 | 1,869 | 26.33x | 78.1 | 4.5 | indvars-decide-poc |
| dse | DSE | 34 / 11 | 7,120 / 1,181 | 7,008 | 1,948 | 3.60x | 49.9 | 2.3 | memoryssa-port |
| indvars | IndVarSimplify | 46 / 19 | 8,245 / 2,343 | 4,912 | 4,951 | 0.99x | 40.5 | 11.6 | indvars-decide-poc |
| unroll | LoopUnroll | 26 / 11 | 6,101 / 1,164 | 2,459 | 3,093 | 0.79x | 15.0 | 3.6 | indvars-decide-poc |
| rotate | LoopRotate | 22 / 35 | 3,050 / 4,774 | 3,705 | 1,236 | 3.00x | 11.3 | 5.9 | indvars-decide-poc |
| decide | SimplifyCFG | 35 / 88 | 8,333 / 11,706 | 9,180 | 718 | 12.79x | 76.5 | 8.4 | simplifycfg-port |

**Upper bound, timing env.** Every llrm per-pass figure in the phase table and in this table was taken with `LLRM_DEBUG=counts,time`, which adds about 6 to 7 K instructions per span (measured on `d_faces` -O2: the old regalloc has 12,075 spans = 81.3 M, greedy 5,471 spans = 31.3 M). They are upper bounds until llvm-difftest re-takes them with plain `perf stat` totals and a span-free per-pass instrument.

Ratios derived by dividing by the pinned 415.5 M (the 5.2x above, and any later quotient) are labelled derived, not measured, unless llrm's numerator was itself re-taken on the pinned build.

**The "none" analyses** (registers, pressure, exposed-frames, annotated, published, shape): under the mirror rule the default verdict is DELETE; keeping one is a DEVIATE with a number. Each goes to the owner of its largest consumer and is deleted in the PR that moves its last consumer. Own cost at `5e2e995ef` (dist, `LLRM_DEBUG=time`, `d_faces` -O2; the split by consumer is `unmeasured`, to come from the `LLRM_TRACE` recorder, [#2076]):

| analysis | runs | own M (total M) | consumers (files that ask) | owner |
|---|--:|--:|---|---|
| annotated | 116 | 38.6 (171.6) | `consts`, `loopidioms`, `floatfacts`, `memcpyopt`, `memoryssa`, `decide` | basicaa-port |
| registers | 202 | 25.4 (46.9) | `alias`, `loopidioms`, `induction`, `indvars`, `loopmotion`, `profit`, `ranges`, `spill`, `window`, `ports` | valuetracking-port |
| pressure | 17 | 22.2 | `hoist`, `loopmotion`, `lsr` (the spill forecast) | memoryssa-port and indvars-decide-poc (LICM and lsr owners) |
| exposed-frames | 282 | 20.0 | alias queries | basicaa-port |
| shape | 160 | 4.2 (19.7) | `alias`, `avail`, `consts`, `exitfold`, `floatfacts`, `floatloop`, `guards`, `induction`, `indvars`, `jumpthread` | by consumer (open) |
| published | 26 | 4.5 | alias | basicaa-port |

**Rows above min(gcc, clang)** (reference of record `rows/main24d`, llvm-order-poc: clang `24.0.0git` `d1106deb71cc` and gcc 13.4.0, `-fno-builtin-strlen` only, for both, other builtins on, the i386 builtins built from the pinned source; 71 programs, m32; llrm is main). Rows above min per level, clocks / executed instructions / memory operands / code bytes: -O1 30 / 48 / 24 / 24; -O2 32 / 48 / 38 / 29; -O3 28 / 46 / 35 / 39; -Os 39 / 29 / 21 / 20. Geomean of llrm over min, clocks .8979 / .9277 / .9088 / .9250 and code bytes .9522 / 1.0443 / 1.3036 / .9596 at -O1 / -O2 / -O3 / -Os. The same count with the distro clang 20.1.8 (void as a reference): -O1 27 / 45 / 24 / 17; -O2 31 / 48 / 37 / 23; -O3 27 / 47 / 35 / 36; -Os 36 / 27 / 19 / 19. A table with `-fno-builtin` on both references is dropped: it weakens gcc and clang on 12 programs and is never used for a "newly above" statement. Code bytes against min are not a separate track: the translations are expected to close them. Each translation PR reports how many rows move against min, per metric and level. Per-row table: `flips-main-vs-main24d.txt` (llvm-order-poc; not in the repo).


`d_faces.c` from the `qbq-llrm` source tree (sha256 over its `.c` and `.h` files `53880fe70961`, `d_faces.c` `1ab72e38cab4`; the same tree for every row), `-m32 -march=i486 -fno-inline-functions`, both compilers; llrm `dist` (fat LTO, one codegen unit) with `CARGO_INCREMENTAL=0`, a fresh target per SHA, one build at a time; `perf stat -e instructions:u,cycles:u`, min of 3, host load about 80 (cycles are soft, instructions are the claim); each run asserts exit 0, an object present and 1 to 3.5 G instructions, because a failed compile is about 8 M. Every row runs with the same environment, `LLRM_EC_COARSE=1` and only the switch under test added. Clang is the pinned build, `clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)` (not the distro clang 20.1.8): 348.0 / 413.4 / 407.0 M instructions (cycles 451.8 / 440.6 / 366.3 M) at -O1 / -O2 / -Os. Earlier figures from `tools/abmeasure.py` for `d_faces` are suspect: its row could compile nothing. Instructions in M, then x clang:

| row | -O1 | -O2 | -Os | against main at its own base (-O1 / -O2 / -Os) | cycles -O2 (soft, host load ~80) | object bytes |
|---|--:|--:|--:|---|--:|---|
| clang 24 | 348.0 | 413.4 | 407.0 | | 440.6 | 10,784 / 11,216 / 10,632 |
| main `cb2a78038` (identical to `9d50a350e`: no code landed between) | 1,675.8 | 2,104.1 | 2,191.2 |  | 1,448 | 7,859 / 8,068 / 7,685 |
| order branch `acd842c5c` (based on `98921a80e`), no switch: main at its own base | 1,789.3 | 2,254.3 | 2,322.8 |  | 1,415 | 7,859 / 8,068 / 7,685 |
| + A (`LLRM_EXP_LLVM=1`, early pass order) | 1,933.3 | 2,084.1 | 2,109.0 | +8.0% / -7.6% / -9.2% | 1,255 | 7,771 / 7,785 / 7,342 |
| + A + B (`LLRM_EXP_EARLYCSE=real`) | 1,900.6 | 2,043.9 | 2,067.2 | +6.2% / -9.3% / -11.0% | 1,206 | 7,771 / 7,785 / 7,342 |
| + A + B + C (`LLRM_EXP_DEAD_SLOTS=1`), the landing chain | 1,928.8 | 2,061.8 | 2,173.0 | +7.8% / -8.5% / -6.4% | 1,187 | 7,808 / 7,810 / 7,343 |
| + A + B + C + `LLRM_EXP_NOFORECAST=1` (experiment, will not land) | 1,747.3 | 1,875.3 | 1,882.6 | -2.3% / -16.8% / -19.0% | 1,395 | 7,783 / 7,785 / 7,326 |

Each chain is read against main at its own base (method M: base is the parent). The order branch is based on `98921a80e`; with no switch it is main at that base, and the switches change nothing in it (the object is byte for byte main's). Stacked on `cb2a78038`, the base of the PRs below, with the same switches A + B + C; the first two rows are read against main at `cb2a78038`, the PR rows against the control A + B + C:

| stack on the control | -O1 | -O2 | -Os | against the control (-O1 / -O2 / -Os) | cycles -O2 (soft) | objects |
|---|--:|--:|--:|---|--:|---|
| control: order merged with main `cb2a78038`, no switch | 1,676.4 | 2,104.9 | 2,192.2 | +0.0% / +0.0% / +0.0% | 977 | 7,859 / 8,068 / 7,685 |
| control, A + B + C: the landing chain on main's base | 1,809.8 | 1,915.1 | 2,058.1 | +8.0% / -9.0% / -6.1% | 844 | 7,808 / 7,810 / 7,343 |
| + [#2106] (`decide-no-alias`) | 1,802.4 | 1,907.1 | 2,050.2 | -0.4% / -0.4% / -0.4% | 1,750 | 7,808 / 7,810 / 7,343 |
| + [#2100] (`indvars-standing`): base `876fbeb54` is 9 commits older, not comparable | 1,871.8 | 2,004.7 | 2,114.6 | not comparable | 1,554 | 7,808 / 7,810 / 7,343 |
| + [#2110] (`coalesce-interval-join`, held) | 1,777.9 | 1,882.8 | 2,015.0 | -1.8% / -1.7% / -2.1% | 1,419 | 7,740 / 7,742 / 7,403 |
| + [#2122] (`instsimplify`, folding Builder) | 1,807.2 | 1,912.5 | 2,055.5 | -0.1% / -0.1% / -0.1% | 878 | 7,808 / 7,810 / 7,343 |

Reading. Main is 4.8x, 5.1x and 5.4x clang at -O1, -O2 and -Os. The landing chain on main's own base (the control, A + B + C) is 1,915.1 M at -O2, 4.6x clang, which is -9.0% against main at `cb2a78038`, -6.1% at -Os and +8.0% at -O1; on the order branch's older base the same switches give -8.5%, -6.4% and +7.8% against its own no-switch row, so the two bases agree. (An earlier version of this section set 2,061.8 M, the chain on the older base, beside main's 2,104.1 M: a comparison across bases, withdrawn.) Switch A alone is +8.0% at -O1, -7.6% at -O2 and -9.2% at -Os; C costs +5.1% at -Os against A + B. The NOFORECAST experiment is the largest drop, -9.0% at -O2 against the chain, and is not landing (hoist stays until the LICM translation). The PRs that change nothing in the object, [#2106] and [#2122], move -O2 by -0.4% and -0.1%, the second inside the noise floor. Cycles are soft: rows of the same instruction count differ by up to 2x in cycles at this load (the control with A + B + C 844 M, the same plus [#2106] 1,750 M), so no claim rests on them.

The `unmeasured` cells: m16, `-fexcess-precision=standard` (x87 excess precision stays fast by default, like gcc; a float row against clang is measured with llrm at `-fexcess-precision=standard`, and each row records the flag), the inline-storage stack and [#2119] (an API stub, no effect).

### 2.1 Landed

| PR | what | measured |
|---|---|---|
| [#1926], [#1934], [#1953], [#1927], [#1957], [#1922], [#1936], [#1968] | first batch: bit-set liveness, dense predecessors, Fx maps, gvn forecast skip, result survives invalidation, dead code after loop, don't ask for what the body cannot use | -10.0% at -O2 together ([compile-gap-series], `5344082af`) |
| [#1988], [#1992] | profile-guided release build | -14.1% to -17.9% instructions on every row, objects identical ([compile-gap-series]) |
| [#1956] | successors and predecessors inline | -0.54% |
| [#1931] | fixed point tells a state by a digest | -0.9% |
| [#1963] | front-end verifier under the check mode only; `-fverify-ir` is the user switch, `tests/timing.rs` asserts the three steps | HIR and MIR verifiers gone from release, -1.40% / -1.23% / -1.05% instructions at -O1/-O2/-Os ([synthesis] row 7) |
| [#1964] | `compile-gap` writes an object for each compiler | measures the object write (22 M) instead of `masm::text` (21 M) |
| [#1975] | LIR clean-up facts held and brought up to date from the blocks that changed | -1.71% / -1.50% / -1.29% at -O1/-O2/-Os, objects byte-identical (its own dirty-block mechanism supersedes [#1921]) |
| [#2007] | check switches read once | -0.40% |
| [#1959], [#1954], [#1961], [#1970], [#1975], [#1984], [#1998], [#1955] | dense sets in dead and spill placement, pointer values read by Annotated, walker chain once, LIR clean-up facts held, verifier names | in [rows-series] `9bce914f3` (all output identical) |
| [#2026] | dense value sets (liveness, hoist, constant cycles) | -0.42% (merged after this was read) |
| [#1912] | one allocation per function below -Omax | -1.95 G at -O1 on `c04886c2e` ([spec] 7) |
| [#2027] | facts rings let go at phase end | peak heap -13.9% (23.97 to 20.65 MB); instructions -0.02%; cycles unchanged |
| [#2017] | a load is offered the loads before it in its block | `d_faces` -0.14% |
| [#1915] | exact ThroughMemory for decisions | correctness fix for [#1909]; +1.0% at -O2 |
| [#1971] | block-address sets and maps are bit sets and id maps | -1.04% / -1.17% / -1.32% at -O1/-O2/-Os, B-tree frames 224.7 to 203.3 M, identity 1583/0 |
| [#2010] | mask test made at each branch | diamond 800: 1,995 to 1,075 M, 419 stack operands to 0; `d_faces` identical |
| [#1999] | `gvn`: equal loads look at each write once | `loop` N=1600 and exponent per the ladder; part of the 1.6 stack |
| [#2015] | `dse`: the cells of one address share their sets | `arith` N=1600 818 to 52 M; `d_faces` +0.01% to +0.03% |
| [#2031], [#2033] | docs: withdrawn `fold` fix; escape structure not kept | no instruction change |

### 2.2 In flight

| PR | owner | state | measured (on its parent) |
|---|---|---|---|
| [#2022] slab allocator | pass-list-parity | ready | -4.2% instructions at every level, -2.9% cycles (min of 11); RSS +7% (`d_faces`), +18% (`mdl_ai`) |
| [#2032] register and live sets dense | ssa-spill-m16 | ready | -0.38% |
| [#2035], [#2037] escape phase | keep-analyses | ready, draft (stacked) | -0.29%, then -0.64% |
| [#2021] LIR rename shares | ssa-spill-m16 | ready | -0.56%; peak heap -15% |
| [#2038] facts bundle edited in place | ssa-spill-m16 | ready | -0.52% (`Facts::of` 86.8 to 70.7 M) |
| [#2012], [#2011], [#2020], [#2034], [#2008] | greedy-port | ready, stacked | everything stacked with [#2010]: ladder `loop` N=1600 30,626 to 10,804 M (-65%), exponent 400 to 1600 = 1.1, `mem` 7,299 to 5,969 M ([#1999] with [#2008] alone: 31,766 to 19,654 M, -38%); diamond N=1600 -13% ([#2034]). `d_faces` per PR: [#2020] +0.07% to +0.11%, [#2034] +0.09%, [#1999], [#2012], [#2011] about -0.15% to +0.15%; together +0.1% to +0.2% (nothing measured is negative) |
| [#2013] dse | edit-rollback | ready | `dse` 3.5x to 4.0x per doubling down to 2.0x on the distinct-offset and `arith` shapes (2.17x then 2.33x on `mem`); `d_faces` +0.03% to +0.04% ([#2013] `e717bd522`, index only past 256 cells) and +0.01% to +0.03% ([#2015] `b6d39ca35`), objects identical. The two conflict in `avail.rs` and `dse_tests.rs` (two hunks; the owner resolves at landing) |
| [#1977], [#1978] bounded on deep nests | greedy-port | ready, stacked | queens -O3 1,703 to 1,306 M; `d_faces` -O2 3,248 to 3,193 M |
| [#1958] split placed from one table | greedy-port | open, ready (not a draft), measured on an old main | -3.4% (3,450 to 3,333 M); re-measure on main |
| [#1921] LIR clean-up on blocks with work | ssa-spill-m16 | closed ([#1975] did its dirty-block mechanism in general; its allocation-free rule windows, -4 M, live on branch `alim/perf/lir-cleanup-2` `6db843a29`, not a PR) | -1.77% on main `254855801`, now stale |
| [#1962] `initialized` events once | attrs-budgets | draft | -0.46%, under the 0.5% bar: bundle with 2.6, not a loose draft |
| [#1965] allocator rewrites from a floor | UNOWNED | draft | -0.3% |
| [#1972], [#1973] footprints by part | keep-analyses | draft | -0.02%, -0.07%; carries [parity] |
| [#1945], [#1943] checkpoint and rollback, `Touched` per slot | edit-rollback | draft | +0.09%; 144 function clones at -O2 left |
| [#1948] counted budgets as tests | attrs-budgets | draft | counter cost 0.03% (callgrind own-function sum); +0.13% by perf stat is noise; [#2076] recorder off: -0.003% / -0.010% / -0.011%, nothing measurable |
| [#2036] | ssa-spill-m16 | docs, open | not-kept entries for spec section 8 |
| [#2039] | keep-analyses | ready | `LLRM_CHECK_SAMEBYTES` read once through `env_flag!` (env fix) |

### 2.3 Measured and not kept

Do not measure these again on the same shape. Reviving one needs a new mechanism and a re-measure on current main ([spec 8]).

| idea | result | source |
|---|---|---|
| idle-step skip on the edit digest | runs -6%, compile +0.6% | [spec 8], [idle] |
| a work check before an idle run's analyses (row 5) | 87 M of 1,332 M skippable, unsound with one program left out | [spec 8] |
| footprint skip and the region form of a pass | 0.8% at -O2, 1.6% at -Os | [spec 8] |
| a pass that is its own fixed point | 0.4 M of 119 M | [spec 8] |
| early scoped CSE (EarlyCSE-shaped) | -3.1% only while stray exit phis hid loops; +1.6% to +1.9% with them folded; 4 vsgcc cells worse | [spec 8], [#1994] |
| fewer rounds with clang's placements | -5.1% at -O2; 35 bench cells worse | [#1933], [#1960], [spec 8] |
| -O2 at two rounds; `jumpthread` in the round | rows lost or -0.3% | [spec 8] |
| `gvn` ahead of `decide` | -4.4% at -O2, +5.4% at -O3, rows lost | [spec 8] |
| promote before gvn | -1.9% instructions, +2.7% object | [#2036] |
| `FloatFacts` filter before the float solve | 6 of 20 solves skipped, +0.04%; the 94 M in the float half is the lazy query of 3.1 | [spec 8] |
| keeping analyses across `fold`'s integer half | unsound in 5 of 11 changing runs; ceiling about 1% | [fold-hoist-decide] ([#2031]) |
| analysis rebuilds that return what was held (`Homes` reporting no change, empty-log `Outer`) | 0.00% and -0.04% | [spec 8] |
| a separate types bit in `Outer` | at most 0.4% | [#1994] |
| keyed-removal updaters for Annotated and Pointers | -0.6 M and -0.2 M | [#1973] |
| `Bounded` updated past a CFG change | +0.3% | [spec 8] |
| `decide`'s facts only where a branch needs them | +0.6% over 531 compiles | [spec 8] |
| hoist's per-block sweep memo; forecast memo | at most 0.8% at -Os, about 0 at -O2 | [spec 8], [decision-dependencies] |
| `gvn` numberings priced from the manager's `Pressure` | +3% | [spec 8] |
| `Pressure::updated` across `lsr`'s rotation | -Os 3,677 to 4,178 M | [#1965] |
| `ThroughMemory` updated exactly from `Touched` | 0.4% to 0.8% | [spec 8] |
| cache over the memory walker; per-block write summaries | ceiling 1.4%; -0.4% | [spec 8] |
| escape structure built on first ask, alone | 0.22% | [#2033] |
| predicate call effect and `captured_before` | dropped after the walk fusion left escapes at about 34 M | `unmeasured` in the repo (session decision) |
| `Function::instruction` made cheaper | 8 instructions a call already | [#2037] |
| skipping a body unchanged since its last stamp | 2 M of 108 M | [spec 8] |
| mimalloc's plain entry points | -2.3% instructions, 0% cycles | [spec 8] |
| scratch arena for short-lived allocations | ceiling about 3% instructions, likely 0 in cycles | [spec 8] |
| `IdMap` for small sparse maps (induction users) | +0.6% | [#2026] |
| holding block identity in the incremental tables | +0.5% | [#2027] |
| split carving constants | not built; the carving was spill pressure that [#2010] removes | [#2036] |
| peephole's shared lane and exit facts | at most 1.0%, dropped | [lir-cleanup] |
| SsaSpill route B as the single engine | rows worse (quicksort -Os +32.5%) | [spec] 4.2 |
| x87 live rows handed from the allocator | at most 0.9%; ranks last, dropped | [lir-cleanup] |
| interval recompute in `Facts::of` as its own step | avoidable bound about 11 M (0.4%), realistic 6 to 7 M (0.25%); kill rule met, 3 to 4 M buffer reuse folded into the next intervals PR | ssa-spill-m16 report |
| `masm::return_overhead_bytes` shared | 13.9 M, not shareable; the masm frame logic is ruled out | attrs-budgets report |
| `exposed-frames` as its own step | one pass per fact computation; belongs to 3.x | attrs-budgets report |
| position and row tables for the split path (B') | built: -0.03% (2,744.59 to 2,743.70 M); hash probes are not the cost, the 506 instructions per row visit are the rows' own work | greedy-port report, [#2036] |
| a one-worklist combine of fold, algebraic, phiopt, dead and decide's branch fold at today's positions and rounds | about 0 compile-time gain (the cost is analyses and idle reruns, not rewriting); branch `notkept/combine-worklist` `fba7c4051..fea9b27f0`, no spec 8 entry yet | pass-list-parity report |

### 2.4 Body size at each pass input, `d_faces` -O2 (llrm against LLVM 24)

From [#1994 part 1](https://github.com/ali-mosavian/llrm/issues/1994#issuecomment-6237686443) and [part 2](https://github.com/ali-mosavian/llrm/issues/1994#issuecomment-6237749851) (body-size-table; llrm on main `7ac662405`, LLVM `opt -print-after-all`; instructions per function at each pass input; cost from `LLRM_DEBUG=time` and callgrind inclusive of `<Pass>::run`; the cost-per-instruction column is an upper bound for the passes that ask analyses first):

| llrm pass | runs | LLVM | runs | llrm M (own / incl.) | LLVM M | runs x | body x | cost per instruction x |
|---|--:|---|--:|--:|--:|--:|--:|--:|
| fold | 35 | InstCombine | 73 | 31.0 / 220.2 | 53.2 | 0.5 | 1.48 | 5.82 |
| gvn | 34 | GVN | 11 | 172.9 / 206.0 | 49.4 | 3.1 | 1.47 | 0.98 |
| promote | 34 | SROA | 39 | 46.7 / 81.2 | 12.3 | 0.9 | 1.02 | 7.44 |
| dse | 33 | DSE and MemCpyOpt | 22 | 40.9 / 56.8 | 4.1 | 1.5 | 1.42 | 6.58 |
| lcssa | 34 | LCSSA | 16 | 23.2 / 23.2 | 0.9 | 2.1 | 0.52 | 22.85 |
| algebraic | 33 | Reassociate | 11 | 50.1 / 75.1 | 1.1 | 3.0 | | |
| jumpthread | 11 | JumpThreading and CVP | 22 and 22 | 48.7 / 101.6 | 17.0 | 0.25 | | |
| hoist | 34 | LICM | 124 loops | 93.2 / 141.9 | 14.0 | | | |
| indvars | 34 | IndVarSimplify | 41 loops | 57.0 / 64.4 | 23.6 | | | |

gvn: 26.3 K per instruction-run (llrm, 206 M over 7,839) against 28.6 K (LLVM, 49.4 M over 1,725): the same price, 4.5 times the volume. The big function reaches gvn with 2,844 instructions against LLVM's 1,385 (997 after its first SROA, EarlyCSE, InstCombine and SimplifyCFG round) because llrm cleans the memory form inside gvn and promote comes after it: 863 loads, 301 stores, 123 allocas and no phis, against 209, 90, 0 and 149. Memory form is about 840 of the 1,459 extra instructions, the sext, zext, trunc, mul and GEP representation about 714 (priced at 69.8 M, 2.7% of the compile, as a lower bound), and 245 values are dead.

Back end (same sources): regalloc 338.0 M over 2,250 LIR instructions = 150 K per instruction against LLVM's 40.0 M over 1,594 = 25 K: 6.0x = body 1.41 x cost 4.3. isel is at parity (53.6 M against 52.4). Peephole 90.8 M against 12.6 (43.2 K against 7.9 K per instruction), jumps 57.2 against 5.9 (25.7 against 3.7), coalesce 44.3 against 10.1 (24.5 against 6.3), floatassign plus floatalloc 64.5 against 1.6 (#2097). The row mapping is by purpose, not identical work.

### 2.5 Slot table: LLVM's pipeline at the pin against llrm

Source: `opt -passes='default<O1>'`/`default<O2>` `-print-pipeline-passes` and `llc -O2 -mtriple=i386 -mcpu=i486 -debug-pass=Structure` of the pinned `24.0.0git d1106deb71cc`; llrm status from llrm's `pipeline()` and the order branch's `llvm_order` (parity-pipeline's slot list). -Os is the -O2 pipeline on optsize IR (the pin's `opt` rejects -Os). -O3 adds `callsite-splitting`, `argpromotion`, `chr`, non-trivial `simple-loop-unswitch`, `loop-unroll<O3>`. -O1 omits the slots marked O2.

State: **translated** = the pass at the pin exists in llrm (a switch-gated or shadow translation counts, the default is noted); **old** = llrm's own pass or a stand-in does the job; **missing** = nothing runs; **n/a** = the target or the feature has no use. Gaps (no owner in the plan until oversight's assignment) are owned by kind: scalar cleanup by scalar-cleanup-port, loop passes by loop-passes-port, module and CGSCC by cgscc-poc, constraint-elimination by domtree-port, code generation by backend-slopes.

| # | LLVM slot | levels | llrm today | state | owner |
|--:|---|---|---|---|---|
| 1 | simplifycfg (early) | O1 O2 | none on main; `decide`, `phiopt` stand in | old | simplifycfg-port |
| 2 | sroa<modify-cfg> (early, and 3 more in the function pipeline) | all | `sroa` (order branch: gated by `LLRM_EXP_LLVM`, [#2164], [#2130]) | translated, off | llvm-order-poc |
| 3 | early-cse (twice, the second with MemorySSA) | all | `gvn::EarlyCse` shadow (54 of 58 identical to `opt`), `LLRM_EXP_EARLYCSE` | translated, off | llvm-order-poc |
| 4 | ipsccp, called-value-propagation | all | `ipacp`, `ipa_ranges` | old | cgscc-poc |
| 5 | globalopt (twice) | all | `GlobalOpt` | old | cgscc-poc |
| 6 | mem2reg | all | `promote` (write-through) | translated, off (order branch) | llvm-order-poc |
| 7 | instcombine (nine slots) | all | `fold`, `algebraic` | old | fold-poc |
| 8 | simplifycfg (eight slots) | all | `decide`, `phiopt` | old | simplifycfg-port |
| 9 | always-inline | all | the inline stage | old | cgscc-poc |
| 10 | globals-aa, profile-summary | all | `globalsaa` | old | basicaa-port |
| 11 | CGSCC: inline (bottom-up, devirt) | all | `interprocedural` stage D, callers first; `InlineCost` behind `LLRM_EXP_INLINEDECIDE` | old (cost translated, off) | cgscc-poc |
| 12 | function-attrs, rpo-function-attrs, recompute-globalsaa | all | `Summaries` | old | cgscc-poc |
| 13 | argpromotion | O3 | `argpromotion` | old | cgscc-poc |
| 14 | speculative-execution | O2 | none (x86 no-op) | n/a | |
| 15 | jump-threading (twice), dfa-jump-threading | O2 | `decide` (known conditions) | old; dfa missing | JumpThreading row (owner decided after LVI) |
| 16 | correlated-propagation (twice), lazy-value-info | O2 | `decide`, `ranger.rs` | old | vt-constantfolding (LVI+CVP row) |
| 17 | jump-table-to-switch | O2 | none | missing | scalar-cleanup-port |
| 18 | aggressive-instcombine | O2 | none | missing | scalar-cleanup-port |
| 19 | libcalls-shrinkwrap | all | none | n/a | |
| 20 | tailcallelim (twice) | O2 (once O1) | `tailrec` | translated | gvn-ci-2160-2172 |
| 21 | reassociate | all | `algebraic` | old | fold-poc |
| 22 | constraint-elimination | O2 | none | missing | domtree-port |
| 23 | loop-instsimplify, loop-simplifycfg | all | none | missing | loop-passes-port |
| 24 | licm (four slots) | all | `hoist`, `loopmotion` | old | memoryssa-port (LICM) |
| 25 | loop-rotate (in LPM1 and again after the CGSCC) | all | `rotate`, late only | old | loop-passes-port; order: llvm-order-poc ([#2194]) |
| 26 | simple-loop-unswitch (trivial; O3 non-trivial), extra-simple-loop-unswitch-passes | all; extra: O2 | `trivialunswitch`, `unswitch` (-O3) | old; extra missing | loop-passes-port |
| 27 | loop-simplify, lcssa (as needed by loop adaptors) | all | `loopsimplify`, `lcssa`, 9 runs | old | loop-passes-port; run count: llvm-order-poc |
| 28 | loop-idiom | all | `fill` | old | loopidiom-port |
| 29 | indvars | all | `indvars` | old | indvars-decide-poc |
| 30 | loop-deletion (twice) | all | `dead` | old | loop-passes-port |
| 31 | loop-unroll-full, loop-unroll<O1/O2/O3> (post pipeline) | all | `unroll`, `peel` | old | loop-passes-port; oracle `tools/unroll_oracle.py` |
| 32 | vector-combine, mldst-motion | O2 | `merge` stands in for mldst | old | scalar-cleanup-port |
| 33 | gvn | O2 | `gvn` (plan [#2155]) | old | gvn-port |
| 34 | sccp, bdce | all | none (`fold` reads `consts`) | missing | scalar-cleanup-port |
| 35 | adce | all | `dead` | old | scalar-cleanup-port |
| 36 | memcpyopt | all | `memcpyopt`, last | old | scalar-cleanup-port |
| 37 | dse | O2 | `dse` | old | memoryssa-port |
| 38 | instsimplify | all | `fold` (InstSimplify translation in progress) | old | fold-poc |
| 39 | deadargelim, globaldce, elim-avail-extern, constmerge | all | `deadargs`, `globaldce`, `availableexternally`; constmerge none | old; constmerge missing | cgscc-poc |
| 40 | float2int, lower-constant-intrinsics | all | `float2int` (lower-constant-intrinsics none) | float2int translated; the other missing | gvn-ci-2160-2172 |
| 41 | loop-interchange, loop-distribute | O2 | `loopinterchange`, `loopdistribute` at -O3 only | old, wrong level | loop-passes-port |
| 42 | infer-alignment, alignment-from-assumptions, loop-load-elim | all (loop-load-elim O2) | none | missing | scalar-cleanup-port |
| 43 | loop-sink, div-rem-pairs, mergeicmps, expand-memcmp | O2 | LIR `sink`, `addresssink`, `window` stand in | old | scalar-cleanup-port |
| 44 | loop-vectorize, slp-vectorizer, loop-distribute's vector side | O2 | none (i486 has no vectors) | n/a | |
| 45 | openmp-opt, coro-*, memprof, ee-instrument, alloc-token, annotation passes | all | none | n/a | |
| 46 | lower-expect, forceattrs, inferattrs | all | front end | old | scalar-cleanup-port |
| 47 | callsite-splitting, chr | O3 | none | missing | scalar-cleanup-port |

Code generation (`llc -O2`, i486; the same passes at -O1 less a few):

| # | LLVM slot | llrm today | state | owner |
|--:|---|---|---|---|
| 48 | loop-reduce (IR LSR), iv-users | `lsr` | old | loop-products-slope (LSR row) |
| 49 | scalar-evolution, loops, domtree, branch-prob, block-freq | `Loops` (LoopInfo translated, [#2195]), `branchprob`, `Frequencies` | LoopInfo translated; the rest old | scev-port, domtree-port, cgscc-poc (BPI/BFI row) |
| 50 | consthoist, partially-inline-libcalls, codegenprepare | isel preparation | old | backend-slopes |
| 51 | x86-isel (SelectionDAG), SwitchLoweringUtils | `isel`, `switchlower` | old (at parity in cost) | backend-slopes (SwitchLowering row) |
| 52 | early-tailduplication, opt-phis, stack-coloring, localstackalloc | none | missing | backend-slopes |
| 53 | early-ifcvt, machine-combiner, x86-cmov-conversion | none | missing | backend-slopes |
| 54 | early-machinelicm, machine-cse, machine-sink, peephole-opt, dead-mi, lrshrink | `peephole`, `pressuresink`, LIR clean-up | old | lir-cleanup-poc |
| 55 | phi-node-elimination, two-address | `phielim`, `twoaddr` | old | lir-cleanup-poc |
| 56 | livevars, liveintervals, register-coalescer | `intervals`, `coalesce` | old | regalloc-core-poc (LiveIntervals), lir-cleanup-poc (coalesce) |
| 57 | machine-scheduler | none | missing | lir-cleanup-poc (map) |
| 58 | greedy, spill-code-placement, virtregrewriter, stack-slot-coloring | `greedy` (parked OFF), old `regalloc` | translated, off | regalloc-core-poc |
| 59 | machine-cp ([`MachineCopyPropagation.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp): forward and backward copy propagation, redundant and dead copies, `CopyTracker` keyed by register unit, block-local), machinelicm, postra-machine-sink | (a) machine-cp: ad-hoc copy rules in `coalesce` and `peephole`, no single counterpart; 111 M vs clang 22 M (d_faces -O2 order tree, 5.0x); (b) machinelicm, postra-machine-sink: LIR clean-up | old | (a) backend-slopes: step 1 measure the 111 M by step and the copies left vs clang; step 2 translate, one pass per block, tracker reset at block ends (O(n)); step 3 join after `allocate`/`parcopy` and delete the ad-hoc copy rules it covers; target <= 1.5x clang, slope test, oracle `llc -O2` copy counts on the same LIR plus the bench gate; (b) lir-cleanup-poc |
| 60 | x86-fp-stackifier (`X86FloatingPoint.cpp`) and the RFP register classes | (a) `floatalloc` (27.0 M; clang 1.4 M, 19x); (b) `floatassign` (40.4 M) with `analysis float-facts` (35.7 M, 18 runs); (c) `floatfold` (d_faces -O2, m16, own, main `d1460fd39`, backend-slopes) | old | (a) translation of `X86FloatingPoint.cpp` (flat registers to stack form, fxch, pop), owner backend-slopes (coordinator, 2026-10-18; it coordinates register-class facts with regalloc-core-poc); DEVIATE to number: `_split_for_the_stack` critical-edge blocks against FPS's EdgeBundles. (b) no separate LLVM pass: the x87 registers are ordinary classes RFP32, RFP64 and RFP80 (FP0 to FP6; FP7 is reserved) allocated by RAGreedy, so `floatassign` and `float-facts` are replaced by the greedy allocation of the RFP class and deleted with it; owner regalloc-core-poc (confirmed; greedy allocates x87 values as RFP32, RFP64 and RFP80 virtual registers onto FP0 to FP6, as LLVM does before `X86FloatingPoint`); our `_crowded` depth check is the stack-depth part of FPS's `setupBlockStack`. (c) `X86InstrInfo::foldMemoryOperand`, lir-cleanup-poc's peephole row |
| 61 | shrink-wrap, prolog-epilog, machine-latecleanup | `shrink-wrap`, frame code | old | lir-cleanup-poc |
| 62 | branch-folder, tailduplication, block-placement, post-RA scheduler | `jumps` | old | lir-cleanup-poc (jumps-tailmerge) |
| 63 | execution-domain, break-false-deps, fixup-bw, fixup-leas | none | missing | lir-cleanup-poc (map) |
| 64 | asm printer, MC object streamer | `assemble`, object write, omf | old | backend-two (emission, rows E1 to E8 in [emission-survey.md](emission-survey.md)); E1 global alignment first |
| 65 | `X86FrameLowering::hasFP` (frame-pointer decision) | `without_frame_register`; interim [#2274] (framefree block lookup slope fix) | old | backend-slopes (the translation replaces `without_frame_register`) |

Count: of 65 slots, 4 are translated (2, 3, 6, 58), all off by default; 2 are partly translated (11: `InlineCost`; 49: LoopInfo); 4 are n/a; the other 55 are old or missing, every one with an owner (24 of them assigned by kind from the former gap list). The default pipeline runs none of the translated slots.

## 3. The plan

### 3.0 Summary

The exit method M and the bars it uses are defined once in section 4.

| step | intended effect (`d_faces` -O2 unless stated) | kill criterion | owner | status |
|---|---|---|---|---|
| S1 fact placement | the share of fact-enabled rewrites at few solver positions; computes per run above 1.0 or saving under 10% kills it | see Phase A | attrs-budgets, edit-rollback, keep-analyses | running |
| S6 edit core | edit primitives under 3% at N=1600 and no exponent kills building more than rollback | see Phase A | edit-rollback | running |
| 0.1 one baseline | one table on one SHA: phases, gap, rows, ladder | two instruments differ by more than 1% on one binary | llvm-compare | to do |
| 0.2 counters as tests | computations per run and spec 6 counters in CI | counter cost above 0.1% | attrs-budgets | draft [#1948] |
| 0.3 `ObjectRef` identity and its oracle (spike S0, answered) | the identity is not a cause; the PR is identity-excludes-facts plus the `LLRM_CHECK_INTERN` oracle (zero cost off) | n/a (answered) | objectref-identity | oracle PR |
| 1.0 inline storage across the hot `Vec` sites | allocations removed as far as the probe says; instructions and cycles on `d_faces`; peak RSS | a site whose inline fraction does not fit, or whose struct growth is not justified | pass-list-parity | to do ([#1956] first instance) |
| 1.1 slab allocator | -4.2% instructions, cycles negative | cycles not below base on a quiet host | pass-list-parity | merged [#2022] |
| 1.2 dense ids | -1.2% ([#1971] -1.17%, [#2032] -0.38%; the two overlap) | below 0.5% together | attrs-budgets, ssa-spill-m16 | [#1971], [#2032] merged |
| 1.3 escape phase | -0.9% | below 0.5% together | keep-analyses | ready |
| 1.4 backend copies | -1.1% instructions ([#2021] -0.56%, [#2038] -0.52%); peak heap -15.4% ([#2021]) and -13.9% ([#2027], merged), each against main alone, stacked peak unmeasured | below 0.5% and no heap change | ssa-spill-m16 | ready |
| 1.5 mask at each branch | diamond 1,951 to 1,075 M; `d_faces` 0 | any object differs | ssa-spill-m16 | merged [#2010] |
| 1.6 gvn, walker, lsr slopes | `loop` N=1600 -65% stacked (30,626 to 10,804 M), exponent 400 to 1600 = 1.1; `mem` 7,299 to 5,969 M | `d_faces` above +0.5% together | greedy-port | ready, stacked |
| 1.7 dse slope | `dse` 2.0x per doubling on the distinct-offset and `arith` shapes, 2.17x to 2.33x on `mem` (partial) | `d_faces` above +0.3% | edit-rollback | [#2015] merged, [#2013] ready |
| 1.8 bounded on deep nests | queens -O3 -23% | any object differs | greedy-port | ready, stacked |
| 1.9 exact ThroughMemory | fixes [#1909] at +1.0% | none (correctness) | keep-analyses | merged [#1915] |
| 1.10 held PRs on main | -3% if they reproduce; [#1921] to be closed, [#1962] bundled with 2.6 | each one below 0.5% on main | greedy-port, attrs-budgets | [#1958] ready, [#1965] draft |
| B1 re-baseline | the phase-1 row of the TL;DR measured | n/a | llvm-compare | after 1.x |
| 2.1 point_values per query | stop: meets its no-effect row (asked 87% / 60% / 63% of solved); 2.1b killed ([#2053]): the exact key passes `LLRM_CHECK_PVSHARED` on all 1,583 objects with 61 hits of 209 solves, net +1.8 / +6.9 / +4.8 M at -O1/-O2/-Os, under 0.5% | projected saving below 15 M, or the oracle differs | keep-analyses | step 0 done |
| 2.2 Annotated per access | up to -1.3% (the 57-of-116 held-result figure comes from [#1994] and is unverified; the pool is 42.4 M of edited rebuilds) | recoverable recomputes below 15 M | keep-analyses, edit-rollback | to do |
| 2.3 persistent MemorySSA | -1.3% to -3.9% | probe below 1% | hoist-bounded, keep-analyses | to do |
| 2.4 split path tables | B' built and killed (-0.03%); A' alone (`carved_moving` from allocator postings, 29 M, 1.0%, never built) | A' below 30 M | greedy-port | dies as written unless A' is kept alone |
| 2.5 interval recompute | closed: avoidable bound about 11 M (0.4%), realistic 6 to 7 M (0.25%) | kill rule met | ssa-spill-m16 | closed; buffer reuse folded into the next intervals PR |
| 2.6 `Frequencies` dense | `unmeasured`; the cost is `frequency.get(&cfg::id(block))` per instruction in `spill::TrafficBase`, not construction; bundles [#1962] | self below 15 M, or density below a quarter (density about 1) | attrs-budgets | to do, after pass-list-parity's hoist and spill pricing PRs |
| 2.7 rollback, not clones | function clones 144 to 0, at most +0.1%; an enabler with no instruction saving of its own (the clones cost about 0.1% of the compile) | above +0.3%, or `LLRM_CHECK_ROLLBACK` fails | edit-rollback | draft [#1945] |
| 2.8 scaling shapes | queens -O3, joins, unrolled no steeper than gcc's | its lead cost belongs to a step already listed | hoist-bounded | to do |
| B2 re-baseline | the phase-2 row measured | n/a | llvm-compare | after 2.x |
| 3.1 facts updated from the change log (re-pointed: edit-rollback's family `Held`, `ThroughMemory`, decide facts, 403 M own, 12.8%) | computations per run 2.8 toward 0.5 for that family only; the pointer families are small now | a prototype costs at least the whole build | edit-rollback, keep-analyses | design |
| 3.2 one pipeline pass | the combine alone about 0; the round drop is the effect and the risk (see 3.2) | a vsgcc row newly above min(gcc, clang), or an enabler the combine cannot absorb | pass-list-parity, hoist-bounded | design |
| 3.3 allocator on the LIR core | `regalloc` 415.5 M on main with [#2010], [#2021], [#2027], [#2032] (444 M is older) to 100 M or less | whole-body work per edit below 1% | greedy-port, ssa-spill-m16 | design |
| 3.4 LIR clean-up | about 290 M on the merged tree to about 250 M; skipping idle visits is worth 10.4 M (killed), block-local and dominator-scoped `copyprop` are killed | an idle skip worth below 15 M: killed; the next probe is the cost of the solves | ssa-spill-m16 | solve-cost probe next |
| 3.5 pricing: `hoist` and `lsr` forecast | the forecast is 2.6% at -O2 and 4.9% at -Os (`hoist` 1.4% / 3.7%, `lsr` 1.2%); `gvn` is 0.04%; the saving is `unmeasured` | forecast below 1% at both levels: not met | hoist-bounded, greedy-port | probe next |
| 3.6 code quality on pressure-heavy loops | stack operands within 1.5x of clang on the `live` shapes | rows worse | greedy-port | study first |
| 3.7 llrm-codegen's machine layer target-generic | no `llrm_x86::` in core; the machine passes run on arm64 selector output | the arm64 experiment shows a pass that needs a target literal the description cannot carry | backend-two | M0 10-20 to FINAL 11-13 |
| B3 re-baseline | the phase-3 row measured | n/a | llvm-compare | after 3.x |
| 4.1 per-pass budgets | each pass 300 instructions or fewer per IR instruction per run | a pass already within budget | each pass's owner | measure first |
| 4.2 budgets from clang | spec 1.3 re-derived per job from clang | n/a (measurement) | llvm-compare | to do |
| 4.3 front end | 81 M to 33 M (1.7%) | n/a | UNOWNED | parked |

### 3.1 Block format and the shared checklist

Every step has the same five parts:

- **(a) Effect:** the metric, the programs, the expected change with its bound, and the gap-table row it moves.
- **(b) Early:** the cheapest probe and its kill criterion, run before anything is built.
- **(c) Gate:** the design page and review, if the step needs one.
- **(d) Exit:** method M plus the step's own metrics, with pass and fail rules.
- **(e) Who:** owner, dependencies and files.

Each block then lists its checklist and a contingency table.

The checklist every step carries, in order. A step is done only when every box is checked, or excused with a stated reason.

- [ ] early probe run; kill criterion evaluated and recorded in the PR or on [#1994]
- [ ] design gate passed (page plus big-picture review), or "none"
- [ ] PR opened, its description headed by `TL;DR` with the measured headline
- [ ] fail-first test seen failing with the change undone
- [ ] each exit metric of the step, one box each
- [ ] intended effect reached (rule in (d))
- [ ] docs updated in the same PR: the [docs/comparison/llvm][readme] page of its row, [proposals], [synthesis]; [spec 8] when closing as not kept
- [ ] `LLVM structure` line in the design page or PR: LLVM's structure with a permalink, ours, `MIRROR` or `DEVIATE` with the reason and a number
- [ ] baseline table re-run at the phase's re-baseline step

## Phase A: architecture spikes

Eight spikes from the [architecture study](architecture-parity.md) run before the large builds, each on a throwaway branch with its kill criterion fixed beforehand; a result goes on [#1994] and, if killed, into [spec 8]. The decision review is at day ten after they start. Phase 3 steps wait only for the spike they name.

| spike | question | owner | kill criterion | state and result |
|---|---|---|---|---|
| S0 `ObjectRef` identity (0.3) | does interning order change output? | objectref-identity | any object differs under scrambled ids | answered (PR [#2046], open): 0 of 1,583 moved |
| S1 fact placement | do the tower's facts need to be fresh at every position of every round? | attrs-budgets (step 0), edit-rollback with keep-analyses (step 1) | step 0: under 90% of fact-enabled rewrites at three or fewer positions per function; step 1: under 10% of `d_faces` -O2, computes per run above 1.0, or a row crosses above min(gcc, clang) | step 0 measured ([#2054], draft; attrs-budgets, `d_faces` -O2, 10,580 edits, object 8,068 bytes, control identical): `ThroughMemory` enables 24.8% of `fold`'s edits (`matmul` -O3 up to 97.7%) and 0 of `decide`'s (a 33 M cost, landed consequence: `decide`'s `ThroughMemory` dropped, net -1.8%, keep-analyses' PR coming); `Annotated` enables 0 of `fold` (73 M, 29.6% of `fold`) and 0 of `decide` (9.9 M), but 67.5% of `hoist`'s, 52.3% of `gvn`'s and 75% of `loopmotion`'s edits need it from their first run; `bounded` enables 0 in `fold`, `decide`, `hoist`, `gvn`, `promote` and `algebraic` at `d_faces` -O2 (`matmul` -O2 `algebraic` 5.5%). So the tower is needed at some passes and not at others: the placement is per pass, not per position |
| S2 a fact-free run against InstCombine | once the tower is not asked, does a run cost what LLVM's costs? | pass-list-parity (llvm-compare supplies the LLVM column) | above 2x InstCombine per run at N=200, or exponent above 1.05 | measured: see below |
| S3 one round with cleanup positions | can a cheap combine replace rounds 2 to 4 without losing rows? | pass-list-parity, hoist-bounded | a row crosses above min(gcc, clang) whose enabler is a global solve, or under 8% at -O2 | after S1 step 1 and S2 |
| S4 allocator attribution, then in-place spill | is the 415.5 M the representation or the decisions? | greedy-port, ssa-spill-m16 | whole-body work per edit under 1% of the compile, or step 1 cuts the cost per spill by under 3x | step 0 measured: see below |
| S5 block-local post-RA dataflow | does LLVM's block-local copy propagation fit? | ssa-spill-m16 | under 15 M saved, or a row worse | KILLED ([#2043]): block-local saves 14.8 M but `d_faces` grows 8,068 to 8,097 bytes; a single-predecessor chain gives the same output (68% against an 80% bar) |
| S6 edit core on the ladders | are the edit primitives a scaling term? | edit-rollback | edit primitives under 3% at N=1600 on every shape and no exponent | measured ([#1994]): the kill is not met on `loop` only (primitives 4.32% at N=1600, `move_to` 925 M, 4.0x per doubling, 8,001 calls from `hoist::hoisted`); the other four shapes are under 1.2% with no exponent. A6 is built beyond rollback only as far as `loop` goes: `Function::move_all(&[InstId], Position)` mirrors LLVM's range splice at the API; the intrusive order only if a slope remains |
| S7 caps and pricing | are pricing and uncapped walks the cost? | hoist-bounded, greedy-port | pricing under 1% at both levels and no walk past 100 steps | not met: forecast 2.6% (-O2) and 4.9% (-Os); massif at `mem` N=1600 is 251 MB, 57% walker jump tables (fix coming); walks past 100 steps over the corpus at -O2 (1,175 files): about 42,000 per compile of the corpus, median 142 steps, p99 483, max 584, 0.6% above 500, asked by `ThroughMemory` `provider_of` 77%, `gvn` `forwardable_walk` 15% and `floatfacts` `solved_over` 6%: nothing runs away, a steps cap would be a safety bound and not a time lever. A cap per consumer (`fold` 6, the others 100, a safety bound of 1,000) is prototyped by edit-contract-replay, re-taken non-incremental over the whole process in instructions:u: `d_faces` -O2 none 2,574.4 M, cap 6 -2.3%, cap 20 -1.4%, cap 100 -0.3%, objects identical in all four; `mem` N=1600 cap 6 -14.3% (an earlier -20.7% was the runs table's with-analyses metric, not the whole process), cap 100 +0.1%; rows for caps 6 and 20 are queued. In CI mode, walker-caps' [#2068] is +0.05% on `d_faces` (a safety bound) and [#2083] -11.2% on `mem` N=1600; facts-per-query's [#2073] takes unrolled-128 -6.6%. Indvars registers brought up to date by the manager ([#2067]): -0.858% / -0.680% / -0.658%. Inline storage ([#2077], CI mode): instructions -0.47% / -0.58% / -0.69% at -O1/-O2/-Os, D1 misses -2.5% to -3.3%, no RSS or cycles claim |

**S2 result** (pass-list-parity; llrm main `4f12e5818`, round 1 of `fold`, `algebraic`, `phiopt`, `dead` own work against `opt -passes='sroa,early-cse,instcombine'` minus `sroa,early-cse`; M instructions at N=200, then 1,600): `arith` 6.01 against 20.10 (0.30x) and 62.5 against 159.0 (0.39x); `mem` 6.14 against 8.08 (0.76x) and 80.6 against 65.5 (1.23x); `diamond` 5.06 against 6.35 (0.80x) and 35.6 against 41.7 (0.85x); `mixed` 8.59 against 8.35 (1.03x) and 67.0 against 61.3 (1.09x). The 2x line passes on every shape (worst 1.03x). The exponent from 200 to 1,600, llrm and LLVM: `arith` 1.14 and 1.01, `mem` 1.24 and 1.01, `diamond` 0.94 and 0.91, `mixed` 0.99 and 0.96; it trips on `arith` (`dead` 1.43, `algebraic` 1.12, being re-measured at N=400 and 800) and on `mem` (`fold` 1.41, whose cost is `Analyses::get::<ThroughMemory>`). Verdict: the rewriting is not the gap, the facts are; the superlinear passes are `gvn` (1.32 to 1.56) and `dse` (1.72 to 1.97), not the combine. The exponents come from two points, with analyses excluded from "own".

**S4 step 0 result** (ssa-spill-m16, [allocator on the LIR core](allocator-lir-core.md), merged as [#2042] and [#2049]): one function of K sequential copies of a 24-statement loop, at about fixed decisions per copy: cost per allocator round 490, 488, 598, 805, 1,181 and 1,871 K at 1x to 32x (bodies 191 to 4,664 instructions); a round is about 431 K fixed plus 309 instructions per body instruction (R^2 above 0.99). The kill (cost per round does not scale with body size) is not met; at 191 instructions the fixed part is 88% of a round; at `d_faces`' large function (1,819 instructions) the fit gives 993 K per round, 57% size-proportional, while the real function costs about 2.0 M per round: the split path is the extra. Decisions on `d_faces`: 837 assignments (LLVM 815), 67 evictions (78); knock-on unassignment 104 of 7,694 corpus rounds (1.4%), 0 on `d_faces`. The fixed part at 1x (37.22 M, 490 K per round): `Facts::of` 9.04 M, `spilled_apart` 5.15, the evict search 4.41 (`movable` 2.51, `LiveRanges::weight` 1.35: LLVM stores the weight in the interval), `recolor` 3.32 (a stage greedy does not have), a `BTreeSet<RegId>` rebuilt per pop 1.94, split 2.56 and carve 1.68, the rest 8.3. Work on a fact that did not change is about 12.3 M (33%, prove-once candidates); decision work about 10.2 M (27%). The fixed 431 K per round is about 20 pieces of 10 to 100 K. greedy-port's measurement of LLVM greedy (on 20.1.8; a re-measure on the reference build is pending): per decision 13.3, 22.3 and 32.5 K at 191, 1,305 and 4,664 instructions and 29.9 K on `d_faces`; llrm per round 490, 805 and 1,871 K, 36 to 58x; LLVM is not flat either (x2.4 for x24, from interference queries).

**S4 phase 1 result** (ssa-spill-m16; branch `alim/exp/in-place-spill` kept as the record): killed. "Created values from the edit record" saves about 0: per update 115.0 and 48.0 K with the new path against 113.7 and 46.7 K with the old at `chain-1`, 232.7 against 231.3 K at `chain-32`, `d_faces`' large function 344.3 against 342.6 K (+1%); 4.1 values are touched per update and 0.8 created, and LLVM's `LiveRangeEdit` also recomputes old ranges. The fixed 431 K per round is about 20 pieces of 10 to 100 K in three lanes: A (ssa-spill-m16) about 55 M, B the split path (greedy-port) about 110 M, C the decisions (the `movable` search, 218 M on `chain-32`) not yet sized. Prove-once is about 12% of `regalloc`. LLVM greedy per decision on the reference build (24.0.0git) agrees with the 20.1.8 figures within 5% (21.4 K on `chain-8`) and is not flat (x2.4).

**What the study changes in this plan.**

- Hold 2.2 and 2.3 until S1 reports (a passing S1 makes 2.2's updater and 3.1's tower updaters unnecessary).
- Stop 2.4 (A'): fold it into S4 step 1; the carve copy goes with the LIR arena.
- Close [#1962] and [#1965] unless each clears 0.5% on main.
- Reshape 3.1 behind S1: the design page names solver positions and what stays per query.
- Do 4.2 (budgets from clang) now: attrs-budgets with llvm-compare.
- S1 and S6 are steps of the plan, with the owners above.

## Phase 0: instruments

### 0.1 One baseline on main

- **(a) Effect:** none on the compiler. It produces one table, at one SHA and one build profile, that every later delta reads against. The rows are:
  - `d_faces` -O1/-O2/-O3/-Os by phase (front end, `mir`, `analysis`, `regalloc`, `lir`, `isel`);
  - LLVM's 415 M ([readme]) and static clang 20.1.8's 397/462/464/459 M ([compile-gap-series]);
  - the ladder exponents;
  - the rows ([rows-series]).

  Today the repo holds -O2 figures of 2,903 M ([synthesis], callgrind), 2,830 M ([compile-gap-series], perf, dist) and 2,908 M ([#2022] parent, perf, release). The callgrind -O2 figure of 2,903 M is the anchor; the other two come from other instruments and profiles.
- **(b) Early:** run callgrind and `perf stat -e instructions:u` on the same release binary of main, `d_faces` -O2. Kill (fix the instrument first) if they differ by more than 1%. Check also that the binary's mtime is after its commit; `cargo build -p llrm-qb` leaves `target/release` stale, so build with `--bins`.
- **(c) Gate:** none.
- **(d) Exit:**
  - `tools/compile-gap.py --clang <static clang 20.1.8>` (dist and release);
  - `LLRM_DEBUG=time,runs` own work by phase;
  - `tools/ladder/ladder.py` and `fit.py --upto 200`, plus the exponent from N=400 to 1600 for every shape;
  - `tools/rows/run.sh`;
  - computations per pass run from `LLRM_WHY`.

  Pass: the table is committed as a new dated-by-SHA section of [compile-gap-series] and a `ladder-<sha>.json`.
- **(e) Who:** llvm-compare. Depends on nothing. Touches docs and `tools/ladder/` only.

Checklist: the shared list. Its own metric boxes:

- [x] phase table ([compile-gap-series], `5e2e995ef`)
- [x] gap table (clang 24 and 20.1.8, four levels)
- [x] ladder json (`tools/ladder/ladder-5e2e995ef.json`)
- [x] rows (71 programs, m32, at `5e2e995ef`, distro clang 20.1.8 as the clang reference, void: superseded by section 2.0's reference of record: clocks geomean 0.8875 / 0.9044 / 0.9021 / 0.9169 at -O1/-O2/-O3/-Os, worst 2.01 to 2.19; [compile-gap-series])
- [x] computations per run (3.4 at -O2 by the `LLRM_WHY` count)
- [x] allocations by DHAT and RSS

| outcome | decision | next |
|---|---|---|
| partial (some rows missing) | publish what agrees; the missing row's tool gets a fail-first test (third rule) | 0.2 |
| instruments disagree by more than 1% | stop; diff callgrind and perf on one function; record the cause on [#1994] | 0.1 again |
| n/a | | |

### 0.2 Counters and budgets as tests

- **(a) Effect:** the spec section 6 counters are checked in CI as ratios at 2N/N ([#1948]):
  - `analysis_computations / pass_runs`;
  - `function_clones`;
  - `full_interval_builds`;
  - `whole_walks`.

  Plus a per-analysis, per-asking-pass count. This gives phase 3 the numbers it is judged by: today 2,742 function-level computes, 631 M own, 545 of them on a body version already computed (41.3 M, 1.4%) ([#1994]). The pass-run count behind 2.8 is from -O1 ([spec] 1.2) and is `unmeasured` at -O2.
- **(b) Early:** counter cost on `d_faces` -O2. Kill if it is above 0.1%. [#1948]'s 0.03% is the callgrind own-function sum on `d_faces` -O2 (the perf-stat total is within the noise floor).
- **(c) Gate:** none.
- **(d) Exit:** M (identical output); `tools/test_budgets.py` green on main; computations per run printed by `LLRM_DEBUG=counts` at -O1/-O2/-Os.
- **(e) Who:** attrs-budgets. Depends on 0.1. Touches `llrm_mir::counts`, `tools/budgets.toml`, `tools/test_budgets.py`.

Checklist: the shared list. Its own metric boxes:

- [ ] counter cost re-measured on current main
- [ ] computations per run at three levels (needs `LLRM_DEBUG=counts` at -O1 and -Os)
- [ ] budget rows re-measured on main

| outcome | decision | next |
|---|---|---|
| partial (some counters missing) | land what exists; a missing counter is a box on its consuming step | 1.1 |
| no effect (counter cost above 0.1%) | compile the counters out of release; read them in a `counts` build only | 1.1 |
| worse elsewhere (any object differs) | revert; the coordinator decides | 1.1 |

### 0.3 `ObjectRef` identity (spike S0, answered)

- **(a) Effect:** none on instructions. Spike S0 asked whether `ObjectRef` ordering by interning id makes skipped or shared solves change output. It does not: `LLRM_CHECK_INTERN` scrambled ids give 0 of 1,583 objects different on main, so invariant A2 holds today. The 131 differing objects of keep-analyses' `point_values` share prototype came from a loose share key: the fingerprint equated bodies whose constants name different globals (`LLRM_CHECK_PVSHARED` fails on cached `Global(1)` against fresh `Global(0)`). A context-free ordering was built and cost +0.7% instructions and removed nothing: reverted.
- **(b) Early:** done (above).
- **(c) Gate:** none.
- **(d) Exit:** objectref-identity's PR is identity-excludes-facts plus the `LLRM_CHECK_INTERN` oracle, at zero cost with the oracle off. Pass: identical output and no instruction change.
- **(e) Who:** objectref-identity. 2.1b is not blocked on it.

Checklist: the shared list. Its own metric boxes:

- [ ] oracle at zero cost with it off
- [ ] identical output

| outcome | decision | next |
|---|---|---|
| partial / no effect / worse | n/a (answered; the PR is an oracle) | 1.0 |

## Phase 1: land what is measured (output identical)

All phase-1 steps run in parallel. Each is re-measured on main by M before merge, because each number in 2.2 was taken on its own parent.

### Allocation: the path to LLVM's count

**Where we stand.** LLVM makes about 63 K allocations (38 MB) on `d_faces` -O2, about 19 per IR instruction ([allocation]). llrm makes about 3.1 M (600 MB; [#2022]'s DHAT counts 3.16 M blocks), about 950 per IR instruction, about 50x. The profile (pass-list-parity, DHAT with system malloc; the site figures are a session report): the biggest site is 3.3% of the blocks ([#2022]) and the top 60 sites 37%; `Vec` `with_capacity` and `collect` 42% (1.34 M), growth past capacity 25% (779 K), `Rc` and `Arc` slices 6.7%, B-trees 16%; 61% of the blocks are 64 bytes or less and 26% 16 bytes or less; 54% die within 100 K instructions.

**What does not reduce the count.** The slab (1.1) cuts the cost of an allocation, about 71 instructions to about 15 to 18 ([#2022]: "about four times cheaper"), not the number of them. Count reducers so far: [#1956] successors inline (259 K allocations), dense sets ([#1971], [#2026], [#2032]), [#2021] (3.163 M to 3.112 M, -1.6%; peak heap -15%). Operands inline in `Instruction` is measured at no more than 2.7% of the blocks ([#2022]) and 0.2% of instructions (pass-list-parity's report): not the lever.

**What gets to LLVM's level, in order.**

1. Inline storage (SmallVec-style) across the hot `Vec` sites: step 1.0, early by the user's order.
2. Build less data: per-query facts and no rebuild (steps 2.x and 3.1; the memory-fact tables are about 19% of the allocations).
3. Edit in place instead of copying: the allocator on the LIR core (3.3), rollback instead of 144 clones (2.7).
4. Dense flat tables (1.2 and 2.6).
5. Reuse scratch buffers across calls through the analysis manager.
6. A per-pass-run arena: measured, not built ([spec 8]): 30.7% of allocations are born and die in one pass step, 65.5% are born outside any step (inline storage's share), the bound is about 1.0% of instructions (about 0.7% with the slab).

**What it buys in instructions.** After the slab, allocator work is about 15 instructions times 3.1 M, about 47 M (1.6%). Matching LLVM's count directly saves at most 2% to 3%; the rest of the 518 M bucket is filling and copying data, which (2) to (4) remove. The cache effect may be larger: allocation is 24% of L1d misses and the working set is 25x to 50x LLVM's ([allocation]).

**Exit measurement for the whole track.** Allocation count and bytes by DHAT per phase, re-baselined at B1, B2 and B3, with the target stated per phase at B1 (`unmeasured` until then).

### 1.0 Inline storage ([#1956] is the first instance)

- **(a) Effect:** allocations removed, as far as the probe says (LLVM about 19 per IR instruction, llrm about 950), with the instructions and cycles on `d_faces`, and peak RSS. Already measured: [#1956] (successors, 4 slots) took 259 K allocations; operands in `Instruction` at 2 to 4 slots is at most 2.7% of the blocks ([#2022]) and 0.2% of instructions (pass-list-parity's report), so it is not a first site. See the [allocation path](#allocation-the-path-to-llvms-count). Moves [synthesis] row 2.
- **(b) Early:** a length-at-drop histogram per `Vec` site over the top 40 sites by blocks (DHAT, `d_faces` -O2 and one large QCport module): the fraction of each site that stays inline at 2, 4 and 8 slots. Stop rule per site: it converts only if that fraction fits and the struct growth is justified (a site whose owner struct grows past a cache line, or whose elements are large, stays a `Vec`).
- **(c) Gate:** none for a site; the choice of inline type (a local `Inline<T, N>` or the `smallvec` crate) needs the owner's decision first, since `llrm-mir` depends on no crate.
- **(d) Exit:** M plus DHAT allocation count; instructions and cycles on the PR's parent; RSS within the limits (`d_faces` +10%, `mdl_ai` +20%); identical.py 0 differ. Pass: the count falls by the histogram's prediction within 20%, and instructions do not rise.
- **(e) Who:** pass-list-parity. Depends on 0.1. Touches the sites the histogram names, and `llrm_mir` for a shared inline type.

Checklist: the shared list. Its own metric boxes:

- [ ] histogram of the top 40 sites at 2, 4 and 8 slots
- [ ] allocation count by DHAT, before and after
- [ ] instructions and cycles on the parent
- [ ] RSS on two programs

| outcome | decision | next |
|---|---|---|
| partial (some sites fit) | convert the sites that fit; record the rest with their histograms | 1.1 |
| no effect (few sites fit) | record in [spec 8] with the histogram; the count falls only by (2) to (4) of the [allocation path](#allocation-the-path-to-llvms-count) | 1.1 |
| worse elsewhere (struct growth costs more than the allocations saved) | revert that site | 1.1 |

### 1.1 Slab allocator ([#2022])

- **(a) Effect:** (this cuts the cost of an allocation, not their count: see the [allocation path](#allocation-the-path-to-llvms-count)) -4.2% instructions at every level (2,908 to 2,787 M at -O2) and cycles -2.9% (min of 11). Moves [synthesis] row 2 (allocation, 518 M).
- **(b) Early:** user-space instructions are the kill criterion (the host's load has been 30 to 38 all week, so a quiet cycle figure cannot be had); cycles are a sign check, `perf stat -e cycles:u` min of 11, alternating base and head, with single cells swinging by 5%, or a named quiet host. Kill if instructions are not below base. A change that moves instructions and not cycles is the mimalloc lesson ([spec 8]).
- **(c) Gate:** none (unsafe invariants stated in the crate docs).
- **(d) Exit:** M. Also max RSS: `d_faces` -O2 within +10% (measured +6.9%), `mdl_ai` -O2 within +20% (measured +17.7%); little room is left, so re-measure RSS at every B-step (2.7's rollback clones could push `mdl_ai` over); 2N/N exponents within ±0.012 of base. Pass: instructions -3.4% or better and cycles not above base.
- **(e) Who:** pass-list-parity. Depends on 0.1. Touches crate `llrm-alloc`, the four binaries' entry points, `tests/bins_use_the_slab.rs` (replaces `bins_use_mimalloc.rs`), the docs ([allocation], `crates.md`, `source-layout.md`, [spec 9]) and `.github/workflows/nib-lsp-release.yml` (the generated-paths test, which is what failed [#2022]).

Checklist: the shared list. Its own metric boxes:

- [ ] instructions
- [ ] cycles sign check (min of 11, alternating), or a named quiet host
- [ ] RSS on two programs
- [ ] scaling exponents

| outcome | decision | next |
|---|---|---|
| partial (instructions between -1% and -3.4%) | callgrind self cost of the allocator entry points; check the global allocator's methods are inlined | 1.2 |
| no effect (cycles not negative) | close; record in [spec 8] next to the mimalloc entry; allocation stays a count problem (B1 re-ranks [synthesis] row 2) | 1.4 |
| worse elsewhere (RSS above the limits) | fix forward: return empty chunks per phase; otherwise revert; the coordinator decides | 1.4 |

### 1.2 Dense ids ([#1971], [#2032])

- **(a) Effect:** [#1971] -1.04% / -1.17% / -1.32% at -O1/-O2/-Os against its parent (main `39501a050`), B-tree frames 224.7 to 203.3 M (-21.5 M at -O2), identity 1583/0; [#2032] -0.38% (with [#2026] landed). Moves [spec 9] container rows (B-tree 10.5%). The early-kill threshold (15 M) is met.
- **(b) Early:** on main, callgrind self cost of B-tree frames keyed by block address and `RegId`. Kill if their sum is below 15 M (0.5%).
- **(c) Gate:** none.
- **(d) Exit:** M. Pass: -1.0% or better together.
- **(e) Who:** attrs-budgets ([#1971]), ssa-spill-m16 ([#2032]). Depends on 0.1. Touches `llrm_mir::dense` (`Dense` for `i64`; `IdSet` `is_subset`, `is_disjoint`, `union_with`, `difference`, `pop_first`; `IdMap::retain`), `ranges` (`LoopContext.positions`), `slotstores`, `spill.rs`, `noreturn`, `induction`, the manager (`dominated_by`) ([#1971]); `llrm-lir` `RegSet`, `allocate::Live` ([#2032]).

Checklist: the shared list. Its own metric box:

- [ ] instructions at -O1/-O2/-Os

| outcome | decision | next |
|---|---|---|
| partial | convert only where the set holds a quarter of its id space or more (the [#2026] `IdMap` loss) | 2.6 |
| no effect | close the one below 0.5% or bundle it with 2.6 | 2.6 |
| worse elsewhere | revert that site | 2.6 |

### 1.3 Escape phase ([#2035], [#2037])

- **(a) Effect:** -0.9% together (8.7 M plus 19.1 M at -O2). Moves [synthesis] row 1 (alias family).
- **(b) Early:** none; this is measured. Re-measure on main with M.
- **(c) Gate:** none.
- **(d) Exit:** M. Pass: -0.6% or better together.
- **(e) Who:** keep-analyses. Depends on 0.1. Touches `llrm-analysis` `alias.rs`.

Checklist: the shared list. Its own metric box:

- [ ] instructions at -O1/-O2/-Os

| outcome | decision | next |
|---|---|---|
| partial | the rest of `alias::escapes` is the escaped set itself; that belongs to 2.1 | 2.1 |
| no effect | close [#2037]; keep [#2035] only if it is 0.3% or better, as a bundle | 2.1 |
| worse elsewhere | revert; coordinator | 2.1 |

### 1.4 Backend copies ([#2021], [#2027], [#2038])

- **(a) Effect:** (the [allocation path](#allocation-the-path-to-llvms-count), item 3)
  - instructions -0.56% ([#2021]) and -0.52% ([#2038]);
  - DHAT peak heap 24.0 to 20.3 MB (-15.4%, [#2021]) and 23.97 to 20.65 MB (-13.9%, [#2027], merged), each against main alone; stacked, the peak is `unmeasured`. [#2038] is an instruction item (-0.52% to -0.64%; callgrind -0.68%, `Facts::of` 86.8 to 70.7 M), not a heap item.

  [#2021], [#2027] and [#2038] moved no cycles beyond noise, so 1.1's cycle sign check does not apply here. Moves [synthesis] row 3 and the [allocation] memory row.
- **(b) Early:** DHAT peak of the three stacked on main. Kill [#2027] if it adds nothing on top of [#2021].
- **(c) Gate:** none.
- **(d) Exit:** M plus DHAT peak. Pass: instructions -0.8% or better together (met by [#2021] and [#2038]: -1.08%), and peak -15% or better.
- **(e) Who:** ssa-spill-m16. Depends on 0.1. Touches `model/lir.rs` (`Insn::renamed`), `phielim`, `coalesce`, `spiller`, `Facts`, `regclass`, `postings`.

Checklist: the shared list. Its own metric boxes:

- [ ] instructions
- [ ] DHAT peak
- [ ] max RSS

| outcome | decision | next |
|---|---|---|
| partial | the remainder is `Indexes::patch` (4.6 M) and the postings following (14 M), both 3.3 | 3.3 |
| no effect | close the PR that adds nothing on top of the others | 3.3 |
| worse elsewhere | revert that PR | 3.3 |

### 1.5 Mask test at each branch ([#2010])

- **(a) Effect:**
  - diamond N=800: 1,951 to 1,075 M;
  - diamond N=1600: 6,268 to 2,474 M (clang 926 M), stack operands 825 to 3 ([#2036]);
  - `d_faces` unchanged and byte-identical.

  This is CodeGenPrepare's `sinkAndCmp0Expression`. Moves [synthesis] row 3 (79 splits).
- **(b) Early:** none; this is measured.
- **(c) Gate:** none.
- **(d) Exit:** M plus `tools/ladder` diamond at N=400/800/1600 and its backend exponent (1.44 before, [readme]). Pass: diamond N=1600 3,000 M or less and identical.py 0 differ.
- **(e) Who:** ssa-spill-m16. Depends on 0.1. Touches `addresssink::masks_sunk`, the target fact `mask_test` in `opcosts.txt` (x86 m16 and m32), and the guard that skips a mask whose operands are values read nowhere else (the CodeGenPrepare `:2224` guard).

Checklist: the shared list. Its own metric boxes:

- [ ] diamond ladder
- [ ] stack operands
- [ ] `d_faces` identical

| outcome | decision | next |
|---|---|---|
| partial | re-measure [regalloc]'s split table on diamond; what is still split goes to 2.4 | 2.4 |
| no effect | instrument suspect (the PR measured it); diff the diamond asm base against head | 1.5 again |
| worse elsewhere | check `waivers/mask-test-runtime.toml` first: the scroll bq32 +1 instruction is a DOSBox REP-slicing artifact, not a regression; only then revert; coordinator | 2.4 |

### 1.6 gvn, walker and lsr slopes ([#1999], [#2012], [#2017], [#2011], [#2020], [#2034], [#2008])

- **(a) Effect:**
  - ladder `loop` N=1600, everything stacked with [#2010] and [#2012]: 30,626 to 10,804 M (-65%); [#1999] with [#2008] alone: 31,766 to 19,654 M (-38%);
  - `loop` exponent from N=400 to 1600: 1.1 stacked (1.41 to 1.34 with [#1999] alone);
  - `mem` N=1600: 7,299 to 5,969 M; peak RSS 331 to 181 MB ([#2011]);
  - diamond -13% ([#2034]).

  `d_faces` per PR: [#2020] +0.07% to +0.11%, [#2034] +0.09%, [#1999], [#2012], [#2011] about -0.15% to +0.15%; together +0.1% to +0.2%, nothing measured is negative. [#2012] is `erase_all` in gvn; [#2017] (merged) is `Nearest::dominating_below`. Moves [synthesis] row 4 (slopes).
- **(b) Early:** stack the seven on main and measure `d_faces` -O1/-O2/-Os. Kill (rework the gating thresholds) if the stack is above +0.3%. A shortcut must pay nothing on short bodies: it is gated at entry by structural counts of work (a chain of at least 64 accesses, a body of at least 1,024, 8 joins), which are counts and not thresholds found by a sweep.
- **(c) Gate:** none.
- **(d) Exit:** M plus `tools/ladder` all shapes. Pass:
  - `loop` N=1600 -35% or better;
  - no shape's exponent up by 0.02 or more;
  - `d_faces` at most +0.3%;
  - cost-gate shape rows at most +0.15% over base.
- **(e) Who:** greedy-port. Depends on 0.1. Touches `memoryssa.rs`, `avail.rs`, `transform.rs`, `lsr.rs`, `spill.rs`. It collides with 1.7 on `avail.rs`, so merge in PR order.

Checklist: the shared list. Its own metric boxes:

- [ ] `loop`, `mem`, `diamond` ladder rows
- [ ] exponents
- [ ] `d_faces` cost
- [ ] RSS on `mem` 1600

| outcome | decision | next |
|---|---|---|
| partial (exponent still at or above 1.1 stacked) | callgrind the `loop` N=1600 compile; the next lead is gvn `subexpressions` for equal loads whose `MemRef`s differ only by pointer value (about 440 M at `mem` N=1600); [#2034] replaced the per-(slot, candidate) `clobbered` memo | 2.3 |
| no effect | the ladder binary is stale or confounded; rebuild both from parents | 1.6 again |
| worse elsewhere (`d_faces` above +0.3%) | drop the PR that pays on short bodies, or tighten its structural gate | 2.3 |

### 1.7 dse slope ([#2013], [#2015])

- **(a) Effect:**
  - `dse` mem N=1600: 1,907 to 179 M;
  - distinct-offset shape: 3,395 to 162 M;
  - arith: 818 to 52 M;
  - 2.0x per doubling on the distinct-offset and `arith` shapes (was 3.5x to 4.0x); `mem` 38.1 / 82.6 / 192.1 M at N=400 / 800 / 1600, 2.17x then 2.33x;
  - `d_faces` +0.03% to +0.04% ([#2013], index only past 256 cells) and +0.01% to +0.03% ([#2015]), objects identical; the +0.2% in the PR text was before the threshold.

  Moves [synthesis] row 4.
- **(b) Early:** none; this is measured.
- **(c) Gate:** none.
- **(d) Exit:** M plus `mem` and `arith` ladders. Pass: `dse` 2.2x or less per doubling, and `d_faces` at most +0.3%. The first part is missed on `mem` (2.17x then 2.33x): that is the partial row.
- **(e) Who:** edit-rollback. Depends on 0.1. Touches `avail.rs` (`Solve::reaching`, `Stored::related`) and `regions.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] ladder
- [ ] exponent
- [ ] `d_faces` cost

| outcome | decision | next |
|---|---|---|
| partial | the remainder is `dse`'s inputs, not pairwise `may_clobber`: `Published` 27 M, `Accesses::managed` 17 M and `reaching` 44.7 M of `Dse::run`'s 185 M at `mem` N=1600 ([#1913] is the draft behind it) | 2.8 |
| no effect | instrument suspect; [#1994] records a fake +2% from a confounded base | 1.7 again |
| worse elsewhere | the 256-cell index threshold is already set by a sweep (0, 64, 128, 256, 400; 256 kept) | 2.8 |

### 1.8 Bounded on deep nests ([#1977], [#1978])

- **(a) Effect:**
  - queens -O3: 1,703 to 1,306 M (-23%);
  - jpeg_fixed -O3: 3,800 to 3,490 M;
  - `d_faces` -O2: 3,248 to 3,193 M.

  Moves the worst ratio of [compile-gap-series] (queens -O3, 36.4x).
- **(b) Early:** none; this is measured. `LLRM_CHECK_LAZY` and `LLRM_CHECK_CAP` are the oracles.
- **(c) Gate:** none.
- **(d) Exit:** M plus queens -O3 and jpeg -O3. Pass: queens -O3 -20% or better and identical.py 0 differ.
- **(e) Who:** greedy-port. Depends on 0.1. Touches `ranges.rs` and `ranges_scopes.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] queens -O3
- [ ] jpeg -O3
- [ ] `d_faces`

| outcome | decision | next |
|---|---|---|
| partial | queens -O3 is still `analysis bounded` plus loop passes ([compile-gap-series]) | 2.8 |
| no effect | instrument suspect; the PR measured it | 1.8 again |
| worse elsewhere | revert the cap, keep the lazy scopes | 2.8 |

### 1.9 Exact ThroughMemory for decisions ([#1915])

- **(a) Effect:** fixes [#1909] (matmul bq32 -O3 at 17,975 against 33,589 bytes); costs +1.0% at -O2. Correctness: the result no longer depends on the order analyses are asked in.
- **(b) Early:** none (correctness).
- **(c) Gate:** none.
- **(d) Exit:** M; [#1909]'s repro equal both ways; `LLRM_CHECK_ORDER` ([#1982]) silent. Pass: the cost is at most +1.2%.
- **(e) Who:** keep-analyses. Depends on 0.1. Touches `passes.rs` (`get_exact`), `fold`, `decide`, `FloatFacts`.

Checklist: the shared list. Its own metric boxes:

- [ ] repro
- [ ] order check
- [ ] cost

| outcome | decision | next |
|---|---|---|
| partial (cost above +1.2%) | the cost is ThroughMemory solves; recovered by 3.1 | 3.1 |
| no effect | n/a | |
| worse elsewhere | a row moves: fix forward; never revert a correctness fix for cost | 3.1 |

### 1.10 Held PRs on main ([#1958], [#1921], [#1962], [#1965])

- **(a) Effect:** each draft's number, re-measured on main:
  - [#1958]: -3.4% on main `90bce0774`-era; an open PR, ready (not a draft);
  - [#1921]: -1.77% on `254855801`; to be closed, since [#1975] (merged) did its dirty-block mechanism in general, and its allocation-free rule windows (-4 M) live on branch `alim/perf/lir-cleanup-2` `6db843a29`, not a PR;
  - [#1962]: -0.46%, under the 0.5% bar: bundled with 2.6;
  - [#1965]: -0.3%.

  Several are stale against [#1975] and [#1938]. Moves [synthesis] rows 3, 6 and 7.
- **(b) Early:** rebase each draft on main and measure it with M. Kill a draft if it is below 0.5% on main, unless it bundles with a same-area step: [#1962] with 2.6, [#1965] with 2.4.
- **(c) Gate:** none.
- **(d) Exit:** M per PR. Pass: 70% or more of its stated effect.
- **(e) Who:** greedy-port ([#1958]), ssa-spill-m16 ([#1921], to close), attrs-budgets ([#1962], with 2.6), UNOWNED ([#1965]; proposed owner greedy-port, with 2.4). Depends on 0.1. Touches `splitkit`, LIR `peep/`, `alias::initialized`, `allocate`.

Checklist: the shared list. Its own metric box:

- [ ] one box per draft re-measured

| outcome | decision | next |
|---|---|---|
| partial | land it; the remainder of `split placed` goes to 2.4, of clean-up to 3.4 | 2.4 |
| no effect | close the draft (it is not counted against the PR cap), record why in [spec 8] | 2.4 |
| worse elsewhere | revert | 2.4 |

### B1 Re-baseline

Re-run 0.1 on main after phase 1 merges.

- **Pass:** the measured row is within 2 points of the TL;DR's -8.3% to -13.6%.
- **If the row is more than 2 points worse:** stop. Find which PR's on-main effect missed, using the per-PR boxes, then re-rank phase 2 by the new [synthesis] table.
- **Owner:** llvm-compare.

## Phase 2: open levers with a bound in the repo (output identical)

### 2.1 point_values as a per-query provenance (stop) and 2.1b repeats

- **(a) Effect:** measured. Callgrind inclusive `alias::point_values` is 123.97 M on main `ff49545a5` (`slotstores::reaching` 37.7, `_direct` 18.9, `MemRef::of` 16.6, `IdMap` 6.3, `learn` 4.9). Step 0's asked-against-solved counts are done: `annotated` asks 87% of what is solved (7,540 of 8,634), `escapes` 60%, `initialized` 63%. By the plan's rule (saving = solve cost less asked values times the walk's per-value cost), the per-query form saves at most the unread 13% to 40% minus its own query cost, and the eager local walk costs +276 M: 2.1 as written meets its "no effect" row. Stop, and record the table in [spec 8]. Moves [synthesis] row 1.
  - **2.1b, repeats:** 209 solves over 96 body versions (113 repeats; QCport 13,269 equal repeats against 8,048 first). An exact-key cache hits 78 times for a net 13.6 M (0.45%); the prototype's 131 differing objects came from a loose share key, which keep-analyses fixes (the global each constant names, the `globals_aa` view, the exposed set, the shape's back edges); `LLRM_CHECK_PVSHARED` must pass on all 1,583 objects, and the step is killed if the net is under 0.5% of `d_faces` -O2. Branch `alim/exp/pv-demand` `4fb50bd931c2`. Owner keep-analyses; it depends on neither identity nor [#2035] and [#2037].
  - Other families: `Annotated`, accesses and `through-memory` consume objects; only the producers intern (`PointerValues`, the escape phase, `CallEffects` summaries, `ExposedFrames`). Skipping a consumer is exact iff its inputs are. Spelling dependence through results kept as sound is `unmeasured` (`LLRM_CHECK_ORDER` seeds 1 to 3 are clean).
- **(b) Early:** done for 2.1 (counts above). For 2.1b: the corrected key, with `LLRM_CHECK_PVSHARED` on the corpus.
- **(c) Gate:** none for the stop; a design page only if 2.1b clears 0.5% net.
- **(d) Exit:** 2.1: the [spec 8] entry. 2.1b: M, identical.py 0 differ with `LLRM_CHECK_INTERN=SEED`, and 60% or more of 13.6 M.
- **(e) Who:** keep-analyses. 2.1b depends on nothing open.

Checklist: the shared list. Its own metric boxes:

- [ ] step-0 own cost
- [ ] asked/solved counts
- [ ] instructions
- [ ] computes
- [ ] oracle

| outcome | decision | next |
|---|---|---|
| partial | the consumers that read every value (set-valued) keep the solve; list them for 3.1's representation change | 3.1 |
| no effect | stop; record in [spec 8] with the asked/solved table; this weakens 3.1's per-query premise for pointer facts, so 3.1's step 0 must show another family first | 2.2 |
| worse elsewhere (output changes) | revert; the per-query answer is not the solve's: fix the oracle gap first; coordinator decides | 2.2 |

### 2.2 Annotated brought up to date per access

- **(a) Effect:** Annotated costs 78.9 M own at -O2. 116 builds; 57 returned the held result, about 39 M (1.3%) ([#1994]). Keyed removal of erasures realised -0.6 M ([#1973]); what remains is re-annotating the accesses whose operands changed ([analysis-manager]). Moves [synthesis] row 1.
- **(b) Early:** `LLRM_WHY` blockers ([#1972]) for Annotated's recomputes on main. Count those whose log holds only operand edits that reach no address operand, and sum their cost. Annotated's same-version recompute is 1.2 M; its 87 edited rebuilds (42.4 M) are the pool, and 5 of 11 changing `fold` runs substitute an address operand (81 of 128 replaced values feed a load, store or `gep`). Kill if the recoverable sum is below 15 M.
- **(c) Gate:** the update rule goes into [spec] 3.2's table, with a review.
- **(d) Exit:** M plus `LLRM_WHY` annotated `same` count. Pass: 50% or more of the probe's sum, and `LLRM_CHECK_REPLAY` silent on the corpus.
- **(e) Who:** keep-analyses (meaning), edit-rollback (record, [#1943] `operands_edited`). Depends on [#1943] and B1. Touches `manager.rs` (Annotated) and `touched.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] blocker table
- [ ] instructions
- [ ] same-result count
- [ ] replay check

| outcome | decision | next |
|---|---|---|
| partial | the rest follows address edits; per access is 3.1's form | 3.1 |
| no effect | close; record in [spec 8] beside the keyed-removal entry | 2.3 |
| worse elsewhere | revert | 2.3 |

### 2.3 MemorySSA that persists (R2 of [parity])

- **(a) Effect:** the structure is rebuilt 54 times (71 M, 1.3%). Walker queries cost 133 M (2.5%), and 65% repeat an earlier version's key ([spec 8]). Ceiling 3.9% ([parity]), against the cache ceiling of 1.4% measured in [spec 8]. Those two disagree; step 0 reconciles them. Moves [synthesis] row 1.
- **(b) Early:**
  - step 0: on main, count structure builds per function, and queries whose answer is unchanged across versions with no memory, call or CFG edit between;
  - estimate the saving as the rebuild cost plus those queries' cost.

  Kill if below 1% (30 M).
- **(c) Gate:** a design page. It covers the manager-owned MemorySSA updated from the edit record, a stamped clobber per (access, cell), and `LLRM_CHECK_CLOBBERS` against a fresh walk. Review before code.
- **(d) Exit:** M; counted test `structure builds per function <= 1 + CFG recomputes`; walker instructions; ladder `mem` and `loop`. Pass: 60% or more of the probe's estimate, and no exponent up.
- **(e) Who:** hoist-bounded, with keep-analyses. Depends on 1.6 (walker tables) and B1. Touches `memoryssa.rs` and `manager.rs`; serialise with greedy-port on `memoryssa.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] build count
- [ ] query share
- [ ] instructions
- [ ] counted test
- [ ] ladder

| outcome | decision | next |
|---|---|---|
| partial | the remainder is per-query walk cost (`clobbered` 48% of the walker, [spec 8]); the per-(slot, candidate) memo | 3.1 |
| no effect | record in [spec 8] beside the walker cache; R2 drops from [parity] | 2.4 |
| worse elsewhere | revert | 2.4 |

### 2.4 Split path tables

- **(a) Effect:** the split path is about 132 M with [#1958] (117 M `placed` without), `unmeasured` in the repo (session report). B' (position and row tables keyed by body position) was built and measured: -0.03% (2,744.59 to 2,743.70 M); hash probes are not the cost, the rows' own work is (506 instructions per row visit: interference about 10 M, `Placement::iterate` 3.9, price 10.1, held 6.7, `Frame::new` 5.7, growth about 9). Of the rest, A' (`carved_moving` fed from the allocator's postings, 29 M, 1.0%) was never built; E (`spilled_apart`) and C are each below 0.5%. `InterferenceCache` (per register and block) is the 10 M interference part and belongs to 3.3. The result is in [spec 8] ([#2036]). Moves [synthesis] row 3.
- **(b) Early:** none left: by its own kill rule (B' and A' together below 30 M) 2.4 dies as written. Keep it only as A' alone, with step 0 on main with [#1958]: callgrind self and inclusive of `split carving` and `carved_moving`.
- **(c) Gate:** none (same algorithm, dense tables).
- **(d) Exit:** M plus those rows. Pass: 60% or more of A''s 29 M.
- **(e) Who:** greedy-port. Depends on 1.10 ([#1958]). Touches `splitkit.rs`, `allocate.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] step-0 rows
- [ ] A' alone

| outcome | decision | next |
|---|---|---|
| partial | the rest is per-carve body copies: LIR model change | 3.3 |
| no effect | record; the allocator gap is not in the split path | 3.3 |
| worse elsewhere (row or object moves) | revert; allocation must be identical | 3.3 |

### 2.5 Interval recompute in `Facts::of` (closed)

- **(a) Effect:** closed by ssa-spill-m16. The interval recompute is 37.4 M ([#2038]: 132 K per update of 10.6 touched values), but the avoidable part is about 11 M (0.4%) and the realistic part 6 to 7 M (0.25%). The kill rule (15 M) is met.
- **(b) Early:** met; recorded in [spec 8].
- **(c) Gate:** none.
- **(d) Exit:** none. The 3 to 4 M of buffer reuse is folded into the next intervals PR.
- **(e) Who:** ssa-spill-m16. The remainder (`Indexes::patch` 4.6 M, postings following 14 M) is 3.3's.

### 2.6 `Frequencies` dense

- **(a) Effect:** `unmeasured`. The cost is `frequency.get(&cfg::id(block))` per instruction in `spill::TrafficBase::of`, `without` and `_add` (`often`), not construction; [#1971] lists `profit::Frequencies` and `Frequency::over` as not converted. The type `BTreeMap<i64, i64>` runs through `llrm-analysis` `branchprob` (`estimated`, `taken_from`), `candidate.rs`, `lsr.rs`, `hoist.rs`, `inline.rs`, `sink.rs` and `spill.rs` (a trial rename gave 9 errors in two crates), and it collides with pass-list-parity's hoist and spill pricing PRs, so it comes after them. Bundles [#1962] (-0.46%, under the 0.5% bar). Moves [spec 9] container rows.
- **(b) Early:** callgrind self of `TrafficBase` and the `frequency.get` calls; key density against the block-id space (about 1: every block has an entry). Kill if below 15 M, or if density is below a quarter ([#2026]'s lesson); the 15 M probe decides.
- **(c) Gate:** none.
- **(d) Exit:** M. Pass: 60% or more of the probe.
- **(e) Who:** attrs-budgets. Depends on 1.2 and on pass-list-parity's hoist and spill pricing PRs. Touches `profit.rs`, `frequency.rs` and the files above.

Checklist: the shared list. Its own metric boxes:

- [ ] probe
- [ ] instructions

| outcome | decision | next |
|---|---|---|
| partial | land as a bundle with 1.2's remainder | 2.7 |
| no effect | close; nothing recorded beyond the probe on [#1994] | 2.7 |
| worse elsewhere | revert | 2.7 |

### 2.7 Rollback, not clones ([#1945], [#1943])

- **(a) Effect:** (the [allocation path](#allocation-the-path-to-llvms-count), item 3) 144 function clones at -O2 down to 0 ([#1945]: `stamped` 84, gvn fork 38, `sink` 11, interprocedural 11), at most +0.1% instructions. It is an enabler with no instruction saving of its own: 3.1's updaters, 3.5's pricing on the live function, and [spec] 2.3. The 144 clones cost about 0.1% of the compile (the scopes that contain them are 6% to 10%, from numbering and forecasting, not from the copy); the +0.1% above is the cost side.
- **(b) Early:** none; draft measured. Kill if a site costs more than +0.3% or `LLRM_CHECK_ROLLBACK` fails.
- **(c) Gate:** [spec] 2.3 is the gate (already written).
- **(d) Exit:** M; counted `function_clones = 0` in the pipeline. Pass: 0 clones and at most +0.1%.
- **(e) Who:** edit-rollback. Depends on B1. Touches `edit.rs`, `module.rs`, `gvn.rs`, `hoist.rs`, `interprocedural.rs`, `sink.rs`. It collides with 3.5 on `gvn.rs` and `hoist.rs`, so land first.

Checklist: the shared list. Its own metric boxes:

- [ ] clones per site
- [ ] instructions
- [ ] rollback check

| outcome | decision | next |
|---|---|---|
| partial | keep the sites that pay nothing; the rest wait for 3.5 | 3.1 |
| no effect | n/a (an enabler: judged by the clone count) | |
| worse elsewhere | revert that site | 3.1 |

### 2.8 Scaling shapes

- **(a) Effect:**
  - queens -O3 after 1.8 at clang's (71 M) or gcc's (634 M) cost or less; measured by hoist-bounded: 1,478 M against gcc 628 M and clang 70 M, the lead cost `analysis bounded` 458 M (190 runs, 30%), which is step 1.8;
  - the joins and unrolled axes no steeper than gcc's. Re-measured on main `bd6e647f6`: the joins slope is 1.33 (gcc 1.38, clang 1.16), the plan's 1.81 did not reproduce; at N=2048 the superlinear rows are `lir coalesce` 2.02, `twoaddr` 2.16, `split placed` 1.80 and `gvn` 1.69 (owners ssa-spill-m16 and greedy-port). Unrolled (un64 to un128 x3.07): slope 1.53 against gcc and clang 1.07 (5.1x gcc at 128), with one root: `lsr::reduced` edits once per loop, 256 body versions, each re-asking whole-function analyses: `Pressure::updated` 3.58 G (7.6% of the compile, x4.05) and `Counted::update` 2.16 G (`congruences_with` 1.84 G, 255 solves of 7.2 M); LLVM's LSR keeps loop-local registers and SCEV answers per value with `forgetValue`;
  - the `mem` shape's resident set per IR instruction: 15.7 KB against clang's 0.30 KB (hoist-bounded; the 11.7 KB of [allocation] was the system allocator at N=1600), against 4.4 KB for `arith`; the massif at N=1600 is 251 MB, 57% of it the memory walker's jump tables (fix coming).
- **(b) Early:** `scaling.py` on main and callgrind of the top three steps at 2N for each axis. Kill (fold into that step) if the lead cost belongs to a step already listed.
- **(c) Gate:** none until a cause is named.
- **(d) Exit:** M plus `scaling.py` and `tools/ladder/rss.py --shape mem`. Pass: each axis's exponent at most gcc's + 0.05.
- **(e) Who:** hoist-bounded. Depends on 1.6, 1.7 and 1.8. Touches whatever the probe names.

Checklist: the shared list. Its own metric boxes:

- [ ] each axis
- [ ] RSS per IR instruction

| outcome | decision | next |
|---|---|---|
| partial | name the remaining pass per axis on [#1994] | 3.2 |
| no effect | the axis cost is in an existing step | that step |
| worse elsewhere | revert | 3.2 |

### 2.9 Vec growth by site

- **(a) Effect:** mimalloc's `realloc` memcpy is 72 M (2.6%) of `d_faces` -O2 and was missing from this plan. Growing in place was measured at +0.34% instructions with cycles flat. The candidate is a per-site `with_capacity` on the six biggest growers (`forest` 4.1 M, `exposed_allocas` 3.2, `live_rows_within` 2.2, and three more), about 15 M, under the 1% rule: it lands only bundled with 1.2's remainder or as a correctness-free cleanup. Moves [synthesis] row 2.
- **(b) Early:** callgrind callers of the `realloc` memcpy, per site, on main. Kill if the six sites sum below 15 M.
- **(c) Gate:** none.
- **(d) Exit:** M. Pass: 60% or more of the probe.
- **(e) Who:** UNOWNED (proposed: attrs-budgets, with 2.6). Touches the sites the probe names.

### B2 Re-baseline

Re-run 0.1 on main after phase 2 merges.

- **Pass:** the measured row is within 2 points of the TL;DR's phase-2 row (about 2,210 M).
- **If it misses:** re-rank the open phase-3 steps by the new table; phase 3 is not held for B2.
- **Owner:** llvm-compare.

## Phase 3: structural work (critical path)

### 3.1 Facts asked per query

- **(a) Effect:** (the [allocation path](#allocation-the-path-to-llvms-count), item 2) analysis computations per pass run from 2.8 toward 0.5 ([spec] 1.2, 6). Analyses own 631 M at -O2 over 2,742 computes ([#1994]). Two measured results shape the step.
  - Preserving across no-change cannot carry it: 545 of 2,742 computes (20%) are on a body version already computed, 41.3 M, 1.4%; 1,965 (72%, 510 M) follow an edit (edit-rollback, [#1994] comment 6234166683). The same-version slice is capped at 1.4%; its largest cause is 0.4% (the `Outer` wipe when only type ancestry differs; under the bar, no PR).
  - Laziness does not carry it in the pointer families either (keep-analyses): of 2,499 computes, 24% (592) are in the lazy families and 44% (1,090) in the cheap many (`exposed-frames`, `registers`, `assumptions`, `dominators`, `loops`, `shape`), which are recomputed after edits that touch what they read (94%), so per-query does not take them to 0.5; they need updaters. The pointer-family ceilings are small now: escapes -27 M landed (0.9%), `point_values` sharing at most 0.45% (2.1b, killed if the net is under 0.5%), predicate call effect dropped.
  - What a decide, hoist or indvars run asks (edit-rollback, [#1994]): the idle runs of the three own 86 M (3.0% of the compile), and no cheap work check says no except `hoist`'s, which exists. The lever for the family is not skipping runs but not re-deriving: facts at the site that asks (`decide`: `_decisions` and `constant_cycles::propagated`, bound 41.3 M, 1.4%), update from the change log (`indvars`' `Standing`, bound 26 M, 0.9%), `Accesses` only when a seeded instruction is a load (`hoist`). Each bound is under 1.5%.
  - The -12% can come only from edit-rollback's measured family: `Held`, `ThroughMemory` and decide facts, 403 M own, 12.8% (`decide` asks 574 branch visits and decides none), brought up to date from the change log (`facts.at`; `Touched` exists). The step is re-pointed at that family, owned by edit-rollback with keep-analyses; its saving is `unmeasured` until one family is built. Moves [synthesis] row 1 and row 5 (idle runs: their cost is the facts, [idle]).
- **(b) Early:**
  - per analysis: computes per run, from 0.2's counter;
  - which pass asks it;
  - entries read per build (the missing number in [#1994]).

  Count first which analyses can be updated from the change log (the per-analysis table of the 545 same-version computes: `registers` 15.5 M, `call-effects` 5.6, `bounded` 3.8, `exposed-frames` 3.3). Prototype one family on a throwaway branch (an updater from the change log, or a per-query form where its consumers ask little): `bounded` asked per loop and block, the shape of [#1978]'s lazy scopes, the ranger form of [analysis-avoidance]. Kill that family if:
  - the prototype's asks × cost per ask is at least the whole build's cost; or
  - any object differs; or
  - the projected saving is below 15 M.
- **(c) Gate:** a design page under [parity] R1 with a big-picture review. It sets:
  - which facts live on the IR, gcc's way (points-to on the SSA name, nonzero bits, addressable flag; [analysis-avoidance]);
  - which are per query with a cap (LLVM `BasicAA`, `computeKnownBits` depth 6);
  - which stay whole-function because their consumers sweep them (annotated, through-memory, float-facts per [analysis-avoidance]);
  - the oracle for each.

  The ablation in [rows-series] is the constraint: keep each fact exact, make it cheap.
- **(d) Exit:** M; counted test `analysis_computations / pass_runs <= 0.5` on the fixtures; `LLRM_CHECK_REPLAY` and `LLRM_CHECK_ORDER` silent. Pass: the ratio is met at -O1/-O2/-Os, and 60% or more of each landed family's probe.
- **(e) Who:** edit-rollback (family, record and updater API), keep-analyses (meaning). Depends on 0.2, 2.2 and 2.7. The first question of the design page is which analyses can be updated from the change log and which are per query. Touches `passes.rs`, `manager.rs`, `ranges.rs`, `consts.rs`, `alias.rs`. One family per PR.

Checklist: the shared list. Its own metric boxes:

- [ ] per-family probe
- [ ] ratio at three levels
- [ ] instructions per family
- [ ] oracles

| outcome | decision | next |
|---|---|---|
| partial (ratio above 0.5) | name the analyses still above 0.5 per run with their askers; each gets a family PR or a recorded reason | 3.2 |
| no effect (prototypes cost as much as builds) | stop; record in [spec 8]; the cost is the fact's own computation, so phase 4's per-analysis budgets come first | 4.1 |
| worse elsewhere (rows or objects) | revert that family; precision lost is output lost ([rows-series] ablation); coordinator | 3.2 |

### 3.2 One pipeline pass: combine worklist, placements, rounds to 1 (R3 of [parity])

- **(a) Effect:**
  - two rounds with the reorder alone: -9.8% ([parity]);
  - round three: 211 M (6.3%) ([spec 8]);
  - rounds of two or more: 35% to 56% of step work ([parity]);
  - idle reruns: 247 M (7.8%) ([compile-gap-series]).

  The combine alone is expected at about 0; the effect is the round drop, which is also the risk. Closed ideas, not to repeat: `gvn`, `dse` and `inferspace` ahead of `decide` (-4.4% at -O2, +5.4% at -O3, `x_hanoi2` crosses above 1.00); early scoped CSE (+8% to +11% with the alias-checked table, -3.1% to -3.4% kill-all but 4 cells worse, +1.6% to +1.9% once the stray exit phi is folded). Moves [synthesis] rows 5 and 9.
- **(b) Early:** step 1 was built and measured before this plan (branch `notkept/combine-worklist`, `fba7c4051..fea9b27f0`): merging fold, algebraic, phiopt, dead and the branch fold of decide into one worklist pass at today's positions and rounds gave about 0 compile time, because the cost is analyses and idle reruns, not rewriting (no spec 8 entry yet; the owner re-measures and writes it). Treat step 1 as expected 0, not part of the -10%. The risky part is dropping repeat runs: round-1-only per pass changes output (`hoist` at least 20 objects, `loopidioms` at least 20, `loopsimplify` 17, `promote` 6, `trivialunswitch` 2; all five together -3.0% at -O2 and -6.0% at -Os on `d_faces`), and the enabling relations are dense (`dead`, `gvn`, `dse`, `lcssa` and `fold` enable nearly everything); global `rounds=1` lost 25, 32 and 51 rows at -O1. So the early check is a per-pass ablation: run pass P in round 1 only, list the objects that change, name the enabling pass. Then:
  1. identical.py, which must be 0 differ;
  2. per-pass ablation table, one named enabler per repeat run that matters;
  3. vsgcc rows and bench cells against main.

  Kill if a vsgcc row newly crosses above min(gcc, clang), or a bench cell is worse, and the combine cannot absorb the named enabler. The known causes are the third-round chain fold, decide, lcssa, gvn ([spec 8]) and the stray exit phi ([spec 8]).
- **(c) Gate:** a design page (pass list per level against clang's, [pass-parity]; the re-queue rule; the loop worklist) with a big-picture review.
- **(d) Exit:** M; `LLRM_CHECK_FIXPOINT` silent (every pass run in full after convergence changes nothing); vsgcc rows, `d_faces` executed, QCport bytes, bench cells all four levels, m16 and m32. Pass:
  - the landing rule of [spec] 4.3;
  - -8% or better at -O2 and -Os;
  - idle runs per compile down 50% or more: row 5 is closed ([#2028], [#2030]), so this can come only from the round drop, not from a skip.
- **(e) Who:** pass-list-parity (pipeline, combine), hoist-bounded (rounds, loop worklist). Depends on 3.1 for the round drop; the per-pass ablation starts now. Touches `pipeline.rs`, `fold.rs`, `algebraic.rs`, `phiopt.rs`, `dead.rs`, `decide.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] identical at today's rounds
- [ ] rows at rounds 1
- [ ] bench cells
- [ ] fixpoint check
- [ ] instructions

| outcome | decision | next |
|---|---|---|
| partial (rounds 2 hold, 1 does not) | land rounds 2 at the levels whose rows hold; the chain that needs round 3 is named and given a combine rule | 3.3 |
| no effect | instrument suspect: compare `LLRM_DEBUG=runs` counts base and head first | 3.2 again |
| worse elsewhere (rows) | revert the round drop, keep the combine pass; coordinator decides per level | 3.3 |

### 3.3 The allocator on the LIR core ([spec] stage 2)

- **(a) Effect:** (the [allocation path](#allocation-the-path-to-llvms-count), item 3) `regalloc` is 415.5 M on main with [#2010], [#2021], [#2027] and [#2032] (444 M is older), against greedy's 39.5 M ([regalloc]). The split path is 132 M with [#1958]: carve O(body) 29 M, `Facts` rescans of untouched values about 50 M, `_next_value_following` 5.6 M, about 85 M (3%). Budget 100 M at -O2 ([spec] 1.3). Spec 4.1 shape:
  - LIR in an arena with MIR-shaped operands;
  - slots on instructions;
  - use lists;
  - intervals built once and edited;
  - an interference cache per (register, block);
  - no whole-body walk per spill or split.

  Moves [synthesis] row 3.
- **(b) Early:** spec 6 counters on `d_faces` -O2: `full_interval_builds`, allocator `visited` per spill or split, body copies per carve ([#1309]). Kill (re-rank against 3.4) if whole-body work per edit is below 1% of the compile.
- **(c) Gate:** a design page (spec 2.1 and 4.1 for LIR) with a big-picture review; the edit contract of spec 2.3 for LIR. The page is [#2042] (ssa-spill-m16 with greedy-port, merged) with its experiment in [#2049]; the S4 results above are its early probe. The prove-once part is about 12% of `regalloc`; "created values from the edit record" was tried and not kept (4.1 touched values against 0.8 fresh; LLVM recomputes old ranges too); one lane (B) takes the exit from 415 M to about 140 M. The decision procedure (queue stages and eviction against RAGreedy's) needs its own page before any build.
- **(d) Exit:** M; counted `full_interval_builds = 1` and allocator `visited` at most 40 × edited range + 4 × blocks touched; identical output (spec stage 2 is identical). Pass: `regalloc` 150 M or less at -O2.
- **(e) Who:** greedy-port (allocator), ssa-spill-m16 (LIR model). Starts now: gated by its early probe and design review, not by B2; it uses 2.4's A' and 3.3's own interval buffers. Touches `model/lir.rs`, `allocate.rs`, `splitkit.rs`, `intervals.rs`, `postings.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] counters
- [ ] `regalloc` instructions
- [ ] identical

| outcome | decision | next |
|---|---|---|
| partial (150 to 300 M) | callgrind the allocator; the next lead is evict's `_free`/`busy` per victim (`unmeasured` in the repo) | 3.4 |
| no effect | record; the cost is the decisions, not the representation: price the in-run decisions of spec 4.2 | 3.5 |
| worse elsewhere | revert the PR that moved output; identical is the rule | 3.4 |

### 3.4 LIR clean-up on dirty blocks and at selection

- **(a) Effect:** merged-tree numbers (ssa-spill-m16): `Peephole` 97.1 M, `jumps` 58.7, `coalesce` 57.8, `FloatAssign` plus `FloatAlloc` 65.1, `phielim` 11.4: 290 M (and `parcopy` 9.8, `twoaddr` 8.3, `pressuresink` 8.1, `loopslots` 7.0 beside it); the 340 M of [synthesis] row 6 is stale. The credible exit is about 250 M until the next probe (290 to 250), not 160. The peephole's shared lane and exit facts (at most 1.0%) and the x87 live rows (0.9%) are dropped. Rewrites that need no physical register could move into selection (DAG-combine style, [lir-cleanup] item 5). Moves [synthesis] row 6.
- **(b) Early:** done by ssa-spill-m16 ([#2043], spec 8). Idle block visits are 97.1% of the clean-up's visits (350 corpus files at -O2; 96.5% on `d_faces`; 23 of 44 sub-passes change no block there), but skipping them is worth 10.4 M (0.37%) of `d_faces` -O2, measured with an oracle (the changed blocks of the rule-group driver recorded and replayed, output byte-identical) with a perfect predicate and no classification cost; the classification itself costs 0.65% (spec 8 idle-step skip). Below the 15 M kill: the idle-visit skip is killed. The rule groups cost about 25 M in all (variable part at most 18.8 M, fixed part at most about 6 M), so fusing them is worth at most 6 M. `Peephole::once` is 97.1 M: about 55 M is six hand-written passes (`copyprop` 22.7, `spillforward` 12.7, `postrasink` 5.0, `machinecse` 4.2, `thrashed` 3.0, `machinedce` 2.7), and about 29 M is cross-block dataflow (`spillforward` `_available` 10.1, lane liveness 8.7, `postrasink` live-in 3.5, flags 2.4, `copyprop` decode about 4). Making `copyprop` and `spillforward` block-local like LLVM's `MachineCopyPropagation` saves 14.8 M (0.52%) but grows `d_faces` from 8,068 to 8,097 bytes (+0.36%) and from 1,835 to 1,843 instructions: killed. A single-predecessor-chain (dominator-scoped) `copyprop` gives the same output as block-local: of 50 block entries holding a fact in the full solve, 34 (68%) have one predecessor and 16 are joins, under the 80% bar: killed. LLVM's `MachineCSE` is dominator-scoped (SSA-like values, sound) and `MachineSink` is cross-block with dominance checks; `postrasink` sinks into a successor, cross-block by definition. The lever is the cost of the solves (the first solve of lane liveness is about 850 instructions per instruction, and availability), not skipping or localising.
- **(c) Gate:** a design page for the selection-time rewrites with a review, only if the probe does not fire.
- **(d) Exit:** M; the cost of the first lane-liveness solve and of availability per instruction (the next probe). Pass: clean-up about 250 M at -O2 until that probe.
- **(e) Who:** ssa-spill-m16. Depends on 3.3. [#1921] is closed, not depended on ([#1975] merged); its allocation-free rule windows (-4 M) are on branch `alim/perf/lir-cleanup-2`. Touches `peephole.rs`, `peep/`, `jumps`, isel combine.

Checklist: the shared list. Its own metric boxes:

- [ ] idle visits
- [ ] clean-up instructions
- [ ] counted visits

| outcome | decision | next |
|---|---|---|
| partial | the remainder is the per-useful-visit cost: phase 4 budgets | 3.5 |
| no effect | record; the cost is per useful visit: phase 4 budgets | 4.1 |
| worse elsewhere | revert | 3.5 |

### 3.5 Pricing where LLVM puts it (R5 of [parity])

- **(a) Effect:** reconciled by ssa-spill-m16 (callgrind inclusive, main plus [#2010], [#2021], [#2027], [#2032]). -O2 (2,612 M): `spill::View::forecast` 67.6 M (17 calls), 2.6%, of it `Pressure::updated` 41.8 M; callers `hoist` 36.6 M and `profit`/`lsr` 31.0 M (`lsr::_plan` 109.5 M inclusive); `profit::weighted` 1.9 M. -Os (2,699 M): `forecast` 130.9 M, 4.9%; `hoist` 99.9 M (3.7%), `lsr` 31.0 M; `weighted` 3.2 M. `gvn`'s pricing is about 1 M (0.04%): the "about 5%" of [parity] is not `gvn`; the share is `hoist` (1.4% at -O2, 3.7% at -Os) and `lsr` (1.2%), through `View::forecast` and `Pressure::updated`. `spill.rs` is MIR, so the back end is not on this path. [decision-dependencies] reads 1.39% and 2.52% because it counted a different part of the forecast; both are recorded here. The back end's gain (rematerialization, sinking, a pressure-aware LIR hoist per loop path) is a separate question and `unmeasured`. Moves [synthesis] row 4.
- **(b) Early:** done (above): the sum is 2.6% at -O2 and 4.9% at -Os, so the kill (below 1% at both levels) is not met and the step stays alive. The next probe is `hoist`'s and `lsr`'s use of `Pressure::updated` (41.8 M) per call site.
- **(c) Gate:** a design page with a review; any row that falls below min(gcc, clang) blocks the pass's drop of pricing ([parity]).
- **(d) Exit:** M; counted `spill forecasts per pass run = 0` in the mid-end; the rows rule of [spec] 4.3. Pass: 60% or more of the probe, with rows held.
- **(e) Who:** hoist-bounded (`hoist`) and greedy-port (`lsr`, [#2008]); the back end only for its separate unmeasured part. Depends on 2.7 for pricing on the live function. Touches `hoist.rs`, `lsr.rs`, `profit.rs`, `spill.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] reconciled bound
- [ ] instructions
- [ ] rows
- [ ] counted forecasts

| outcome | decision | next |
|---|---|---|
| partial | keep pricing at -Omax only, where rows need it | 4.1 |
| no effect | record; the bound was the 1.39% | 4.1 |
| worse elsewhere (rows) | revert that pass's drop; the back end still cannot recover it ([#1883]) | 4.1 |

### 3.6 Code quality on pressure-heavy loops

- **(a) Effect:** closed at parity by greedy-port's study: with the `chain-1` source hand-factored to clang's form llrm has 90 instructions and 24 stack operands against clang's 82 and 23 (186 and 83 unfactored), and on six real bench functions llrm has fewer stack operands than clang (`jpeg_fixed` 156 of 1,076 against 581 of 2,331, and others). The difference is the source's factoring, which LLVM's Reassociate and InstCombine `FactorizeBinOp` do (`A*B+A*C` to `A*(B+C)`, `(x<<c1)+(x<<c2)` to `x*K`): a low-priority row for pass-list-parity, generator-shaped, a probe over the corpus first. Before the study: stack operands were 2.3x to 3.6x clang's on pressure-heavy loops (83, 733 and 3,172 against 23, 315 and 1,218 at the three sizes of the `live` axis): llrm spills 51, 443 and 1,215 values where LLVM splits (9, 101 and 301 new virtual registers) and rematerializes. This is output quality, not compile time; it moves the rows rule of [spec] 4.3.
- **(b) Early:** a study before any build: what LLVM's `InlineSpiller` and `SplitEditor` do on the `live` shape that the spill path of llrm does not (split before spill; rematerialization of constants and cheap definitions).
- **(c) Gate:** the study page with a review.
- **(d) Exit:** stack operands within 1.5x of clang's on the `live` shapes; rows held.
- **(e) Who:** greedy-port. Depends on the S4 result. Touches the spill path of `allocate.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] study
- [ ] stack operands on the `live` shapes

| outcome | decision | next |
|---|---|---|
| partial | the remaining spills have no remat form: record per shape | 4.1 |
| no effect | the cost is the decision procedure, not the path: S4's page | 3.3 |
| worse elsewhere (rows) | revert; rows are the rule | 3.3 |

### 3.7 llrm-codegen's machine layer target-generic

LLVM shares one target-independent CodeGen; `llrm-arm64`'s private allocator is spike-only. The machine passes in `llrm-codegen` take their target facts from the target description (target hooks), and no `llrm_x86::` appears in core ([machine facts rule](#4-cross-step-guards)).

- **(a) Effect:** `llrm_x86::` uses in `llrm-codegen` go to 0; `llrm-arm64` runs the core machine passes and drops its private allocator. Output on x86-64 identical.
- **(b) Early:** arm64-spike's experiment: the core machine passes on arm64 selector output. Kill: a pass that needs a target literal the description cannot carry names the missing hook.
- **(c) Gate:** a design page for the hook set, with a review.
- **(d) Exit:** method M; zero `llrm_x86::` in core (a test counts them); the x86-64 output identical.
- **(e) Who:** backend-two. Inputs: the target-facts audit ([#2862], [#2867]), the iced inventory, x86-64-spike's shared edits. Touches `crates/codegen/llrm-codegen`, `llrm-x86`, `llrm-arm64`.

Milestones (backend-two's dates, 2026; each step byte-identical at m16/m32 by bench). Baseline: about 120 `llrm_x86*` references in about 40 core files.

| step | date | content |
|---|---|---|
| M0 | 10-20 | [#2862], [#2867] land |
| M1 | 10-21 | ratchet widened (`llrm_x86` paths, bare bits `==`/`!=` 16 or 32); `objbuild` mode tests move to `ObjectFormat`/`Layout` |
| M2 | 10-23 | slot steps use `stack_slot_bytes` |
| M3 | 10-24 | register tables use `registers.regs`; frame base is `frame_registers()` |
| M4 | 10-26 | ABI and QB widths come from `calling.toml` |
| M5 | 10-30 | target hooks: rules, `REGISTER_INFO`, `PRESERVED`, machine tables, cpu, shadow, effects, farcall, division and arithmetic through `Target` |
| M6 | 11-04 | `Encoding` trait and the x86 impl (shape agreed with arm64-spike) |
| M7 | 11-06 | core's `Cargo.toml` drops `llrm-x86*` from `[dependencies]`; ratchet baseline 0 |
| M8 | 11-09 | arm64-spike's experiment runs on the generic layer |
| FINAL | 11-13 | core builds and passes with no x86 crate (the ratchet plus a CI `cargo tree` check) |

The dates slip only if arm64-spike's `RegId`/`Flag` or LIR vocabulary lands late (M5 to M7).

Checklist: the shared list. Its own metric boxes:

- [ ] arm64 experiment
- [ ] `llrm_x86::` uses in core

| outcome | decision | next |
|---|---|---|
| partial | the remaining uses are listed per file with the hook each needs | 3.7 |
| no effect | record the missing hook; the core stays x86-first | 3.4 |
| worse elsewhere (rows) | revert; rows are the rule | 3.4 |

### B3 Re-baseline

Re-run 0.1 after phase 3.

- **Pass:** within 3 points of about 1,460 M.
- **Then:** write the per-pass table for phase 4.
- **Owner:** llvm-compare.

## Phase 4: per-pass cost

### 4.1 Per-pass budgets ([spec] stage 8)

- **(a) Effect:** each MIR pass at 300 or fewer instructions per IR instruction per full run, against about 19 k today ([spec] 1.2). MIR pipeline budget 400 M at -O2 ([spec] 1.3).
  - The first pass is `hoist`: 366 M own at -O1 on `c04886c2e` (`_movable` 254 M, affordable 139 M).
  - Then `gvn`, `decide`, `interprocedural`, Stamp, `lsr`.
- **(b) Early:** a per-pass table (own instructions per IR instruction per run at -O2) from `LLRM_DEBUG=time,runs`. Kill a pass's PR if it is already within budget.
- **(c) Gate:** a page per pass whose fix changes its algorithm, with a review.
- **(d) Exit:** M; a counted test per pass on the scaling axes (`visited` at most 2 × function size per run). Pass: the budget is met.
- **(e) Who:** each pass's owner (hoist-bounded: `hoist`, `decide`; greedy-port: `gvn`, `lsr`; keep-analyses: Stamp). Depends on B3.

Checklist: the shared list. Its own metric boxes:

- [ ] per-pass table
- [ ] one box per pass

| outcome | decision | next |
|---|---|---|
| partial | the pass's remaining cost is named on [#1994] | 4.2 |
| no effect | record per pass | 4.2 |
| worse elsewhere | revert | 4.2 |

### 4.2 Budgets from clang

- **(a) Effect:** [spec] 1.3's budgets are gcc-derived (720 M at -O2, 1.7x LLVM). Re-derive them per job from clang's phases and passes. Measured by attrs-budgets on the reference build (24.0.0git `d1106deb71cc`; [#2056], head `a4375136d`; an older draft body read the clang 20 figures, 555 per IR instruction and 14x, and is stale): front end, optimizer and codegen at -O1 45 / 121 / 184 M, at -O2 45 / 178 / 193 M, at -Os 45 / 176 / 188 M; about 430 instructions per IR instruction per run in the optimizer at -O2 against llrm's 7,664 (18x). The "220 floor" of [spec] 1.2 is wrong: InstCombine is 4,061, GVN 25,649 and IndVarSimplify about 6 K instructions per IR instruction per run. The gap by role at -O2, M: `fold` plus `algebraic` +290, `gvn` +199, `hoist` +157, `lsr` +149, `decide` +119, `promote` plus `sroa` +116, `jumpthread` +108, `indvars` +60, `dse` +60, `lcssa` +24; sum 1,341 M, the mid-end scoreboard. Owners by row: pass-list-parity (`fold`, `algebraic`), greedy-port (`gvn`, `lsr`), hoist-bounded (`hoist`), keep-analyses (`decide`), attrs-budgets (`promote`, `sroa`, `jumpthread`, `indvars`), edit-rollback (`dse`, `lcssa`).
- **(b) Early:** none (measurement); in progress as [#2056].
- **(c) Gate:** the user decides whether the per-level goal is clang 24's or the static clang 20.1.8's column; the target is clang 24.
- **(d) Exit:** a spec 1.3 table per level, and each pass's budget moved into `tools/budgets.toml`.
- **(e) Who:** attrs-budgets with llvm-compare.

Checklist: the shared list.

| outcome | decision | next |
|---|---|---|
| partial | n/a | |
| no effect | n/a | |
| worse elsewhere | n/a | |

### 4.3 Front end

- **(a) Effect:** 81 M against 33 M (2.5x, 1.7% of the compile; [synthesis] row 8):
  - `stream::parse` 21.3 M;
  - `translate::program` 16.8 M;
  - `mir::emit` 17.4 M ([#1994]).
- **(b) Early:** none until phase 4.
- **(c) Gate:** a design (three IRs where clang builds one).
- **(d) Exit:** M.
- **(e) Who:** UNOWNED. Parked until B3.

Checklist: the shared list.

| outcome | decision | next |
|---|---|---|
| partial / no effect / worse | n/a while parked | |

## 4. Cross-step guards

**Translate, do not paraphrase.** Every LLVM-mirroring pass or file is a translation of LLVM's C++ at `d1106deb71cc`, read from a local checkout and not from memory or docs. The Rust follows the C++ structure: its functions, worklists, caps and thresholds with the `cl::opt` defaults, and the order of the checks. Each function carries a one-line permalink comment. Anything not translated is a `DEVIATE` with the reason and a number in the PR; the PR's `LLVM structure` line names files and lines; review reads the two side by side.

**Translate into idiomatic Rust.** Translating from language A to B is always into idiomatic B, not a straight copy. For LLVM's C++ into Rust: keep what must mirror (the algorithm, the order of checks, the worklists, caps and thresholds with their `cl::opt` defaults, the permalinks, the numbered DEVIATEs), written as Rust: ownership and borrowing instead of raw-pointer emulation, enums plus `match` instead of opcode-switch ladders or kind fields, iterators and closures instead of index loops, `Option`/`Result` instead of null or sentinel values, no getter/setter boilerplate, no C++ class hierarchies emulated with trait objects where an enum fits, Rust naming. Reviews check it; existing translations become idiomatic when their owner next touches the file, without a churn wave; an urgent file goes to oversight.

**Mirror LLVM's structure by default.** Every design page and every structural PR carries a `LLVM structure` line: LLVM's structure (name and permalink at `d1106deb71cc`), ours, and `MIRROR` or `DEVIATE` with the reason. The default is to mirror. A deviation needs a reason (our MIR or LIR differs; output would change and the rows rule applies; measured not better) and a number. The mapping so far:

| LLVM structure | ours | status |
|---|---|---|
| [`SmallVector`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/SmallVector.h#L1251) (inline storage) | `BlockList` ([#1956]); an inline vector across the hot `Vec` sites (step 1.0) | mirrored for successors and predecessors; in flight |
| [`BumpPtrAllocator`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/Allocator.h#L68) | slab allocator ([#2022]) | in flight; DEVIATES in form (a global slab, not a per-compile arena): it cuts the cost of an allocation, not the count |
| [`DenseMap`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/DenseMap.h#L878) and dense id tables | `llrm_mir::dense` `IdSet`, `IdMap`, `Bits` ([#1971], [#2026], [#2032]) | mirrored; the rest of the B-trees are 2.6 and 1.2's remainder |
| [`User` co-allocated operands](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/User.cpp#L137) | `Instruction.operands: Vec<Operand>` | DEVIATES: at most 2.7% of the blocks and 0.2% of instructions (measured, [#2022]) |
| [`SlotIndexes`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/SlotIndexes.h) and [`LiveRangeEdit`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveRangeEdit.h) (in-place spill) | in-place `Vec<Insn>` arena spill (S4 step 1, 3.3) | in flight (design: [allocator on the LIR core](allocator-lir-core.md)) |
| [`AnalysisManager` and `PreservedAnalyses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PassManagerImpl.h#L80) (preserved sets, invalidate) | `Analyses` with keep and invalidate ([analysis-manager]) | partly mirrored ([#1922]); S1 decides how much more |
| [InstCombine worklist](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5780) | `fold`, `algebraic`, `phiopt`, `dead` in rounds | DEVIATES, re-test after 3.1/3.2: a combine at today's positions measured about 0 (S2: the rewriting is 0.30x to 1.03x of InstCombine; the facts are the gap) |
| [`RAGreedy` stage queue](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1975) | `allocate.rs` queue stages | DEVIATES: llrm adds a `recolor` stage greedy lacks (3.32 M per 1x); the decision-procedure page ([#2049]) decides |
| [`CalcSpillWeights`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CalcSpillWeights.cpp#L221) (weight stored in the interval) | `LiveRanges::weight`, about 1 K instructions per call | not mirrored; a candidate of S4's fixed part |
| [`sinkAndCmp0Expression`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2341) | `addresssink::masks_sunk` ([#2010]) | mirrored (merged) |
| [LICM caps and MemorySSA walker caps](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L149) | structural gates of work (chain of 64 accesses, 1,024-instruction body, 8 joins; [#1999] and the stack) | mirrored in effect: counts of work, not a tuned cap |
| [`LastRunTrackingAnalysis`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LastRunTrackingAnalysis.cpp#L30) | the settled mark of `Fixed::fixed` | DEVIATES, re-test after 3.1/3.2: an idle skip nets +0.6% ([spec 8]) |
| [`MachineCopyPropagation`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp) (block-local) | cross-block `copyprop` and `spillforward` | DEVIATES, re-test after 3.1/3.2: block-local saves 14.8 M but grows `d_faces` by 0.36% in bytes (S5, [#2043]) |
| [`EarlyCSE`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1066) (generation counters) | none | DEVIATES, re-test after 3.1/3.2: measured, not kept (-3.1% to -3.4% kill-all, four rows worse; [spec 8]) |
| preserved analyses across `fold`'s `_numbers` | none | DEVIATES, re-test after 3.1/3.2: the premise failed in today's pipeline (a `MemRef` is made of operands, [#2031]) |
| [`X86FPStackifier`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FloatingPoint.cpp#L81) | `floatassign` plus `floatalloc` | DEVIATES: llrm allocates the x87 registers as allocnos (IRA-like); the hand-off is 0.9%, ranks last |

The "re-test after 3.1/3.2" rows were measured as pieces inside today's pipeline, which is not LLVM-shaped; their numbers show they do not pay today and are kept, but the rows are open until the pipeline is one pass with lazy facts.

Decide's branch decisions over the full C corpus (1,580 compiles, 33,932 `decide` runs, 3,002 decisions; indvars-decide-poc): a literal `br i1 true/false` 2,298 and a switch on a constant 160 (together 2,458, 82%, which is `ConstantFoldTerminator` in SimplifyCFG), constant only by the optimistic cycles (SCCP) 244, computed-from-registers constant 6, ranges (CVP, LVI) 186, guards (ConstraintElimination) 108. On the LLVM-order branch the same decisions are still made by `decide` (78,962 runs, 3,043 decisions): no earlier pass folds a literal branch there, so the SimplifyCFG slot owns them. `decide`'s 124.7 M on `d_faces` is the sources that decide nothing (facts asked first 72.8, guards 43.3, constants-cycles 16.0, threading 15.7); `br const` folding is not built in `decide` but translated as `ConstantFoldTerminator` at SimplifyCFG's LLVM slots (78 runs on `d_faces`), with CE, CVP and SCCP as their own passes at LLVM's 11, 22 and 11 slots. The order branch's 78,962 `decide` runs against main's 33,932 go away when SimplifyCFG, SCCP and CVP take the decisions at LLVM's slots.

**Decisions of 2026-10-18 (oversight).** (1) A `noipa` attribute for the Nib bench kernels, named DEVIATE (GCC's `noipa` precedent), honoured by every IPO pass; owners cgscc-poc (IPO and the attribute) and llvm-difftest (the sources; re-bless `crc` and `matmul`). (2) gvn-port ([#2179]): flags in the key until scev-port's S2 flag inference, and phi simplification skipped until `x_hanoi2` and `quicksort` are fixed; both are temporary, numbered DEVIATEs, each with its exit. (3) Peephole: the 8 sub-passes that never fire are not deleted; they go in the home-table work with their replacements' rows. (4) Every oracle or comparison harness asserts its premise that the reference can give more than one answer: a comparison whose reference can only say one answer proves nothing (memoryssa-port's alias analysis with no facts reported "zero differences"). (5) The quiet window of 2026-10-18 at 04:00 failed at a load of 23 (the self-hosted runner building PR gates) and was moved to 05:30 to 06:15, with no pushes from 05:15; the other tmux sessions and the runner are this project's own jobs, so pausing them is within remit; stopping the runner service awaits the user's direct confirmation.

**Quiet-host cycles.** A container or allocation PR needs a quiet-host cycles line before it merges, as the gate line is needed: min of 11, pinned to one core, load under 4, base the parent. The quiet window (announced by the coordinator, for example 04:00 to 04:45 CEST on 2026-10-18; from 5 minutes before it nothing runs on the host: no builds, benches, tests, gate runs, sweeps or `abmeasure`, and a session stops only its own processes): the runner (cgscc-poc) takes the base dist binary, the head dist binary and the command, pins one core, takes the min of 11 alternated runs, records `uptime`, and refuses at a load of 4 or more. Requests reach the runner 15 minutes before the window. No git pushes from 15 minutes before it either: each push starts CI builds on the host (the 04:00 window of 2026-10-18 failed at a load of 23 from the self-hosted runner building PR gates, and was moved to 05:30 to 06:15); check the runner queue first. [#2051] merged on instructions only (-0.55%); [#2069] and [#2077] wait for that run.

**Bundling.** Constant-factor PRs under 0.5% are bundled per area; slopes, soundness and architecture items are separate PRs.

**The quiet window of 2026-10-18 (05:30 to 06:15 CEST; the procedure that worked).** The CI runners were stopped idle-only with the user's direct permission, restarted afterwards and confirmed online; load was 1.0 to 1.4; the measurement ran on core 8, outside the CI cpusets. The A/A control is a copy of the base binary, not the same file twice: a repeated file stays warm in the page cache and fakes +6% to +13%. Results: [#2173] `d_faces` +0.3% to +0.5% cycles for +0.09% to +0.20% instructions; bench inputs of 15 to 60 ms are unreadable (an A/A of identical binaries swings 0.1% to 8.9%); the fold chain is inside the noise; [#2051] post hoc: cycles -0.14% against an A/A of 0.36, instructions -0.277%.

**Quiet-window switch and a finding.** The scheduled perf-on-main workflow honours the repository variable `QUIET_WINDOW=1` (`gh variable set` and `delete`; llvm-difftest's #2191), set by the window owner together with the runner stop. The three `tools/test_budgets.py` failures seen on the #2154 gate (`lir twoaddr`, `lir coalesce`, `isel` and `analysis pressure`) were never green on a measured main: `lir coalesce` is a x2.31 debt (issue #2189).

**Void rows.** `rows/main` and `rows/base` were built with the Ubuntu clang 20.1.8 and are not a reference ([#2226]); `main24` to `main24d` are pinned and valid. Figures read from the void rows: `rows-series.md` (every table against min(gcc, clang)), `compile-gap-series.md` (the rows of `5e2e995ef`), the plan's 0.1 rows box and the `[rows-series]` ablation constraint in the 3.2 text, and the numbers in the PR bodies that quote them (owners redo them on `main24d`).

**Decisions of 2026-10-18, second batch (oversight).**
1. Greedy allocator: the turn-on is parked OFF and the per-row cause work is stopped (quality waits on S2); the mirror stays. Measured: greedy -56.4 M (-2.2%) at `d_faces` -O2 by plain `perf stat`; the old regalloc is 376 M against LLVM's RAGreedy at about 27 to 29 M on the pin. Next: profile greedy's own 143 M per function against RAGreedy; the line stops if it cannot get within about 2x per decision.
2. GVN ([#2179]): about +1% compile is accepted, on the condition of the per-IR-instruction cost per run against LLVM's 25,680.
3. JUMPS step 1: go, faithful, no hotness guard; D1 returns only as a numbered temporary DEVIATE if a row ends above min.
4. SimplifyCFG D5 is accepted as temporary with exits [#2186] and [#2194]; the owner of [#2194] (hoist and rotate order) is llvm-order-poc, mirror: LPM1 runs loop-rotate before LICM.
5. INLINEDECIDE stays off: the +28% to +87% compile is the per-re-run fixed-point steps (llvm-order-poc's 3.2); the walk PR is not opened.
6. TTI: two cost systems are accepted as LLVM's split; the MIR passes migrate to read TTI and `OperationCosts` stays for LIR (the migration is owed in `tti-plan.md`).
7. `noipa`: [#2196] is closed (merged via [#2140]); the parity kernels stay missing until executed; `matmul` is re-blessed, no waiver.
8. The fold split is approved ([#2218]); `KnownFPClass` Stage A only, Stage B waits for a named reader. Front-end item 2 is parked ([#2221]). The perf cron is daily only ([#2191]).
9. `LoopStrengthReduce.cpp` is translated at the pin. Owner loop-products-slope (was loopidiom-lp0 and indvars-decide-poc). It starts after scev-port S3 to S5 (it needs `SCEVExpander` and AddRec). Oracle: LLVM's LSR output. The llrm `lsr` pass and the interim loop-products and pressure slope fix are deleted when it becomes default.
10. [#2260] `lir loopslots` is REPLACED by RAGreedy region splitting (SplitKit). Exit: deleted with the old pass when greedy is default. The acceptance axis (its own slope within the limit at 2N) is on regalloc-core-poc.

**Decisions of 2026-10-18, third batch (oversight).**
1. `BranchProbabilityInfo.cpp` and `BlockFrequencyInfo` / `BlockFrequencyInfoImpl` are translated at the pin `d1106deb71cc`: heuristics plus trip counts from SCEV (`BackedgeTakenInfo`), no loop simulation. Owner cgscc-poc (the inliner and hot/cold consumer, a TTI neighbour). Order: after scev-port S5 (BTC). The llrm `branchprob` `counted` simulation and `Frequencies` are deleted when it becomes default.
2. The exit of the `analysis counted` row ([#2260], x5.43 nest, unplaced until now) is that translation plus `BackedgeTakenInfo` (and the LSR translation's `BackedgeCounts`): the cone dependence comes from `branchprob` simulating loops.
3. `SwitchLoweringUtils` is translated at the pin, behind a switch. Oracle: its cluster, jump-table and bit-test decisions against the pinned `llc`. Every row is priced against min(gcc, pin) before any default change; where gcc's shape wins rows it stays as a numbered DEVIATE citing those rows. Owner backend-slopes, whose `mir switchlower` slope fix ([#2272]) lands first. Order: after the critical-path items (3.1 to 3.3, the allocator, GVN, LICM, SCEV). It is a code-shape trade, not a compile-time win.
4. `LoopSimplify.cpp` and `LCSSA.cpp` (with the `LoopUtils` pieces they use) are translated at the pin `d1106deb71cc`. Owner loop-passes-port, ordered after the DomTree (domtree-port) and LoopInfo translations. llrm's `loopsimplify` and `lcssa` are deleted when it becomes default. Interim: backend-slopes' one-sweep needs-check fix, deleted with the old pass.
5. Pipeline row, owner llvm-order-poc: LLVM's new pass manager runs LoopSimplify and LCSSA only where a loop-pass adaptor needs them (a handful of times at -O2); ours runs them 9 times. The run count must match the pinned pipeline's.
6. `LazyValueInfo.cpp` and `CorrelatedValuePropagation.cpp` are translated at the pin `d1106deb71cc`. LLVM -O2 runs CVP, and JumpThreading reads LVI, so it is the real counterpart of `ranger.rs`'s uses there. Owner vt-constantfolding (valuetracking-port, which translated `ConstantRange` and `ValueTracking`), ordered after the DomTree stack lands (domtree-port). llrm's `ranger.rs` uses are deleted when it becomes default.
7. `JumpThreading.cpp` is translated after it. JumpThreading is our worst ratio (26.33x in the per-pass table; 28.8x by oversight's reading). Owner decided when the LVI row lands.
8. Glue (projection, 108 M on `d_faces`): `Stamp`, `Summaries` and the interprocedural stage. Owner cgscc-poc (the plan page is theirs). Each is replaced by LLVM's mechanism or deleted.
9. Emission and `masm` (21 M; slot 64): owner backend-slopes, ranked last until the split below. Not one pass (backend-slopes; files checked to exist at the pin):
   - Frame sequences and return cleanup (`masm` listing, shrinkwrap, cleaned returns, duplicated returns): LLVM does them before the AsmPrinter, in `PrologEpilogInserter.cpp`, `ShrinkWrap.cpp`, `X86FrameLowering.cpp` (`hasFPImpl`, `emitPrologue`, `emitEpilogue`) and X86ExpandPseudo. Translation, one row with the `hasFP` row (slot 65); part of its cost is counted under the LIR clean-up, not emission.
   - Encode, branch relaxation, relocations: `X86MCInstLower.cpp`, `X86MCCodeEmitter.cpp`, `MCObjectStreamer.cpp`, `MCAssembler.cpp`. Translation, DEVIATE: llrm relaxes a flat item list at one fixed point, LLVM uses MCFragments; the `_trampolined` size step has no LLVM counterpart and stays a numbered DEVIATE.
   - Object writers: ELF, COFF and MachO have counterparts; OMF: LLVM has no such pass (`llvm/lib/MC` has no OMF writer), llrm keeps it.
   - `masm::text` (-S and the BASIC driver): AsmPrinter and `X86IntelInstPrinter`; LLVM's object path never prints, so the C and Nib object path has no printing step to translate.
   - Split measured (backend-slopes; `d_faces` -O2, default m16, `CARGO_INCREMENTAL=0` release of main, `LLRM_DEBUG=time` own instructions; the m32 i486 compile fails in the Watcom front end there, so it is not used): total about 22.5 M. Object write 15.25 (inclusive of `masm::listing`; callgrind 14.67 inside `built_inner`), `assemble` own 3.41, shrink-wrap tails 1.75, `masm` return overhead 0.74, duplicated returns 0.49, data layout 0.24, stack checks 0.18, cleaned returns 0.17, `mir` near code 0.14, `lir` frame 0.12, debug info 0.06. Inside object write: `masm::listing` 6.89 (frame sequences; `shrinkwrap::wrapped` 3.02 and `saved_of` 1.76, recomputed per call), `_items` / `_encoded` 5.09 (`select::emit_in` 3.88), relax, `object_of` and OMF write about 2.7. By counterpart: frame sequences and return glue about 10 M (PEI, ShrinkWrap, `X86FrameLowering`; one row with `hasFP`), MC encode about 5 M (`X86MCCodeEmitter`), relax and write about 2.7 M, assemble glue about 3.4 M. Order: frame sequences first, MC encode second; OMF write stays.
10. The C translate stage (`frontend translate`): owner hir-branch-icmp-2211 (frontend-aggregates); GO for step 1 (coordinator). Instrument: callgrind, inclusive, `d_faces` -O1, main plus [#2211]: 40.7 M of 1,945 M, the sum of `stream::parse` 17.9 (self 9.8; the rest allocation: a `String` per token, an `IndexMap` per line, `tokens.remove(0)`), `translate::program` 15.4, `hir::unit` 5.3 and drops 1.7; 6,231 records; in line with the 42.0 M of `fold-hoist-decide.md`. The projection's ~29 M was not a stage figure: it is one recursive row (`Body::eval`, 28.7 M inclusive, `d_faces` -O2), overlapping `translate::program`, without parse; withdrawn (llvm-difftest). Counterparts at the pin: `program` and `unit` follow `CodeGenFunction::EmitStmt`, `EmitScalarExpr` and `EmitLValue` (fed by Open Watcom's call tree, not an AST); `stream::parse` corresponds to clang's Lexer, Preprocessor and Parser (`clang/lib/Lex`, `clang/lib/Parse`): no middle-end pass, but a front-end counterpart, and it exists as a separate step because the Watcom front end and llrm are two processes joined by a text stream. Order: (1) `stream::parse` first, no dependency (borrowed tokens, one arena, no per-line map; 17.9 to 4 M or less, linear already; test: slope on `straight`); (2) `program` and `unit` after it (clang's whole cc1 front end is 33 M against our 81 M); (3) dropping the text stream is the structural fix, nobody's yet.
11. The scoreboard is the projection (llvm-difftest's table), refreshed half-hourly.

**Method M (every exit measurement).**

1. **Binaries.** The base is the PR's first parent (`git rev-parse <head>^1`), never main of another day and never the merge-base after main moved. Build both with `cargo build --release --bins` in separate worktrees on the perf host. Check each binary's mtime is after its commit, and record both SHAs in the PR.
2. **Instructions.** `valgrind --tool=callgrind` and `perf stat -e instructions:u` (min of 3) on `d_faces` -O1/-O2/-Os, flags `-m32 -march=i486 -fno-inline-functions`. For lru, bintree, queens and sieve at four levels, use `tools/compile-gap.py`.
3. **Cycles.** `perf stat -e cycles:u`, min of 11, alternating base and head, pinned to one core, `uptime` recorded. A cycle figure is claimed only at a load average below 4. Otherwise it reads "not claimed", and the step's cycle box stays open until a quiet run.
4. **Slopes.** `tools/ladder/ladder.py` and `fit.py` for both binaries, every shape, N=25 to 1600; `scaling_gate.py` 2N/N per axis.
5. **Output.** Run `tools/identical.py BASE HEAD` with `QCPORT` and `QCPORT_INC` set; the corpus is 1,583 objects with every `LLRM_CHECK_*` on, and a smaller corpus is a failure. For output-changing steps, also run:
   - `tools/rows/run.sh`, `analyse.py` and `qcport.py`; a rows report leads with the number of rows worse than base for each metric (clocks, executed instructions, memory operands, code bytes) at each level, then the geomean (#2110 was reported "geomean flat or better, worst rows unchanged or better" while 16 clocks, 16 instructions, 22 memory-operand and 26 code-byte rows were worse); a summary refuses a failed or missing row and prints "N rows failed" first ([#2145]);
   - `tools/bench/bench.py --opt O1 O2 O3 Os --no-judge --json` on both sides, then `bench_table.py`.
6. **Memory.** `/usr/bin/time -v` max RSS on `d_faces` -O2 and `mdl_ai` -O2; DHAT peak where the step claims memory.
7. **Gate.** The CI line `GATE <tier> PASS at <head12>` must name the PR's head; premerge OK on main.
8. **Environment.** An A/B where any `LLRM_CHECK_*` or `LLRM_EXP_*` variable differs between the two runs is invalid; only the switch under test is added, with the same value on every row. `tools/envguard.py` (#2109) refuses such a run, and A/B deltas are quoted from `tools/abmeasure.py` only after it can no longer count a failed compile (until then, run the dist binaries by hand and assert the object exists and the count is in range).
9. **Input.** The `d_faces` source is a measurement input: record its tree and checksum in every figure, the same tree for every row of a table; keep the floor check (a count near 8 M is a failed compile). A dist binary embeds the path of its own the Open Watcom front end, so its target directory stays until it has been measured.
10. **Floats.** x87 excess precision stays fast by default, like gcc; a float row against clang is measured with llrm at `-fexcess-precision=standard`, and each row records the flag.
11. **Pushes.** Never rewrite a pushed commit of a PR that is open; kill only the processes a session started.

**Translate first (user, 2026-10-18).** Before implementing anything, check whether LLVM has that pass or analysis at the pin `d1106deb71cc`; if it does, translate it, with the changes we need. Keep the result as close to O(n) as possible. The PR description names the LLVM source file and function translated (permalink), or states "LLVM has no such pass" and why. Any super-linear step needs a stated reason and a slope test. A fix to our own pass that has an LLVM counterpart is a translation row first.

**The integration branch `feat/parity` (user decision, 2026-10-18).** LLVM-parity work lands on `feat/parity`, cut from main, not on main.

- Every parity PR (translations, joins, migrations, block-list stages, greedy) targets `feat/parity`; a stacked PR keeps its stack base.
- Merge rule: a ready (non-draft) PR is merged as soon as it builds merged with the branch (`cargo check -D warnings`) and passes the per-merge tests, total budget 10 s (build, a curated smoke set, the ratchet tests). No perf gate, no rows rule, no cap on open PRs, no CI wait. Fix forward; each fix carries its fail-first regression test.
- `LLRM_EXP_LLVM` is on by default on the branch; a join flips its slot there directly.
- Main keeps the full gate and the rows rule. Main is merged into the branch daily, no rebase. Fixes main also needs (miscompiles, instrument fixes) go to main as their own PRs.
- Scoreboard: `d_faces` and three programs on the branch head against main and clang (dist, one method) every half hour; rows once a day on the branch.
- Back to main in phases: the mid-end when its buckets and rows hold, then the backend. Each phase is one PR to main with the full gate, the rows rule and the old code deleted.

**Parity slot and join step (user, 2026-10-18).** Every port PR names its parity slot (section 2.5) and its join step. A translation is done when it has joined the parity pipeline with its oracle passing, not when it merges off. A PR that joins a translated slot outranks a new translation, and no new translation starts while one of the owner's is translated but not joined.

**Pass rules shared by every step:**

- identical.py 0 differ, or [spec] 4.3's landing rule for output-changing steps;
- no ladder exponent up by 0.02 or more;
- no cost-gate shape row more than 0.15% above base;
- cycles no worse than +1% (when claimed);
- RSS within the step's limit.

**When an intended effect is missed:** stop, run the step's first diagnostic, and record the result.

- Below 0.5%: a [spec 8] entry with the numbers and the branch under `notkept/`.
- Then re-rank the phase's remaining steps by the newest table before the next one starts.

**Output never worse.** Steps marked identical must show 0 differences over 1,583 objects. Steps that change output go by the [spec] 4.3 rule:

- no row with more executed instructions or memory operands;
- no -Os row with more bytes;
- no row at or below min(gcc, clang) moving above it.

These apply at -O1/-O2/-O3/-Os, m16 and m32. A row is never traded for compile time below -Omax.

**The 0.5% bar and bundling.** A PR whose effect is below 0.5% of `d_faces` -O2 lands only:

- bundled with same-area changes that clear the bar together; or
- if it moves a ladder exponent down by 0.05 or more at 0.15% or less on `d_faces`; or
- if it fixes correctness.

**Non-incremental builds on both sides.** A figure taken with `CARGO_INCREMENTAL=0` on both binaries (the CI mode) is the one this plan cites; an incremental-mode figure is re-checked against that rule before it is relied on, and the earlier incremental figures of this plan are not yet re-checked.

**Noise floor.** Even clean builds carry about ±0.15% total-instruction noise for any unrelated edit (inlining shifts; callgrind-attributed on [#1948], the counters' own code is 0.03% of a +0.13% total). A delta under about 0.2% is not a claim in either mode; a claim under 1% cites callgrind own-function sums, with the unrelated-shift line reported.

**Cycles, not just instructions.** An allocation or container change claims cycles, or it does not claim a win. The mimalloc entry points moved 2.3% of instructions and 0% of cycles ([spec 8]).

**Doubt the measurement.** Errors this project has made, and the rule each left:

| error | what it read | rule |
|---|---|---|
| base binary from a different tree than the PR's parent | early CSE -7.3% (true -3.1%, then +1.6% with the phi fold) ([#1994]); a fake +2% on dse (session report) | base = parent, both SHAs recorded |
| a probe that removed more than the thing under test | escape structure 43 M (true 6.6 M) ([#2033]) | a probe changes one thing; verify against the raw profile |
| an experiment left in the working tree | promote-before-gvn contaminated other numbers (`unmeasured` in the repo, session report) | probes on throwaway branches only |
| a 10-module QCport copy | hid output differences; identity checked on 66 files, not 131 ([identical.py][identical]) | the full 1,583-object corpus; a smaller corpus fails |
| DOSBox slice counting | +1 instruction on scroll bq32 was a REP/runtime-link artifact (session report; bench waiver) | a one-instruction row change needs the raw trace |
| load-dependent cycles | "IPC 40% higher" retracted; 4.8x and 6.4x from the same pair ([readme]) | cycles claimed only on a quiet host, pinned, min of 11 |
| allocator retention | RSS 82 MB with mimalloc, 44 MB with the system allocator ([readme]) | RSS compared on the same allocator; DHAT for live heap |
| an average cost per build | rebuilds "191 M" were 20x overstated ([spec 8]) | count the builds actually skipped, not count × mean |
| a scope's whole cost read as the lever | hoist "3.1%" was 0.8% ([decision-dependencies]) | measure the repeated part, not the scope |
| the IR shape read without the target | 100 selects looked like no diamonds; i486 expands them to branches ([#2036]) | read the emitted code, not the IR |
| instructions without cycles | mimalloc entry points ([spec 8]) | see above |

## 5. Parallelism and critical path

**Critical path:** 0.1 → 0.2 → (phase 1 merges) → B1 → 3.1 (design, then one family per PR) → 3.2 (round drop) → B3 → 4.1.

The back-end track runs beside it from B1: 2.4 (A' alone) → 3.3 → 3.4 → 3.5. Spec 7 notes these touch disjoint crates.

| can run together | blocked by |
|---|---|
| all of phase 1 (1.6 and 1.7 serialised on `avail.rs`) | 0.1 |
| 2.1b, 2.2, 2.4 (A'), 2.6, 2.7, 2.8, 2.9 | B1; 2.2 after [#1943]; 2.4 after [#1958] |
| 2.3 | 1.6 merged (`memoryssa.rs`) |
| 3.1 families; 3.3 | 3.1: the early probe and design review; 3.3: its early probe and design review (neither waits for B1 or B2) |
| 3.2 per-pass ablation, then the round drop | B1 for the ablation; 3.1 for the round drop |
| 3.4 | 3.3 |
| 3.5 | 3.3 and 2.7 |

**File collisions to serialise:**

- `memoryssa.rs`: greedy-port and hoist-bounded;
- `avail.rs`: greedy-port and edit-rollback;
- `manager.rs`: keep-analyses, and edit-rollback for 2.2;
- `gvn.rs` and `hoist.rs`: edit-rollback 2.7 before 3.5;
- `pipeline.rs`: pass-list-parity only.

**In-flight non-PR work, if the user chooses to focus:**

| work | owner | recommendation | reason |
|---|---|---|---|
| point_values demand counts per consumer | keep-analyses | done | 2.1 stops; 2.1b fixes the share key |
| lazy call-effects and `captured_before` design | keep-analyses | cancel | dropped after the walk fusion; the escape side left is about 34 M |
| cheap-many redundancy sizing | edit-rollback | close | answered: 1.4%, largest cause 0.4% ([#1994]) |
| Annotated update from the record | edit-rollback, keep-analyses | keep | it is 2.2 |
| per-(slot, candidate) `clobbered` memo | greedy-port | cancel | [#2034] replaced it; the next lead is gvn `subexpressions` for equal loads whose `MemRef`s differ by pointer value |
| split path B', A' | greedy-port | B' killed, A' alone | B' built: -0.03% |
| split carving constants | greedy-port | cancel | the carving was spill pressure that [#2010] removes |
| interval recompute | ssa-spill-m16 | close | recorded in spec 8; buffer reuse folded into the next intervals PR |
| `Frequencies` dense map | attrs-budgets | keep | it is 2.6, small |
| idle-run work (hoist and decide idle facts) | hoist-bounded | pause | idle cost is the facts (3.1); move to 2.3 and 2.8 |
| LLVM pages beyond the re-baselines | llvm-compare | pause | refresh [proposals] and [synthesis] at B1 to B3 only |
| pass-list-parity after [#2022] | pass-list-parity | start the 3.2 design | on the critical path |
| [#1938] loopexit turns | UNOWNED | pause | only pays with [#1933]'s order, which is closed |
| [#1918], [#1919], [#1920] layout and x87 cells | UNOWNED | pause | output work, not compile time |
| [#1893], [#1899], [#1906], [#1924], [#1925], [#1928] LIR names and text | UNOWNED | pause | debugging aids |
| [#1894] spiller survivors, [#1897] alloc judge | UNOWNED | pause | route B is closed to -Omax ([spec] 4.2) |
| [#1917] stale waivers | UNOWNED | keep | trivial, unblocks bench noise |
| [#1982] order check | keep-analyses | keep | the oracle for 1.9 and 3.1 |
| `ObjectRef` identity oracle | objectref-identity | keep | step 0.3, answered: an oracle PR |

## 6. Timeline

| phase | starts when | exits when |
|---|---|---|
| 0 instruments | now | 0.1's table committed; 0.2's counters in CI |
| 1 land the measured | 0.1 | every 1.x checked or closed; B1 within 2 points of -8.3% (ready set) |
| 2 open levers | B1 | each 2.x landed or recorded in [spec 8]; B2 within 2 points of its row |
| 3 structural | now, in parallel with phases 1 and 2; each step by its early probe and design review | computations per run 0.5 or fewer; one round at -O1 to -Os with rows held; `regalloc` 150 M or less; clean-up 160 M or less; B3 within 3 points of about 1,460 M |
| 4 per-pass cost | B3 | every pass within its counted budget; budgets re-derived from clang (4.2); `d_faces` at each level at clang's column or with a named remainder |

[synthesis]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/synthesis.md
[proposals]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/proposals.md
[readme]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/readme.md
[measured]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/measured.md
[analysis-manager]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/analysis-manager.md
[memoryssa]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/memoryssa.md
[regalloc]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/regalloc.md
[allocation]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/allocation.md
[idle]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/idle.md
[lir-cleanup]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/lir-cleanup.md
[fold-hoist-decide]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/comparison/llvm/fold-hoist-decide.md
[spec]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/architecture/compile-time-spec.md
[spec 8]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/architecture/compile-time-spec.md#8-tried-and-not-kept
[spec 9]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/architecture/compile-time-spec.md#9-container-profile
[compile-gap-series]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/architecture/compile-gap-series.md
[rows-series]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/architecture/rows-series.md
[analysis-avoidance]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/architecture/analysis-avoidance.md
[decision-dependencies]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/architecture/decision-dependencies.md
[pass-parity]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/docs/architecture/pass-parity.md
[identical]: https://github.com/ali-mosavian/llrm/blob/ef40f30a7fc4a06e2693bd1c6b0d69bee9a19446/tools/identical.py
[parity]: https://github.com/ali-mosavian/llrm/blob/d61e9edd3c5d3300dc8e9e9475ba1950be91f12a/docs/architecture/llvm-parity-redesign.md
[#1994]: https://github.com/ali-mosavian/llrm/issues/1994
[#1909]: https://github.com/ali-mosavian/llrm/issues/1909
[#1309]: https://github.com/ali-mosavian/llrm/issues/1309
[#1883]: https://github.com/ali-mosavian/llrm/pull/1883
[#1893]: https://github.com/ali-mosavian/llrm/pull/1893
[#1894]: https://github.com/ali-mosavian/llrm/pull/1894
[#1897]: https://github.com/ali-mosavian/llrm/pull/1897
[#1899]: https://github.com/ali-mosavian/llrm/pull/1899
[#1906]: https://github.com/ali-mosavian/llrm/pull/1906
[#1912]: https://github.com/ali-mosavian/llrm/pull/1912
[#1913]: https://github.com/ali-mosavian/llrm/pull/1913
[#1915]: https://github.com/ali-mosavian/llrm/pull/1915
[#1917]: https://github.com/ali-mosavian/llrm/pull/1917
[#1918]: https://github.com/ali-mosavian/llrm/pull/1918
[#1919]: https://github.com/ali-mosavian/llrm/pull/1919
[#1920]: https://github.com/ali-mosavian/llrm/pull/1920
[#1921]: https://github.com/ali-mosavian/llrm/pull/1921
[#1922]: https://github.com/ali-mosavian/llrm/pull/1922
[#1924]: https://github.com/ali-mosavian/llrm/pull/1924
[#1925]: https://github.com/ali-mosavian/llrm/pull/1925
[#1926]: https://github.com/ali-mosavian/llrm/pull/1926
[#1927]: https://github.com/ali-mosavian/llrm/pull/1927
[#1928]: https://github.com/ali-mosavian/llrm/pull/1928
[#1931]: https://github.com/ali-mosavian/llrm/pull/1931
[#1933]: https://github.com/ali-mosavian/llrm/pull/1933
[#1934]: https://github.com/ali-mosavian/llrm/pull/1934
[#1936]: https://github.com/ali-mosavian/llrm/pull/1936
[#1938]: https://github.com/ali-mosavian/llrm/pull/1938
[#1943]: https://github.com/ali-mosavian/llrm/pull/1943
[#1945]: https://github.com/ali-mosavian/llrm/pull/1945
[#1948]: https://github.com/ali-mosavian/llrm/pull/1948
[#1953]: https://github.com/ali-mosavian/llrm/pull/1953
[#1954]: https://github.com/ali-mosavian/llrm/pull/1954
[#1955]: https://github.com/ali-mosavian/llrm/pull/1955
[#1956]: https://github.com/ali-mosavian/llrm/pull/1956
[#1957]: https://github.com/ali-mosavian/llrm/pull/1957
[#1958]: https://github.com/ali-mosavian/llrm/pull/1958
[#1959]: https://github.com/ali-mosavian/llrm/pull/1959
[#1960]: https://github.com/ali-mosavian/llrm/pull/1960
[#1961]: https://github.com/ali-mosavian/llrm/pull/1961
[#1962]: https://github.com/ali-mosavian/llrm/pull/1962
[#1963]: https://github.com/ali-mosavian/llrm/pull/1963
[#1964]: https://github.com/ali-mosavian/llrm/pull/1964
[#1965]: https://github.com/ali-mosavian/llrm/pull/1965
[#1968]: https://github.com/ali-mosavian/llrm/pull/1968
[#1970]: https://github.com/ali-mosavian/llrm/pull/1970
[#1971]: https://github.com/ali-mosavian/llrm/pull/1971
[#1972]: https://github.com/ali-mosavian/llrm/pull/1972
[#1973]: https://github.com/ali-mosavian/llrm/pull/1973
[#1975]: https://github.com/ali-mosavian/llrm/pull/1975
[#1977]: https://github.com/ali-mosavian/llrm/pull/1977
[#1978]: https://github.com/ali-mosavian/llrm/pull/1978
[#1982]: https://github.com/ali-mosavian/llrm/pull/1982
[#1984]: https://github.com/ali-mosavian/llrm/pull/1984
[#1988]: https://github.com/ali-mosavian/llrm/pull/1988
[#1992]: https://github.com/ali-mosavian/llrm/pull/1992
[#1998]: https://github.com/ali-mosavian/llrm/pull/1998
[#1999]: https://github.com/ali-mosavian/llrm/pull/1999
[#2007]: https://github.com/ali-mosavian/llrm/pull/2007
[#2008]: https://github.com/ali-mosavian/llrm/pull/2008
[#2010]: https://github.com/ali-mosavian/llrm/pull/2010
[#2011]: https://github.com/ali-mosavian/llrm/pull/2011
[#2012]: https://github.com/ali-mosavian/llrm/pull/2012
[#2013]: https://github.com/ali-mosavian/llrm/pull/2013
[#2015]: https://github.com/ali-mosavian/llrm/pull/2015
[#2017]: https://github.com/ali-mosavian/llrm/pull/2017
[#2020]: https://github.com/ali-mosavian/llrm/pull/2020
[#2021]: https://github.com/ali-mosavian/llrm/pull/2021
[#2022]: https://github.com/ali-mosavian/llrm/pull/2022
[#2026]: https://github.com/ali-mosavian/llrm/pull/2026
[#2027]: https://github.com/ali-mosavian/llrm/pull/2027
[#2028]: https://github.com/ali-mosavian/llrm/pull/2028
[#2030]: https://github.com/ali-mosavian/llrm/pull/2030
[#2031]: https://github.com/ali-mosavian/llrm/pull/2031
[#2032]: https://github.com/ali-mosavian/llrm/pull/2032
[#2033]: https://github.com/ali-mosavian/llrm/pull/2033
[#2034]: https://github.com/ali-mosavian/llrm/pull/2034
[#2035]: https://github.com/ali-mosavian/llrm/pull/2035
[#2036]: https://github.com/ali-mosavian/llrm/pull/2036
[#2037]: https://github.com/ali-mosavian/llrm/pull/2037
[#2038]: https://github.com/ali-mosavian/llrm/pull/2038
[#2039]: https://github.com/ali-mosavian/llrm/pull/2039
[#2042]: https://github.com/ali-mosavian/llrm/pull/2042
[#2053]: https://github.com/ali-mosavian/llrm/pull/2053
[#2054]: https://github.com/ali-mosavian/llrm/pull/2054
[#2056]: https://github.com/ali-mosavian/llrm/pull/2056
[#2041]: https://github.com/ali-mosavian/llrm/pull/2041
[#2072]: https://github.com/ali-mosavian/llrm/pull/2072
[#2067]: https://github.com/ali-mosavian/llrm/pull/2067
[#2051]: https://github.com/ali-mosavian/llrm/pull/2051
[#2069]: https://github.com/ali-mosavian/llrm/pull/2069
[#2073]: https://github.com/ali-mosavian/llrm/pull/2073
[#2125]: https://github.com/ali-mosavian/llrm/pull/2125
[#2132]: https://github.com/ali-mosavian/llrm/pull/2132
[#2145]: https://github.com/ali-mosavian/llrm/pull/2145
[#2150]: https://github.com/ali-mosavian/llrm/pull/2150
[#2210]: https://github.com/ali-mosavian/llrm/pull/2210
[#2093]: https://github.com/ali-mosavian/llrm/pull/2093
[#2223]: https://github.com/ali-mosavian/llrm/pull/2223
[#2135]: https://github.com/ali-mosavian/llrm/pull/2135
[#2155]: https://github.com/ali-mosavian/llrm/pull/2155
[#2195]: https://github.com/ali-mosavian/llrm/pull/2195
[#2140]: https://github.com/ali-mosavian/llrm/pull/2140
[#2186]: https://github.com/ali-mosavian/llrm/pull/2186
[#2191]: https://github.com/ali-mosavian/llrm/pull/2191
[#2194]: https://github.com/ali-mosavian/llrm/pull/2194
[#2226]: https://github.com/ali-mosavian/llrm/pull/2226
[#2196]: https://github.com/ali-mosavian/llrm/pull/2196
[#2218]: https://github.com/ali-mosavian/llrm/pull/2218
[#2221]: https://github.com/ali-mosavian/llrm/pull/2221
[#2260]: https://github.com/ali-mosavian/llrm/issues/2260
[#2274]: https://github.com/ali-mosavian/llrm/pull/2274
[#2272]: https://github.com/ali-mosavian/llrm/pull/2272
[#2173]: https://github.com/ali-mosavian/llrm/pull/2173
[#2179]: https://github.com/ali-mosavian/llrm/pull/2179
[#2151]: https://github.com/ali-mosavian/llrm/pull/2151
[#2134]: https://github.com/ali-mosavian/llrm/pull/2134
[#2094]: https://github.com/ali-mosavian/llrm/pull/2094
[#2110]: https://github.com/ali-mosavian/llrm/pull/2110
[#2111]: https://github.com/ali-mosavian/llrm/pull/2111
[#2112]: https://github.com/ali-mosavian/llrm/pull/2112
[#2113]: https://github.com/ali-mosavian/llrm/pull/2113
[#2076]: https://github.com/ali-mosavian/llrm/pull/2076
[#2100]: https://github.com/ali-mosavian/llrm/pull/2100
[#2106]: https://github.com/ali-mosavian/llrm/pull/2106
[#2119]: https://github.com/ali-mosavian/llrm/pull/2119
[#2122]: https://github.com/ali-mosavian/llrm/pull/2122
[#2068]: https://github.com/ali-mosavian/llrm/pull/2068
[#2077]: https://github.com/ali-mosavian/llrm/pull/2077
[#2083]: https://github.com/ali-mosavian/llrm/pull/2083
[#2074]: https://github.com/ali-mosavian/llrm/pull/2074
[#2046]: https://github.com/ali-mosavian/llrm/pull/2046
[#2049]: https://github.com/ali-mosavian/llrm/pull/2049
[#2043]: https://github.com/ali-mosavian/llrm/pull/2043
[#2862]: https://github.com/ali-mosavian/llrm/pull/2862
[#2867]: https://github.com/ali-mosavian/llrm/pull/2867
