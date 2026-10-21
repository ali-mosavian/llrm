# Compile ledger plan: w-1's buckets (a) regalloc glue and (b) scalar, 2026-10-21

target: clang -m32 -march=i486 (triple i386, cpu i486, no SSE/MMX by arch); datalayout e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32 (ledger `~/scratch/results/ledger-c4fade734-O2.txt`).

Measured: callgrind Ir of `llrm-c -m32 -mabi=sysv -march=i486 -O2` over d_faces (the ledger's command), dist build of c4fade7345 (llrm-c md5 68c5bd191371): 1,313.9 M (ledger 1,317.7 M). Profile `~/scratch/w1-ledger/cg-c4fade.out`; callee lists `ann100.txt` there. Pin numbers are the ledger's. Every PR below carries Ir before/after on dist and objects identical (objsweep) or a bytes/clocks row; nothing buys compile time with quality. Dates are PR-open dates.

## (b) scalar: EarlyCSE 42.3 / DSE 15.0 / MemCpyOpt 14.7 (ours 72.0 M; pin 7.8 + 2.9 + 2.0 = 12.7 M)

One cause: MemorySSA. `Accesses::run` is built 33 times (EarlyCSE 22, DSE 3, MemCpyOpt 11 asks) for 51.5 M; 41.8 M of that is `alias::calls_annotated` (`points_to` 15.5 incl. `escapes` 88 solves, `_unknown_base` 10.3, `SliceSet::absorb` 5.5, closure 4.9). In the pin EarlyCSE, DSE and MemCpyOpt each `PA.preserve<MemorySSAAnalysis>()` (EarlyCSE.cpp#L1826, DeadStoreElimination.cpp#L2844, MemCpyOptimizer.cpp#L2249), so one build serves the run of them.

| pass | ours M | pin M | top callees (M) |
|---|--:|--:|---|
| EarlyCSE | 42.3 | 7.8 | MemorySSA build 22.7, Walk 9.6 (simplify 4.6), `get<MemorySsa>` 4.7, `Function::fork` 2.6 |
| DSE | 15.0 | 2.9 | MemorySSA build 7.7 + `Mssa::build` 1.0, `eliminate_dead_defs_of` 3.6, `fork` 1.4 |
| MemCpyOpt | 14.7 | 2.0 | `get<MemorySsa>` 10.9, `Pointers::run` 3.2 |
| GVN | 40.7 | 49.2 | at or below the pin: no lever |
| SROA | 9.4 | 10.0 | at the pin: no lever (w-5 has SCCP) |

Update 2026-10-21: EarlyCSE already updates and keeps MemorySSA; 11 of its 22 runs are `earlycse@1180` (EarlyFPM), which the pin runs without MemorySSA (`EarlyCSEPass()`, PassBuilderPipelines.cpp#L1180) and ours built it for. Translated (`EarlyCse { memory_ssa }`, EarlyCSE.cpp#L1074): measured d_faces -O2 1274.7 M -> 1262.3 M (-12.4 M, -0.97%, `tools/abmeasure.py`, dist). Split with w-7 (funcattrs-port): they own levers (1) and (3), I own (2).

Levers, in order: (1) 11-02: one `calls_annotated` solve per function version, shared by MemorySSA's build and the call-effects analysis (`memoryssa.rs:475` solves again beside `manager.rs:1020`; to be confirmed in the first step), target -20 M or more of the 41.8; (2) 11-04: EarlyCSE, DSE and MemCpyOpt preserve MemorySSA to the next pass as the pin's do (builds 33 to about 15), target -15 M; (3) 11-06: `escapes` (17.5 M, 88 solves) and `_unknown_base` (10.3 M, 553 calls) memoized per value for the life of the analysis (the pin's capture cache in the AAQI), target -10 M. EarlyCSE's `Function::fork` (4.4 M with its drops) goes if (2) leaves the copy unused.

## (a) regalloc glue (ours 31.5 M for these three; the ledger bucket is 184.6 vs 67.2: RA 127.4 is splitkit's, Coalescer 25.8 shared)

| pass | ours M | pin M | top callees (M) |
|---|--:|--:|---|
| PhiElimination | 15.0 | 2.4 (+ LiveVariables 4.0) | `eliminated_priced` 8.2 (one body: split edges 2.0, `_widths` 1.2, depths 0.8, `ssaspill::floating` 0.7), `lowered` 6.7 (`live_rows` 1.9, `_widths` 1.1, split edges 1.0, `floating` 0.8) |
| TwoAddress | 8.6 | small (not listed) | `live_rows` 2.3, `_interference_among` 1.4, `_reused` 1.0, `_next_value` 0.9 (6 whole-body scans) |
| ParallelCopy | 7.9 | none | `liveness::dead_at_exit` 6.5 in one call |

Correction: ParallelCopy's `dead_at_exit` is the first from-scratch solve of physical-lane liveness after allocation (later phases update it incrementally), so replacing it saves nothing; the cost is the per-instruction effect decode, `peephole::_register_effects_worked` 14.1 M (table decode 7.5 M, its cache `EffectsHeld` 5.8 M for a 47% hit rate: the cache barely pays).

Levers: PhiElimination (1) #3441 removes `eliminated_priced` for the phis it kept (blocked on the base gap QO-1/QO-6; -25 M measured on the dist pair); (2) 11-02: `_widths` and `ssaspill::floating` once per body, shared by both stages (-2.5 M); (3) 11-04: one `live_rows` per body shared by PhiElimination, TwoAddress and ParallelCopy (-3 M). Lane-liveness effect decode: 11-06 a cheaper key or no cache for `_register_effects_worked` (target -4 M, objects identical). TwoAddress: 11-06 `_next_value` kept as a counter on the body instead of a scan per call (-0.9 M).

Not claimed: Coalescer 25.8 (shared with splitkit), RA 127.4 (splitkit).
