> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/regthrash.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/regthrash.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/regthrash.rs sha256:1386ba0584ff048f5888a8bbdff3781c1ff1f9b295832e42da82fa0475c98295 -->

# regthrash

Renames a definition's register so that a copy after it is a copy of a
register to itself.

## How it works
Registers are already chosen, so this is the inverse of coalescing. It takes

```text
 OP(...) -> Y        ...        mov Z, Y     (and Y dies there)
```

and writes the producer's result into Z, which leaves the move copying Z to
itself. Each block is handled alone, to a fixed point of its own, since a
rename leaves the block's live-in as it was.

For each plain `mov Z, Y` (register to register, one width, no pinned
operands, no group), Y must die at the move. The producer is found by
walking back, as Open Watcom's `RegThrash` does (`bld/cg/c/scthrash.c`).
Every step in between must leave Y live and untouched and Z dead, and may
not be a terminator, a barrier or a spill store. The definition that writes
Y must own all of Y's lanes and must not read Z. Then `_renamed` rewrites it
to write Z, and accepts the result only if `select::priced_in` still encodes
it and the decoded register effects moved from Y to Z and nothing else.

Two cases differ in what happens to the copy:

```text
 not tied (producer never reads Y)    tied (two-address, reads Y too)
   add  Y, a, b  ->  add  Z, a, b       mov  Y, x  ->  mov  Z, x
   mov  Z, Y         mov  Z, Y (anchor) add  Y, a      add  Z, a
                                        mov  Z, Y      (copy moved up)
```

When not tied, the copy stays as an anchor with its virtual definition,
since later operands may still name that value. When tied, the renamed form
reads Z, so the copy is relocated before the producer and Y's range ends
there instead of at the old move (Watcom's `PrefixIns`).

At most `ROUNDS` (8) renames are made per block per run, because each
changes the liveness later candidates are judged against. Liveness after
each instruction is worked out again only for the stretch the rename moved,
and the search resumes at the first instruction whose liveness moved.
`LLRM_CHECK_THRASH` (always under test) asserts that equals a full
recompute.

## Example
A value is built in EAX, copied to EDI and read later as EDI (the first
block of the fixture; the second block, which reads EDI and then overwrites
every other register, is trimmed):

```lir
=== before
    block 0x0000
      0x0000  EAX := mov 0x7  ; inserted
      0x0001  EDI := mov EAX  ; inserted
    block 0x0001
      0x0009  push EDI  ; inserted
      ; ...
=== after
    block 0x0000
      0x0000  EDI := mov 0x7  ; inserted
      0x0001     ; inserted
    block 0x0001
      0x0009  push EDI  ; inserted
      ; ...
```

The producer writes EDI directly. The copy is left as an anchor with no
operation, which keeps the virtual value it defined.

From unit test `test_removed_copy_keeps_its_virtual_definition` in
`regthrash.rs`.

## Runs
First sub-pass of [`crate::backend::peephole`](peephole.md), gated on a register copy
being present. It is the only pass there that removes a copy the coalescer
refused on colourability.

## Needs
Register-lane liveness at block exits and instruction effects:
[`crate::backend::liveness`](liveness.md), [`crate::backend::peephole::Lanes`](peephole.md).

## See also
Open Watcom `RegThrash`, `bld/cg/c/scthrash.c`.
