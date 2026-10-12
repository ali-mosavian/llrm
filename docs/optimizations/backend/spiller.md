> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spiller.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spiller.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spiller.rs sha256:563b7fcc489db49fab86061940604cf67fa38f24fb3da52840cfeecc3fce855b -->

# spiller

Keeps a value the allocator would not hold in memory instead: a store after
each definition, a reload before each use, or the value made again where it
is read.

## How it works
Spilling is planned before any instruction is written (`planned`). Each
value of the set gets one of these, in this order:

- a constant, an address, or a zero- or sign-extension of another value:
  made again where it is read, as a `mov`, a `lea`, or the extension;
- a stable load of a frame cell nothing writes while the value lives, or a
  cell [`crate::backend::storedhomes`](storedhomes.md) proves holds it: read from that cell,
  with no slot of its own;
- otherwise stored: it gets a frame slot.

A plain copy of a value made again is made again the same way.

Slots are shared by [`crate::backend::slots`](slots.md). Stored values are sorted
widest first, and each takes the first slot that is wide enough and whose
holders never overlap its live range, preferring the slot of a value it is
copied to or from, so that copy vanishes. After `setjmp` returns twice no
slot is shared.

`materialized` writes the plan. Every definition of a stored value is
followed by a store into its slot and every read is preceded by a reload
into a fresh value that lives across that one instruction.

```text
   v := a + b                 t1 := a + b ; [slot] := t1     store
   x := v + 1                 x := [slot] + 1                folded read
   y := f(v)                  t2 := [slot] ; y := f(t2)      reload
```

An instruction that can read the slot directly (a source operand, a tied
update in place) takes the memory operand and needs no reload. A
just-defined value that an update immediately consumes is kept in a register
through that update (short update runs, local updates). Afterwards the
definitions of values made again that nothing reads, and moves of a value
onto itself, are removed, and the fresh values are returned for the
allocator to place.

`siblings` finds the values copied to and from a spilled value that are
cheaper sharing its slot; the allocator spills them together.

## Runs
Inside the register allocator ([`crate::backend::allocate`](allocate.md)): once for the
values it chose to spill (`spilled_apart`), and once per body before the
candidate allocations for values read once from a cell that holds
(`_folded_reads`). Both run wherever the allocator runs.

## See also
LLVM's
[`InlineSpiller`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/InlineSpiller.cpp).
