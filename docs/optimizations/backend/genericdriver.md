> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/genericdriver.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/genericdriver.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/genericdriver.rs sha256:088406900bf2039df09717e8bb83c68283fb569a0147a537d63067ab84c1da7b -->

# genericdriver

`GenericScheduler` over a dependence graph: the loop that picks a node from
the top or the bottom zone, schedules it and releases its neighbours (row
65, S2 of ARM64-SCHED-HOIST).

## How it works
Roots with no predecessor start in the top zone, roots with no successor in
the bottom zone. Each step picks a node (`Scheduler::pick_node`): in a
one-direction region from that zone; in a bidirectional one the zones'
best candidates are compared across zones. The node's zone advances its
clock (`bumpNode`) and the node's neighbours in that direction are
released once all of their other neighbours are scheduled, with the ready
cycle the edge's latency gives. The order is the top nodes in the order
picked, then the bottom nodes in the reverse.

This is `GenericScheduler::pickNode`, `pickNodeBidirectional`,
`pickNodeFromQueue`, `setPolicy`, `schedNode` (MachineScheduler.cpp#L3254-
L4300) and `ScheduleDAGMI::schedule`, `releaseSucc`, `releasePred`.

## Not translated
- SB3: `ExitSU` as a node. The terminator stays a schedulable node, so it
  takes an issue slot the pin's `ExitSU` does not. The edge it holds for an
  unread def (`buildSchedGraph`, latency less one) is translated for a model
  that asks (`SchedulingModel::exit_edges`, arm64); the x86 models do not
  and keep the deepest root as the critical path.
- SB4: `computeCyclicCriticalPath`: `IsAcyclicLatencyLimited` stays off. It
  is only asked of a model with a micro-op buffer (Cyclone), not the
  in-order A510.
- SB5b: the fast-cluster path of `BaseMemOpClusterMutation` (threshold
  1000).
- SB5: macro fusion, and `tryBiasPhysRegs`/`reschedulePhysReg` (no region
  holds a physical register, MS4).

- SB9: a pre/post-indexed access reports no memory operand to the cluster
  mutation (`getMemOperandsWithOffsetWidth`); it exists only after
  `ldst_update`.

Owner arm64-phi; exit with the model branch that first needs each.

## Example
A region where `A` and `B` have no predecessor, `C` reads both and `D` reads
`C`. Picking `A` (top), `D` (bottom), `B` (top), `C` (bottom) gives the
order below (node names are illustrative).

```text
top picks:     A  B
bottom picks:  D  C
order:         A  B  C  D     ; top in pick order, then bottom reversed
```

## Used by
Named by `machinesched`, `schedmodel`.
