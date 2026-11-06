> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/liverows.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/liverows.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/liverows.rs sha256:019bc7fe61a8ecc62b129a353b2d65585a688c225ef7f5473f5ff21496b3e366 -->

# liverows

Which values are live where, per block and per instruction row, for the
passes that place values (the allocator's consumers).

## How it works
[`live`](live.md) and [`live_out`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/selectiondag/live_out.rs) give, per block, the values live on entry and on
exit. `LiveRows` answers the same per instruction row through the
`LiveAt` trait: a dense table for small bodies, a walk of the blocks of a
web for large ones (`WebRows`). `live_reference` is the plain
iteration the others are checked against.

## Used by
`backend::allocate` and the passes that read liveness of a lowered body
(`flow`, `phielim`, `peephole`, `regclass`).

## Example
`v1 := mov 1; v2 := add v1, 2; ret v2`: `v1` is live from row 0 to row 1,
`v2` from row 1 to the `ret`; [`live_out`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/selectiondag/live_out.rs) of the only block is empty.
