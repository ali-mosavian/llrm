> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cold_chrec.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cold_chrec.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cold_chrec.rs sha256:3dbac36310c8b23d53bb2cbf1770667b2a756368ca88ef832c63a11cc081c075 -->

# cold_chrec

DEVIATE COLD-CHREC: a chained recurrence of an enclosing loop that an inner
loop's entry skips on some trips is not expanded for the inner loop.

`SCEVExpander` makes one phi a level in the outer loop, stepped on every
trip, for a start the written code computes where the inner loop is entered
(sieve: `i * i` feeding a prime's marking loop, 11 entries in 1022 trips).
The pin expands it in `WidenIV` and in LSR; gcc's ivopts weighs a use's cost
by the frequency of its block.

## Used by
Named by `lsr_use`, `widen`.
