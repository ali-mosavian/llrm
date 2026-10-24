# Loops compile-time bucket: dated plan to 1.0x of the pin

Owner w-3. Ledger command (docs/working/ledger-command.md): qbq-llrm render/d_faces.c, `-m32 -mabi=sysv -march=i486 -O2`, callgrind, dist build of a206759f8. Bucket: 160.5 M ours, 86.8 M pin (1.85x, +73.6 M). Callee view below is inclusive M on that run; "pin" is the pin function(s) the step mirrors.

| # | Where (ours, M) | Pin mirror | Cause | Removes | PR date |
|---|---|---|---|---|---|
| L1 | Licm 48.0 (pin 14.0): `MemorySsa` get x11 = 35.1, `sink_and_hoist` 9.2 | `MemorySSAAnalysis` / `buildMemorySSA` ([L1522](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1522)): 3.0 M for the compile | per-build cost: CallFootprints, Annotated, `Accesses::plain` (docs/working/accesses-basicaa-plan.md) | ~25 | S1b 11-13, S2 11-20, S3 11-27 |
| L2 | the SCEV engine's cost per operation. IndVars 45.0 (pin 23.3), and the same factor in every loop pass that asks SCEV. Same IR, `opt -passes='default<O2>'` against ours, inclusive M / call edges: `getBackedgeTakenCount` 15.4 / 1774 against 6.1 / 1644; `computeExitLimit` 21.3 / 270 against 10.7 / 274; `getAddExpr` 16.8 / 7318 against 7.2 / 7414; `evaluatePredicateAt` 20.6 / 115 against 11.5 / 166; `getRangeRef` 19.5 against 8.8; `getSCEV` 23.3 against createSCEVIter 12.7 | `ScalarEvolution::{getBackedgeTakenCount, computeExitLimit, getAddExpr, StrengthenNoWrapFlags, getRangeRef}` ([ScalarEvolution.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp)) | measured, not assumed: the number of real computations equals the pin's (SE sharing is not it: 21 cold starts for 11 functions), the cost of each is 2 to 2.5x (getAddExpr 2300 against 970 Ir per call; `intern` 190 Ir self per node, `Kind::clone` 1.4 M, `cached_range` 1.5 M self) | ~6 identified (line-level callgrind, release with line tables): interning 5.4 M self (`Key::hash` 2.3, `intern` 1.6, `Kind::clone` 0.9, table find 0.6) -> ~2.5; `get_range_ref`/`cached_range` self 2.3 -> ~1; `is_basic_block_entry_guarded_by_cond` -> `is_implied_cond` chain 2108 calls at 6.5 K Ir (pin 1.5x) -> ~2; no other function is over 0.2% self: the rest of the 2x is spread | steps: L2a interning: TRIED 10-24, no gain (a borrowed-key lookup makes a hit allocate nothing, tested, but d_faces 1186.40 -> 1186.50 M Ir: the virtual calls of the hash and the equality cancel the saved allocations; an inline-vector key needs a dependency): dropped, the interning stays at 5.4 M; L2b `cached_range` and `get_range_ref` self 11-13; L2c the guard chain (`is_implied_cond_preds`) 11-20; then re-profile 11-27 |
| L2b | `getSCEV` 14.5k call edges against the pin's 5.9k, `getRangeRef` 34.7k against 19.8k | verify first: the pin's `createSCEVIter` is a worklist and ours recurses per operand, so edges are not comparable. Count real cache misses in both. Owner w-3 | 0 until shown | measurement 11-06 |
| L3 | LCSSA 7.0 (pin 0.8): `closed_in` 6.0: `formed_with` 4.8 (`outside_reads` 2.2, `lcssamerges::closed` 1.6) | `formLCSSAForInstructions` (LCSSA.cpp) | users scanned per instruction, not per use list | ~5 | 11-20 |
| L4 | LoopLoadElim 7.5 (pin 2.3): `get_backedge_taken_count` 2.5, `AccessAnalysis::can_check...` 1.4, `are_deps_safe` 0.9 | `LoopLoadEliminationPass` / `LoopAccessInfo` | trip count and access analysis made per loop, not shared with IndVars' SE | ~3 | 12-04 |
| L5 | LoopIdioms 4.7 + Merge 1.3 + LoopIdiom 5.1 = 11.1 (pin 4.1) | `LoopIdiomRecognize` | two implementations: the llrm-only `loopidioms` still runs beside the translated `loopidiom::pass` | ~6 | 11-27 (retire `loopidioms` where `loopidiom` covers it; objects compared) |
| L6 | LoopSimplify 4.1 (pin 1.0 + SimplifyCFG 1.0) | `LoopSimplify` | `_simplified` re-analyses; loops found per call | ~2 | 12-04 |
| L7 | LSR 17.1 (pin 12.8): `generate_all_reuse_formulae` 5.0, `implement_solution` 3.3, `IvUsers::add_users_if_interesting` 2.7 | `LSRInstance::GenerateAllReuseFormulae`, `Rewrite`, `IVUsers` | formula sets cloned per use | ~3 | 12-11 |
| L8 | the rest: unroll 10.5 (pin 9.1), deletion 6.3 (7.3), rotate 2.9 (7.8): at or below the pin | - | - | 0 | - |

