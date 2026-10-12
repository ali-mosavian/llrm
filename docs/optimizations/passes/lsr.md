> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr.rs sha256:d0a713969e98027ea15635c90f2b67c18421a0b4d3af591d5950387f07f875ad -->

# lsr

A loop's induction variables chosen once, by cost: LLVM's
LoopStrengthReduce and GCC's ivopts.

Every read of a counter, or of a value affine in one, is one
`induction::Recurrence`, `pointer + start + step * trip`, with symbols
allowed (`induction::users`). A candidate is a recurrence the loop could
carry in a register. A use is realized from a candidate `r` as
`base + k * r + c`: an address the target's forms take, or the adds,
shifts and multiplies that compute it. The exit either keeps its compare
or tests one candidate for its last value, which is free where that is
zero.

A set of candidates costs an add a trip each, its uses' cheapest
realizations, and a spill a trip for each register past the target's.
The cheapest set is found by adding, removing and swapping candidates
from the loop's own counters and from none, as GCC's
`find_optimal_iv_set`, and is carried out only where it beats the
counters the loop has. Prices, registers and address forms are the
target's; the counters left unread go to `dead`.

It runs once, after the loop passes, on each loop innermost first, as
LLVM runs its LSR late: nothing after it chooses a loop's counters.
