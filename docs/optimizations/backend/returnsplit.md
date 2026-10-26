> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/returnsplit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/returnsplit.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/returnsplit.rs sha256:09173702eff52d60627eddb020775bf3b20f42a55659b6bd99a976f61c990e9e -->

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

## Runs
In `assemble`, ahead of `jumps::duplicated_returns`, at -O1 and above.

## Deviations from the pin
- DEVIATE RS1: no counterpart. The pin gets the early path from
  `ShrinkWrap::tryToSplitRestore` and tail duplication after PEI, and its
  constant stays in `eax` through the phi lowering instead of a cell. Exit:
  the allocator keeps the constant out of a cell.
