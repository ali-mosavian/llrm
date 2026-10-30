> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/edges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/edges.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/edges.rs sha256:d5db32b3e12ecd2c7551d942f94817e75b3beaee3fce4b6736a8912352833a95 -->

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
