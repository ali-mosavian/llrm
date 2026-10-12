# Loop copying: unroll, peel, unroll-and-jam

For contributors to `crates/opt`. Covers the passes that copy a loop out completely or partly: `unroll`, `peel`, `unroll-jam`, and `peelsize`, the analysis that decides whether a copy pays.

## Passes

| Pass | File | What it copies | Levels |
|---|---|---|---|
| `unroll` | [`unroll.rs`](passes/unroll.md) | a loop with an exact trip count, tested in its header, whose body is a chain of blocks to the latch: one straight line | -O1 and up (`-funroll-loops`); grows code only at -O3 and -Omax |
| `peel` | [`peel.rs`](passes/peel.md) | every block of a loop with an exact or bounded trip count of at least 2, once per trip, keeping the loop as a residual that `fold`, `decide` and `dead` then prove unreachable | -O3 and -Omax (`-fpeel-loops`) |
| `unroll-jam` | [`unroll_jam.rs`](passes/unroll_jam.md) | the outer loop of a perfect two-loop nest by N, the N inner loops fused | -O3 only (`-floop-unroll-and-jam`) |

```
 unroll (trip count 3)                    peel
 header: i<3 ?                            copy of loop, trip 1
 body                                     copy of loop, trip 2
 latch: i++ --> header                    copy of loop, trip 3
         |                                residual loop (unreachable after fold)
         v
 body(i=0); body(i=1); body(i=2)
```

`unroll` is for the shape the pass can lay out in one line. `peel` covers a loop whose count is known only once an enclosing loop is specialised (a triangular inner loop, `j` from `i + 1`): it first closes the loop's live-outs (`lcssa`), clones every block with `loopclone::peeled`, and the ordinary fixed point must remove the residual loop. Both leave the function alone if an instruction in it has no price in the target's operation costs (`profit::priced`).

## Whether a copy pays

`peelsize::admitted` ([`peelsize.rs`](analyses/peelsize.md)) follows gcc's `try_unroll_loop_completely` with LLVM's size estimate: each iteration is run over the values `consts` knows, what folds is free, and only the successors a folded branch leaves are followed. The budgets count MIR instructions, so no machine number enters but the target's `unroll_budget`.

1. The language may say how many copies it permits, or never; fewer than the trip count is a refusal (there is no partial unroll).
2. More than `max_unroll_iterations` (16) trips: refused.
3. A copy no larger than the loop always pays. Otherwise refused if code may not grow (`Limits::grows`, off below -O3), the loop is cold (entered less than 5% as often as its function), it calls something that touches memory, it has more than 32 undecided branches, or the estimate (two thirds of the copies' size) exceeds the budget raised by the share of rolled work the copy no longer does (up to 400%).
4. A loop holding another loop is copied only if that shrinks it.

The budget is the target's `unroll_budget` times `target_percent` (100; 200 at -O3 and -Omax). The numbers and their gcc and LLVM counterparts are in [levels](../reference/levels.md).

## Unroll-and-jam

Applies to a perfect nest of two header-tested loops where the inner loop leaves only from its header, its count is the same in every outer iteration, it produces nothing the outer loop reads, and only pure work lies between the loops. N is the largest factor up to 4 that the dependences allow (`dependence.rs`) and that divides the outer trip count; the pass requires the jam to remove at least 1% of the nest's accesses. Not handled: a count N does not divide, loads between the loops, a rotated inner loop, and an access whose address is not affine in the two counters.

## See it

`LLRM_DEBUG=unroll` prints one line per loop with its size, unrolled size, boost, branches and the reason for a refusal. `-fno-unroll-loops`, `-fno-peel-loops`, `-fno-loop-unroll-and-jam` turn a pass off. `--dump DIR` shows the module around `unroll` and `peel`.

Catalog: [passes](passes.md). Analyses: [analyses](https://github.com/ali-mosavian/llrm/tree/main/crates/opt/llrm-analysis/src).
