> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/returnsplit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/returnsplit.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/returnsplit.rs sha256:aa4809a5c62f3d2ccdb48fbacdc2c4c39795d90b7179e9bc20e62fad88beb538 -->

# returnsplit

Gives a return that reloads a spilled constant its own block on the edges
that stored the constant, where that lets the frame be set up later.

## How it works
A return block `mov r, [cell]; ret` shared by an early path and the body
keeps the early path in the frame: the path stores its constant to the cell
(`mov [cell], 1; cmp; je ret`) and the shared return reloads it. For each
edge into the block whose source ends in such a store, with nothing between
the store and the end that touches memory or a call, the edge goes to a copy
`mov r, imm; ret` and the store goes when no path from the source reads
the cell before writing it. The edges are kept only when shrink wrapping
then sets the frame up after the source (`shrinkwrap::wrapped`): a copy of a
return whose epilogue still runs is a size cost with nothing skipped.

## Example
`place` of `bench/queens/queens.c` at m16 -O2 (abridged; the left column is
`LLRM_NO_RETSPLIT=1`):

```text
 without                           with
 mov word ptr [bp-6], 1            cmp dx, cx
 cmp dx, cx                        je early
 je ret                            ...
 ...                               early:
 ret: mov ax, [bp-6]                 mov ax, 1
      ...                            ret
```

The store of 1 and the reload are gone, and the frame is set up after the
test.

## Runs
In `assemble`, ahead of `jumps::duplicated_returns`, at -O1 and above.

## Deviations from the pin
- DEVIATE RS1: no counterpart. The pin gets the early path from
  `ShrinkWrap::tryToSplitRestore` and tail duplication after PEI, and its
  constant stays in `eax` through the phi lowering instead of a cell. Exit:
  the allocator keeps the constant out of a cell.
