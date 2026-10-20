> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_pass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_pass.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_pass.rs sha256:4c82d01e8be5633fe785c051ec77c93c095840db0fea4987b860c13a2c5ee7fa -->

# combine_pass

The pass: InstCombine as a function pass of the pipeline.

[`combine::combine`](combine.md) over the function, with the
module's globals and metadata from the pass manager. It runs only where
`Options::combine` is set (`LLRM_NO_COMBINE` clears it).

## Example
With `Options::combine` set, `%y = add i32 %x, 0` is gone and its uses read
`%x`; with it clear, the function is returned unchanged.

## Used by
`pipeline` (`combine@N`, in front of the old `fold@N` of each InstCombine
slot of LLVM's order).
