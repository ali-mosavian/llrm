> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/difference.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/difference.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/difference.rs sha256:5873cd793934f50b781613da9ef057568f844fa663262857456bd42e7acfa517 -->

# difference

What a set of compares proves of the unknowns they test, by difference
bounds: LLVM's ConstraintElimination.

## How it works
Each side of a compare is a `Scev` that is one unknown plus a constant
offset, or a constant. A compare of two sides is a constraint
`x - y <= c`. `proves` builds a graph with one node per unknown plus node 0
for the constant zero (`Z`), puts the constraints in as edges, and closes it
with Floyd-Warshall so `dist[a][b]` is the most `a - b` can be. The asked
compare holds when the closure bounds its `left - right` tightly enough.

```text
  facts:  x < y        y - x <= -1      node y --(-1)--> node x
          y <= 10      y - Z <= 10      node y --(10)--> Z
  asked:  x < 11       x - 11 <= -1     closure: x - Z <= (x - y) + (y - Z)
                                                   <= -1 + 10 = 9  -> holds
```

Other facts the system takes in:
- A stated range `[low, high]` of an unknown bounds it against `Z`.
- Unsigned compares. An unsigned compare is a signed one where both sides
  are known non-negative. `small[n]` marks unknowns that are: `Z` itself, an
  unknown whose stated low is at least 0, a `x <u y` with `y` non-negative,
  and any unknown the closure bounds below by zero (the closure is then run
  again, at most four rounds). Unsigned facts that are not so are ignored,
  as are `ne` facts.
- A negative cycle means the facts contradict each other: everything is
  proved.

An offset side is trusted only if the closure keeps `unknown + offset`
inside the signed range, so the sum cannot wrap; an unsigned goal also needs
both sides' least value to be at least zero. Facts with an offset on an
unknown are dropped (only offset-free sides state a fact), and a system
with more than 24 nodes (`MOST_NODES`; the closure is cubic) proves
nothing. The answers: `slt`/`ult` is `left - right <= -1`, `sle`/`ule` is
`<= 0`, `sgt` and `sge` swap the sides, `eq` is both ways, `ne` is either
way strictly.

## Example
The compares of quicksort's slice loop, over 16-bit unknowns `lo`, `hi` and
`len`:

| Facts | Range | Asked | Result |
| --- | --- | --- | --- |
| `hi <u len`, `lo <s hi`, `lo <u len` | `len` in `[0, 32767]` | `hi - 1 <u len` | proved |
| same | same | `lo <=u hi - 1` | proved |
| same | none | `hi - 1 <u len` | not proved |
| same | none | `lo <=u hi - 1` | not proved |

Without a range on `len`, `lo <u len` leaves `lo` possibly negative (`len`
0xFFF0, `lo` 0x8008 passes), so the last index `hi - 1` is not below `len`.

From unit tests `test_a_slice_loops_last_index_is_below_the_length` and
`test_an_unbounded_length_proves_nothing_of_the_last_index` in
`difference_tests.rs`.

## Provides
`proves(width, facts, ranges, predicate, left, right)`. Its one consumer is
[`crate::guards`](guards.md) (`Given::holds`), which supplies the guards and the
declared ranges.

## Needs
[`crate::guards::Guard`](guards.md)s and [`crate::induction`](induction.md)'s `Scev` and `Monomial`.

## See also
LLVM `ConstraintElimination`.
