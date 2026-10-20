> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/dependence.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/dependence.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/dependence.rs sha256:0f378a54f1004a6638a27928cb41d3bc35017b4c4f836cf25aa2535cd642de05 -->

# dependence

Data dependence distances over a two-deep loop nest, and whether its two
loops may swap.

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

`interchangeable(accesses, outer_trips, inner_trips)` asks whether the loops
may swap: a vector `(x, y)` with `x > 0` and `y < 0` becomes `(y, x)`, the
later access first, so none may exist for a pair with a write. `walk_cost`
prices a walk by the bytes an access steps in the loop that runs innermost,
at most a cache line, none where it does not move.

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

The write of `px[j]` is read again by the next outer pass at `(1, 0)`.

## Provides
`distances`, `interchangeable`, `walk_cost`. Consumer: `loopinterchange`.

## See also
gcc `tree-data-ref.cc` distance vectors; LLVM `DependenceAnalysis`.
