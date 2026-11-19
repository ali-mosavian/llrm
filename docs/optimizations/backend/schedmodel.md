> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/schedmodel.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/schedmodel.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/schedmodel.rs sha256:db625c955a78e0c08f387fce24a8a422fb17f0022eb6ca4d336fc86ac2b8bbf8 -->

# schedmodel

What a target tells the pre-RA scheduler about its processor:
`TargetSchedModel` of the pin, as the generic scheduler asks it.

## How it works
A `SchedulingModel` gives the zone's machine (issue width, micro-op
buffer, resources), the region policy (`overrideSchedPolicy`), each
instruction's `Unit` (`SUnit`: micro-ops, resource writes) and the
latency of what it defines (`computeInstrLatency`, which the dependence
graph puts on its data edges). `GenericModel` is X86.td's `GenericModel`,
which also stands for a target that states none.

## Example
```text
GenericModel: issue 4, buffer 32, no resources; a load defines in 4
cycles, a copy in 0, the rest in 1; bottom-up only.
```

## Used by
[`machinesched`](machinesched.md).
