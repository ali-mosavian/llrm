> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopinterchange_transform.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopinterchange_transform.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopinterchange_transform.rs sha256:e0458dc05889bb93cafbe0233adbd46f05947e7181179e1cf89aa3c30b368d7d -->

# loopinterchange_transform

`LoopInterchangeTransform` of `LoopInterchange.cpp` at `d1106deb71cc`: the
rewrite that swaps the roles of two adjacent loops of a perfect nest. See
[`crate::loopinterchange`](loopinterchange.md) for the pass.

## How it works
`transform` splits the inner latch and clones the exit compare and the
induction updates into the new latch, splits the inner header so it has one
successor, and moves the code of the inner preheader into the outer header.
`adjust_loop_branches` points the outer predecessor at the inner preheader,
the inner header at the outer preheader and the latches at each other's
exits, then moves the LCSSA and reduction phis to the block that is now
their loop's, swaps the contents of the two preheaders and makes LCSSA
again for the values defined in the old outer header.

## Deviations from LLVM
- D1: `restructureLoops` and the dominator tree updates are replaced by
  `LoopInfo::analyze` and `DomTree::of` after the branches are rewired (see
  [`crate::loopinterchange`](loopinterchange.md)).

## Example
Before, `i` outer and `j` inner: `i.header -> j.preheader -> j.header ->
j.body -> j.latch -> i.latch`. After, `j.preheader -> j.header ->
i.preheader -> i.header -> j.body -> i.latch -> j.latch`: the old inner
header and latch bound the new outer loop.

## Used by
Named by `loopinterchange`.