Sum of the steps: about 50 M of the 73.6 M excess, so the bucket lands near 110 M (1.27x). The last ~23 M is not attributed to a callee yet: L9, 12-18, re-measure after L1 to L3 on the ledger command and plan the next steps from the callee view; the goal stays 1.0x.

Each step is one PR with its cost measured on the ledger command (dist, before and after, same target dir) and objects compared (objsweep, QCport) where the change can reach output; none is in this PR.

## From about 1.25x to 1.0x (not dated yet)

After L1 to L7 the bucket is near 108 M against 86.8 M. What is left, with what each needs before it can get a date:

| # | Where (ours after L1 to L7, est. M; pin) | Needs |
|---|---|---|
| R1 | IndVars ~37 (pin 23.3): `ExitOpt` 6.9 (pin's `optimizeLoopExits` ~3.9), `simplify_users_of_iv` outside `eliminate_iv_user` 4.4, `get_scev` | measurement: SCEV nodes made per pass on the ledger command, ours against the pin's `-stats`-less count (callee view of `createSCEV`) |
| R2 | cold `ScalarEvolution` starts: `delete_loops` x60, `ScevSession::within` x51, unroll x37, LoopDeletion x32, IndVars x19 (the pin keeps one SE for the loop pipeline) | blocker: which passes drop `Scevs` between the loop passes (`PreservedAnalyses`); measurement: nodes made after each |
| R3 | Licm ~23 (pin 14.0): `sink_and_hoist` 9.2 x62 against `hoistRegion` 10.6 in the pin | measurement: `can_sink_or_hoist_inst` x914 (3.5) against the pin's `canSinkOrHoistInst` 8.9 on the same loops |
| R4 | LoopLoadElim ~4.5 (pin 2.3), LCSSA ~2 (0.8), LoopSimplify ~2 (1.0 + 1.0), LSR ~14 (12.8) | after L3, L4, L6, L7 are measured |
| R6 | `CallFootprints` + `Writes` (38.9 M, 76 runs) and their five readers: `consts::_killed`, `floatfacts`, `floatloop`, `ThroughMemory`, `FloatFacts` (all llrm-only) | blocker: they are a forward dataflow that kills known cells (abstract resolved addresses with no pointer operand) by overlap with each call's write list, so a per-(call, cell) AA query has no operand to ask about. The pin has no such pass: the values it finds in memory come from per-load walks (GVN / MemDep and EarlyCSE through `MemorySSA::getClobberingMemoryAccess`, whose call test is `getModRefInfo(Call, Loc)`), and float facts from `ValueTracking` / `LazyValueInfo`. Owner w-3. First design note 11-27 (docs/working: which pin walk replaces each reader, with the readers' call sites counted on the ledger command); no code date until it names the pin pass per reader. |
| R5 | rotate 2.9, deletion 6.3, unroll 10.5 against the pin's 7.8, 7.3, 9.1 | none: at or below the pin; keep out of the way |

## Per pass, ours against the pin (d_faces -O2, 10-24)

Inclusive M and call edges of each loop pass's entry, ours on the dist tip a206759f8 (`callgrind_annotate --tree`), the pin's on `ct-pin.cg` (clang's own compile of the same file). Unit of a call: ours is one run over a function body (11 functions), the pin's is one visit of a loop in the loop pass manager (LICM 124, IndVars 41), so counts compare only for the function passes (LoopSimplify, LCSSA). The pin's LSR (not matched in this profile) is 12.8 M in `ledger2/clang.incl`.

| pass | ours M | calls | pin M | calls | excess M | where it is |
|---|---|---|---|---|---|---|
| LICM | 48.0 | 53 | 19.9 | 124 | +28.1 | 35.1 is the MemorySSA build (S2/S3, L1); the pin's MemorySSA is outside its LICM figure (3.0 M for the compile) |
| IndVars | 45.0 | 19 | 29.6 | 41 | +15.4 | SCEV per-op cost, spread (L2b, L2c) |
| LCSSA | 7.0 | 58 | 1.0 | 45 | +6.0 | FLAT (funcattrs-port, 10-24): 44 pin-shaped slots + 14 stage runs, the pin also runs it unconditionally, 11 of 58 change anything; per run 103 K = `outside_reads` 37 + exits 21 + `opened_with` 17 + Shape 18; skipping the llrm-only `opened_with` gains 1.1 M (0.1%), object identical: dropped |
| LSR | 17.1 | 7 | 12.8 | - | +4.3 | formula generation (L7) |
| LoopLoadElim | 7.5 | 11 | 3.1 | 6 | +4.4 | trip counts and access analysis (L4) |
| Unroll | 10.5 | 11 | 7.4 | 43 | +3.1 | `get_backedge_taken_info`, cost estimator |
| LoopSimplify | 4.1 | 58 | 2.0 | 45 | +2.1 | 58 runs against 45, 71 K a run against 44 K (L6) |
| idioms (two implementations) | 5.1 + 6.0 | 11 + 11 | 6.2 | 41 | +4.9 | the llrm-only `loopidioms` still runs (L5) |
| Rotate | 2.2 | 11 | 10.3 | 57 | -8.1 | below the pin |
| Deletion | 6.3 | 11 | 6.4 | 65 | -0.1 | at the pin |
| Unswitch | 0.5 | 11 | 0.2 | 35 | +0.3 | |

The excess adds to the bucket's 73.6 M within a few M. Where it sits: LICM's MemorySSA (S2/S3) is 35 of the 70, IndVars 15, LCSSA, LoopLoadElim, LSR and the second idiom pass 4 to 6 each. The count differences that matter are the function passes: LoopSimplify and LCSSA run 1.3x as often as the pin's (58 against 45) and cost 1.6x and 5.6x more a run; why is not yet known (to measure by 11-06: for each of our 58 runs, the slot it follows and whether it changed anything; the pin's loop pass manager keeps the form by `LoopSimplifyID`/`LCSSAID` being required, not rerun).

