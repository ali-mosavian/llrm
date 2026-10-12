> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/edges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/edges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/edges.rs sha256:40bad1626031bfac4f59fa2f1e2e780fd26ad9429b9797dab315ead3c5be88d9 -->

# edges

Adapted from llrm-core's `optimize/edges.rs`, the port of
`qbopt/optimize/edges.py`: place semantic operations on conditional
edges, not on either arm.

`fresh` is not ported: block ids are never reused, so `create_block`
is the fresh label. Its old callers were unswitch, loadjoins, inline,
loopclone, loopsimplify, lower_switches and the BC raise's dispatch.
