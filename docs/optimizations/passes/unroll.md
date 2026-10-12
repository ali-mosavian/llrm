> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/unroll.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/unroll.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/unroll.rs sha256:4e2515279e98c12af9dfd8820105e70191f49abee6e8e1a7993e9e4f6c552f56 -->

# unroll

Full unrolling of a loop with an exact trip count of at least two: its
trips become one straight line in the latch and the loop is gone.

## How it works
A loop qualifies when it has one latch and one entering block; its header
tests with a conditional `br` whose one successor is the body and the other
the exit; the exit has the header as its only predecessor; the body is a
chain of single-successor blocks from the header to the latch with no
phis; and every header phi has exactly the two arms, entry and latch.
The body and latch may not call out (`calls_out`) or touch volatile memory.
The header's work runs once more than the body, so it may not store, call
or allocate. `induction::trip_count` must prove the count, and
`peelsize::admitted` (in `llrm_analysis`) must price it as worth copying:
the budgets, the hint a language may state, and the reasons it refuses are
documented there.

The first trip is the original blocks. Each later trip is a clone of the
header's work and of the body's, appended in the latch before its jump,
with operands renamed to the previous trip's values; the header phis'
latch arms are what carry a value from one trip to the next. A last clone
of the header's work supplies what the exit reads. Then the header phis are
replaced by their entry values and erased, the header's test becomes a
`br` to the body, and the latch jumps to the exit.

```text
  before                           after (count = 3)
  E: br H                          E: br H
  H: i = phi [0,E] [i1,L]          H: c0 = i0 < 3        ; dead
     c = i < 3                        br B
     br c, B, X                    B: w0 = work(i0)
  B: w = work(i)                   L: i1 = i0 + 1
  L: i1 = i + 1 ; br H                c1 = i1 < 3 ; w1 = work(i1)
  X: uses i                           i2 = i1 + 1
                                      c2 = i2 < 3 ; w2 = work(i2)
                                      i3 = i2 + 1 ; c3 = i3 < 3
                                      br X
                                   X: uses i3
```

Uses outside the loop are rewritten to the last trip's values. The
comparisons left behind are removed by `dead`, and constants that appear
(a copy's indices) by `fold` and the rest of the fixed point.

A function holding an instruction the target does not price
(`profit::priced`) is left alone. The pass expands one loop at a time and
starts again, so a loop around an expanded one may follow. A loop `peelsize`
refused is not priced again while its blocks are unchanged.

Loops with a branch in the body are copied by [`crate::peel`](peel.md) instead; a
nest's outer loop can be unrolled only by [`crate::unroll_jam`](unroll_jam.md).

`LLRM_DEBUG=unroll` prints one line per loop priced (size, unrolled size,
boost, undecided branches, and the reason for a refusal; or "admitted")
and `expanded a loop` for each loop copied.

## Runs
In the pipeline's fixed point, after each round of scalar passes and when
the body changed since the last time it found nothing. After it expands a
loop, `sroa` runs first (a copy's constant indices are new exact leaves),
then the scalar passes. On at -O1 and above and -Os; off at -Oz and by
`-fno-unroll-loops`. A copy may grow the code only where
`Limits::grows`: -O3 and -Omax (`target_percent` 200). Elsewhere a copy
must be no larger than the loop. See `docs/reference/levels.md`.

## Needs
`induction` (trip count), `peelsize`, `consts` and `Registers`
(`llrm_analysis`); [`crate::profit`](profit.md) for prices and the site.

## Keeps
Nothing: a change invalidates every analysis.

## See also
LLVM's LoopUnroll, full unroll (`UnrollLoop` at the trip count; cost model
`analyzeLoopUnrollCost`); gcc's `try_unroll_loop_completely`.
