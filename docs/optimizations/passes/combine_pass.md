> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_pass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_pass.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_pass.rs sha256:211e4419dd6ce785057e45ec2c08b63f998c8812fadb70ed9a50080f2ee09353 -->

# combine_pass

The pass: InstCombine as a function pass of the pipeline.

[`combine::combine`](combine.md) over the function, with the
module's globals and metadata from the pass manager. `LLRM_NO_COMBINE`
turns it off.

## Used by
`pipeline` (`combine@N`, in front of the old `fold@N` of each InstCombine
slot of LLVM's order).
