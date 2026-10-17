# Compile time at clang's level: the plan

## 1. TL;DR

`d_faces` -O2 compiles in 2,903 M instructions against LLVM's 415 M (6.8x; 4.8x to 6.4x in cycles) ([synthesis]). Bounded step by step, this plan does not close that gap. It removes about half of it, and it names the work that would close the rest:

| after | `d_faces` -O2, M instructions | over LLVM's 415 M | what that row counts |
|---|--:|--:|---|
| today | 2,903 | 6.8x | [synthesis], main `21f5c9fc8` |
| phase 1 (the in-flight PRs that are ready) | about 2,660 | 6.4x | each PR's own measurement on its parent, multiplied: -8.3% |
| phase 1 plus the held drafts | about 2,510 | 6.0x | also the stale drafts [#1958], [#1921], [#1962], [#1965] at their old numbers: -13.6% |
| phase 2 (open levers, each at its bound) | about 2,210 | 5.3x | point_values, Annotated, persistent MemorySSA, split path, interval recompute: -12% more |
| phase 3 (structural tracks, each at its bound) | about 1,460 | 3.5x | facts per query, one pipeline pass, allocator on the LIR core, LIR clean-up, pricing: -34% more |
| phase 4 (each pass at the spec's per-run budget) | 720 | 1.7x | [spec] section 1.3, a budget derived from gcc, not from clang |
| parity | 415 to 462 | 1.0x | needs budgets re-derived from clang's own phases (front end 33 M, optimizer 178 M, codegen 196 M, [readme]) |

The pools overlap, so each product overstates what the steps save together.

**Why the listed steps cannot close it.** The gap is not the number of pass runs (746 against 856). It is the cost of each run: a MIR pass run costs about 19 k instructions per IR instruction, against LLVM's 220 ([spec] 1.2). Every llrm pass sweeps the whole function with whole-function facts, recomputed 2.8 times per pass run. An LLVM pass makes one worklist walk over a body that SROA and EarlyCSE have already shrunk. It asks per-query facts with caps, and 0.53 analysis computations per run ([spec] 1.2, [measured]). The in-flight set lowers constants: allocation, containers and quadratic loops. It does not change that shape.

**What the plan puts on the critical path:**

1. Facts asked per query, taking analysis computations per pass run from 2.8 to 0.5 or fewer (step 3.1; keep-analyses with edit-rollback). Its first instance is `point_values` as a per-query provenance (step 2.1).
2. One pipeline pass with a combine worklist instead of rounds (step 3.2; pass-list-parity with hoist-bounded).
3. The allocator on the LIR core, 444 M against greedy's 39.5 M (step 3.3; greedy-port with ssa-spill-m16).
4. Each pass held to a counted per-run budget (phase 4; each pass's owner).

## 2. Baseline snapshot

Main was `ef40f30a7` when this was read and is `64429bfbb` now. `d_faces` flags are `-m32 -march=i486 -fno-inline-functions`, as in [synthesis]. Percentages are of `d_faces` -O2 instructions, measured on the PR's own parent unless stated.

### 2.1 Landed

| PR | what | measured |
|---|---|---|
| [#1926], [#1934], [#1953], [#1927], [#1957], [#1922], [#1936], [#1968] | first batch: bit-set liveness, dense predecessors, Fx maps, gvn forecast skip, result survives invalidation, dead code after loop, don't ask for what the body cannot use | -10.0% at -O2 together ([compile-gap-series], `5344082af`) |
| [#1988], [#1992] | profile-guided release build | -14.1% to -17.9% instructions on every row, objects identical ([compile-gap-series]) |
| [#1956] | successors and predecessors inline | -0.54% |
| [#1931] | fixed point tells a state by a digest | -0.9% |
| [#1963] | front-end verifier under the check mode only | HIR and MIR verifiers gone from release ([synthesis] row 7) |
| [#2007] | check switches read once | -0.40% |
| [#1959], [#1954], [#1961], [#1970], [#1975], [#1984], [#1998], [#1955] | dense sets in dead and spill placement, pointer values read by Annotated, walker chain once, LIR clean-up facts held, verifier names | in [rows-series] `9bce914f3` (all output identical) |
| [#2026] | dense value sets (liveness, hoist, constant cycles) | -0.42% (merged after this was read) |
| [#1912] | one allocation per function below -Omax | -1.95 G at -O1 on `c04886c2e` ([spec] 7) |

### 2.2 In flight

| PR | owner | state | measured (on its parent) |
|---|---|---|---|
| [#2022] slab allocator | pass-list-parity | ready | -4.2% instructions at every level, -2.9% cycles (min of 11); RSS +7% (`d_faces`), +18% (`mdl_ai`) |
| [#1971] block-address ids dense | attrs-budgets | ready | -1.17% |
| [#2032] register and live sets dense | ssa-spill-m16 | ready | -0.38% |
| [#2035], [#2037] escape phase | keep-analyses | ready, draft (stacked) | -0.29%, then -0.64% |
| [#2021] LIR rename shares | ssa-spill-m16 | ready | -0.56%; peak heap -15% |
| [#2027] facts rings let go | ssa-spill-m16 | ready | peak heap -13.9%; instructions -0.02% |
| [#2038] facts bundle edited in place | ssa-spill-m16 | ready | -0.52% (`Facts::of` 86.8 to 70.7 M) |
| [#2010] mask test made at each branch | ssa-spill-m16 | ready | diamond 800: 1,995 to 1,075 M, 419 stack operands to 0; `d_faces` 0 (byte-identical) |
| [#1999], [#2012], [#2017], [#2011], [#2020], [#2034], [#2008] | greedy-port | ready, stacked | ladder `loop` N=1600 31,766 to 19,654 M ([#1999] with [#2008]); diamond N=1600 -13% ([#2034]); `d_faces` +0.14% ([#2011]), +0.11% ([#2020]), +0.14% ([#2034]), -0.07% ([#2012]), -0.14% ([#2017]) |
| [#2013], [#2015] dse | edit-rollback | ready | `dse` 3.5x to 4.0x per doubling down to 2.0x; `d_faces` +0.2% and +0.02% |
| [#1977], [#1978] bounded on deep nests | greedy-port | ready, stacked | queens -O3 1,703 to 1,306 M; `d_faces` -O2 3,248 to 3,193 M |
| [#1915] exact ThroughMemory for decisions | keep-analyses | ready | correctness fix ([#1909]); +1.0% at -O2 |
| [#1958] split placed from one table | greedy-port | ready, measured on an old main | -3.4% (3,450 to 3,333 M) |
| [#1921] LIR clean-up on blocks with work | ssa-spill-m16 | draft | -1.77% on main `254855801` |
| [#1962] `initialized` events once | attrs-budgets | draft | -0.46% |
| [#1965] allocator rewrites from a floor | UNOWNED | draft | -0.3% |
| [#1972], [#1973] footprints by part | keep-analyses | draft | -0.02%, -0.07%; carries [parity] |
| [#1945], [#1943] checkpoint and rollback, `Touched` per slot | edit-rollback | draft | +0.09%; 144 function clones at -O2 left |
| [#1948] counted budgets as tests | attrs-budgets | draft | counter cost +0.04% |
| [#2033], [#2036] | keep-analyses, ssa-spill-m16 | docs | not-kept entries for spec section 8 |

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
| `FloatFacts` filter before the float solve | 6 of 20 solves skipped, +0.04% | [spec 8] |
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
| x87 live rows handed from the allocator | at most 0.9%; ranks last, parked | [lir-cleanup] |

## 3. The plan

### 3.0 Summary

The exit method M and the bars it uses are defined once in section 4.

| step | intended effect (`d_faces` -O2 unless stated) | kill criterion | owner | status |
|---|---|---|---|---|
| 0.1 one baseline | one table on one SHA: phases, gap, rows, ladder | two instruments differ by more than 1% on one binary | llvm-compare | to do |
| 0.2 counters as tests | computations per run and spec 6 counters in CI | counter cost above 0.1% | attrs-budgets | draft [#1948] |
| 1.1 slab allocator | -4.2% instructions, cycles negative | cycles not below base on a quiet host | pass-list-parity | ready [#2022] |
| 1.2 dense ids | -1.5% | below 0.5% together | attrs-budgets, ssa-spill-m16 | ready |
| 1.3 escape phase | -0.9% | below 0.5% together | keep-analyses | ready |
| 1.4 backend copies | -1.1%, peak heap -25% | below 0.5% and no heap change | ssa-spill-m16 | ready |
| 1.5 mask at each branch | diamond 1,951 to 1,075 M; `d_faces` 0 | any object differs | ssa-spill-m16 | ready [#2010] |
| 1.6 gvn, walker, lsr slopes | `loop` N=1600 -38%; exponent 1.41 to 1.34 or lower | `d_faces` above +0.5% together | greedy-port | ready, stacked |
| 1.7 dse slope | `dse` 2.0x per doubling | `d_faces` above +0.3% | edit-rollback | ready |
| 1.8 bounded on deep nests | queens -O3 -23% | any object differs | greedy-port | ready, stacked |
| 1.9 exact ThroughMemory | fixes [#1909] at +1.0% | none (correctness) | keep-analyses | ready [#1915] |
| 1.10 held drafts on main | -3% if they reproduce | each one below 0.5% on main | greedy-port, ssa-spill-m16, attrs-budgets | drafts |
| B1 re-baseline | the phase-1 row of the TL;DR measured | n/a | llvm-compare | after 1.x |
| 2.1 point_values per query | up to -4.1% | projected saving below 15 M, or the oracle differs | keep-analyses | step 0 running |
| 2.2 Annotated per access | up to -1.3% | recoverable recomputes below 15 M | keep-analyses, edit-rollback | to do |
| 2.3 persistent MemorySSA | -1.3% to -3.9% | probe below 1% | hoist-bounded, keep-analyses | to do |
| 2.4 split path tables | about -2% | `split placed` with `carving` self below 30 M | greedy-port | sized |
| 2.5 interval recompute | up to -1.3% | avoidable below 15 M | ssa-spill-m16 | sized |
| 2.6 `Frequencies` dense | `unmeasured` | self below 15 M, or density below a quarter | attrs-budgets | to do |
| 2.7 rollback, not clones | function clones 144 to 0, at most +0.1% | above +0.3%, or `LLRM_CHECK_ROLLBACK` fails | edit-rollback | draft [#1945] |
| 2.8 scaling shapes | queens -O3, joins, unrolled no steeper than gcc's | its lead cost belongs to a step already listed | hoist-bounded | to do |
| B2 re-baseline | the phase-2 row measured | n/a | llvm-compare | after 2.x |
| 3.1 facts per query | computations per run 2.8 to 0.5 or fewer; up to -12% | a per-query prototype costs at least the whole build | keep-analyses, edit-rollback | design |
| 3.2 one pipeline pass | -10% or more; rows held | a vsgcc row newly above min(gcc, clang) at one round with combine | pass-list-parity, hoist-bounded | design |
| 3.3 allocator on the LIR core | 444 M to 100 M or less | whole-body work per edit below 1% | greedy-port, ssa-spill-m16 | design |
| 3.4 LIR clean-up | about 340 M to 80 M | idle block visits below 30% of clean-up | ssa-spill-m16 | after 3.3 |
| 3.5 pricing in the back end | -1.4% (-O2) to -2.5% (-Os) | forecast and pressure below 1% at both levels | greedy-port, ssa-spill-m16, hoist-bounded | after 3.3 |
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
- [ ] baseline table re-run at the phase's re-baseline step

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

- [ ] phase table
- [ ] gap table
- [ ] ladder json
- [ ] rows
- [ ] computations per run

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
- **(b) Early:** counter cost on `d_faces` -O2. Kill if it is above 0.1% ([#1948] states +0.04%).
- **(c) Gate:** none.
- **(d) Exit:** M (identical output); `tools/test_budgets.py` green on main; computations per run printed by `LLRM_DEBUG=counts` at -O1/-O2/-Os.
- **(e) Who:** attrs-budgets. Depends on 0.1. Touches `llrm_mir::counts`, `tools/budgets.toml`, `tools/test_budgets.py`.

Checklist: the shared list. Its own metric boxes:

- [ ] counter cost
- [ ] computations per run at three levels
- [ ] budget rows re-measured on main

| outcome | decision | next |
|---|---|---|
| partial (some counters missing) | land what exists; a missing counter is a box on its consuming step | 1.1 |
| no effect (counter cost above 0.1%) | compile the counters out of release; read them in a `counts` build only | 1.1 |
| worse elsewhere (any object differs) | revert; the coordinator decides | 1.1 |

## Phase 1: land what is measured (output identical)

All phase-1 steps run in parallel. Each is re-measured on main by M before merge, because each number in 2.2 was taken on its own parent.

### 1.1 Slab allocator ([#2022])

- **(a) Effect:** -4.2% instructions at every level (2,908 to 2,787 M at -O2) and cycles -2.9% (min of 11). Moves [synthesis] row 2 (allocation, 518 M).
- **(b) Early:** on a host with load below 4, pinned to one core, run `perf stat -e cycles:u` min of 11, alternating base and head. Kill if cycles are not below base. A change that moves instructions and not cycles is the mimalloc lesson ([spec 8]).
- **(c) Gate:** none (unsafe invariants stated in the crate docs).
- **(d) Exit:** M. Also max RSS: `d_faces` -O2 within +10%, `mdl_ai` -O2 within +20%; 2N/N exponents within ±0.012 of base. Pass: instructions -3.4% or better and cycles negative.
- **(e) Who:** pass-list-parity. Depends on 0.1. Touches crate `llrm-alloc`, the four binaries' entry points, the nib-lsp path list.

Checklist: the shared list. Its own metric boxes:

- [ ] instructions
- [ ] cycles on a quiet host
- [ ] RSS on two programs
- [ ] scaling exponents

| outcome | decision | next |
|---|---|---|
| partial (instructions between -1% and -3.4%) | callgrind self cost of the allocator entry points; check the global allocator's methods are inlined | 1.2 |
| no effect (cycles not negative) | close; record in [spec 8] next to the mimalloc entry; allocation stays a count problem (B1 re-ranks [synthesis] row 2) | 1.4 |
| worse elsewhere (RSS above the limits) | fix forward: return empty chunks per phase; otherwise revert; the coordinator decides | 1.4 |

### 1.2 Dense ids ([#1971], [#2032])

- **(a) Effect:** -1.17% and -0.38% (with [#2026] landed). Moves [spec 9] container rows (B-tree 10.5%).
- **(b) Early:** on main, callgrind self cost of B-tree frames keyed by block address and `RegId`. Kill if their sum is below 15 M (0.5%).
- **(c) Gate:** none.
- **(d) Exit:** M. Pass: -1.0% or better together.
- **(e) Who:** attrs-budgets ([#1971]), ssa-spill-m16 ([#2032]). Depends on 0.1. Touches `llrm_mir::dense`, `ranges`, `slotstores`, `spill.rs` ([#1971]); `llrm-lir` `RegSet`, `allocate::Live` ([#2032]).

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

- **(a) Effect:**
  - instructions -0.56% ([#2021]) and -0.52% ([#2038]);
  - DHAT peak heap 24.0 to 20.3 MB ([#2021]) and 23.97 to 20.65 MB ([#2027]).

  Both PRs claim the 24 MB peak; together the peak is `unmeasured`. Moves [synthesis] row 3 and the [allocation] memory row.
- **(b) Early:** DHAT peak of the three stacked on main. Kill [#2027] if it adds nothing on top of [#2021].
- **(c) Gate:** none.
- **(d) Exit:** M plus DHAT peak. Pass: instructions -0.8% or better together, and peak -15% or better.
- **(e) Who:** ssa-spill-m16. Depends on 0.1. Touches `model/lir.rs` (`Insn::renamed`), `phielim`, `coalesce`, `spiller`, `Facts`, `regclass`, `postings`.

Checklist: the shared list. Its own metric boxes:

- [ ] instructions
- [ ] DHAT peak
- [ ] max RSS

| outcome | decision | next |
|---|---|---|
| partial | the interval recompute (37.4 M) is the remainder ([#2038]) | 2.5 |
| no effect | close the PR that adds nothing on top of the others | 2.5 |
| worse elsewhere | revert that PR | 2.5 |

### 1.5 Mask test at each branch ([#2010])

- **(a) Effect:**
  - diamond N=800: 1,951 to 1,075 M;
  - diamond N=1600: 6,268 to 2,474 M (clang 926 M), stack operands 825 to 3 ([#2036]);
  - `d_faces` unchanged and byte-identical.

  This is CodeGenPrepare's `sinkAndCmp0Expression`. Moves [synthesis] row 3 (79 splits).
- **(b) Early:** none; this is measured.
- **(c) Gate:** none.
- **(d) Exit:** M plus `tools/ladder` diamond at N=400/800/1600 and its backend exponent (1.44 before, [readme]). Pass: diamond N=1600 3,000 M or less and identical.py 0 differ.
- **(e) Who:** ssa-spill-m16. Depends on 0.1. Touches `addresssink`, `opcosts.txt`.

Checklist: the shared list. Its own metric boxes:

- [ ] diamond ladder
- [ ] stack operands
- [ ] `d_faces` identical

| outcome | decision | next |
|---|---|---|
| partial | re-measure [regalloc]'s split table on diamond; what is still split goes to 2.4 | 2.4 |
| no effect | instrument suspect (the PR measured it); diff the diamond asm base against head | 1.5 again |
| worse elsewhere | revert; coordinator | 2.4 |

### 1.6 gvn, walker and lsr slopes ([#1999], [#2012], [#2017], [#2011], [#2020], [#2034], [#2008])

- **(a) Effect:**
  - ladder `loop` N=1600: 31,766 to 19,654 M for [#1999] with [#2008], with the rest on top;
  - `loop` exponent from N=400 to 1600: 1.41 to 1.34 or lower;
  - mem N=1600 peak RSS 331 to 181 MB ([#2011]);
  - diamond -13% ([#2034]).

  `d_faces` net about +0.1% (per-PR +0.14, +0.11, +0.14, -0.07, -0.14). Moves [synthesis] row 4 (slopes).
- **(b) Early:** stack the seven on main and measure `d_faces` -O1/-O2/-Os. Kill (rework the gating thresholds) if the stack is above +0.3%. A shortcut must pay nothing on short bodies: gate it at entry, with the threshold measured by a sweep.
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
| partial (exponent still at or above 1.34) | callgrind the `loop` N=1600 compile; the next lead is the per-(slot, candidate) `clobbered` memo ([memoryssa]) | 2.3 |
| no effect | the ladder binary is stale or confounded; rebuild both from parents | 1.6 again |
| worse elsewhere (`d_faces` above +0.3%) | raise the entry thresholds by a sweep; otherwise drop the PR that pays on short bodies | 2.3 |

### 1.7 dse slope ([#2013], [#2015])

- **(a) Effect:**
  - `dse` mem N=1600: 1,907 to 179 M;
  - distinct-offset shape: 3,395 to 162 M;
  - arith: 818 to 52 M;
  - 2.0x per doubling (was 3.5x to 4.0x);
  - `d_faces` +0.2%.

  Moves [synthesis] row 4.
- **(b) Early:** none; this is measured.
- **(c) Gate:** none.
- **(d) Exit:** M plus `mem` and `arith` ladders. Pass: `dse` 2.2x or less per doubling, and `d_faces` at most +0.3%.
- **(e) Who:** edit-rollback. Depends on 0.1. Touches `avail.rs` (`Solve::reaching`, `Stored::related`) and `regions.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] ladder
- [ ] exponent
- [ ] `d_faces` cost

| outcome | decision | next |
|---|---|---|
| partial | pairwise `may_clobber` of wild array accesses is the remainder ([#1913]) | 2.8 |
| no effect | instrument suspect; [#1994] records a fake +2% from a confounded base | 1.7 again |
| worse elsewhere | raise the 256-cell index threshold by a sweep | 2.8 |

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

### 1.10 Held drafts on main ([#1958], [#1921], [#1962], [#1965])

- **(a) Effect:** each draft's number, re-measured on main:
  - [#1958]: -3.4% on main `90bce0774`-era;
  - [#1921]: -1.77% on `254855801`;
  - [#1962]: -0.46%;
  - [#1965]: -0.3%.

  Several are stale against [#1975] and [#1938]. Moves [synthesis] rows 3, 6 and 7.
- **(b) Early:** rebase each draft on main and measure it with M. Kill a draft if it is below 0.5% on main, unless it bundles with a same-area step: [#1962] with 2.1, [#1965] with 2.4.
- **(c) Gate:** none.
- **(d) Exit:** M per PR. Pass: 70% or more of its stated effect.
- **(e) Who:** greedy-port ([#1958]), ssa-spill-m16 ([#1921]), attrs-budgets ([#1962]), UNOWNED ([#1965]; proposed owner greedy-port, with 2.4). Depends on 0.1. Touches `splitkit`, LIR `peep/`, `alias::initialized`, `allocate`.

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

### 2.1 point_values as a per-query provenance

- **(a) Effect:** `point_values` costs 124 M (4.1%) at -O2. That figure is `unmeasured` in the repo (session report); [#1994] has `point_values` about 6 M inside `initialized` and pointer-values 69.6 M own. A local walk matches the solve on 99.39% of 659,040 accesses, with 0 of 1,583 objects different, but costs +276 M eagerly (also `unmeasured` in the repo). Only a per-query form pays. Moves [synthesis] row 1.
- **(b) Early:**
  - step 0: on main, measure the `point_values` own cost with `LLRM_DEBUG=time` and callgrind inclusive;
  - count, per consumer, the values asked against the values solved, on `d_faces` -O2, with a throwaway counter. Consumers: Annotated, escapes, `initialized`, Summaries; `initialized` is Stamp's 88 M ([spec 8]).

  The projected saving is the solve cost less (values asked × the local walk's per-value cost). Kill if it is below 15 M (0.5%), or if the answer-level oracle differs on any of the 1,583 objects.
- **(c) Gate:** a design page with a big-picture review before code. It covers:
  - the query, `provenance(value)`, owned by one analysis;
  - its consumers;
  - a dense-id memo per body version;
  - the exactness oracle, including the 0.61% where the local walk is coarser or unanswered;
  - the expected instructions at -O1/-O2/-Os.
- **(d) Exit:** M plus `LLRM_WHY` pointer-values computes. Pass:
  - 50% or more of the projected saving;
  - pointer-values computes down by the asked/solved ratio;
  - identical.py 0 differ with the new `LLRM_CHECK_PROVENANCE` on.
- **(e) Who:** keep-analyses. Depends on B1 and 1.3. Touches `alias.rs`, `manager.rs` (PointerValues), consumers in `Annotated` and `initialized`.

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
- **(b) Early:** `LLRM_WHY` blockers ([#1972]) for Annotated's recomputes on main. Count those whose log holds only operand edits that reach no address operand, and sum their cost. Kill if below 15 M.
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

- **(a) Effect:** `unmeasured` in the repo (session report): the split path is about 132 M after [#1958]. The parts:
  - B': position tables keyed by body position, about 1.0%;
  - A': `carved_moving` fed from the allocator's postings, about 0.5%;
  - E: `spilled_apart`, about 0.35%.

  About 2% in all. Moves [synthesis] row 3.
- **(b) Early:**
  - step 0: on main with [#1958], callgrind self and inclusive of `split placed`, `split carving` and `spilled_apart` on `d_faces` -O2;
  - count hash probes per row visit in `placement::iterate`.

  Kill if B' and A' together are below 30 M (1%).
- **(c) Gate:** none (same algorithm, dense tables).
- **(d) Exit:** M plus those rows. Pass: 60% or more of the probe sum per PR.
- **(e) Who:** greedy-port. Depends on 1.10 ([#1958]). Touches `splitkit.rs`, `spillplacement.rs`, `allocate.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] step-0 rows
- [ ] B'
- [ ] A'
- [ ] E

| outcome | decision | next |
|---|---|---|
| partial | the rest is per-carve body copies: LIR model change | 3.3 |
| no effect | record; the allocator gap is not in the split path | 3.3 |
| worse elsewhere (row or object moves) | revert; allocation must be identical | 3.3 |

### 2.5 Interval recompute in `Facts::of`

- **(a) Effect:** the interval recompute is 37.4 M (132 K per update of 10.6 touched values) ([#2038]); bound 1.3%. Moves [synthesis] row 3.
- **(b) Early:** count the intervals recomputed per update whose values the rewrite did not touch. Kill if the avoidable share is below 15 M.
- **(c) Gate:** none if exact; spec 4.1's "local edits" rule.
- **(d) Exit:** M; counted test that intervals are recomputed only for touched values. Pass: 60% or more of the probe.
- **(e) Who:** ssa-spill-m16. Depends on 1.4. Touches `intervals.rs` and `Facts`.

Checklist: the shared list. Its own metric boxes:

- [ ] probe
- [ ] instructions
- [ ] counted test

| outcome | decision | next |
|---|---|---|
| partial | the rest is `Indexes::patch`: per-block versions (3.3) | 3.3 |
| no effect | record in [spec 8] | 3.3 |
| worse elsewhere | revert | 3.3 |

### 2.6 `Frequencies` dense

- **(a) Effect:** `unmeasured`. [#1971] lists `profit::Frequencies` and `Frequency::over` as not converted. Moves [spec 9] container rows.
- **(b) Early:** callgrind self of both and their maps; key density against the block-id space. Kill if below 15 M, or if density is below a quarter ([#2026]'s lesson).
- **(c) Gate:** none.
- **(d) Exit:** M. Pass: 60% or more of the probe.
- **(e) Who:** attrs-budgets. Depends on 1.2. Touches `profit.rs` and `frequency.rs`.

Checklist: the shared list. Its own metric boxes:

- [ ] probe
- [ ] instructions

| outcome | decision | next |
|---|---|---|
| partial | land as a bundle with 1.2's remainder | 2.7 |
| no effect | close; nothing recorded beyond the probe on [#1994] | 2.7 |
| worse elsewhere | revert | 2.7 |

### 2.7 Rollback, not clones ([#1945], [#1943])

- **(a) Effect:** 144 function clones at -O2 down to 0 ([#1945]: `stamped` 84, gvn fork 38, `sink` 11, interprocedural 11), at most +0.1% instructions. It enables 3.1's updaters, 3.5's pricing on the live function, and [spec] 2.3.
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
  - queens -O3 after 1.8 at clang's (71 M) or gcc's (634 M) cost or less;
  - the joins and unrolled axes no steeper than gcc's (1.81 and 1.80 against 1.41 and 1.04, [spec] 1.3);
  - the `mem` shape's 11.7 KB per IR instruction against 4.4 KB for arith ([allocation]).
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

### B2 Re-baseline

Re-run 0.1 on main after phase 2 merges.

- **Pass:** the measured row is within 2 points of the TL;DR's phase-2 row (about 2,210 M).
- **If it misses:** re-rank phase 3 by the new table before any phase-3 build starts.
- **Owner:** llvm-compare.

## Phase 3: structural work (critical path)

### 3.1 Facts asked per query

- **(a) Effect:** analysis computations per pass run from 2.8 to 0.5 or fewer ([spec] 1.2, 6). Analyses own 631 M at -O2 over 2,742 computes ([#1994]). At the target ratio the bound is about 518 M; net of 2.1 and 2.2 it is about 355 M (12%). Moves [synthesis] row 1 and row 5 (idle runs: their cost is the facts, [idle]).
- **(b) Early:**
  - per analysis: computes per run, from 0.2's counter;
  - which pass asks it;
  - entries read per build (the missing number in [#1994]).

  Prototype one family per query on a throwaway branch: `bounded` asked per loop and block, the shape of [#1978]'s lazy scopes, the ranger form of [analysis-avoidance]. Kill that family if:
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
- **(e) Who:** keep-analyses (meaning), edit-rollback (record and updater API). Depends on 0.2, 2.1, 2.2 and 2.7. Touches `passes.rs`, `manager.rs`, `ranges.rs`, `consts.rs`, `alias.rs`. One family per PR.

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

  Expected -10% or more, with rows held. Moves [synthesis] rows 5 and 9.
- **(b) Early:** on a throwaway branch, merge fold, algebraic, phiopt, dead and the branch fold of decide into one worklist pass, at today's positions and today's round count. Then:
  1. identical.py first, which must be 0 differ;
  2. then rounds at 1 with the combine;
  3. vsgcc rows and bench cells against main.

  Kill if a vsgcc row newly crosses above min(gcc, clang), or a bench cell is worse, and the combine cannot remake the cause. The known causes are the third-round chain fold, decide, lcssa, gvn ([spec 8]) and the stray exit phi ([spec 8]).
- **(c) Gate:** a design page (pass list per level against clang's, [pass-parity]; the re-queue rule; the loop worklist) with a big-picture review.
- **(d) Exit:** M; `LLRM_CHECK_FIXPOINT` silent (every pass run in full after convergence changes nothing); vsgcc rows, `d_faces` executed, QCport bytes, bench cells all four levels, m16 and m32. Pass:
  - the landing rule of [spec] 4.3;
  - -8% or better at -O2 and -Os;
  - idle runs per compile down 50% or more.
- **(e) Who:** pass-list-parity (pipeline, combine), hoist-bounded (rounds, loop worklist). Depends on 3.1 for the round drop (steps 1 and 2 of R3 can start at B1). Touches `pipeline.rs`, `fold.rs`, `algebraic.rs`, `phiopt.rs`, `dead.rs`, `decide.rs`.

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

- **(a) Effect:** `regalloc` 444 M against greedy's 39.5 M ([regalloc]). Budget 100 M at -O2 ([spec] 1.3), about -250 M (8.6%) net of 2.4 and 2.5. Spec 4.1 shape:
  - LIR in an arena with MIR-shaped operands;
  - slots on instructions;
  - use lists;
  - intervals built once and edited;
  - an interference cache per (register, block);
  - no whole-body walk per spill or split.

  Moves [synthesis] row 3.
- **(b) Early:** spec 6 counters on `d_faces` -O2: `full_interval_builds`, allocator `visited` per spill or split, body copies per carve ([#1309]). Kill (re-rank against 3.4) if whole-body work per edit is below 1% of the compile.
- **(c) Gate:** a design page (spec 2.1 and 4.1 for LIR) with a big-picture review; the edit contract of spec 2.3 for LIR.
- **(d) Exit:** M; counted `full_interval_builds = 1` and allocator `visited` at most 40 × edited range + 4 × blocks touched; identical output (spec stage 2 is identical). Pass: `regalloc` 150 M or less at -O2.
- **(e) Who:** greedy-port (allocator), ssa-spill-m16 (LIR model). Depends on 2.4, 2.5 and B2. Touches `model/lir.rs`, `allocate.rs`, `splitkit.rs`, `intervals.rs`, `postings.rs`.

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

- **(a) Effect:**
  - `lir peephole` 130 M;
  - `jumps` 65 M;
  - `coalesce` 60 M;
  - x87 71 M;
  - `phielim` 14 M;
  - about 340 M against about 40 M ([synthesis] row 6).

  Budget 80 M ([spec] 1.3). Rewrites that need no physical register move into selection (DAG-combine style, [lir-cleanup] item 5). Moves [synthesis] row 6.
- **(b) Early:** count, per sub-pass, the block visits that change nothing on `d_faces` -O2 (the open question in [lir-cleanup]). Kill if idle block visits are below 30% of clean-up cost.
- **(c) Gate:** a design page for the selection-time rewrites with a review.
- **(d) Exit:** M; counted `block_visits` at most the dirty blocks after the first visit (spec 6). Pass: clean-up 160 M or less at -O2.
- **(e) Who:** ssa-spill-m16. Depends on 3.3 and 1.10 ([#1921]). Touches `peephole.rs`, `peep/`, `jumps`, `floatassign.rs`, isel combine.

Checklist: the shared list. Its own metric boxes:

- [ ] idle visits
- [ ] clean-up instructions
- [ ] counted visits

| outcome | decision | next |
|---|---|---|
| partial | the x87 live rows (25.7 M) come next, last in rank | 3.5 |
| no effect | record; the cost is per useful visit: phase 4 budgets | 4.1 |
| worse elsewhere | revert | 3.5 |

### 3.5 Pricing where LLVM puts it (R5 of [parity])

- **(a) Effect:** the spill forecast is 1.39% of -O2 and 2.52% of -Os ([decision-dependencies]), against "gvn pricing ~5%" in [parity]. Step 0 reconciles the two. The back end gains rematerialization, sinking, and a pressure-aware LIR hoist per loop path; gvn and hoist then drop whole-function pricing. Moves [synthesis] row 4.
- **(b) Early:** step 0: callgrind inclusive of `View::forecast`, `Pressure::of` and `profit::weighted` under gvn and hoist at -O2 and -Os on main. Kill if their sum is below 1% at both levels.
- **(c) Gate:** a design page with a review; any row that falls below min(gcc, clang) blocks the pass's drop of pricing ([parity]).
- **(d) Exit:** M; counted `spill forecasts per pass run = 0` in the mid-end; the rows rule of [spec] 4.3. Pass: 60% or more of the probe, with rows held.
- **(e) Who:** greedy-port and ssa-spill-m16 (back end), hoist-bounded (gvn and hoist). Depends on 3.3 and 2.7. Touches `gvn.rs`, `hoist.rs`, `profit.rs`, `spill.rs`, the LIR hoist.

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

- **(a) Effect:** [spec] 1.3's budgets are gcc-derived: 720 M at -O2, 1.7x LLVM. Re-derive them per job from clang's phases:
  - front end 33 M;
  - optimizer 178 M (clang 24) or 272 M (distro clang);
  - codegen 196 to 220 M ([readme], [compile-gap-series]).
- **(b) Early:** none (measurement).
- **(c) Gate:** the user decides whether the per-level goal is clang 24's or the static clang 20.1.8's column.
- **(d) Exit:** a spec 1.3 table per level, and each pass's budget moved into `tools/budgets.toml`.
- **(e) Who:** llvm-compare.

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

**Method M (every exit measurement).**

1. **Binaries.** The base is the PR's first parent (`git rev-parse <head>^1`), never main of another day and never the merge-base after main moved. Build both with `cargo build --release --bins` in separate worktrees on the perf host. Check each binary's mtime is after its commit, and record both SHAs in the PR.
2. **Instructions.** `valgrind --tool=callgrind` and `perf stat -e instructions:u` (min of 3) on `d_faces` -O1/-O2/-Os, flags `-m32 -march=i486 -fno-inline-functions`. For lru, bintree, queens and sieve at four levels, use `tools/compile-gap.py`.
3. **Cycles.** `perf stat -e cycles:u`, min of 11, alternating base and head, pinned to one core, `uptime` recorded. A cycle figure is claimed only at a load average below 4. Otherwise it reads "not claimed", and the step's cycle box stays open until a quiet run.
4. **Slopes.** `tools/ladder/ladder.py` and `fit.py` for both binaries, every shape, N=25 to 1600; `scaling_gate.py` 2N/N per axis.
5. **Output.** Run `tools/identical.py BASE HEAD` with `QCPORT` and `QCPORT_INC` set; the corpus is 1,583 objects with every `LLRM_CHECK_*` on, and a smaller corpus is a failure. For output-changing steps, also run:
   - `tools/rows/run.sh`, `analyse.py` and `qcport.py`;
   - `tools/bench/bench.py --opt O1 O2 O3 Os --no-judge --json` on both sides, then `bench_table.py`.
6. **Memory.** `/usr/bin/time -v` max RSS on `d_faces` -O2 and `mdl_ai` -O2; DHAT peak where the step claims memory.
7. **Gate.** The CI line `GATE <tier> PASS at <head12>` must name the PR's head; premerge OK on main.

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

**Critical path:** 0.1 → 0.2 → (phase 1 merges) → B1 → 2.1 → 3.1 (design, then one family per PR) → 3.2 (round drop) → B3 → 4.1.

The back-end track runs beside it from B1: 2.4, 2.5 → 3.3 → 3.4 → 3.5. Spec 7 notes these touch disjoint crates.

| can run together | blocked by |
|---|---|
| all of phase 1 (1.6 and 1.7 serialised on `avail.rs`) | 0.1 |
| 2.1, 2.2, 2.4, 2.5, 2.6, 2.7, 2.8 | B1; 2.2 after [#1943]; 2.4 after [#1958] |
| 2.3 | 1.6 merged (`memoryssa.rs`) |
| 3.1 families; 3.2 steps 1-2 (identical at today's rounds); 3.3 | B2 for 3.1 and 3.3; B1 for 3.2 steps 1-2 |
| 3.2 round drop | 3.1 |
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
| point_values demand counts per consumer | keep-analyses | keep | step 0 of 2.1, which is on the critical path |
| lazy call-effects and `captured_before` design | keep-analyses | cancel | dropped after the walk fusion; the escape side left is about 34 M |
| cheap-many redundancy sizing | edit-rollback | close | answered: 1.4%, largest cause 0.4% ([#1994]) |
| Annotated update from the record | edit-rollback, keep-analyses | keep | it is 2.2 |
| per-(slot, candidate) `clobbered` memo | greedy-port | keep | the next slope after 1.6 |
| split path B', A' | greedy-port | keep | it is 2.4 |
| split carving constants | greedy-port | cancel | the carving was spill pressure that [#2010] removes |
| interval recompute | ssa-spill-m16 | keep | it is 2.5 |
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

## 6. Timeline

| phase | starts when | exits when |
|---|---|---|
| 0 instruments | now | 0.1's table committed; 0.2's counters in CI |
| 1 land the measured | 0.1 | every 1.x checked or closed; B1 within 2 points of -8.3% (ready set) |
| 2 open levers | B1 | each 2.x landed or recorded in [spec 8]; B2 within 2 points of its row |
| 3 structural | B1 for 3.2 steps 1-2; B2 for the rest | computations per run 0.5 or fewer; one round at -O1 to -Os with rows held; `regalloc` 150 M or less; clean-up 160 M or less; B3 within 3 points of about 1,460 M |
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
[#2031]: https://github.com/ali-mosavian/llrm/pull/2031
[#2032]: https://github.com/ali-mosavian/llrm/pull/2032
[#2033]: https://github.com/ali-mosavian/llrm/pull/2033
[#2034]: https://github.com/ali-mosavian/llrm/pull/2034
[#2035]: https://github.com/ali-mosavian/llrm/pull/2035
[#2036]: https://github.com/ali-mosavian/llrm/pull/2036
[#2037]: https://github.com/ali-mosavian/llrm/pull/2037
[#2038]: https://github.com/ali-mosavian/llrm/pull/2038
