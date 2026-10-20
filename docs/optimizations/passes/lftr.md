> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lftr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lftr.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lftr.rs sha256:45dedd1629a5f068a908be9ece8c06b62d701c2c2ec34f9ad1fcd22f8b21c75d -->

# lftr

A loop's exit test is rewritten to compare a counter with the value it has
when the loop's trip count is spent. LLVM's `linearFunctionTestReplace` and
its kin (`IndVarSimplify.cpp`).

## How it works
For each loop, innermost first, and each exiting block that ends in a
conditional branch and belongs to the loop itself:

- `needsLFTR`: the test is not already `==`/`!=` of a simple counter with a
  loop-invariant value.
- The block's exact exit count is known, is not zero, is cheap to expand
  (`isHighCostExpansion`, budget 4) and is safe to expand.
- `FindLoopCounter` picks a header phi that is a unit-stride counter at
  least as wide as the count, with a concrete start, preferring one that
  counts from zero or is wider, and not one that only this test keeps alive.
- `genLoopLimit` writes the counter's value after the count (a post-inc
  value where the test is in the latch) before the branch, and the branch
  tests `counter != limit` (`==` where the loop is left on true). A counter
  wider than the count is compared by a truncation, or the limit is extended
  (`wide.trip.count`) where that equals it.

The test it replaces is left to `dead`.

## Example
`for (i = 0; i < n; i++)` with `n` an `i32`: the test `i slt n` becomes
`%exitcond = icmp ne i32 %i.next, %n` in the latch.

## Used by
[`crate::indvars`](indvars.md) runs `replaced` after the exit steps, as
`IndVarSimplify::run` does.

## See also
`linearFunctionTestReplace`, `FindLoopCounter`, `genLoopLimit`,
`needsLFTR`, `docs/architecture/scev-port.md`.
