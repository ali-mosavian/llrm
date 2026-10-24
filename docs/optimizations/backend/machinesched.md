> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/machinesched.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/machinesched.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/machinesched.rs sha256:fc2615aad48971d72cef025dba6b127a8db9814650bc3c4da2a1560b57c6f2d7 -->

# machinesched

The dependency graph of a scheduling region of LIR, before allocation:
`ScheduleDAGInstrs` for the pre-RA machine scheduler (row 65).

## How it works
A block is cut into regions at the instructions the scheduler may not move
across or reorder: terminators and calls, barriers, anything with a fixed
register or a clobber, a group, a spill or frame marker, a volatile
access, an instruction of unknown effect, and an x87 operation that
`floatregions::boundary` calls one. Within a region an edge runs from an
earlier instruction to a later one that must stay after it:

```text
 value        def -> use (data), use -> def (anti), def -> def (output)
 lanes        the same on the physical lanes and flags `liveness::effect` reads and writes
 memory       a store -> every later access, a load -> every later store
```

This is the pin's `buildSchedGraph` (MachineScheduler.cpp,
ScheduleDAGInstrs.cpp) without alias analysis: two memory accesses where one
writes are ordered.

## Example
`x = load [m]; y = x + 1; z = load [n]; store [m], y`: the first load feeds
the add (data), the add feeds the store (data), both loads precede the store
(a load before a store), and the load of `n` has no edge to the first
load or the add.

## Used by
The bottom-up list scheduler (pieces 2 and 3 of
docs/working/machine-scheduler-plan.md).
