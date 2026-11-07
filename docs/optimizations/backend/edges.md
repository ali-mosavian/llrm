> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/edges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/edges.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/edges.rs sha256:650bcbf8d262ac6147fda44ff831f55f8ba3f938a6916f7aabed603577347c92 -->

# edges

Breaks one edge of a body: a block of its own that jumps on.

## How it works
`split` makes a block holding a jump to the edge's target, points the
source's branch (or its fall-through, by a jump after the branch) at it,
and makes the target's phis read it where they read the source. The
block's label is above every physical offset, in a namespace per body
entry, as the blocks `phielim` makes for copies on edges are.

The block is laid out right after the source, as the pin's
`SplitCriticalEdge` places it, and `without_jumps_to_next` drops the jump
each of the two ends in to the block now next, or flips the branch that
goes to it (`updateTerminator`).

## Example
The edge `b1 -> b4` of `b1: je b4; b2: ...; b4:` becomes `b1 -> e -> b4`
with `b1: jne b2; e: jmp b4; b2: ...`, and `b4`'s phi that read `b1` reads
`e`.

## Used by
`machinesink`, and `phielim` for the blocks it makes.
