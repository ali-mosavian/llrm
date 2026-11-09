> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lftr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lftr.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lftr.rs sha256:f017fb36c3007461b286c31d872a318b0fc8943ccdc51f348e1d2729ae2ff973 -->

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
- `FindLoopCounter` picks a header phi that is a counter (an affine add
  recurrence of step one, `isLoopCounter`) at least as wide as the count,
  with a concrete start, preferring one that counts from zero or is wider,
  and not one that only this test keeps alive.
- `genLoopLimit` writes the counter's value after the count (a post-inc
  value where the test is in the latch) before the branch, and the branch
  tests `counter != limit` (`==` where the loop is left on true). A counter
  wider than the count is compared by a truncation, or the limit is extended
  (`wide.trip.count`) where that equals it.

The test it replaces is left to `dead`.

A counter with another step (`for (o = 0; o <= 3998; o += 2)`, `--i`) is left
as the pin leaves it. The former deviation LFTR1 (any constant step that
cannot wrap) is removed (2026-11-09): it cost wasm `scroll_down` +29 B.

## Example
`for (i = 0; i < n; i++)` with `n` an `i32`: the test `i slt n` becomes
`%exitcond = icmp ne i32 %i.next, %n` in the latch.

## Used by
[`crate::indvars`](indvars.md) runs `replaced` after the exit steps, as
`IndVarSimplify::run` does.

## See also
`linearFunctionTestReplace`, `FindLoopCounter`, `genLoopLimit`,
`needsLFTR`, `docs/architecture/scev-port.md`.
