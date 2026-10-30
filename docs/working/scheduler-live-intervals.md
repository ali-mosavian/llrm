# MachineScheduler on the shared LiveIntervals — design note

Measured on d_faces m32 -O2 (dist, `callgrind-check` clean, toggle on the pass): MachineScheduler 16.9 M against the pin's 10.4 M. 5.0 M of ours is liveness the pin charges once to `LiveIntervals::analyze` (8.4 M): `live_rows` 2.18 (+ closure 1.25), `regclass::collected` 0.73, `liveness::worked_out` 0.70, `liveness::effect` 1.12 (memoized per instruction already: 4,826 distinct of 29,574 calls). Without it ours is about 12 M, +1.5 M on the pin.

## The pin

- Order: `TwoAddressInstructionPass`, `RegisterCoalescer` (TargetPassConfig.cpp:1525-1526), then `MachineScheduler` (:1534). LiveIntervals is built once, before the coalescer, and every later pass reads it.
- `MachineSchedulerLegacy` requires and preserves it (MachineScheduler.cpp:425-426); every moved instruction is told to it (`LIS->handleMove`, :1035); the pressure tracker reads intervals (`RegisterPressure.cpp` `getLiveLanesAt`, :443).

## Ours

- Order: `PhiElimination`, `MachineScheduler`, `FloatPrep`, `FloatAlloc`, `TwoAddress`, `Coalescer`, `RegAlloc` (flow.rs). The scheduler runs before the coalescer, so it has no `LiveIntervals` to read: the coalescer builds one and hands it to the allocator (`liveness::liveintervals::hand_over` / `take_handed_over_edited_to`).
- The scheduler asks for liveness only for a region of more than limit/2 instructions: `live_rows` ran 3 times in the whole compile, all in the scheduler. There is no second consumer of those rows to share them with.

## Steps

1. Land #5036 (the scheduler after the coalescer, the pin's order). It is parked on the allocator side (x_dct m32 -O2 +80, w13).
2. `LiveIntervals::handle_move` (LiveIntervals.cpp `handleMove`): a moved instruction's indexes and the ranges that touch it are updated, so the allocator's handed-over analysis survives the scheduler (today any pass between the coalescer and the allocator makes the allocator rebuild unless `edited_to` applies).
3. The scheduler reads live-out and the classes from the handed-over intervals instead of `live_rows`/`regclass::collected`.

Ceiling: 2.2 M for step 3 alone (0.24% of d_faces); the allocator's rebuild saved by step 2 is w13's number to state. Not started: step 1 blocks it.
