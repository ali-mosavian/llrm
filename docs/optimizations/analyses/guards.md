> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/guards.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/guards.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/guards.rs sha256:d809953495bd6f99c4e03d090b2b8e8ac4da650b2f580b7ebd3d119aecddb9b9 -->

# guards

What the branches that dominate a block prove on entry to it.

## How it works
`guards(unit, at)` climbs the dominator chain from block `at`, at most 64
blocks (`MAX_DOMINATORS`), and at each dominator `above` collects two
things. The first is the conditions the block assumes (`assume` calls, from
[`crate::assumptions`](assumptions.md)), which hold below it wherever its terminator goes.
The second is the compare of `above`'s conditional branch, when the edge
toward `at` alone enters the block it leads to: that block dominates `at`
and every other predecessor of it is dominated by it (it is a back edge), so
the branch outcome holds on every path to `at`. A compare is read as `Guard
{ predicate, left, right }` over `Scev`s. `_proven` also opens an `and`
that is true or an `or` that is false into both parts, and flips the
predicate when the edge is the false one.

```text
   b0: br (n > 2), b1, b9          guards(b2) = { n > 2, n != 0 }
    |
   b1: br (n != 0), b2, b9         b1 and b2 each have one predecessor, so
    |                              each branch holds on every path to b2
   b2:  <- here
```

`on_edge(unit, from, to)` is the guards of `from` plus what its own branch
says of the edge to `to`. `decides(unit, facts, condition)` answers a
compare with true or false when a guard implies it or its inverse.

`implies(guard, predicate, left, right)` is direct implication: the same two
sides (either way round) under a predicate at least as strong (`_stronger`:
`eq` over `ule`, `ult` over `ule` and `ne`, ...); `a != 0` proves `0 <u a`;
equality and disequality hold at any offset, as a wrapping sum is a
bijection. `implies_at_offset` does the signed case where the sides differ
only in constants (`n > 2` proves `1 < n - 1`), exact only where the caller
knows nothing wraps.

`holds(unit, at, predicate, left, right)` is true when both sides are
constants and the compare is true (`evaluated`), when a guard implies it,
when it is an unsigned compare of zero extensions that a guard proves in the
narrow type (`narrowed`), or when it is a compare of a phi against a value
fixed over the loop, where each incoming edge proves it of the value it
brings (`through_phi`: a loop entered from a guard and from its own latch
test). `holds_given` and `Given` add the `assumed` guards a caller supplies,
the guards on each loop's followers of its counters (`start <= follower <=
counter`) and the program's stated ranges, and ask
[`crate::difference::proves`](difference.md); `Given` finds the guards of a block once for
as many questions as are asked of it.

## Example
The edge into `loop` is the only way there, so its branch holds in `loop`
but not in `done`:

```mir
define i16 @f(i16 %n, i16 %len) {
entry:
  %fits = icmp ult i16 %n, %len
  br i1 %fits, label %loop, label %done
loop:
  br label %done
done:
  ret i16 0
}
```

| Query | Result |
| --- | --- |
| `holds` in `loop`: `n <u len`, `n <=u len`, `len >u n` | true |
| `holds` in `loop`: `len <u n` | false |
| `guards(done)` | empty (the other edge reaches `done`) |

From unit test `test_a_branch_proves_its_compare_where_its_edge_alone_leads`
in `guards_tests.rs`.

## Provides
Guard facts and the implication queries above. Consumers: `decide`,
`exitfold` and `jumpthreading` in `llrm-transforms`, and
[`crate::induction`](induction.md) (loop entry guards).

## Needs
[`crate::cfg::Shape`](cfg.md), [`crate::assumptions`](assumptions.md), [`crate::induction`](induction.md)'s `Scev`
(`term`, `followers_of`), [`crate::ranges::declared`](ranges.md) and
[`crate::difference`](difference.md).

## See also
LLVM `isBasicBlockEntryGuardedByCond`, as far as direct implication goes;
`ScalarEvolution::collectFromBlock`, which climbs the unique-predecessor
chain only
([`ScalarEvolution.cpp:16199`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L16199)).
