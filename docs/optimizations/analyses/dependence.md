> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/dependence.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/dependence.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/dependence.rs sha256:ae91decdbee785e8133b3c16a1dfdcf6efd9ec506143704f9513786acf130c83 -->

# dependence

Data dependence distances over a two-deep loop nest, and the outer unroll
factor that unroll-and-jam may use for them.

## How it works
An `Access` is one memory reference of the nest body: an object, a list of
`Subscript`s (`constant + outer * i + inner * j`, `i` and `j` the iteration
numbers; `a[i][j]` has two, a flattened address one), loop-invariant
`symbols`, a width and a write flag. The caller has found the objects.

`distances(first, second, limit, inner_trips)` solves
`outer * x + inner * y = constant difference` in each subscript, in both
directions, for outer distances `x` in `0..=limit` and one inner distance
`y`. Pairs with different objects are `Independent`. Pairs whose symbols,
width or coefficients differ are `Unknown`. Otherwise the result is the
lexicographically non-negative `(x, y)` vectors, with `y` bounded by
`inner_trips` where known. A reference no subscript moves with in the inner
loop repeats at `(x, 0)` and, for `x = 0`, `(0, 1)`.

`factor(accesses, max_unroll, min_percent, inner_trips)` looks at every pair
of accesses. Unrolling the outer loop by N turns `(x, y)` into `(x / N, y)`,
so a vector bounds N when it would reorder the pair in the fused loop:

```text
 for i                     for i step N
   for j        ==>          for j
     S(i, j)                   S(i,j) S(i+1,j) ... S(i+N-1,j)

 vector (x, y) ok if x >= N, or y > 0; y = 0 with x < N needs two reads or
 two accesses that both vary with j; else N is capped at x (y < 0).
```

Two reads never order anything; an `Unknown` pair with a write gives
`unroll = 0`. `profit` is one more than the largest outer distance at which
a reference repeats at `y = 0` (copies that make it go away), `removed`
counts such vectors, and the factor is refused (`profit = 1`) if
`removed * 100 / accesses` is below `min_percent`. `min_percent = 0` applies
always, with at least 2. The result is `min(unroll, profit, max_unroll)`.

## Example
The 100 passes of `px[j] = px[j] + vx[j]` (bench/particle). The nest has no
MIR form here: the caller hands `Access`es, each a `Subscript { constant,
outer, inner }` per dimension, width 2:

```text
nest[0]  read   px[j]   subscripts [ (0, 0, 1) ]
nest[1]  write  px[j]   subscripts [ (0, 0, 1) ]
nest[2]  read   vx[j]   subscripts [ (0, 0, 1) ]    (same object as the others)
```

| Query | Result |
| --- | --- |
| `distances(nest[0], nest[1], 4, Some(101))` | `Vectors([(0, 0), (1, 0)])` |
| `factor(nest, 4, 1, Some(101))` | `unroll = 2`, `profit = 2` |

The write of `px[j]` is read again by the next outer pass at `(1, 0)`, so
two copies make a reference go away.

From unit test
`an_array_walked_again_by_each_outer_pass_allows_a_jam_by_two` in
`dependence_tests.rs`.

## Provides
`distances`, `factor`. Consumer: `unroll_jam`
(`crates/opt/llrm-transforms/src/unroll_jam.rs`).

## See also
gcc `tree-data-ref.cc` distance vectors read by `adjust_unroll_factor`
([`gimple-loop-jam.cc:403`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc#L403)),
`tree_loop_unroll_and_jam`
([`gimple-loop-jam.cc:504`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc#L504));
LLVM `DependenceAnalysis` and `LoopUnrollAndJam`.
