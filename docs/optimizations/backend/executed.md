> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/executed.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/executed.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/executed.rs sha256:f13c335a5d3fbe19fae5d9ef981fb9725b08c88c0361746397c94be53abb9658 -->

# executed

A finished body's executed work per call, estimated without a profile.

## How it works
[`executed`](executed.md) weighs each instruction by how often its block runs.
`analysis::frequency` gives each block's expected executions per call
(the branch heuristics, a loop's proven trips) and each edge's odds.
Because the body is the allocator's output, spill code the MIR
estimate cannot see is counted.

```text
  block B, runs = 10
    mov  ax,[spill]    reaching 10   -> instructions += 10, reloads += 10
    cmp  ax,bx         reaching 10
    jl   L  (odds .3)  reaching 10   -> branches += 10, taken += 3
    mov  [x],ax        reaching  7   (runs less those the branch took)
```

Only instructions that print bytes count (anchors do not). Per
instruction the pass adds to: `instructions`; `memory`, one per memory
operand; `reloads` (a spill-reload, a read of a slot that some spill store
writes, or a read of an argument's incoming home in a block that runs more
often than the function); `stores`; `remats`; conditional `branches` and
how many are `taken`; and `jumps`. x87 spills are left out. A
body with an irreducible loop has no finite estimate: the result is none.

`work` is `instructions + memory`, the number the allocator and the
route choice compare alternatives by (`allocate::_emitted`, and the cost of
a candidate in `assemble.rs`, in bytes at -Os). `Executed::jump_cycles`
prices the branches and jumps on a CPU profile: taken branches, untaken
branches and short jumps each at their form's cost.

`predict` records what the MIR spill model forecast for a function;
`pressure` prints that forecast beside the reloads and stores the
allocator left.

## Runs
Not a pass: `assemble.rs` calls it on each finished body to choose
between routes, at every -O level; `LLRM_DEBUG=cost` and
`LLRM_DEBUG=pressure` print it.

## Provides
`Executed`, `work`, `summary` (the `cost` debug channel's line,
`LLRM_DEBUG=cost`) and `pressure` (`LLRM_DEBUG=pressure`).
Consumers: `allocate.rs`, `assemble.rs`.

## Needs
`crate::analysis::frequency`, `crate::analysis::loops` and
[`crate::backend::cpu`](cpu.md) for jump prices.

## See also
The phase map: `crate::backend`
