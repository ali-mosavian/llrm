> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/unroll.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/unroll.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/unroll.rs sha256:95165b7d1a9b651f2ca7de24b788b7b0a342f9857cc3fb813a117e1b47f84a3a -->

# unroll

Bounded full unrolling of small exact-trip loops: llrm-core's
`optimize/unroll.rs`, the port of `qbopt/optimize/unroll.py`, adapted to
the rich MIR.
LLVM: LoopUnroll's full unroll (`UnrollLoop` at the trip count), priced by
its cost model, `analyzeLoopUnrollCost`.

A loop tested in its header, whose body is a chain of blocks to its
latch, becomes one straight line in the latch: the first trip runs the
original blocks, each later trip is a copy of the header's work and the
body's, and a last copy of the header's work supplies what leaves. The
header's test is gone; Dead takes the compares left behind.

The copy budgets are peelsize's `Limits`, which count instructions. The
one machine number is the target's costs: a function with an instruction
the target does not price is left alone (`profit::priced`).

What changed with the IR: an expanded value is a clone with a fresh
result, and its origin the `Cloned` change, so the old fresh ids, byte
ownership (`absorbed`, `symbol`, `raised`), pointer and integer-range
side tables are gone; a use outside the loop is rewritten by its block,
not by dominance. The header's test and the latch's jump are a `br`
each, so the old frontends' explicit and implicit back edges are one.
Dropped: `repetitions` (the old body's record of each expansion, for
nothing here to read), and the floating-loop rules -- the x87 header
contract and `floatfacts`' exactness check on more than four trips; a
float is an ordinary value, so a floating loop meets the integer rules.
The stage records and `watch` hook are the pass manager's dump.

Tests, in `unroll_tests.rs`, are new: the old ones read BC fixtures
(FPDEEP, FPCSE) through the whole pipeline, or tested DSE's x87
checkpoints. `unroll_budget_tests.rs` was `profit::spill_risk`'s, in
`profit_tests.rs`.
