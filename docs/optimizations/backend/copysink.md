> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/copysink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/copysink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/copysink.rs sha256:f04dc13d6d20094b14b4e0d4aceaee623738a1fea11efd33191530db008f3a40 -->

# copysink

Move a register copy out of a loop to its exit when only code after the
loop reads it.

## How it works
A phi whose value is computed in a loop and read after it becomes a
copy inside the loop, run every trip to hand over a value nothing in
the loop looks at. The copy belongs at the top of the exit block.

```text
  H: ...                           H: ...
     mov dx, ax                       mov dx, ax
     mov di, dx    (read only    =>   cmp ax, bx
     cmp ax, bx     after the         jl  H
     jl  H          loop)          X: mov di, dx     <- once
  X: ... di ...                       ... di ...
```

A loop is considered when it has one way out, from one block to one exit
block that has no other predecessor. Within it a candidate is a plain
`mov reg,reg` (same width, distinct, no constraints, relocation or spill
marker) in a block that dominates the exiting block, so every way out
has run it. Without that a loop left from its header before the first
trip never ran the copy, and the destination must keep what it held.
Lane sets (byte lanes of the registers, from decoded effects) decide the
rest:

- The destination is dead on every way round the loop, nested loops
  included: a backward liveness over the loop's blocks, with the exit edge
  left out because the exit's read is what the sink is for.
- Between the copy and the exit, in its block and every loop block on a path
  that avoids the copy's block, no instruction writes the destination or the
  source, or reads the destination.

A copy that passes is removed and put first in the exit block, in the
order found.

## Runs
Inside the peephole phase, after copy propagation and extension
rewrites, before `spillforward`. Once per peephole run. No flag of its own.

## Needs
`crate::analysis::loops` for loops and dominance, and
[`crate::backend::liveness`](liveness.md) for the backward walk.

## See also
`crate::backend`. `copy_of` and `touches` are shared with
`postrasink`.
