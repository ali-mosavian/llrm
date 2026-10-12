> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/peelsize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/peelsize.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/peelsize.rs sha256:2a2f2c3eefe7d1353bdd3adf45185d236d82024351161c1224aa36e70342107e -->

# peelsize

Whether a loop is worth copying out completely, decided before anything is
cloned.

## How it works
`admitted(unit, loop, count, facts, limits, site)` answers for a loop whose
trip `count` is known. The decision is GCC's `try_unroll_loop_completely`;
the size it is given is LLVM's `analyzeLoopUnrollCost`. Budgets count MIR
instructions, as GCC's `--param`s count insns, so no machine number enters.

Size. `unrolled` runs each of the `count` iterations over the values it
knows (the header phis take the previous iteration's values) and the memory
it has written. An instruction that folds (`consts`) costs nothing; one that
does not adds its size (1, or its operand count for a call lowered to one).
A branch or `switch` on a known value follows only the chosen successor; an
undecided one adds 1 to the size and to `branches`. It gives up once the
size passes the budget. It also sums `rolled`, the instructions the rolled
loop would execute over all iterations.

```text
  rolled loop (count = 3)           copied out
  header: i = phi [0, i+1]          i = 0   (folds)
          t = i * 4   (folds)       t = 0   (folds)
          x = load p[t]             x0 = load p[0]    cost 1
          br i < 3                  (decided: no branch kept)
  size = unfolded ops over 3 iterations; rolled = all ops over 3
```

The tests, in order:

1. The language's `unroll` hint on a latch: 0 refuses; fewer permitted
   copies than `count` refuses (no partial unrolling); otherwise the copy is
   asked for and is judged against `HINTED_OPERATIONS` (16384).
2. Unhinted, more than `max_unroll_iterations` (16; 0 is unbounded) refuses.
3. A loop holding another loop (`_ordered` fails) is copied only when that
   shrinks it: `count * (size - folded) <= size`.
4. The simulated copy's size is scaled by two thirds, for what later passes
   remove. A copy no larger than the loop, or one asked for, is admitted.
   Otherwise it is refused when `grows` is false (-Os), when the loop is
   cold (`Site::entries` under 5% of an entry), when it holds a call that
   touches memory (`Site::writes`), when undecided branches exceed 32
   (`max-peel-branches`), or when the scaled size exceeds
   `max_unrolled_operations` times the boost. The boost is LLVM's
   `getFullUnrollBoostingFactor`: `rolled / size` in percent, capped at 400.

`Limits::on` replaces `max_unrolled_operations` by `target_percent` of the
target's `unroll_budget` where both are set. Each decision is logged under
the `unroll` debug tag.

## Provides
`admitted`, `Limits`, `Site`. Consumers: the `peel` and `unroll` passes in
llrm-transforms (`profit` builds the `Site`).

## Needs
[`crate::consts`](consts.md) (folding of an instruction over known values),
[`crate::induction`](induction.md) (`basics`, the loop's counters), [`crate::cfg`](cfg.md),
[`crate::graph`](graph.md) loops.

## See also
GCC `try_unroll_loop_completely`, `estimated_unrolled_size`
(`gcc/tree-ssa-loop-ivcanon.cc`); LLVM `analyzeLoopUnrollCost` and
`shouldFullUnroll` in `LoopUnrollPass`.
