# Shared liveness (POC, no code)

TL;DR: callers of `live_rows_by` cost about 4 M Ir of d_faces' 827 M (m32 -O2, 2026-10-27); sharing one result across scheduler and allocator is worth at most that, under 0.5% of the compile. It does not explain the scheduler's 6 M excess over the pin.

## Who computes liveness today
Each asks for rows of the values it cares about, from the body it was handed:

| caller | values asked | d_faces inclusive Ir |
|---|---|---|
| machinesched `scheduled` | values of tracked blocks | 1.7 M (3 bodies) |
| `regclass::collected` (model of the scheduler, others) | classes, not rows | 6.2 M (not liveness) |
| `allocate::live_rows` | every value | 0.8 M |
| floatassign `prepared` | float values | 0.7 M |
| fpstack `converted` | float values | 0.6 M |
| phielimination `edges_to_split` | phi sources | 0.5 M |
| coalesce, storedhomes, intervals, occurrences | their own sets | not in the top lines |

## What invalidates a result
Rows are block live-in/out per value. A pass keeps them if each block's set of defs and uses is unchanged: reordering inside a block (the scheduler, sinks within a block) keeps them. Inserting, deleting or renaming an operand (coalescer, phi elimination, spill, copy insertion) drops them. Passes return a new `LirBody`, so a cache in `body.facts` is dropped by default; a pass that reorders only would have to hand it on.

## Upper bound and cost
The saving is the repeated walks: the scheduler's 1.7 M and the allocator's own, once the scheduler and `RegAlloc` see the same rows (they are adjacent after #5036). The rows differ in the value set each caller asked for, so one shared result must cover the union, which the restriction experiment (branch w2-ms-live-restrict, not pushed: 16.46 M -> 16.61 M) says is no cheaper than the single walk.

Invalidation costs a flag per pass (`preserves_liveness`) and a check under `LLRM_CHECK_LIVE` that compares a kept result with a fresh walk.

## Recommendation
Not worth a body-level cache for compile time alone. The pin's `LiveIntervals` is built once because its passes update it incrementally; ours rebuild bodies. Revisit with the LIR data model, where a kept result becomes natural.
