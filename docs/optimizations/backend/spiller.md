> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spiller.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spiller.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spiller.rs sha256:5ef5761048d648dd22aab508355f357b7f6fe63032aec29fb58c0f30e37ead8b -->

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

Making an update short (`_short_update_runs`) or local (`_local_updates`)
looks at the definitions of the spilled values, which the postings give, and
copies a block only where a run is made: on `cells` at N=896, `spill short
updates` was 335 Minstr, 3.6 now; `spill local updates` 258, 35.
`live_among` reads the occurrences of the pieces a split made from the postings
too (`split after liveness`: 401 Minstr, 162), as does a local split for the references
of its value (`split local`: 291, 57).
A stable load asks only the writers that may meet its cell (`Writers`). A
frame cell is never met by a store to a data cell. A data cell is met, by its
bytes, by the stores to its own symbol; the stores to another external symbol
meet an external symbol's cell by region, whatever the displacement
(`overlap::may_overlap`), and are held by block (`Background`, as LLVM's
`AliasSetTracker` holds its unknown-alias set) and not named for each load:
300 loads against 300 stores cost 1,226 Minstr at -O2 -m32, 272 now. A body
with a `spares` pair names the stores of the other space one by one.

Slots are shared by [`crate::backend::slots`](slots.md). Within an allocation each
stored value takes a slot of its own (`Frame::defer_colours`); once the
allocation is kept, `slots_colored` shares them, as LLVM's
`StackSlotColoring` does after register allocation and GCC's IRA reuses
stack slots after its spill decisions: the homes, most named first, each
take the first slot that is as wide and whose holders never overlap its
cells' live range, preferring the slot of a home it is copied to or from, so
that copy vanishes. Outside an allocation (the spiller before it, the
SSA spiller) `_color_slots` still colours as each value is stored. After
`setjmp` returns twice no slot is shared.

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
A group grows one candidate at a time, and a candidate is asked of when it
comes up: whether it interferes with the group is answered by the intervals
the allocator holds where none of them meets the group's (nothing is live
together), and by the interference graph of the group and the candidate
alone otherwise, never by the web the copies make whole.

## Example
Eight values loaded from `p` are live across a call to `g`; the allocator
has fewer registers than that. Before, the allocator's input:

```lir
=== Coalescer
    block 0x0000
      0x0009  v6 := mov [None v1@unplaced]  ; inserted
      0x000b  v7 := mov [None v1@unplaced]  ; inserted
      0x000d  v8 := mov [None v1@unplaced]  ; inserted
      0x000f  v9 := mov [None v1@unplaced]  ; inserted
      0x0010  call   ; inserted; requires v2@EAX; delivers v10@EAX; clobbers EAX
      ; ...
      0x0015  v15 := mov v6  ; inserted
      0x0015  v15 := imul v15, v7  ; inserted
      ; ...
      0x0017  v17 := mov v8  ; inserted
      0x0017  v17 := imul v17, v9  ; inserted
```

After:

```lir
=== RegAlloc
    block 0x0000
      0x0009  EBX := mov [None v1@EAX]  ; inserted
      0x0009  [[bp-0xc]] := mov EBX  ; inserted
      0x000b  ESI := mov [None v1@EAX]  ; inserted
      0x000d  EBX := mov [None v1@EAX]  ; inserted
      0x000d  [[bp-0x4]] := mov EBX  ; inserted
      0x000f  EAX := mov [None v1@EAX]  ; inserted
      0x000f  [[bp-0x8]] := mov EAX  ; inserted
      0x0010  EAX := mov ECX  ; inserted
      0x0010  call   ; inserted; requires v19@EAX; delivers v20@EAX; clobbers EAX
      ; ...
      0x0015  EBX := mov [[bp-0xc]]  ; inserted
      0x0015  EBX := imul EBX, ESI  ; inserted
      ; ...
      0x0017  ECX := mov [[bp-0x4]]  ; inserted
      0x0017  ECX := imul ECX, [[bp-0x8]]  ; inserted
```

`v6`, `v8` and `v9` got slots and a store after their definitions. `v6` and
`v8` are reloaded before their uses; `v9`'s use reads its slot directly.

From `ISEL_DUMP=1` of the function in the `ssaspill` example at -O2 -m32,
allocator route.

## Runs
Inside the register allocator ([`crate::backend::allocate`](allocate.md)): once for the
values it chose to spill (`spilled_apart`), and once per body before the
candidate allocations for values read once from a cell that holds
(`_folded_reads`). Both run wherever the allocator runs.

## See also
LLVM's
[`InlineSpiller`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/InlineSpiller.cpp).
