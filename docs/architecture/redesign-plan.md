# Compile time at clang's level: the plan

**Target.** Parity is clang 24 at `d1106deb71cc` (415 M instructions on `d_faces` -O2) with rows held against min(gcc, clang); the static clang 20.1.8 (462 M) is the secondary column. Phase 3 (structural) starts now, in parallel with phases 1 and 2: each step is gated by its early probe and its design review, not by B1 or B2.

## Status

As of main `c37be2a9c` plus the merges since (states checked with `gh`). Refreshed at least every few merges by llvm-compare.

| step | owner | state | last measured |
|---|---|---|---|
| S1 fact placement | attrs-budgets, edit-rollback, keep-analyses | running | n/a |
| S2 fact-free run | pass-list-parity | measured: combine 0.30x to 1.03x of InstCombine at N=200 | exponent leaks: `dead` 1.43, `fold` on `mem` 1.41 (the `ThroughMemory` fact) |
| S4 allocator attribution | greedy-port, ssa-spill-m16 | step 0 measured ([#2049]); kill not met | 431 K fixed plus 309 per body instruction per round |
| S5 block-local dataflow | ssa-spill-m16 | KILLED ([#2043]) | -14.8 M but +0.36% bytes |
| S6 edit core | edit-rollback | measured; kill not met on `loop` only | `move_to` 925 M of 997.9 M primitives at N=1600 (4.32%) |
| S7 caps and pricing | hoist-bounded, greedy-port | running | forecast 2.6% / 4.9% |
| 0.1 one baseline | llvm-compare | in progress | none yet |
| 0.2 counters as tests | attrs-budgets | draft [#1948] | counter cost +0.04% (older main; re-measure) |
| 0.3 `ObjectRef` identity (spike S0) | objectref-identity | answered: not a cause on main | `LLRM_CHECK_INTERN` scrambled ids: 0 of 1,583 differ; the 131 came from a loose share key; the context-free ordering cost +0.7% and was reverted |
| 1.0 inline storage | pass-list-parity | to do | [#1956]: 259 K allocations |
| 1.1 slab | pass-list-parity | [#2022] open, ready | -4.2% instructions, -2.9% cycles |
| 1.2 dense ids | attrs-budgets, ssa-spill-m16 | [#1971] merged; [#2032] open | -1.17%; -0.38% |
| 1.3 escape phase | keep-analyses | [#2035] open, [#2037] draft | -0.29%, -0.64% |
| 1.4 backend copies | ssa-spill-m16 | [#2027] merged; [#2021], [#2038] open | -0.56%, -0.52%; peak heap -15.4%, -13.9% |
| 1.5 mask test | ssa-spill-m16 | [#2010] merged | diamond 800: 1,995 to 1,075 M; `d_faces` identical |
| 1.6 gvn, walker, lsr | greedy-port | [#1999], [#2017] merged; [#2012], [#2011], [#2020], [#2034], [#2008] open | `loop` N=1600 -65% stacked |
| 1.7 dse | edit-rollback | [#2015] merged; [#2013] open | 2.0x per doubling (`arith`), 2.17x to 2.33x (`mem`) |
| 1.8 deep nests | greedy-port | [#1977], [#1978] open | queens -O3 -23% |
| 1.9 exact ThroughMemory | keep-analyses | [#1915] merged | +1.0% (correctness) |
| 1.10 held PRs | greedy-port, attrs-budgets | [#1958] open, ready; [#1962], [#1965] drafts; [#1921] closed | per PR, stale |
| 2.1 point_values | keep-analyses | stop; 2.1b fixes the share key | asked 87% / 60% / 63% of solved; repeats 13.6 M |
| 2.2 Annotated per access | keep-analyses, edit-rollback | to do | pool 42.4 M |
| 2.3 persistent MemorySSA | hoist-bounded, keep-analyses | to do | `unmeasured` |
| 2.4 split path | greedy-port | B' killed; A' alone | B' -0.03% |
| 2.5 interval recompute | ssa-spill-m16 | closed | 0.25% realistic |
| 2.6 `Frequencies` dense | attrs-budgets | to do, after pricing PRs | `unmeasured` |
| 2.7 rollback | edit-rollback | draft [#1945] | +0.09% |
| 2.8 scaling shapes | hoist-bounded | to do | `unmeasured` |
| 2.9 Vec growth by site | unowned | to do | about 15 M |
| 3.1 facts from the change log | edit-rollback, keep-analyses | design, starts now | family 403 M own (12.8%) |
| 3.2 one pipeline pass | pass-list-parity, hoist-bounded | design, starts now | combine alone about 0 |
| 3.3 allocator on the LIR core | greedy-port, ssa-spill-m16 | design page [#2042] (draft), starts now | `regalloc` 415.5 M |
| 3.4 LIR clean-up | ssa-spill-m16 | idle skip killed (10.4 M, [#2043]); solve-cost probe next | 290 M; exit about 250 M |
| 3.5 pricing (`hoist`, `lsr` forecast) | hoist-bounded, greedy-port | probe next | forecast 2.6% (-O2), 4.9% (-Os) |
| 3.6 code quality on pressure-heavy loops | greedy-port | study first | stack operands 2.3x to 3.6x clang |

## 1. TL;DR

`d_faces` -O2 compiles in 2,903 M instructions against LLVM's 415 M (6.8x; 4.8x to 6.4x in cycles) ([synthesis]). Bounded step by step, this plan does not close that gap. It removes about half of it, and it names the work that would close the rest:

| after | `d_faces` -O2, M instructions | over LLVM's 415 M | what that row counts |
|---|--:|--:|---|
| today | 2,903 | 6.8x | [synthesis], main `21f5c9fc8` |
| phase 1 (the in-flight PRs that are ready) | about 2,660 | 6.4x | each PR's own measurement on its parent, multiplied: -8.3% |
| phase 1 plus the held PRs and drafts | about 2,510 | 6.0x | also [#1958] (an open ready PR, measured on an old main) and the drafts [#1921], [#1962], [#1965] at their old numbers: -13.6%. [#1921] is to be closed (see 1.10) and [#1962] is bundled with 2.6, so this row is an upper bound |
| phase 2 (open levers, each at its bound) | about 2,210 | 5.3x | point_values, Annotated, persistent MemorySSA, split path, interval recompute: -12% more |
| phase 3 (structural tracks, each at its bound) | about 1,460 | 3.5x | facts per query, one pipeline pass, allocator on the LIR core, LIR clean-up, pricing: -34% more |
| phase 4 (each pass at the spec's per-run budget) | 720 | 1.7x | [spec] section 1.3, a budget derived from gcc, not from clang |
| parity | 415 to 462 | 1.0x | needs budgets re-derived from clang's own phases (front end 33 M, optimizer 178 M, codegen 196 M, [readme]) |

The pools overlap, so each product overstates what the steps save together. The phase 2 and 3 rows are the plan's original bounds: 2.4 and 2.5 have since shrunk or closed (see their blocks), 3.1's saving is `unmeasured` until a family is built, and the "held drafts" row includes [#1921], which is to be closed. B1 re-derives the rows.

**Why the listed steps cannot close it.** The gap is not the number of pass runs (746 against 856). It is the cost of each run: a MIR pass run costs about 19 k instructions per IR instruction, against LLVM's 220 ([spec] 1.2). Every llrm pass sweeps the whole function with whole-function facts, recomputed 2.8 times per pass run; only 1.4% of the computes are on a body version already computed, so the ratio falls only by updating facts from the change log or asking them per query (step 3.1). An LLVM pass makes one worklist walk over a body that SROA and EarlyCSE have already shrunk. It asks per-query facts with caps, and 0.53 analysis computations per run ([spec] 1.2, [measured]). The in-flight set lowers constants: allocation, containers and quadratic loops. It does not change that shape.

**Architecture is necessary, not sufficient.** The repo's bounds for each architectural invariant take `d_faces` -O2 to about 1,500 to 2,000 M (3.6x to 4.8x); parity needs each pass at a per-run budget derived from clang (step 4.2). The study is [architecture-parity](architecture-parity.md); its spikes S0 to S7 run first (Phase A).

**What the plan puts on the critical path:**

0. Answered (spike S0): `ObjectRef` interning order is not a cause on main (`LLRM_CHECK_INTERN` scrambled ids: 0 of 1,583 differ); the 131 differing objects of the `point_values` share prototype came from a loose share key (the fingerprint equated bodies whose constants name different globals; `LLRM_CHECK_PVSHARED` fails on cached `Global(1)` against fresh `Global(0)`), which keep-analyses fixes in 2.1b.
1. Facts updated from the change log, taking analysis computations per pass run from 2.8 toward 0.5 for edit-rollback's family (step 3.1; edit-rollback with keep-analyses). The pointer families are small after the escape PRs; the family that carries the saving is edit-rollback's `Held`, `ThroughMemory` and decide facts (403 M own, 12.8%).
2. One pipeline pass with a combine worklist instead of rounds (step 3.2; pass-list-parity with hoist-bounded).
3. The allocator on the LIR core, 444 M against greedy's 39.5 M (step 3.3; greedy-port with ssa-spill-m16).
4. Each pass held to a counted per-run budget (phase 4; each pass's owner).

## 2. Baseline snapshot

Main was `ef40f30a7` when this was read and is `c37be2a9c` as of this edit; PR states below were checked with `gh` on that day. `d_faces` flags are `-m32 -march=i486 -fno-inline-functions`, as in [synthesis]. Percentages are of `d_faces` -O2 instructions, measured on the PR's own parent unless stated.

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
| [#1948] counted budgets as tests | attrs-budgets | draft | counter cost +0.04% |
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
| 1.1 slab allocator | -4.2% instructions, cycles negative | cycles not below base on a quiet host | pass-list-parity | ready [#2022] |
| 1.2 dense ids | -1.2% ([#1971] -1.17%, [#2032] -0.38%; the two overlap) | below 0.5% together | attrs-budgets, ssa-spill-m16 | [#1971] merged, [#2032] ready |
| 1.3 escape phase | -0.9% | below 0.5% together | keep-analyses | ready |
| 1.4 backend copies | -1.1% instructions ([#2021] -0.56%, [#2038] -0.52%); peak heap -15.4% ([#2021]) and -13.9% ([#2027], merged), each against main alone, stacked peak unmeasured | below 0.5% and no heap change | ssa-spill-m16 | ready |
| 1.5 mask at each branch | diamond 1,951 to 1,075 M; `d_faces` 0 | any object differs | ssa-spill-m16 | merged [#2010] |
| 1.6 gvn, walker, lsr slopes | `loop` N=1600 -65% stacked (30,626 to 10,804 M), exponent 400 to 1600 = 1.1; `mem` 7,299 to 5,969 M | `d_faces` above +0.5% together | greedy-port | ready, stacked |
| 1.7 dse slope | `dse` 2.0x per doubling on the distinct-offset and `arith` shapes, 2.17x to 2.33x on `mem` (partial) | `d_faces` above +0.3% | edit-rollback | [#2015] merged, [#2013] ready |
| 1.8 bounded on deep nests | queens -O3 -23% | any object differs | greedy-port | ready, stacked |
| 1.9 exact ThroughMemory | fixes [#1909] at +1.0% | none (correctness) | keep-analyses | merged [#1915] |
| 1.10 held PRs on main | -3% if they reproduce; [#1921] to be closed, [#1962] bundled with 2.6 | each one below 0.5% on main | greedy-port, attrs-budgets | [#1958] ready, [#1965] draft |
| B1 re-baseline | the phase-1 row of the TL;DR measured | n/a | llvm-compare | after 1.x |
| 2.1 point_values per query | stop: meets its no-effect row (asked 87% / 60% / 63% of solved); 2.1b repeats at most 0.45% (13.6 M), killed under 0.5% net | projected saving below 15 M, or the oracle differs | keep-analyses | step 0 done |
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

## Phase A: architecture spikes

Eight spikes from the [architecture study](architecture-parity.md) run before the large builds, each on a throwaway branch with its kill criterion fixed beforehand; a result goes on [#1994] and, if killed, into [spec 8]. The decision review is at day ten after they start. Phase 3 steps wait only for the spike they name.

| spike | question | owner | kill criterion | state and result |
|---|---|---|---|---|
| S0 `ObjectRef` identity (0.3) | does interning order change output? | objectref-identity | any object differs under scrambled ids | answered (PR [#2046], open): 0 of 1,583 moved |
| S1 fact placement | do the tower's facts need to be fresh at every position of every round? | attrs-budgets (step 0), edit-rollback with keep-analyses (step 1) | step 0: under 90% of fact-enabled rewrites at three or fewer positions per function; step 1: under 10% of `d_faces` -O2, computes per run above 1.0, or a row crosses above min(gcc, clang) | running |
| S2 a fact-free run against InstCombine | once the tower is not asked, does a run cost what LLVM's costs? | pass-list-parity (llvm-compare supplies the LLVM column) | above 2x InstCombine per run at N=200, or exponent above 1.05 | measured: see below |
| S3 one round with cleanup positions | can a cheap combine replace rounds 2 to 4 without losing rows? | pass-list-parity, hoist-bounded | a row crosses above min(gcc, clang) whose enabler is a global solve, or under 8% at -O2 | after S1 step 1 and S2 |
| S4 allocator attribution, then in-place spill | is the 415.5 M the representation or the decisions? | greedy-port, ssa-spill-m16 | whole-body work per edit under 1% of the compile, or step 1 cuts the cost per spill by under 3x | step 0 measured: see below |
| S5 block-local post-RA dataflow | does LLVM's block-local copy propagation fit? | ssa-spill-m16 | under 15 M saved, or a row worse | KILLED ([#2043]): block-local saves 14.8 M but `d_faces` grows 8,068 to 8,097 bytes; a single-predecessor chain gives the same output (68% against an 80% bar) |
| S6 edit core on the ladders | are the edit primitives a scaling term? | edit-rollback | edit primitives under 3% at N=1600 on every shape and no exponent | measured ([#1994]): the kill is not met on `loop` only (primitives 4.32% at N=1600, `move_to` 925 M, 4.0x per doubling, 8,001 calls from `hoist::hoisted`); the other four shapes are under 1.2% with no exponent. A6 is built beyond rollback only as far as `loop` goes: `Function::move_all(&[InstId], Position)` mirrors LLVM's range splice at the API; the intrusive order only if a slope remains |
| S7 caps and pricing | are pricing and uncapped walks the cost? | hoist-bounded, greedy-port | pricing under 1% at both levels and no walk past 100 steps | not met: forecast 2.6% (-O2) and 4.9% (-Os); massif at `mem` N=1600 is 251 MB, 57% walker jump tables (fix coming) |

**S2 result** (pass-list-parity; llrm main `4f12e5818`, round 1 of `fold`, `algebraic`, `phiopt`, `dead` own work against `opt -passes='sroa,early-cse,instcombine'` minus `sroa,early-cse`; M instructions at N=200, then 1,600): `arith` 6.01 against 20.10 (0.30x) and 62.5 against 159.0 (0.39x); `mem` 6.14 against 8.08 (0.76x) and 80.6 against 65.5 (1.23x); `diamond` 5.06 against 6.35 (0.80x) and 35.6 against 41.7 (0.85x); `mixed` 8.59 against 8.35 (1.03x) and 67.0 against 61.3 (1.09x). The 2x line passes on every shape (worst 1.03x). The exponent from 200 to 1,600, llrm and LLVM: `arith` 1.14 and 1.01, `mem` 1.24 and 1.01, `diamond` 0.94 and 0.91, `mixed` 0.99 and 0.96; it trips on `arith` (`dead` 1.43, `algebraic` 1.12, being re-measured at N=400 and 800) and on `mem` (`fold` 1.41, whose cost is `Analyses::get::<ThroughMemory>`). Verdict: the rewriting is not the gap, the facts are; the superlinear passes are `gvn` (1.32 to 1.56) and `dse` (1.72 to 1.97), not the combine. The exponents come from two points, with analyses excluded from "own".

**S4 step 0 result** (ssa-spill-m16, [allocator on the LIR core](allocator-lir-core.md), merged as [#2042] and [#2049]): one function of K sequential copies of a 24-statement loop, at about fixed decisions per copy: cost per allocator round 490, 488, 598, 805, 1,181 and 1,871 K at 1x to 32x (bodies 191 to 4,664 instructions); a round is about 431 K fixed plus 309 instructions per body instruction (R^2 above 0.99). The kill (cost per round does not scale with body size) is not met; at 191 instructions the fixed part is 88% of a round; at `d_faces`' large function (1,819 instructions) the fit gives 993 K per round, 57% size-proportional, while the real function costs about 2.0 M per round: the split path is the extra. Decisions on `d_faces`: 837 assignments (LLVM 815), 67 evictions (78); knock-on unassignment 104 of 7,694 corpus rounds (1.4%), 0 on `d_faces`. The fixed part at 1x (37.22 M, 490 K per round): `Facts::of` 9.04 M, `spilled_apart` 5.15, the evict search 4.41 (`movable` 2.51, `LiveRanges::weight` 1.35: LLVM stores the weight in the interval), `recolor` 3.32 (a stage greedy does not have), a `BTreeSet<RegId>` rebuilt per pop 1.94, split 2.56 and carve 1.68, the rest 8.3. Work on a fact that did not change is about 12.3 M (33%, prove-once candidates); decision work about 10.2 M (27%). The fixed 431 K per round is about 20 pieces of 10 to 100 K. greedy-port's measurement of LLVM greedy (on 20.1.8; a re-measure on the reference build is pending): per decision 13.3, 22.3 and 32.5 K at 191, 1,305 and 4,664 instructions and 29.9 K on `d_faces`; llrm per round 490, 805 and 1,871 K, 36 to 58x; LLVM is not flat either (x2.4 for x24, from interference queries).

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
- **(b) Early:** counter cost on `d_faces` -O2. Kill if it is above 0.1%. [#1948]'s +0.04% was measured earlier on `d_faces` -O2: re-measure on current main before the PR is ready (a checklist box below).
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
6. A per-pass-run arena: `unmeasured`, it fits the lifetime profile; size it as a probe after 1.0.

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
  - the joins and unrolled axes no steeper than gcc's (1.81 and 1.80 against 1.41 and 1.04, [spec] 1.3);
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
- **(b) Early:** step 1 was built and measured before this plan (branch `notkept/combine-worklist`, `fba7c4051..fea9b27f0`): merging fold, algebraic, phiopt, dead and the branch fold of decide into one worklist pass at today's positions and rounds gave about 0 compile time, because the cost is analyses and idle reruns, not rewriting (no spec 8 entry yet; the owner re-measures and writes it). Treat step 1 as expected 0, not part of the -10%. The risky part is dropping repeat runs: round-1-only per pass changes output (`hoist` at least 20 objects, `fill` at least 20, `loopsimplify` 17, `promote` 6, `trivialunswitch` 2; all five together -3.0% at -O2 and -6.0% at -Os on `d_faces`), and the enabling relations are dense (`dead`, `gvn`, `dse`, `lcssa` and `fold` enable nearly everything); global `rounds=1` lost 25, 32 and 51 rows at -O1. So the early check is a per-pass ablation: run pass P in round 1 only, list the objects that change, name the enabling pass. Then:
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

- **(a) Effect:** stack operands are 2.3x to 3.6x clang's on pressure-heavy loops (83, 733 and 3,172 against 23, 315 and 1,218 at the three sizes of the `live` axis): llrm spills 51, 443 and 1,215 values where LLVM splits (9, 101 and 301 new virtual registers) and rematerializes. This is output quality, not compile time; it moves the rows rule of [spec] 4.3.
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
[#2046]: https://github.com/ali-mosavian/llrm/pull/2046
[#2049]: https://github.com/ali-mosavian/llrm/pull/2049
[#2043]: https://github.com/ali-mosavian/llrm/pull/2043
