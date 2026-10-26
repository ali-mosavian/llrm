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

## Handover to w12 (2026-10-25)

`machinesched.rs` moves to w12; w9 writes `regpressure.rs`. Branch `lir-cleanup-port/misched-faithful` holds piece 1 (live-through values counted in `scheduled`, the MS3 peak guard dropped; `peak` deleted). Its pair (base 121f6f17ef) was not read: its rows, sizes and objsweep were still running. What is left is pieces 2 to 4 above: swap `pressure_after` for w9's tracker, then `ReduceLatency` and the exact `tryCandidate` order. The pin is bottom-up only on this target, so there is no top-down side to write.


## Remaining deviations from the pin's GenericScheduler (w12, 2026-10-25, after #4735)

Landed by #4735: bottom-up only, `tryCandidate` (`RegExcess`, `RegCritical`, `RegMax`, `NodeOrder`) over the per-set tracker (`regpressure.rs`), `initLiveThru`, `RegionCriticalPSets`, pressure tracked only above half the integer registers. Whole-scheduler effect, 71 programs m32 -O2, static instructions (pincount, on the old pin triple with stack alignment 16: redo after #4780 before quoting): dag +0.4% vs the pin's `llc` (before: +5.9%). No item has been ablated alone (an ablation needs its own switch).

Order to land (cheapest evidence first):

1. MS4 regions. We end a region at fixed registers, clobbers, groups, volatile accesses, barriers, spill markers, unknown effects and x87 boundaries; the pin orders them by edges and ends regions only at calls, terminators, labels and SP writes. Effect: scheduling room. Measure the volatile share first (75 cuts): instruction delta with the cuts removed.
2. Memory edges without alias analysis. Every store orders every later access; the pin's `buildSchedGraph` uses underlying objects and AA. Effect: room again, mostly in loops with two arrays; waits on the AA row (`accesses-basicaa-plan.md`).
3. Pass position. Ours runs before TwoAddress (tied defs through `twoaddr::ties`); the pin after TwoAddress and the coalescer, so the pin sees the copies and their pressure. Effect: pressure counts differ by the tied copies; move with the TwoAddress reorder (`twoaddress-reschedule-plan.md`), not alone.
4. `tryBiasPhysRegs`. Needs a physical-register copy graph; with item 1 the copies at a call or a return sit outside the region. Effect: argument and return copies; unmeasured.
5. MS6 `Rem.IsAcyclicLatencyLimited`. The pin turns on its height/depth heuristic in a loop whose in-flight micro-ops pass the buffer (32). Effect: loops of 32+ instructions only; unmeasured.
6. `pressuresink.rs` still runs before the scheduler (plan piece 4). Delete it once rows show nothing worse; its loads are what the scheduler places.
7. MS5 targets with no pressure sets (m16) count every register as one set; m16 has no `pressure()` data.
8. The DAG's source-order flag (`LLRM_SRC_ORDER`, #4717): off is the default, the right one on the same measurement.
9. The DAG scheduler (`ScheduleDAGRRList` priority functions: source/hybrid/ILP; `schedule.rs`). First differing BURR pick on x_dct (w13): #8 of 74, the pin picks the SUB, ours the ADD, with equal Sethi-Ullman numbers and heights; the pin's depth/FIFO tie-break matches neither sign of our depth rule. Owner: w13 found it; w12 translates the priority functions.

Not applicable on the i486 (no model): clusters, weak edges, stalls, resources, `ReduceLatency` (never set bottom-up). Compile-time effect (d_faces) of the scheduler and of each item: not measured; the pass runs on regions over half the registers only.
