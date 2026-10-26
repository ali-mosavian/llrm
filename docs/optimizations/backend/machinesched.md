> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/machinesched.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/machinesched.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/machinesched.rs sha256:296117de365e965a64737a6edd0906e78594d37c1f21dd33a988a817daa07564 -->

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

## Runs
`MachineScheduler`, a machine phase after `phielim` and before
`floatfold` (`machinesched@5`): each region longer than half the allocatable
registers is ordered by `bottom_up` (pieces 2 and 3 of
docs/working/machine-scheduler-plan.md). DEVIATE MS3: values that pass
through a block are not counted in its pressure, and one limit stands for
every register set.
