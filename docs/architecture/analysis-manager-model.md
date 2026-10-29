---
architecture-index: the analysis-manager model: invalidation rules, audit and per-analysis PRs
architecture-order: 335
---
# The analysis-manager model

TL;DR: one compile of d_faces `-O2` asks the manager 365 (pass, analysis) miss rows and builds a dominator tree 684 times, 516 of them outside it. The five `fold@N` slots show about 116 M of analyses beneath them, which is nested billing (an analysis is charged to its first asker), not waste. The waste is the equal-result rebuilds counted by basicaa-port-2163: 35 of 82 `call-effects` builds, 33 of 90 `annotated`, 7 of 10 `globals-aa`, and 0 of 48 `through-memory`. The pin keeps what a pass leaves true; we drop what a pass does not name, and 26 of our pass files return `none()` on change. This page fixes one set of invalidation rules and the order of the per-analysis PRs. "Measured" is a counter or a profile run; "inferred" is arithmetic on those.

Context: [LLVM's analysis manager against llrm's `Analyses`](../comparison/llvm/analysis-manager.md) measures the cost of the drop step; this page is the rules and the plan.

## 1. The pin's model

Pin `d1106deb71cc`. Source paths below are relative to `https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/`.

| mechanism | what it does | source (path#lines) |
|---|---|---|
| `PreservedAnalyses` | `none()`, `all()`, `preserve<A>()`, `preserveSet<S>()` (a set such as `CFGAnalyses`), `abandon<A>()` (not preserved even if a set or `all` covers it) | llvm/include/llvm/IR/Analysis.h#L112 |
| `PreservedAnalysisChecker` | `preserved()`, `preservedSet<S>()`; both false for an abandoned analysis; `preservedWhenStateless()` is only "not abandoned" | Analysis.h#L233-L262 |
| `allAnalysesInSetPreserved` | true if nothing was abandoned and `all` or the set is preserved; `invalidate` returns at once on `AllAnalysesOn<IR>` | Analysis.h#L300-L309 |
| `getResult` / `getCachedResult` | compute on first ask and cache; the second never computes | llvm/include/llvm/IR/PassManager.h#L436, #L455 |
| `Invalidator` | a result's `invalidate(IR, PA, Inv)` asks `Inv.invalidate<Dep>` for each analysis it is built on; one answer per analysis per round, a cycle is an error | PassManager.h#L318-L350 |
| default `invalidate` | a result with no `invalidate` method dies unless its analysis or `AllAnalysesOn<IR>` is preserved | llvm/include/llvm/IR/PassManagerInternal.h#L250-L257 |
| `invalidate(IR, PA)` | per IR unit; walks the results, each decides with the dependents it asks about | llvm/include/llvm/IR/PassManagerImpl.h#L169-L207 |
| `clear`, `registerPass` | `clear(IR)` deletes without invalidating (IR removed); `registerPass` binds an analysis | PassManager.h#L414-L422, #L497 |
| the loop | `PassPA = Pass->run`, then `AM.invalidate(IR, PassPA)`, then `PI.runAfterPass`, then `PA.intersect(PassPA)` | PassManagerImpl.h#L72-L90 |
| function inside module | `FunctionAnalysisManagerModuleProxy` (an `InnerAnalysisManagerProxy`) clears every function result unless the module pass preserved the proxy, else asks each function's `PA` | PassManager.h#L698; llvm/lib/IR/PassManager.cpp#L53-L100 |
| module from function | `OuterAnalysisManagerProxy`: a function analysis reads only the module results already cached, and registers the dependency so a module invalidation reaches it | PassManager.h#L741-L800 |
| CGSCC | `CGSCCAnalysisManagerModuleProxy`, `FunctionAnalysisManagerCGSCCProxy`, `CGSCCAnalysisManagerFunctionProxy` | llvm/include/llvm/Analysis/CGSCCPassManager.h#L168, #L372-L409 |

What the results do with it:

| result | dies when | source |
|---|---|---|
| `DominatorTree` | not preserved, not `AllAnalysesOn<Function>`, not `CFGAnalyses` | llvm/lib/IR/Dominators.cpp#L71 |
| `AssumptionCache` | never (self-updating) | llvm/include/llvm/Analysis/AssumptionCache.h#L118 |
| `AAResults`, `BasicAAResult` | stateless; die only if `AssumptionCache`, `DominatorTree` or `TargetLibraryInfo` die | llvm/lib/Analysis/AliasAnalysis.cpp#L80, BasicAliasAnalysis.cpp#L83 |
| `MemorySSA` | not preserved, or `AAManager` or `DominatorTree` died | llvm/lib/Analysis/MemorySSA.cpp#L2376 |
| `MemoryDependenceResults` | not preserved, or `AAManager`, `AssumptionCache` or `DominatorTree` died | MemoryDependenceAnalysis.cpp#L1789 |
| `LazyValueInfo` | not preserved | LazyValueInfo.cpp#L1987 |
| `ScalarEvolution` | not preserved, or `AssumptionCache`, `DominatorTree` or `LoopInfo` died | ScalarEvolution.cpp#L15117 |
| `GlobalsAAResult` (module) | only when abandoned | GlobalsModRef.cpp#L825 |

The pin keeps no whole-function map of alias, call-effect or memory-location facts. They are answered per query inside the stateless `AAResults`, so there is nothing to rebuild.

## 2. What we have

`PreservedAnalyses` (llrm-mir `passes.rs#L737`, permalink at 21aadae64a91a16912ee0dc54355248a77a9492d) has `all`, `none`, `preserve<A>`, `preserve_updated<A>` (the pass put its updated result back with `Analyses::replace`, #L1580), `preserve_module<M>`, `preserve_cfg`. There is no `abandon`. `Analysis` (#L303) has `NAME`, `READS_OUTER` (the proxy: reads the module), `INCREMENTAL` (`update` from the change log), `SKIPS`/`SOUND`/`NAMES_VALUES`, `depends()` (a declarative `Result::invalidate` against the change log, `depends.rs`) and `preserved()`. Module analyses are held with `hold::<M>()` (#L2449; GlobalsAA's `GlobalsAAResult::invalidate`) or `freeze::<M>()`.

Where we deviate (by grep and reading; counts are measured in section 3):

| deviation | evidence | pin |
|---|---|---|
| Only `Dominators`, `Loops` and `Shape` override `preserved()` to accept the CFG set. `preserve_cfg()` is called by no pass. 26 pass files spell the set as `.preserve::<Dominators>().preserve::<Loops>()` (missing `Shape`) | `fn preserved(` in `passes.rs` and `cfg.rs` only | every CFG reader honours `CFGAnalyses` |
| 26 pass files return `none()` on change (list in section 4); `Combine` returns `none().preserve::<LastRunTracking>()` | `PreservedAnalyses::none()` returns in `crates/opt/llrm-transforms/src` | InstCombine, SROA, Mem2Reg, MemCpyOpt, DSE, EarlyCSE return `CFGAnalyses` |
| No `Invalidator`: 24 analyses are dropped one by one by their own `preserved()`. The graph is deep (`Bounded` asks 16 analyses; `Shape` and `ExposedFrames` are under 11 and 10 of them) | `analyses.get::<..>` in the `run` of each | a base's death reaches every dependent |
| No `abandon`: a pass cannot say "not this one even though the set is kept" | `PreservedAnalyses` fields | `abandon<A>()`, used by CVP for `LazyValueInfo` |
| Analyses computed outside the manager: `DomTree::of` and `LoopInfo::analyze` called directly by loop unroll, deletion, unswitch, rotate, DFA jump threading, `mustexecute`, `bpi`, `scev` (twice), `loopaccess`, `gvn`, `ssaupdaterbulk`; `memory::exposed_frames`, `consts::known`, `alias::points_to`, `alias::calls_annotated`, `interprocedural::stamped` are free functions with several callers | `DomTree::of` at `domtree.rs#L2770` | loop passes take `LoopStandardAnalysisResults` from the manager |
| Ad-hoc caches: thread-local `ACCESSES: Vec<Held>` behind `MemRef::of` (8 versions, linear scan), `DEFINITIONS_READ`, `regions::SHAPES` (interner), `licm::DECIDED`, `settled::FACTS`, `gvn` `TABLES` | `thread_local!` in `llrm-analysis`, `llrm-transforms` | none; `MemoryLocation` is a value |
| Analyses with no `INCREMENTAL`/`SKIPS`/`SOUND`/`depends()`: `Annotated`, `FloatFacts`, `Followers`, `Assumes`, `AssumptionCache` | `impl Analysis` blocks | n/a; `AssumptionCache` never dies |
| Module analyses held by `hold`/`freeze` die at any module pass that does not name them (`Walk`, `Interprocedural`, `Stamp`) | `preserve_module` at two sites in `walk.rs` and `pipeline.rs` | only `abandon` kills `GlobalsAA` |

`Semantics::clone` (backend `masm::frame_omitted`) is a copy, not an analysis cache, and is out of scope.

## 3. Measured

One run of `llrm-cc -m32 -mabi=sysv -march=i486 -O2` on `d_faces`, `21aadae64`, release. `LLRM_DEBUG=counts` for asks and misses (11 bodies per slot), `LLRM_DEBUG=time` for own M (instructions, the same run's profile of the process). The glue figures are from the callgrind glue table of `8e278bbf2`.

Open factor, resolved (measured, cycles-queue-quiet-host, `feat/parity` 13a6a6ade, d_faces `-O2`): the 89 is not a count of compiled bodies. Ours has 11 defines entering the pipeline and 7 after globaldce; the pin (`opt default<O2>`, same flags) goes 11 to 6. Per-pass runs (`LLRM_DEBUG=runs`, 11 bodies): combine 55, fold 55, simplifycfg 33, licm 33, sroa 33, gvn 11; the pin runs InstCombine 73 times and SimplifyCFG 78. So there is no 8x factor: 89 counted analysis shapes, and the step lines' in=21/out=21 are defines plus declarations. The only extra define at `-O2` was `clip_w_` (inline refused, a decision gap; closed by #3354, see compile-gap-series.md row C1). Our `-O1` inlined nothing (the pin's goes 11 to 6); fixed by #3249. Per-body figures below divide by 11.

| analysis | asks (hit+miss) | misses | per body | own M (runs) | misses by asking pass (measured) | pin |
|---|---|---|---|---|---|---|
| dominators | 406 | 168 | 15 | 6.1 (168) | outside a pass 51, fold 29, rotate 14, walk 13, earlycse 11 | kept by any CFG-preserving pass |
| shape | 1,157 | 161 | 15 | 4.8 (161) | as dominators | no counterpart |
| loops | 210 | 161 | 15 | 1.9 (161) | as dominators | kept by `CFGAnalyses` |
| exposed-frames | 893 | 115 | 10 | 11.2 (124) | fold 40, earlycse 22, licm 15, ports 11, homes 7 | none stored |
| registers | 563 | 115 | 10 | 21.8 (115) | as exposed-frames | none stored |
| assumptions | 576 | 107 | 10 | 2.1 (107) | fold 40, earlycse 22, licm 11, ports 11 | never invalidated |
| bounded | 105 | 96 | 9 | 39.6 (132) | fold 40, earlycse 22, licm 11, homes 7, decide 5 | `LazyValueInfo` on demand |
| counted | 285 | 96 | 9 | 21.3 (104) | as bounded | none stored |
| annotated | 295 | 90 | 8 | 25.9 own, 102.9 with its asks (90) | fold 40, earlycse 22, licm 11, homes 7, memcpyopt 6 | none stored |
| pointer-values | 264 | 90 | 8 | 21.5 (117) | as annotated | none stored |
| call-effects | 106 | 82 | 7 | 54.5 (111) | fold 34, earlycse 22, licm 10, homes 7, memcpyopt 5 | `getMemoryEffects` per query |
| through-memory | 79 | 48 | 4 | 28.6 (48) | fold 48 | MemDep per load |
| float-facts | 22 | 17 | 2 | 23.7 (17) | fold 17 | none stored |
| globals-aa (module) | 77 | 10 | - | 44.4 (10) | outside a pass 6, interprocedural 2, stamp 1, walk 1 | built once, kept until abandoned |
| summaries (module), stale: measured before #3139, now `attrsummary::summaries` (12 runs, 3.1 M own, 9.3 total; funcattrs-port) | 64 | 16 | - | 7.1 (16) | walk 7, outside a pass 6, interprocedural 2, stamp 1 | FunctionAttrs on the call graph |

Built outside the manager (measured counters, whole run): dominator trees 684 against 168 manager misses; loop forests 633 against 161; `Shape` 250 against 161; exposed-frames scans 217 against 115; known-constant solves 50 (no manager analysis). The differences (516, 472, 89, 102) are builds no manager holds (inferred: the counter sits in `DomTree::of`, `LoopInfo::analyze`, `cfg.rs`, `memory::exposed_frames`, `consts::known`). At the manager's mean cost per build the 516 trees are about 19 M (inferred).

Glue table (callgrind, measured, M, `8e278bbf2`; several rows are stale): `stamped` 6.6 (now 4.6: `post_order` per SCC plus the F6 fallback), `globalsaa::found_with` 5.7, `calls_annotated` 4.4; `MemRef::of` 19.2 self (146,893 calls, 30,026 misses; the thread-local scan 13.3); `functionattrs::post_order` 20.7 inclusive with 13.0 in `consts::known_from`. `alias::initialized` is gone (#3226: `inferInitializes` is a use walk, no points-to), and `Summaries` (18.9) is stale: it is no longer a fixpoint. `post_order` also runs per SCC in the walk (#3087), so `known_from` (13 M) is the sharing target (estimate, funcattrs-port with basicaa-port-2163, 2026-10-27). Matched d_faces `-O2` builds, funcattrs-port's measurement: total 1301.5 to 1240.7 Minstr, mir stamp 42.1 to 4.6, interprocedural 45.0 to 21.2. bench-instruments-2206 fixed the glue instrument: the parse row is re-rooted at `selected_checking` (19.0 M), compile main (rest) 40.5 to 21.4 M, glue 185.6 to about 166.6 M; its new guard names three more silent zero roots (HIR to MIR emit, emission masm text/listing/frame, pass manager (managed ...)).

### Billed inside `fold@N`

Under the five `fold@N` slots, recomputed (measured by simplify-combine-wiring-2128, `LLRM_DEBUG=time LLRM_TIME_TOP=400`, release at parity d2b6377b2): 128.8 M inclusive, 12.6 M own (1242 2.33, 677 2.60, 768 2.57, 801 2.59, 837 2.54), so about 116 M beneath. The first version of this page said 162.8 M total, 31.6 M own, 131.2 M beneath, from `21aadae64`. Its raw output sums correctly (own 12.3 + 10.9 + 2.9 + 2.8 + 2.8), but `fold@677` and `fold@1242` own are about 4x their three siblings, in ms as well as instructions, while the same slots own 2.3 to 2.6 M in the later run. The cause is not found, so the later figure is used and the inferred M below were scaled by 116/131.2.

**This 116 M is nested billing, not waste.** An analysis a pass asks is charged to that pass, and the ones it asks in turn too, so `annotated` shows 102.9 M with its asks and 25.9 M own. Most of it is first computes that some pass has to pay for. The apportionment is own M of each analysis times the share of its misses asked from `fold@N` (inferred, scaled by 0.886, assumes equal cost per build; an earlier page found the average overstates cheap builds); it says who is billed, not what can be saved:

| analysis | fold misses / all | M billed under fold (inferred) |
|---|---|---|
| through-memory | 41 / 48 | 21.6 |
| float-facts | 15 / 17 | 18.5 |
| call-effects | 27 / 82 | 15.9 |
| bounded | 33 / 96 | 12.0 |
| annotated | 33 / 90 | 8.4 |
| pointer-values | 33 / 90 | 7.0 |
| counted | 33 / 96 | 6.5 |
| plain-accesses | 33 / 79 | 6.1 |
| registers | 33 / 115 | 5.6 |
| exposed-frames | 33 / 115 | 2.8 |
| eight others (followers, assumptions, dominated-edges, writes, dominators, loops, shape, table-loads) | | 5.3 |
| total | | 109.7 (of about 116 measured) |

What is waste is the rebuild that came out equal, not the first compute. Size the wins by these counts. `LLRM_WHY` of one compile, measured by basicaa-port-2163 (basicaa-port-2163's `LLRM_WHY` aggregate of one d_faces `-O2` compile at cb2ce8f3e, raw output kept by the owner; its slots are named `combine@N` there; the build counts 10, 48, 82, 90 equal ours above). Its aggregate is counts and wall ms (`globals-aa` 4.9 ms own for 10 runs, `through-memory` 2.6 ms for 48), not M instructions; the M column is parity-pipeline's (own M times equal share, inferred, our method, upper bound).

| analysis | builds | equal result | equal share | first computes | M at that share (inferred) |
|---|---|---|---|---|---|
| globals-aa | 10 | 7 | 70% | 1 | 31.1 |
| call-effects | 82 | 35 | 43% | 42 | 23.3 |
| annotated | 90 | 33 | 37% | 42 | 9.5 |
| through-memory | 48 | 0 | 0% | 48 | 0 (first computes only; billed to whoever asks first) |

`globals-aa`: 10 builds are 1 first, 7 equal result and 2 changed. "Equal result" is not "unchanged IR": module analyses carry no body mark, so the unchanged-mark class is not recorded for them. Rebuilt after: pipeline x3, simplifycfg@1438 x2, sccp, decide@1244, rotate. The pin runs GlobalsAA once for this source (`opt` 24.0.0git `default<O2>`, `-debug-pass-manager`: one "Running analysis: GlobalsAA", one `RecomputeGlobalsAAPass`), so the target is one run, kept until a module pass abandons it.

So the win for `through-memory` is not caching but asking less or computing less on the first ask. The pin's counterpart is MemorySSA (25 runs) plus BasicAA (47 runs) over 11 functions; do not compare run counts with ours: our 89 shapes are analysis shapes, not bodies (resolved above), and the pin's counts are over 11 functions. The others are won by keeping what a pass left true.

## 4. The design

### One set of rules

1. **A pass returns what it left true.** A pass that changes instructions and no block returns `preserve_cfg()` plus the analyses it names; `none()` is for a pass that changes blocks or edges. `all()` only when nothing changed (as now).
2. **CFG readers take the set.** `Dominators`, `Loops`, `Shape` today; any analysis of the CFG alone (block frequencies, branch probabilities) joins by overriding `preserved()` the same way.
3. **Function-state maps die by their footprint, not by being unnamed.** `Annotated`, `CallEffects`, `PointerValues`, `ExposedFrames`, `ThroughMemory`, `Registers`, `Counted`, `Bounded` and the rest are the pin's stateless AA stack made explicit. A pass returning `none()` does not drop one whose `depends()` says the change log did not touch it; an analysis without `depends()` declares one in its PR.
4. **Dependents follow bases.** A result built on another is kept only if its bases are kept. Added as an `Invalidator` in the shape of the pin's, by the first per-analysis PR that keeps a dependent across a pass that drops its base (rule 3 makes this the common case).
5. **Module analyses die only by a module pass.** A function pass never drops `GlobalsAA` or `Summaries`; a module pass that changes call edges `abandon`s them by name.
6. **Nothing is computed outside the manager.** `DomTree::of`, `LoopInfo::analyze`, `exposed_frames`, `consts::known`, `points_to`, `calls_annotated` become `get::<A>()` of a held result; a free function stays only as `A::run`.
7. **Every analysis has a counting test.** The computes of A per body in a pipeline that does not change what A reads equals one; fail first.

### Invalidation table

| analysis | invalidated by | preserved by | depends on |
|---|---|---|---|
| dominators, loops, shape | a changed block or edge | any pass returning `preserve_cfg()` | (shape: dominators, loops) |
| assumptions (AssumptionCache) | an edit to an `llvm.assume` | all passes (pin: never) | - |
| exposed-frames | a changed alloca use or lifetime marker (`depends()`) | `preserve_cfg()` passes that leave allocas' uses; addresssink, licm name it | shape |
| pointer-values, call-effects | an edit to a pointer or call operand (`depends()`) | passes that change neither | exposed-frames, shape |
| annotated, plain-accesses, writes | a changed memory operation or call (`depends()`, to add for `annotated`) | licm names them today; any pass that changes no memory operation | exposed-frames, pointer-values, call-effects, registers, counted, dominated-edges |
| through-memory | a store, load or call changed | passes with no memory edit (`SOUND` today) | annotated, plain-accesses, writes, registers |
| registers, counted, bounded, dominated-edges, table-loads, followers | an edit to what each models (`INCREMENTAL` updates most) | `preserve_cfg()` passes that leave arithmetic as it is | shape, exposed-frames, assumptions |
| float-facts | an edit to a float operation or its inputs | passes with no float edit (`depends()` to add) | annotated, plain-accesses, registers |
| memory-ssa (`accesses`) | a memory-operation edit the pass did not apply through the updater | `preserve_updated` (earlycse) | annotated, call-effects |
| scev | a changed loop or CFG, or an assumption | loop passes that keep `Loops` (pin: `getLoopPassPreservedAnalyses`) | dominators, loops, assumptions |
| globals-aa, summaries (module) | a module pass that adds or removes a call or a global store | every function pass | declarations, call-graph |
| callee-effects, call-registers, global-sizes, type-ancestry, declarations | a module pass | every function pass | module |

The "invalidated by" and "preserved by" cells of the middle rows are read from the `analyses.get::<..>` calls in `manager.rs`, `memoryssa.rs` and `cfg.rs`, not measured.

### Passes that return `none()` where the pin returns a set

By file, from the return expressions (inferred). Pin lines in parentheses (llvm/lib/Transforms/...).

| ours returns `none()` | the pin returns |
|---|---|
| `sroa`, `promote`, `mem2reg` | `CFGAnalyses` and `DominatorTree` (Scalar/SROA.cpp#L6498, Utils/Mem2Reg.cpp#L67) |
| `memcpyoptimizer`, `deadstore` | `CFGAnalyses`, `MemorySSA` (Scalar/MemCpyOptimizer.cpp#L2247, Scalar/DeadStoreElimination.cpp#L2842) |
| `combine` (keeps only `LastRunTracking`), `fold` | `CFGAnalyses`, `LastRunTracking` (InstCombine/InstructionCombining.cpp#L6287). For `combine` this is a wiring gap, not deliberate: it edits no block or edge, so `setPreservesCFG` applies. Fix by 10-21 (wiring), with a test, in `combine_pass.rs`; PR 1 need not wait (estimate, simplify-combine-wiring-2128) |
| `jumpthreading` | `LazyValueInfo`, `DominatorTree`, `BranchProbability`, `BlockFrequency` (Scalar/JumpThreading.cpp#L3227) |
| `simplifycfg`, `decide`, `switchlower` | `DominatorTree` (Scalar/SimplifyCFGPass.cpp#L389) when the pass asked for it |
| `looprotate`, `loopunrollpass`, `simpleloopunswitch`, `loopdeletion`, `loopinterchange`, `trivialunswitch`, `loopsimplify`, `loopidioms`, `window` | `DominatorTree`, `LoopInfo`, `ScalarEvolution`, `MemorySSA` (llvm/lib/Analysis/LoopAnalysisManager.cpp#L141) |
| `lsr_pass`, `rotate`, `floatloop`, `dfajumpthreading`, `merged` | by their LLVM counterpart; to read per pass in the audit PR |

Already close to the pin: `earlycse` (Dominators, `MemorySsa` updated), `gvn`, `cvp`, `sccp`, `adce`, `bdce`, `indvars`, `lcssa`, `instsimplify`, `reassociate`: their sets differ from the pin's only by `Shape`, `Loops` or the pin's `LazyValueInfo`.

The file counts (26) are by grep over return expressions, not an exact pass list.

### API changes

- `PreservedAnalyses::abandon::<A>()`, and `drop_unpreserved` honours it over a kept set (pin `abandon`).
- An `Invalidator` argument on `Analysis::preserved` (default: unchanged behaviour), with `inv.invalidate::<Base>()`; a result calls it for each base it reads. Lands with the first PR that needs it.
- A `PreservedAnalyses::cfg_only()` helper that is `none().preserve_cfg()`, to replace the 26 spellings of Dominators plus Loops.
- No other trait change. `READS_OUTER` stays the function-from-module proxy; `hold`/`freeze` stay.

### PRs, owners, order

One PR per analysis, each with a computes-per-body counting test written to fail first (`LLRM_DEBUG=counts`, a pipeline that does not change what the analysis reads). Order is by inferred M won, largest first, then by what unblocks the next.

| # | PR | owner | why first | dates |
|---|---|---|---|---|
| 0 | this note | parity-pipeline | rules and table | 2026-10-20 (estimate, parity-pipeline) |
| 1 | pass audit: `preserve_cfg()` for the passes in the table above, `abandon`, `cfg_only`; test: dominators computed per body in a combine-only pipeline | parity-pipeline | mechanical, unblocks every CFG reader; ceiling 12.8 M of dominators, loops and shape (inferred) | 2026-10-23 (estimate, parity-pipeline) |
| 2 | GlobalsModRef once per module (the pin runs it once), kept until a module pass abandons it. The use scan once per module in place of the per-body solve is built and tested, waiting on a rows pair; keeping it across passes that add no memory operation is a second PR | basicaa-port-2163 | 7 of 10 builds equal result (31.1 M is parity-pipeline's inferred figure) | table with the `LLRM_WHY` classes 2026-10-22; second PR ceiling 2026-10-28 (estimates, basicaa-port-2163) |
| 2b | `through-memory`: ask less or compute less on the first ask (no rebuild waste: 0 of 48 equal) | basicaa-port-2163 | 48 builds, all first computes (28.6 M own, parity-pipeline) | none stated |
| 3 | call-effects as a per-call attribute query (memoryssa/dse 10-24, gvn/fold 10-26, table deleted 10-28); `annotated`'s readers are basicaa-port-2163 / mssa-green-2127, not this row's owner | funcattrs-port | 35 of 82 equal for call-effects (inferred); `Summaries` dropped (nothing left to cache), `stamped` is 4.6 M | estimates, funcattrs-port |
| 4 | InstCombine/`fold@N` analyses: what `fold` asks on each slot | simplify-combine-wiring-2128 | about 116 M nested billing under the slots (not waste); sized by rows 2 and 3 equal-result counts | 2026-10-30, after the idle-slot per-visit work (estimate, simplify-combine-wiring-2128) |
| 5 | interprocedural: `stamped_all` through the held results | cycles-queue-quiet-host | 105.6 M inclusive in the glue table | reply 2026-10-22 (estimate, cycles-queue-quiet-host) |
| 6 | scev: held across the loop passes that keep `Loops` | scev-createscev-2239 | 9 builds; small | none stated |
| 7 | direct `DomTree::of` and `LoopInfo::analyze` callers take the held result | parity-pipeline | 516 trees outside the manager, about 19 M (inferred) | none stated |

Row 1 is the first PR proposed: it is the smallest, it is in one owner's hands, and rows 2 to 7 depend on its `abandon` and `cfg_only`.
