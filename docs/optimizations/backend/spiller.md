> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spiller.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spiller.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spiller.rs sha256:a82ec20afe69c9e80a0d813bea0640b25fb4dab4a16f57cb18f9438cb169684f -->

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
   v = add a, b        t1 = add a, b ; store [slot0], t1      store
   x = add v, 1        x = add [slot0], 1                     folded read
   y = call f(v)       t2 = load [slot0] ; y = call f(t2)     reload
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
═══ f_ · Coalescer · m32 ═══════════════════════════════════════════════════════
fn f_(t1@eax, t2@edx) -> @eax   ; calls clobber eax

B0  entry  → exit                                                 ×1.0
     #5  t6         = load.i32    [t1+12]
     #6  t7         = load.i32    [t1+16]
     ; ...
     #8  t9         = load.i32    [t1+24]
     #9  t10        = load.i32    [t1+28]
    #10  t11@eax    = call        g_(t2@eax)
     ; ...
    #13  t14        = imul.i32    t14, t6
    #24  t16        = mov.i32     t7
    #15  t16        = imul.i32    t16, t8
    #16  t13        = add.i32     t13, t16
    #26  t18        = mov.i32     t9
```

After:

```lir
═══ f_ · RegAlloc · m32 ════════════════════════════════════════════════════════
fn f_(t19@eax, t20@edx) -> @eax   ; calls clobber eax
frame  slot0 [bp-4] 4 B  slot1 [bp-8] 4 B  slot2 [bp-12] 4 B  slot3 [bp-16] 4 B

B0  entry  → exit                                                 ×1.0
     #5  edx        = load.i32    [t1@eax+12]
    #29               store.i32   [slot3], edx                          ; spill
     #6  edx        = load.i32    [t1@eax+16]
    #30               store.i32   [slot2], edx                          ; spill
     ; ...
     #8  edx        = load.i32    [t1@eax+24]
    #31               store.i32   [slot0], edx                          ; spill
     #9  eax        = load.i32    [t1@eax+28]
    #32               store.i32   [slot1], eax                          ; spill
    #33  eax        = mov.i32     esi
    #10  t22@eax    = call        g_(t21@eax)
     ; ...
    #13  ebp        = imul.i32    ebp, [slot3]
    #24  ecx        = load.i32    [slot2]
    #15  ecx        = imul.i32    ecx, edi
    #26  ecx        = load.i32    [slot0]
    #17  ecx        = imul.i32    ecx, [slot1]
```

`t6`, `t7`, `t9` and `t10` got slots and a store after their definitions
(`; spill`). `t7` and `t9` are reloaded before their uses (`#24`, `#26`);
`t6` and `t10` are read from their slots directly (`#13`, `#17`).

From `ISEL_DUMP=1` of a C function that loads eight values from `p` and
calls `g` (`-m32 -O2`), stages `Coalescer` and `RegAlloc` (allocator route).

## Runs
Inside the register allocator ([`crate::backend::allocate`](allocate.md)): once for the
values it chose to spill (`spilled_apart`), and once per body before the
candidate allocations for values read once from a cell that holds
(`_folded_reads`). Both run wherever the allocator runs.

## See also
LLVM's
[`InlineSpiller`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/InlineSpiller.cpp).
