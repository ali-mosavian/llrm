> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/edges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/edges.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/edges.rs sha256:28759ba721f8697e310ce79ffa586ca4c5fd220997093b65a7fe7a9f814521b0 -->

# edges

Breaks one edge of a body: a block of its own that jumps on.

## How it works
`split` makes a block holding a jump to the edge's target, points the
source's branch (or its fall-through, by a jump after the branch) at it,
and makes the target's phis read it where they read the source. The
block's label is above every physical offset, in a namespace per body
entry, as the blocks `phielim` makes for copies on edges are.

## Example
The edge `b1 -> b4` of `b1: je b4; b2: ...; b4:` becomes `b1 -> e -> b4`
with `e: jmp b4`, and `b4`'s phi that read `b1` reads `e`.

## Used by
`machinesink`.