## MemorySSA builds (L1: S2, S3), 10-24

Same two profiles. The pin: `MemorySSAAnalysis::run` 25 calls, 3.5 M inclusive (140 K Ir a build). Ours: `MemorySsa::run` 40 calls, 46.5 M inclusive (1.16 M a build), of which `PlainAccesses` 25.7 M (57 runs, of which `Annotated` 44.9 M over 69 runs is shared with other users) and `Mssa::build` 8.0 M (42 builds).

| | builds | M | M a build |
|---|---|---|---|
| pin | 25 | 3.5 | 0.14 |
| ours | 40 | 46.5 | 1.16 |
| ratio | 1.6x | 13x | 8.3x |

So the count is a smaller lever than the cost of a build: the 15 extra builds are worth about 2 M at the pin's cost and about 17 M at ours; the cost of a build, 1.02 M more each over 40, is worth about 41 M. Order: S3 (what `PlainAccesses` and `Annotated` make for a build: 34 of the 46.5 M) before preserving the graph across more passes. Builds per user in ours (callers of `Analyses::get::<MemorySsa>`, 44 gets for 40 builds): LICM 11, MemCpyOpt 11, EarlyCse 22; per function (11) 3.6 against the pin's 2.3 if its 25 are over the same 11 functions. Why ours builds more is not yet known: to find by 11-06, for each build the slot it is made in and the pass that dropped the graph before (the pin keeps it through EarlyCSE, which updates it, and its other users within a loop pass manager run).

## MemorySSA rows after #4194 (parity 840c8832a, dist, d_faces -O2, 1008.6 M), 10-24

`MemorySsa::run` 44.5 M over 40 builds (1.11 M a build; 46.5 before `Bounded` went: #4194 moved it 2 M). The pin's build is 0.14 M, and ours, `Mssa::build` 7.2 M (0.18 M a build), is the one part with a counterpart there. Each other part, with its pin function or its DEVIATE and exit:

| part of a build | M | runs | pin counterpart | exit | date |
|---|---|---|---|---|---|
| `PlainAccesses` (own 24.2: `Accesses::plain` 9.7, `Writes` 6.2, the `Annotated` fetch 8.2) | 37.3 inclusive of its asks | 46 | `MemorySSA::buildMemorySSA` ([L1522](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1522)) classifies by opcode and `getModRefInfo(Call)`; no per-access location | DEVIATE AN1: S3b, `Accesses` kinds only, the build asks no analysis | 11-10 |
| the walker's store/load defs by `MemRef` overlap (`clobbered()`, `Reach`) | in the walks | - | `instructionClobbersQuery` ([L320](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L320)) asks `AA.getModRefInfo(DefInst, UseLoc)` | S3a: the walker asks the stack for every def, not only calls (#3991, #4155 did calls and `AliasQuery`) | 11-06 |
| `Annotated` (43.1 inclusive over 58 runs: `PointerValues` 11.1, `Counted` 10.4, `MemRef::of` 7.0 on 3,719 accesses, `value_tracking_at` 3.4) | 43.1 | 58 | none: the pin makes `MemoryLocation::get(I)` from the instruction | DEVIATE: S3c, the other readers (`licm/mod.rs` keeps it, `loopaccessinfo`, `references`, `lazyvalueinfo`) take a `MemoryLocation`; then delete `Annotated` and `Counted` | 11-17 (with funcattrs-port's lazy-Annotated plan, first to land wins) |
| `CallFootprints` and `Writes` | 21.2 and 15.7 | 58, 47 | `getModRefInfo(Call, Loc)` per query | DEVIATE R6: the dataflow readers; design note | 11-27 |
| `PointerValues` (`point_values` 12.9, `ExposedFrames` 6.2) | 20.7 | 58 | none (BasicAA decomposes per query) | w-5's route: MemCpyOpt first (needs S2 rest, 11-06), then the rest | 11-27, earlier if S2 rest lands |
| `Mssa::build` | 7.2 | 40 | `buildMemorySSA` | at parity (1.3x) | - |

S3a and S3b are what take a build from 1.11 M toward 0.18 M; S3c and the `PointerValues` route remove the analyses the other readers still keep alive, so those two are worth what no other reader asks for.
