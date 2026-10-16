> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spillforward.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spillforward.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spillforward.rs sha256:37466edb7e83a0f058bd0cb4df9932e7c3633ad45578516aa696b83304419fa6 -->

# spillforward

Removes reloads of a frame slot that a register already holds on every path
to it, and turns other reads of such a slot into reads of the register.

## How it works
A fact is a pair (register, slot) meaning the register holds the slot's
bytes, and whether the register is what a store wrote the slot from. A
register a reload filled may be dead by the time it is read, so only a
stored register may be named by a rewrite.

```text
 mov [bp-4],ax     facts: (ax,[bp-4]) stored
 mov bx,[bp-4]     becomes   mov bx,ax
 add cx,[bp-4]     becomes   add cx,ax
 mov ax,[bp-4]     redundant: removed
```

Within a block the facts are carried forward. A store of a register to a
frame cell, and a load of one, adds the pair. A read of the slot as the one
memory source of `add`, `sub`, `and`, `or`, `xor`, `cmp`, `test`, `adc` or
`sbb` is rewritten to the register. Any write to a register's lanes drops
its pairs, a write to a cell drops overlapping pairs, a write to the frame
register or to memory the displacement does not name, and any instruction
whose effects cannot be read, drops every pair. A reload of a pair that
holds is removed, leaving an anchor so the byte ownership is kept.

Across blocks the facts are an availability problem: forward dataflow with
intersection as the meet, so a fact survives only if every predecessor
carries it (and it is stored in all of them). A block not yet reached is the
top element, so a loop header's back edge does not kill the fact the header
is there to establish.

## Example
`di` is stored to `[slot3]` and reloaded into `di`, with no write to `di` or
the cell on any path between. Before, the input of `Peephole`:

```lir
═══ _fivestreams@3 · ParallelCopy · m16 ════════════════════════════════════════
        B5  ← B3  → B3                                                    ×93.0
            #63               store.i16   [slot3], di                           ; spill
            #64  si         = load.i16    [slot6]                               ; reload
; ...
            #52               add.i16     [slot9], 10
            #65  di         = load.i16    [slot3]                               ; reload
            #53  di         = add.i16     di, 20
```

After `Peephole`:

```lir
═══ _fivestreams@3 · Peephole · m16 ════════════════════════════════════════════
        B5  ← B3  → B3                                                    ×93.0
            #63               store.i16   [slot3], di                           ; spill
            #64  si         = load.i16    [slot6]                               ; reload
; ...
            #52               add.i16     [slot9], 10
            #65  lsr.iv21   = elided
            #53  di         = add.i16     di, 20
```

The reload is gone and leaves an `elided` anchor under its label. `Peephole`
runs all its sub-passes, so the dump shows their combined effect; in this
function nothing else changes these lines.

From `ISEL_DUMP=1` of `tests/fixtures/c/fivestreams.c` at -O2 -m16 (the last
`_fivestreams@3` dumped).

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), once per body, after `copysink`
and before `postrasink`, which uses the reloads it removed.
