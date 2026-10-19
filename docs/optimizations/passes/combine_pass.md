> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_pass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_pass.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_pass.rs sha256:ade8dee2e65bb4479e2e4ce4137a6bdcc467e9e2b72e12e764023d8ddba8fb87 -->

# combine_pass

The pass: InstCombine as a function pass of the pipeline.

[`combine::combine`](combine.md) over the function, with the
module's globals and metadata from the pass manager. It runs only where
`Options::combine` is set (`LLRM_NO_COMBINE` clears it).

## Used by
`pipeline` (`combine@N`, in front of the old `fold@N` of each InstCombine
slot of LLVM's order).
