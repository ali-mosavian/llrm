> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/dependence.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/dependence.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/dependence.rs sha256:12a79a05243b309e6a2334bd51d283b63f250c0572b913d8886275a256c2e490 -->

# dependence

Data dependence distances over a two-deep loop nest, the outer unroll
factor that unroll-and-jam may use for them, and whether two loops may be
fused.

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

`fusable(first, second, trips)` asks it of two loops of the same trip
count, `first` run before `second`: fused, iteration `i` of `second` runs
before iteration `i + 1` of `first`, so a pair that touches the same bytes
with the later loop's access at an earlier iteration than the earlier
loop's is reordered. Both loops' accesses are `inner` subscripts. For a
pair with one write, on one object, of one coefficient `c` and the same
symbols, the accesses at `a + c * i` (width `w`) and `b + c * j` (width `v`)
overlap where `b - a - w < c * (i - j) < b - a + v`; the pair prevents
fusion when some `i - j` in `1..trips` does (`trips` unknown: any). A pair
with other coefficients or symbols is `Unknown` and prevents it.

`meetings(first, second, trips)` is the same question of two accesses of one
body, `first` earlier in it: whether they meet with `first` at or before
`second` (`forward`, `i - j` in `1 - trips..=0`) or after it (`backward`,
`1..trips`). Loop distribution puts the two in one loop where both hold and
in two, in body order, where only `forward` does.

```text
  loop 0: a[i] = ..        loop 1: .. = a[i + 1]     i - j = 1: prevents
  loop 0: a[i] = ..        loop 1: .. = a[i - 1]     i - j = -1: allows
```

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

`interchangeable(accesses, outer_trips, inner_trips)` asks whether the loops
may swap: a vector `(x, y)` with `x > 0` and `y < 0` becomes `(y, x)`, the
later access first, so none may exist for a pair with a write. `walk_cost`
prices a walk by the bytes an access steps in the loop that runs innermost,
at most a cache line, none where it does not move.

## Provides
`distances`, `factor`, `fusable`, `meetings`, `interchangeable`,
`walk_cost`. Consumer: `loopinterchange`.

## See also
gcc `tree-data-ref.cc` distance vectors read by `adjust_unroll_factor`
([`gimple-loop-jam.cc:403`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc#L403)),
`tree_loop_unroll_and_jam`
([`gimple-loop-jam.cc:504`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc#L504));
LLVM `DependenceAnalysis` and `LoopUnrollAndJam`; `fusable` is LLVM
`LoopFuse`'s `dependencesAllowFusion`
([`LoopFuse.cpp:1178`](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LoopFuse.cpp#L1178)).
