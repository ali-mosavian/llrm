> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/nearcode.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/nearcode.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/nearcode.rs sha256:158bce5d7a710c8503e39cc685270eaca96fc8770b2ff1daa38fd676d482c72a -->

# nearcode

Puts a function a frontend allows near, which only its own module calls and
only directly, in the near code space, so callers enter it by a near call.

## How it works
MIR states linkage and calls; which instruction enters the code is a target
fact, so the move happens here. `placed` takes the functions
`llrm_mir::callgraph::direct_only` returns (defined, internal or private,
not an interrupt handler, never named but as a callee), keeps those with
the `nearcode` attribute that are not already in the target's near space,
and sets their address space to it. A module with none is returned as it
is, borrowed.

```text
  internal @f  (far space, nearcode, only called directly)
      call @f  was far call, return address with a segment
      call @f  now near call, shared code segment, no segment
```

A far caller needs no segment in the return address because every caller is
in the same module, and so in the same code segment.

## Example
No honest example: the pass changes the address space of MIR functions and
touches no LIR. Read `test_*` in this file for the MIR it accepts.

## Runs
Once per module at the start of `assemble::assembled_by` (`mir near code`),
before global names are made and before any function is selected. Whether
the attribute is set is the frontend's decision.
