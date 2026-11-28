> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/pagecount.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/pagecount.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/pagecount.rs sha256:dc82b5c40f6b909e6bd8f5e82f09c95bf4bd5a6122822d02a4594a5e18d3aed7 -->

# pagecount

A counted loop that walks by page ends on its index and a page count, not
on a second counter. The mechanism is
`IndVarSimplify::linearFunctionTestReplace` (`FindLoopCounter`,
`genLoopLimit`) applied to the walk's own counter.

## How it works
A loop that walks by page (see [`crate::pageindex`](pageindex.md)) has a trip count
`k = 256 * page + index`. A header phi `c` that steps by one and is read
only by its exit test is `c = start + d * k` (`d` is 1 or -1), so the test
`c == end` holds on the trip `k = d * (end - start) - (c's step already
taken)`, the limit. The test becomes `index == lo(limit)`, and only on that
trip `left == 0`, where `left` starts at `hi(limit)` and drops a page where
the index wraps: one byte compare a trip, in place of the counter's own
step and a word test. The limit is made once before the loop.

DEVIATE (6502-6): the pin's LFTR keeps the counter it chooses at the width
of the trip count and tests it whole (`FindLoopCounter`: "AR may not be a
narrower type"). The walk's index is one byte; the trip count's other byte
is the pages left, a second phi tested on the low byte's hit. The pin's pass
does not fire on this loop (`needsLFTR` finds `k2 == 0` canonical). Exit:
none, the index is narrower than the count by design.

## Example
`memcmp`'s loop, `%k2 = add %k, -1 ; br %k2 == 0, done, head`, becomes

```text
head: %left = phi i8 [hi, pre], [%left, step], [%left1, bump]
      ...  br %i == lo, high, step
high: br %left == 0, done, step
bump: %left1 = add i8 %left, -1
```

## Runs
By [`crate::pageindex`](pageindex.md) as it rewrites the loop, when asked to count pages.
