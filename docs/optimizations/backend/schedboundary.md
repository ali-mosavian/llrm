> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/schedboundary.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/schedboundary.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/schedboundary.rs sha256:c685f2640d8bddec157bf03b2e11e3432ea0e3168b30a48086258f1522198f63 -->

# schedboundary

`SchedBoundary`: the clock, the ready queues and the resource counts of one
side of a scheduling region (row 65, S2 of ARM64-SCHED-HOIST).

## How it works
A zone grows from the top or the bottom of the region. It holds the nodes
that may be scheduled next (`Available`), the ones that are ready in the
dependence sense but would stall (`Pending`), the current cycle, how many
micro-ops issued in it, and how many units of each processor resource the
scheduled nodes used, scaled so resources with different numbers of units
compare (`resource factor`).

```text
releaseNode  ready at cycle c > now on an unbuffered machine  -> Pending
             hazard (issue width, group, a reserved resource)  -> Pending
             else                                              -> Available
pickOnlyChoice  release pending, defer new hazards, bump the clock until
             something is Available; one node there is the only choice
bumpNode     advance the clock for stalls, count micro-ops and resources,
             reserve unbuffered resources, move ExpectedLatency
```

This is the pin's SchedBoundary (MachineScheduler.cpp#L2492-L3198) and the
factors of `TargetSchedModel::init` (TargetSchedule.cpp#L53-L66).

## Not translated
`ScheduleHazardRecognizer` (the pre-RA AArch64 scheduler has none) and
`ResourceSegments` (`enableIntervals`, off for A510 and Cyclone). DEVIATE
SB1, owner arm64-phi, exit with the first model that turns them on.

## Used by
Named by `genericdriver`, `machinesched`, `schedmodel`.
