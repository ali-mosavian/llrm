> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cold_chrec.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cold_chrec.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cold_chrec.rs sha256:2654c9bab35039a6fba65c4105a347472d94d342f20ec002e56737fb33880f82 -->

# cold_chrec

DEVIATE COLD-CHREC: a chained recurrence of an enclosing loop that an inner
loop's entry skips on some trips is not expanded for the inner loop.

`SCEVExpander` makes one phi a level in the outer loop, stepped on every
trip, for a start the written code computes where the inner loop is entered
(sieve: `i * i` feeding a prime's marking loop, 11 entries in 1022 trips).
The pin expands it in `WidenIV` and in LSR; gcc's ivopts weighs a use's cost
by the frequency of its block.

## Example
A start value `i * i` of an inner loop, a recurrence of the outer loop, is left
as the written multiply instead of becoming an outer phi stepped on every trip.

```text
outer: for i:  if (flag[i]) { for (j = i*i; j < n; j += i) mark(j); }
kept:  j.start = mul i, i     ; computed only where the inner loop is entered
not:   %sq = phi [0, pre], [%sq.next, latch]   ; stepped on every outer trip
```

## Used by
Named by `lsr_use`, `widen`.
