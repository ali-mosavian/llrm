> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/maskscale.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/maskscale.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/maskscale.rs sha256:2a9d1e8d5946d4ae52965b2c034c27b788537ed607df16340129e9d823c1bcaa -->

# maskscale

A masked counter read as `a[i & M]` is pre-scaled to the byte offset.

## How it works
A loop counter `i` (width `w`, constant step `c`) whose only readers are
its own latch add, one `and i, M` and an exit `icmp eq/ne` of `i` or `i + c`
against a constant `C`, where the `and` is read only as the index of
`gep`s over elements of `S = 2^k` bytes, becomes `i' = S * i` (wrapping):
start, step and `C` times `S`, mask `M * S`, and the `gep`s over bytes. The
address then needs no scaled index, which real mode forms with a prefix and
an extra `shl`.

The rewrite is exact when the scaled counter reaches the exit value on the
same trip as the original: `S * t == S * C` is `t == C` mod `2^(w-k)`. Both
first hits are solved in closed form (an odd step or not) and must agree,
which is the trip count staying within the scaled counter's period. `M * S`
must stay below `2^(w-1)`, so the byte offset is the same under any
extension. The latch add loses its wrap flags. Only a target that prices a
scaled index (`Tti::scaled_index_insns`) runs it. Cost: O(uses) per loop.

DEVIATE LP-7 (no pin pass: LLVM's LSR has no formula for a masked, scaled
counter, and gcc's ivopts (`iv_period`, `iv_elimination_compare`,
tree-ssa-loop-ivopts.cc) rewrites an exit test by the counter's period but
never the mask; exit: pin LSR gains the formula, this is deleted; owner
w-2).

## Example
The ring loop of c's `ring` at m16 (schematic):

```text
 %i = phi i16 [ 3 ], [ %next ]            %i = phi i16 [ 12 ], [ %next ]
 %m = and i16 %i, 1023                    %m = and i16 %i, 4092
 %p = gep [4 x i8], %buf, %m        ==>   %p = gep i8, %buf, %m
 %next = add i16 %i, 5                    %next = add i16 %i, 20
 %c = icmp eq i16 %next, 30003            %c = icmp eq i16 %next, 54476
```

From the unit test `the_ring_counter_is_the_byte_offset` in
`maskscale_tests.rs`.

## Runs
After `lsr` and `differences`, in the strength slot
([`crate::pipeline::strength_slot`](pipeline.md)); option `maskscale`.
