> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/executed.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/executed.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/executed.rs sha256:1de49df108c7c0d8570c17829bbff88da3828ab4ec6bbc51bce8b9c8d4f99c8c -->

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
route choice compares alternatives by (the cost of
a candidate in `assemble.rs`, in bytes at -Os). `Executed::jump_cycles`
prices the branches and jumps on a CPU profile: taken branches, untaken
branches and short jumps each at their form's cost.

`predict` records what the MIR spill model forecast for a function;
[`pressure`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/licm/pressure.rs) prints that forecast beside the reloads and stores the
allocator left.

## Example
A QB `__main` that fills `S(0..100)` and sums it, two loops of 101 trips,
on `-m16 -march=i386 -O2`. The allocator's output, as masm prints it:

```asm
L0_2:
    mov word ptr S%+202[ebx*2], ax
    inc ax
    inc ebx
    jne L0_2
; ...
L0_15:
    add ax, word ptr S%+202[ebx*2]
    inc ebx
    jne L0_15
```

`LLRM_DEBUG=cost` prints what `summary` makes of it:

```text
[cost] __main executes 717 instructions, 206 memory operands; 0 reloads, ...; 202 branches, ...
```

The loops weigh 4 and 3 instructions at 101 each, 3 + 3 set up before them
and 4 after: 404 + 303 + 6 + 4 = 717. Each `jne` is one of the 202 branches.

From `LLRM_DEBUG=cost` of that program.

## Runs
Not a pass: `assemble.rs` calls it on each finished body to choose
between routes, at every -O level; `LLRM_DEBUG=cost` and
`LLRM_DEBUG=pressure` print it.

## Provides
`Executed`, `work`, `summary` (the `cost` debug channel's line,
`LLRM_DEBUG=cost`) and [`pressure`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/licm/pressure.rs) (`LLRM_DEBUG=pressure`).
Consumers: `allocate.rs`, `assemble.rs`.

## Needs
[`crate::analysis::frequency`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/scev/analysis.rs), [`crate::analysis::loops`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/scev/analysis.rs) and
[`crate::backend::cpu`](cpu.md) for jump prices.

## See also
The phase map: `crate::backend`
