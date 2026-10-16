> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/parcopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/parcopy.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/parcopy.rs sha256:db9fd2f45e4640a569169b76ec1e45ffabeab152c3134b9dc68022c49e871da7 -->

# parcopy

Writes each parallel copy group (the copies a phi becomes on an edge, which
happen at once) as a sequence of moves that computes the same thing.

## How it works
After allocation which copies conflict is a question about locations. A
group is the run of instructions that share a `group` number. Inside it:

1. An identity copy (same place both sides) emits nothing but stays as an
   anchor, because it still defines the virtual value its destination stands
   for.
2. A copy is ready when no other copy still to come reads the place it
   writes (`ready`). Ready copies are written, in their order, and the step
   repeats.
3. When none is ready, every remaining copy waits on another: a cycle
   (`in_cycle`, `_rotated`). It is broken by the cheapest of three routes,
   in this tie order.

```text
  cycle  a <- b <- c <- a

  Exchanges   xchg a,b ; xchg b,c                 (n-1 exchanges; every
                                                  adjacent pair must be
                                                  exchangeable, one width)
  Register    mov t,<closing source> ; the other moves in order ;
              mov <closing dest>,t                (t: a register dead
                                                  after the group that no
                                                  copy names)
  Stack       push <closing source> ; the other moves ; pop <closing dest>
```

The target's prices (`CyclePrices`: `mov_rr`, `xchg_rr`, `push_r`, `pop_r`)
choose the route. Where the CPU is tuned for size there are no prices and
the route is exchanges where they apply, else the stack. A scratch register
is a candidate only if the convention does not preserve it or the body
already uses it, so no push and pop is added to save it. A cycle no move of
which the stack can carry is malformed, and so is a group that is not a
permutation of its places.

A frame-to-frame copy of equal width (1, 2 or 4 bytes, a byte moved as a
word) becomes `push` then `pop`, which needs no scratch register and keeps
the flags.

## Example
A loop that swaps `a` and `b` each trip and updates two more values. The
back edge's three copies are one group; `ax` and `dx` form a cycle, and
`cx = mov si` is ready (nothing in the group reads `cx`):

Before:

```lir
    B6  ← B2  → B2
        #45  ax         = mov.i16     dx                                    ; copy
        #46  dx         = mov.i16     ax                                    ; copy
        #47  cx         = mov.i16     si                                    ; copy
        #49               jmp         B2
```

After:

```lir
    B6  ← B2  → B2
        #47  cx         = mov.i16     si
        #45  ax, dx     = xchg.i16    dx, ax
        #46  y3.1       = elided      y2.1
        #49               jmp         B2
```

The ready copy is written first, the cycle becomes one `xchg`, and an anchor
(`elided`) keeps the virtual definition the dropped copy stood for.

From `ISEL_DUMP=1 llrm-c -m16 -O2` of a C loop that does `t = a; a = b;
b = t;` and updates `c` and `d`: block `B6` in the `RegAlloc` and
`ParallelCopy` stages.

## Runs
After `regalloc`, before `prologue` and the peephole, once per body, at
every level. A body with no group is returned as it is. `parcopy` is the
phase name.

## Needs
[`crate::backend::liveness`](liveness.md) (through `regthrash::_dead_after`) for the
registers dead after a group, and the groups `phielim` marks.
