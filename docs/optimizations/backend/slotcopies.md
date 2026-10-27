> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/slotcopies.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/slotcopies.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/slotcopies.rs sha256:c7c0fa3581a0bb483118a4f7c6fdc7b940a15228f0c2111700ea6ceec2fa7039 -->

# slotcopies

Keeps one frame cell where a value is copied from a cell into a spill cell.

## How it works
`mov r, [A] ; mov [B], r` copies the cell `A` into the spill cell `B`. When
`B` is written by that store alone, and `A` is not written on any path from
the copy to a read of `B`, every read of `B` reads what `A` holds: `B` is
renamed to `A` and the copy, now a store of `r` over the cell it was just
loaded from, is dropped. The cell `B` is named no more and the frame loses
its bytes.

The rule is taken over fixed frame cells only, and not where an address of
the frame is taken (a pointer may then write either cell). Each cell takes
part in one rename.

## Example
x87 `lrint` stores through a stack temporary and loads it back; the value
then lives across a call, and the spiller gives it a cell of its own:

```text
 before                      after
 fistp dword [bp-4]          fistp dword [bp-4]
 mov eax, [bp-4]             mov eax, [bp-4]
 mov [bp-8], eax
 call p                      call p
 mov eax, [bp-8]             mov eax, [bp-4]
```

From unit test `test_a_spill_copied_from_a_cell_reads_that_cell` in
`peephole_tests.rs`.

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), after `postrasink`.

## Keeps
None is returned where no copy qualifies, and the frame is left as it is.

## Deviations from the pin
- DEVIATE S1: the pin keeps both cells. `StackSlotColoring` shares only the
  cells `InlineSpiller` made, so the stack temporary of an x87 conversion
  and the spill of its result stay apart (`llc -O2 -mtriple=i386-code16` on
  the IR of `long v = (long)x; p(v); q(v);` with a clobber-all asm after `p`
  spills `movl %eax, -20(%ebp)` from `fistpl -24(%ebp)`). The old allocator
  reloaded from the first cell, and m16 `-Os` must not grow past main. Exit:
  none; this is a size rule.
