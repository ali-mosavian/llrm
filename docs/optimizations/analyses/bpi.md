> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/bpi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/bpi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/bpi.rs sha256:ff233bbff2caffdf5d1c31f20f4bb5c91f344e6574c406da9e9545831d4b122b -->

# bpi

`BranchProbabilityInfo` of a MIR function: LLVM's static branch
probabilities, translated from llvm/lib/Analysis/BranchProbabilityInfo.cpp
at d1106deb71cc.

Each block of two or more successors takes the first of: its
`branch_weights` metadata, the estimated block weights (unreachable,
noreturn, unwind and cold blocks, loop exits scaled by a trip count), the
pointer, zero and floating-point heuristics. A block none of them speaks for
has 1/N on each edge.

DEVIATE B1: `TargetLibraryInfo` names the routines whose result is a
three-way compare (`strcmp` and its kin); here the language states it of the
routine (the `threeway` fact). Exit: none, the fact is llrm's form of it.

DEVIATE B2: `getTerminatingDeoptimizeCall`: MIR has no deoptimize
intrinsic, so a block is unreachable only by its `unreachable`.

## Example
`pointer` in `bpi_golden.ll`, `if (p == NULL)`: the pointer heuristic takes
the equal edge 12 times in 32, as `opt -passes='print<branch-prob>'` does:

```text
edge %entry -> %yes probability is 0x30000000 / 0x80000000 = 37.50%
edge %entry -> %no probability is 0x50000000 / 0x80000000 = 62.50%
```

From unit test `test_probabilities_are_llvms_on_every_heuristic` in
`bpi_tests.rs`.

## Used by
`blockfreq` (`BlockFrequencyInfo`'s edge probabilities, once its callers
move from `branchprob`).
