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
- `FindLoopCounter` picks a header phi that is a counter (a unit stride, or
  a constant one that cannot wrap: DEVIATE LFTR1) at least as wide as the
  count, with a concrete start, preferring one that counts from zero or is
  wider, and not one that only this test keeps alive.
- `genLoopLimit` writes the counter's value after the count (a post-inc
  value where the test is in the latch) before the branch, and the branch
  tests `counter != limit` (`==` where the loop is left on true). A counter
  wider than the count is compared by a truncation, or the limit is extended
  (`wide.trip.count`) where that equals it.

The test it replaces is left to `dead`.

DEVIATE LFTR1 (owner llrm-w12-lsr; exit: the pin's `isLoopCounter`
(IndVarSimplify.cpp#L913, `m_scev_One`) takes any constant step): a loop
that steps by two, `for (o = 0; o <= 3998; o += 2)`, kept `ult` for the
pin's LSR, which turns only equality tests into zero-ending counters; main's
earlier LSR made it `ne`. A step that cannot wrap reaches `start + count *
step` exactly, so `!=` holds on the trip the original test ended (gcc's
`iv_elimination_compare` relies on the same). Fail-first tests: strides
2, 3, 4, 8 and -2 in `lftr_tests.rs`.

## Example
`for (i = 0; i < n; i++)` with `n` an `i32`: the test `i slt n` becomes
`%exitcond = icmp ne i32 %i.next, %n` in the latch.

## Used by
[`crate::indvars`](indvars.md) runs `replaced` after the exit steps, as
`IndVarSimplify::run` does.

## See also
`linearFunctionTestReplace`, `FindLoopCounter`, `genLoopLimit`,
`needsLFTR`, `docs/architecture/scev-port.md`.
