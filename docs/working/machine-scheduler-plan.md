# Machine Instruction Scheduler (codegen row 65)

TL;DR: translate the pin's pre-RA `GenericScheduler` bottom-up pass in four landing pieces, 11-06; `pressuresink.rs` goes when it does. The pin's pass is worth +0.13% / +0.19% clocks (geomean, -O2 / -O3) on i486 and up to +7% on x_list (`docs/working/premise-peephole-sink-scheduler.md`).

target: i486-unknown-linux-gnu, cpu i486 (no schedule model, no itinerary): the default `MCSchedModel` (in order, issue width 1, no buffer). Latencies (`TargetInstrInfo::defaultDefLatency`, `TargetSchedModel::computeOutputLatency`): a copy or other transient instruction 0, a load 4 (`DefaultLoadLatency`), any other def 1; output dependence 1, anti 0, memory chain 0. Not "1 per instruction" (corrected 2026-10-25): heights and depths, and so `ReduceLatency` (the scheduled cycles plus the remaining chain over the critical path), follow these. No resource limits.

## The pin, at d1106deb71cc

- `GenericScheduler::initPolicy` (MachineScheduler.cpp#L3688): bottom-up only; pressure tracked when the region has more than half the integer registers' worth of instructions.
- `tryCandidate` (#L3965), in order: `tryBiasPhysRegs`, `RegExcess`, `RegCritical`, cluster, weak edges, `RegMax`, resource, latency (`ReduceLatency`), then original order (`NodeOrder`: later first when bottom-up).
- `ScheduleDAGInstrs` builds the region's graph: data, anti and output edges on registers, chain edges for memory and side effects, barriers at calls, terminators and labels.

## Pieces

1. `machinesched.rs`: the region graph over LIR (values, physical lanes and flags from `liveness::effect`, memory by `may_write`, boundaries at calls, terminators, fixed registers, groups, spill and frame markers). Unit tests on hand-built blocks, red first. No reordering yet. 11-02.
2. Register pressure of a region (live-in/out from `live_rows`, classes from `regclass`), the `RegExcess`, `RegCritical` and `RegMax` deltas. 11-04.
3. The bottom-up list scheduler with `tryCandidate` (latency 1, `NodeOrder`), wired before `phielim`'s successor where `pressuresink` runs; a check that the order respects the graph. 11-05.
4. Delete `pressuresink.rs` once rows show nothing worse (its loads are what the scheduler places). 11-06.

## Not translated (numbered, on the PR that lands the piece)

- MS2: clustering and weak edges (no load/store pairing on x86), `tryBiasPhysRegs` (no copy bias without a physical-register copy graph yet), the resource model (i486 has none).
- MS1 (no scheduler) ends with piece 3.
- P5 pairing is the post-RA `schedule.rs`; the pre-RA pass has no P5 model either (the pin's i486 and P5 differ only in the model, which P5 would add).

Rows: x_list, sieve, matmul (the pin's largest scheduler effects), the 71 rows in clocks, QCport -Os bytes, spill marks in our listing (a queued instrument) before and after.

## Against the pin, 2026-10-25 (w12's list checked)

| Claim | Verdict |
|---|---|
| Ours is bottom-up only, the pin bidirectional | Wrong. `initPolicy` sets `OnlyBottomUp = true` (#L3711) and X86 has no `overrideSchedPolicy`, so the pin is bottom-up on this target too. |
| A region of at most half the allocatable registers is ordered only if it has a load | Intended, same result. Without a schedule model and without tracked pressure every candidate is ready at once and `NodeOrder` (latest first) gives source order; a load's latency 4 is the one thing that changes it. |
| DEVIATE MS3: a reorder is kept only if it lowers the peak | A real deviation: the pin takes the order its heuristics make. |
| Live-through values not counted | A real deviation: the pin's tracker starts from every value live out of the region. |
| One limit for every set | Partly: the GR32 sets of the x86-32 table have limits 6 (before reserved registers), so one limit is near; x87 (RFP32 7) and the 8/16-bit sets differ. |

Pieces to make it the pin's (each a PR, priced by the 71-program static diff and AM-BT with `OBJSWEEP_FLAGS=--isel=dag`):

1. Count live-through values and take the order the heuristics make (drop the MS3 guard).
2. A `RegPressureTracker` per pressure set from the target's table (`RegExcess`, `RegCritical`, `RegMax` on the set deltas, not the total).
3. `ReduceLatency` and the `tryCandidate` order exactly (`tryBiasPhysRegs`, cluster, weak edges stay out: MS2).
4. Delete the remaining shortcuts (skip of regions without a load) once 1 to 3 show the same lists.
