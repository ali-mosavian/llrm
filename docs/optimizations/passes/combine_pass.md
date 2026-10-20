> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_pass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_pass.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_pass.rs sha256:7caf8eacdd59d0185b4f55e95306652eeac2f6cef444800458edb8f19eae0259 -->

# combine_pass

The pass: InstCombine as a function pass of the pipeline.

[`combine::combine`](combine.md) over the function, with the
module's globals and metadata from the pass manager. It runs only where
`Options::combine` is set (`LLRM_NO_COMBINE` clears it).

DEVIATE: none for `LastRunTrackingAnalysis`
(`LastRunTracking`): `InstCombinePass::run` starts with
`LRT.shouldSkip(&ID)` and ends with `LRT.update(&ID, Changed)`; here the
pass does the same on a per-function analysis that it preserves and every
changing pass drops.

## Example
With `Options::combine` set, `%y = add i32 %x, 0` is gone and its uses read
`%x`; with it clear, the function is returned unchanged.

## Used by
`pipeline` (`combine@N`, in front of the old `fold@N` of each InstCombine
slot of LLVM's order).
