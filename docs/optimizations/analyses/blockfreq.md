> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/blockfreq.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/blockfreq.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/blockfreq.rs sha256:38f4aa3b4e66fc8516ff884e18cf639dc7ffd89a6835389847c096bbbd3eae84 -->

# blockfreq

`BlockFrequencyInfo` of a MIR function: LLVM's `BlockFrequencyInfoImpl`
(`llrm_mir::blockfrequency`) over `branchprob`'s edge probabilities, as
`BlockFrequencyInfoWrapperPass` runs it over `BranchProbabilityInfo`.

DEVIATE D1b: the edge probabilities are `branchprob::estimated`'s
heuristics (LLVM's loop, call and return weights, and GCC's), not LLVM's
`BranchProbabilityInfo`, and no trip count stands in for the loop
heuristic, as LLVM's has none. Exit: `BranchProbabilityInfo` translated.
With `branch_weights` metadata on every branch the two agree.

## Used by
`inlinecost` (cold and hot call sites, D1).
