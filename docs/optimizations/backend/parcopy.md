> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/parcopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/parcopy.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/parcopy.rs sha256:a9147035ff11d709893452d3c78bb94d2c37546a8d8f0d9c6696d33b535ea8b0 -->

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
`swaps` in a loop exchanges `a` and `b` each trip. The back edge's three
copies are one group, and `AX`, `DX` form a cycle (`BX` only reads `CX`):

Before:

```lir
block 0x000b
  0x000b  BX := mov CX  ; inserted; parallel-copy 2
  0x000b  DX := mov AX  ; inserted; parallel-copy 2
  0x000b  AX := mov DX  ; inserted; parallel-copy 2
  0x000b  jmp   ; inserted
```

After:

```lir
block 0x000b
  0x000b  BX := mov CX  ; inserted
  0x000b  DX, AX := xchg AX, DX  ; inserted
  0x000b     ; inserted
  0x000b  jmp   ; inserted
```

The ready copy is written first, the cycle becomes one `xchg`, and an anchor
keeps the virtual definition the dropped copies stood for.

From `ISEL_DUMP=1` of a C function whose loop does `t = a; a = b; b = t;`,
compiled with `llrm-c -O2 -m16`: the `RegAlloc` and `ParallelCopy` stages.

## Runs
After `regalloc`, before `prologue` and the peephole, once per body, at
every level. A body with no group is returned as it is. `parcopy` is the
phase name.

## Needs
[`crate::backend::liveness`](liveness.md) (through `regthrash::_dead_after`) for the
registers dead after a group, and the groups `phielim` marks.
