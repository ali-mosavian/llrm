> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpreduction.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpreduction.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpreduction.rs sha256:dab2c08fa57e7b3c7c2e216901d99a5dff92822f6fe3877481a0809df87fdc1b -->

# slpreduction

`SLPVectorizer`'s horizontal reductions: a chain of one associative
integer operation over loads (or extensions of loads) of consecutive
cells becomes vector loads and one `llvm.vector.reduce.*`.

## How it works
`vectorizeHorReduction` (l.34447) takes the last operation of a chain as
the root. `matchAssociativeReduction` (l.30842) walks the chain's links (an
operand of the same operation with one use, in the root's block) and
collects the other operands, the reduced values, in root-to-leaf order;
those that are loads, alike in their extension and their object, form a
slice, each slice read from its last value to its first (ascending cells).
`tryToReduce` (l.31874) cuts a slice into windows: the width starts at
`GetVectorFactor(n)` (the largest the registers hold) and falls where a
window is not profitable; after a window the rest is cut again. A window is
a tree of two entries, the loads and their extension; its cost is the
vector load less the scalar ones, the extension (free: the reduction
absorbs it, `IsArithmeticExtendedReduction`), and the reduction (the first
part: the extended reduction, less the scalar chain; the later parts: one
vector operation and one extension). It is taken below the
`slp-threshold` of 0. The parts are joined by `emitReduction` (l.34077):
the narrower part is added to the low lanes of the wider, and
`llvm.vector.reduce.*` of the result is combined with the values left over
as `op.rdx` (flags other than wrap are what `createOp` keeps).

DEVIATE SLP-RDX1 (owner w11, exit 2026-11-24): the old load-slice path
runs first and the tree path only when it finds nothing; the tree's total
on jpeg_fixed is -6 where the pin's is -4 (no per-entry oracle yet); the
windows of loads that are already in ascending order and whose scalars
have one use; no
reordered or externally used lanes (`ExternalUses`, the reorder
shuffle), no scale by the loop's trip count (it multiplies every entry and
the reduction alike, so the sign of the total is the one used here), no
`computeMinimumValueSizes` narrowing, the move of a load past a store by
alias analysis (here: nothing between the first and last load of a window
touches memory), the reductions of floats, min/max, booleans, the loop
accumulator form and `AdjustReducedVals`' shifted windows.

## Runs
With the store chains, at the pin's `slp-vectorizer` slot.

## Used by
`slpvectorize`.

## Example
Forty `sext i16` loads of consecutive cells, summed by a chain of `add i64`,
become two vector loads and one reduction (the tile test).

```text
%x0 = sext i16 (load [i]) to i64 ; ... x39, summed by add i64
        =>
load <32 x i16>, load <8 x i16>; the narrower is added into the low
lanes of the wider; one @llvm.vector.reduce.add.v32i64 of the result
```
