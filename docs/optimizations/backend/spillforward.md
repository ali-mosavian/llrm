> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spillforward.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spillforward.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spillforward.rs sha256:7bf9eb12ca8a508ccb217eda5e810f3a4ed7534f6ae0b397dd824cbbf9969a3f -->

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
`CX` is stored to `[bp-0x8]` in one block and reloaded into `CX` in a later
one, with no write to `CX` or the cell on any path between. Before, the
input of `Peephole`:

```lir
=== ParallelCopy
    block 0x0005
      0x0009  [[bp-0x8]] := mov CX  ; inserted
      ; ...
    block 0x0010
      0x0011  DI := mov 0x168  ; inserted
      0x0011  CX := mov [[bp-0x8]]  ; inserted; spill reload
      0x0011  AX := mov CX  ; inserted
```

After `Peephole`:

```lir
=== Peephole
    block 0x0005
      0x0009  [[bp-0x8]] := mov CX  ; inserted
      ; ...
    block 0x0010
      0x0011  DI := mov 0x168  ; inserted
      0x0011     ; inserted
      0x0011  AX := mov CX  ; inserted
```

The reload is gone and leaves an empty anchor at its address. `Peephole`
runs all its sub-passes, so the dump shows their combined effect; in this
function nothing else changes these lines.

From `ISEL_DUMP=1 LLRM_CANDIDATES=spiller` of `bench/recgcd/recgcd.c`
at -O2 -m16.

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), once per body, after `copysink`
and before `postrasink`, which uses the reloads it removed.
