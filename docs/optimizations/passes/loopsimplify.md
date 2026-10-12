> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopsimplify.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopsimplify.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopsimplify.rs sha256:c3f01a46a4dbc48531f492c4658f299e906f6e730d150afee4ef8907c78bea41 -->

# loopsimplify

Adapted from llrm-core's `optimize/loopsimplify.rs`, the port of
`qbopt/optimize/loopsimplify.py`: every reducible loop gets one
preheader, one latch and dedicated exits.

The old body was immutable, so a refused grouping left it as it was; here
each loop is simplified on a copy of the function, kept only when every
grouping it needed went through. Every block ends in a terminator, so the
old fall-through source, a block whose last operation merely precedes
`target`, is a `br label`.
