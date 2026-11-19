> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/entryweight.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/entryweight.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/entryweight.rs sha256:4ea8a1e71739130bb8c542c8efb9bdbce9768cc3d1f60264882c87dfbbfa39f5 -->

# entryweight

How often a function runs: the frequency of a block over its function's entry (`BlockFrequencyInfo` over
`BranchProbabilityInfo`, as the inliner's `GetBFI` has it) and, from the direct calls to a function, how many
times it runs per run of the program.

## Used by
Named by `inlinecost`, `walk`.
