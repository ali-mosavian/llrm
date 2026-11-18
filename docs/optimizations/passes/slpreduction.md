> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpreduction.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpreduction.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpreduction.rs sha256:33cb65b70c259020f863f608e4568cdad86aeac11765e7d1068fbdb3c8bbe1c5 -->

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

DEVIATE SLP-RDX1 (owner w11, exit 2026-11-24): the windows of loads that
are already in ascending order and whose scalars have one use; no
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
