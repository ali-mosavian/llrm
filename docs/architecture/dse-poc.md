# DSE in LLVM's shape on llrm's memory SSA: what it costs, and which part is the gap

TL;DR: a POC of LLVM's DSE on llrm's memory SSA costs 27,300 instructions per IR instruction on `d_faces` -O2 against LLVM's 2,923. Per store it is 4.1x, not 9x: llrm's `dse` sees 13x the stores in 3x the runs because `promote` leaves them. Of the POC's 190.5 M, 106.5 M (56%) is the facts it asks for (Accesses with `Annotated`, `Pointers`, `Published`, the memory graph), and the walk is 84.0 M at 2,870 instructions a step. A cache of alias pairs keyed by cell value saves 21% of the walk. The walker layer is a fifth of the pass; the fact set and the input volume are the rest.

Pinned: LLVM [`d1106deb71cc`][llvm], llrm main `5d1c56b54`. Instructions are `instructions:u` of one pass run on a dumped body minus the parse of the same file, `CARGO_INCREMENTAL=0`. Inputs: the 138 `dse` bodies of `~/scratch/replay/inputs` (edit-contract-replay's dump, [#2091]). The POC is branch `notkept/dse-poc` (`d224fad43`) (`crates/opt/llrm-transforms/src/dse_poc.rs`, not wired into the pipeline except by `LLRM_DSE_POC`).

## 1. What was built

LLVM's [`getDomMemoryDef`][gdm] and the forward read check: killing stores bottom-up; for each, the defs above it are walked with the scan limit 150, the walker-step limit 90 (same block 1, other block 5) and the partial limit 5 ([`DeadStoreElimination.cpp#L127-L160`][limits]); the first def it overwrites completely or in part is the candidate; the candidate is dead when a forward search over its users meets a killing store before any read, and the exits are covered by killing blocks unless the object is private; a second sweep takes stores to private objects nothing reads. Not built: memset/memcpy trimming, lifetime ends, no-op stores, dominating-condition stores.

## 2. Measured, `d_faces` -O2

| | LLVM `DSEPass` | llrm `dse` today | POC |
|---|---:|---:|---:|
| runs / IR instructions | 11 / 1,181 | 33 / 6,980 | 33 / 6,980 |
| stores / loads in the inputs | 68 / 201 | 901 / 1,492 | same |
| instructions | 3.5 M | 148.6 M | 190.5 M |
| per IR instruction | 2,923 | 21,286 | 27,296 |
| per store | 51 k | 165 k | 211 k |
| per run | 0.32 M | 4.5 M | 5.8 M |

LLVM: callgrind of `clang -cc1` with `tools/clang_pass_budgets.py` ([#2056]); stores and loads counted in `-print-before=dse`. The POC's facts are 106.5 M (Accesses with `Annotated` 21.3 M on the largest body, `Pointers` and `Published`, the memory structure); its walk is 84.0 M over 18,082 upward steps and 11,169 forward scans, 2,870 per step (5,100 before dense sets replaced the hash sets and the `Access` clones, 3,610 before the pair cache).

Which fact the candidates read (POC variants over the same inputs, M instructions, stores removed): all facts 265.6 M / 116 (before dense sets); no `Pointers` and `Published` 173.7 / 28; plain `Accesses` without `Annotated` 149.2 / 222; both removed 96.4 / 169. `Pointers` and `Published` are read by the exit rule and the end-of-function sweep (88 of the 116 removals) and cost 92 M; `Annotated` provenance is not read by the candidates (the same walk is 116 M cheaper). Removals that rise without it are not shown sound.

## 3. Where the walk's cost sits (largest body, callgrind)

| part | share of the pass's own cost |
|---|---:|
| `may_clobber`: 16,493 slow calls after the pair cache, 884 each (`regions::overlapping` 446, `pointerfacts::Offsets::stepped` 29,132 calls) | 31% |
| `covers`, `covered`, `placed` (a `Vec<(i128, i128)>` per call) | 8% |
| `MemorySSA::access` (binary search over ids) | 3% |
| the step logic | the rest |

Alias pairs repeat: 58,573 calls and 19,997 distinct pairs a run (66% repeat) on `d_faces`, 48% on `mem400`, 99.8% on `loop400`. A cache keyed by the address of the two `MemRef`s (no hashing of a `MemRef`) cut the d_faces pass from 212.2 to 190.5 M (10%, the walk by 21%) when keyed by the cell's value (each `MemRef` hashed once, then pairs of numbers); keyed by the `MemRef`'s address it saved 5%. Per query the slow path (884) is below BasicAA's 1,800 ([alias.md](../comparison/llvm/alias.md)); the difference is the count. The POC asks 8.4 alias queries per IR instruction; LLVM's `DSEState::isOverwrite` is 0.47 M inclusive of the compile (about 260 queries at that price, 0.2 per IR instruction).

## 4. Edits against today's `dse`

`d_faces` -O2: today removes 337 stores, the POC 95 (39 by a kill, 56 at the end); 253 missed, 11 extra. Of the misses 209 (83%) are stores to an alloca nothing loads or lets escape: `promote` writes through and leaves them, LLVM's `SROA`/`mem2reg`/`InstCombine` delete the alloca with its stores (on the 11 `sroa` inputs `sroa,early-cse` ends with 0 allocas and 91 of 427 stores, llvm-order-poc); 43 are stores to an alloca read elsewhere and killed later on every path (the phi fan-out and the limits); 1 is not an alloca.

`mem400` per input: the unpromoted body (5,853 instructions, `load ptr` of the slot before every gep) today's `dse` leaves alone (0 removed), the POC removes 123; the promoted body (1,189 instructions): today 386, POC 310, missed 76. The 76 are stores overwritten more than 90 steps or 150 defs away: with the limits times 100 the POC removes all 386, at 111 M against 29 M (limits times 1) and today's 7 M. LLVM has the same limits and misses them. DEVIATE: keep today's dataflow for this shape (7 M) unless rows show the limits lose nothing.

## 5. What this decides

| part | LLVM | llrm | decision |
|---|---|---|---|
| input volume | SROA/EarlyCSE/InstCombine leave 68 stores in 1,181 instructions | `promote` leaves 901 in 6,980 | llvm-order-poc: remove the dead alloca's stores in `promote`/`sroa` |
| fact set | `MemorySSA` over BasicAA, lazily; capture by [`isInvisibleToCallerAfterRet`][invisible], cached per object | `Annotated` + `Pointers` + `Published` + memory graph, eagerly, 106.5 M | MIRROR: plain `Accesses` for the candidates and one cached per-object 'observable on return' query in `alias.rs` (facts-per-query owns the capture fact; oracle: equals `escaped.contains` on 1,583 objects) |
| runs | 11 | 33 | pass placement (S3), not this page |
| alias pair cache | [`AAQueryInfo::AliasCache`][aaqi], per query object | none | MIRROR: 10% of the pass (21% of the walk), keyed by the cell's value |
| access lookup, user links | pointers and intrusive use lists ([`MemorySSA.h#L140`][access]) | dense ids, binary search, no users | MIRROR user links and a direct index when `Structure` is next touched; 3%, below the bar alone |
| limits | 150 / 90 / 5 | none in `dse` | MIRROR in the POC only; DEVIATE for the 76 distant stores (7 M against 111 M) |

The ordering of the estimate: input volume (13x stores) and the fact set (53% of the POC) before the step cost. The target 'at most 300 instructions a step' does not follow from these numbers: LLVM's DSE makes few steps because it has few stores, and a step costs the POC 2,870 with the pair cache, of which `may_clobber` is a third.

## 6. Soundness

The first POC was unsound and the check found it. With the POC in `dse`'s place on the 71 vsgcc programs at four levels, held to the programs' `.out` self-checks by the harness (`LLRM_DSE_POC=1`): `mandel` and `crc` -O1 ran to the 300 M-instruction limit, then, after that was fixed, `lru`, `quicksort`, `x_base64`, `x_bigadd`, `x_bubble`, `x_dct`, `x_heapsort`, `x_reverse` and `x_rle` returned wrong results at all four levels. Three causes, all in the walk and none in today's pass (a cell-by-cell dataflow, which never takes a store as its own candidate):

1. The walk from a store in a loop reaches the store itself through the loop's phi and took it as its own dead candidate. No isolated body reproduced it; it fired only together with cause 2.
2. The forward search skipped a phi whenever a killer's block dominated it (LLVM's `DT.properlyDominates(KI->getParent(), UseAccess->getBlock())`, [`DeadStoreElimination.cpp#L1847`][skip]). A loop's phi is entered along the back edge from the candidate, an edge that never meets the killer; skipping it dropped the value an inner loop carries (`mandel`'s counter). LLVM's walk is guarded by `isGuaranteedLoopIndependent` and its walk order; mirroring the skip alone is not sound. Test `test_a_loop_carried_store_is_read_through_a_phi_the_killer_dominates`, fails with the skip restored.
3. A store that overwrites only some of the candidate's bytes was taken as the killer that ends every path. The bytes it leaves are read. Test `test_a_partial_overwrite_does_not_kill_the_wider_store`, fails with the old killer set.

The POC now follows every phi, never takes the killer as a candidate and counts a partial overwrite as no kill (the union of partial overwrites, LLVM's interval map, is not built). Result of the run with the fixes (`CARGO_INCREMENTAL=0`, the same tree with and without `LLRM_DSE_POC=1`): all 284 rows (71 programs, four levels) give the expected result; cycles are identical in 283 and +0.8% in one (`matmul` -O3). The pipeline's later passes remove what the POC leaves, so the 253 missed stores cost no row.

[llvm]: https://github.com/llvm/llvm-project/tree/d1106deb71cc81016406a1917d0508d2df296266
[gdm]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DeadStoreElimination.cpp#L1648
[limits]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DeadStoreElimination.cpp#L127-L160
[skip]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DeadStoreElimination.cpp#L1847
[invisible]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DeadStoreElimination.cpp#L1040
[aaqi]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/AliasAnalysis.h#L239
[access]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L140
[#2056]: https://github.com/ali-mosavian/llrm/pull/2056
[#2091]: https://github.com/ali-mosavian/llrm/pull/2091
