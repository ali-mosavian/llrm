> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/blockfreq.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/blockfreq.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/blockfreq.rs sha256:06d69f07b7bc677ec3247c9708d9f71622905f734365b38f693d7a62630e9bbf -->

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

## Example
A diamond whose branch weighs 3 to 1 gives the arms 0.75 and 0.25 of the
entry's frequency, and the join the entry's:

```text
entry: br i1 %c, label %a, label %b   ; !{branch_weights, 3, 1}
; entry 1.0, a 0.75, b 0.25, join 1.0
```
