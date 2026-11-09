> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lftr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lftr.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lftr.rs sha256:d9896bef38f874a409e6cde8ae76ccb3140f35d676d54c90245af41405493414 -->

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
  a constant one that cannot wrap where the target sets
  `Tuning::lftr_any_stride`: DEVIATE LFTR1) at least as wide as the
  count, with a concrete start, preferring one that counts from zero or is
  wider, and not one that only this test keeps alive.
- `genLoopLimit` writes the counter's value after the count (a post-inc
  value where the test is in the latch) before the branch, and the branch
  tests `counter != limit` (`==` where the loop is left on true). A counter
  wider than the count is compared by a truncation, or the limit is extended
  (`wide.trip.count`) where that equals it.

The test it replaces is left to `dead`.

DEVIATE LFTR1 (owner w12; review 2026-12-20; `Tuning::lftr_any_stride`, x86
m16/m32/m64 only; the pin's `isLoopCounter` (IndVarSimplify.cpp#L913,
`m_scev_One`) takes a step of one): a loop that steps by two, `for (o = 0;
o <= 3998; o += 2)`, kept `ult` for the pin's LSR, which turns only equality
tests into zero-ending counters. A step that cannot wrap reaches `start +
count * step` exactly, so `!=` holds on the trip the original test ended
(gcc's `iv_elimination_compare` relies on the same). Removing it was tried
2026-11-09 and is worse on x86: m64 jpeg_single +16 B (O1, O2, O3), m16
jpeg_fixed/single +4 B (all four levels), m32 heapsort +1.8% clk,
ld_horner +2.7%, with idioms -16 B and x_idioms -455 clk the only gains;
other targets get the pin (wasm `scroll_down` -29 B). Fail-first tests:
strides 2, 3, 4, 8 and -2 in `lftr_tests.rs` with the knob on, and the
pin's refusal with it off.

## Example
`for (i = 0; i < n; i++)` with `n` an `i32`: the test `i slt n` becomes
`%exitcond = icmp ne i32 %i.next, %n` in the latch.

## Used by
[`crate::indvars`](indvars.md) runs `replaced` after the exit steps, as
`IndVarSimplify::run` does.

## See also
`linearFunctionTestReplace`, `FindLoopCounter`, `genLoopLimit`,
`needsLFTR`, `docs/architecture/scev-port.md`.
