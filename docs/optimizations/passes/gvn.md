> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/gvn.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/gvn.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/gvn.rs sha256:8a974e2fbf5e90f62a50f1d6c25f7712d91751ce62bcdeb028b350092d7c291a -->

# gvn

Global value numbering and load reuse: LLVM's GVN and gcc's FRE/PRE. One
pass owns reuse of a value, whether it came from arithmetic, a load or a
store. `Gvn` is the pass; `EqualCalls` is its call part alone.

## How it works
`Gvn` runs these steps on a body, in order.

1. Branch equality (`propagated`, LLVM's `propagateEquality`). In what the
   edge to a successor dominates, the branch's condition is that edge's
   constant.
2. Numbering (`transform::forwarded`, `transform::subexpressions`,
   `transform::equal_calls`). A load a known value serves becomes that
   value: a dominating store of the same bytes and type, or an earlier load
   of them, with nothing between that may write them. Two pure operations on
   the same operands are one, a copy counting as its source and a
   commutative pair ordered. Two calls with the same callee and arguments
   that read and write no memory are one.
3. Join PRE (`joined`). An expression every predecessor of a join already
   computes becomes a phi of those values. With `pre`, a predecessor that
   lacks it gets the computation inserted before its unconditional `br`, if
   no loop is entered or left by the edge and the operands are available
   there. Divisions (which may trap on a path that did not divide), loads
   and floating expressions are never inserted here.
4. Load PRE (`loadjoins::reused`). A load at a join whose bytes every
   predecessor holds becomes a phi of those values; with `pre`, a
   predecessor holding none gets its own load on its edge where that
   speculates nothing.

```text
 b0: store i16 %x, ptr @g          b0: store i16 %x, ptr @g
     %r = load i16, ptr @g   =>        ret i16 %x
     ret i16 %r

 b1: store %x, @g   b2: store %y, @g    b3: %r1 = phi i16 [%x,%b1],[%y,%b2]
 b3: %r = load @g                 =>        ret i16 %r1
```

A volatile access is never numbered. Floats are numbered like integers; a
value serves a read only of its own type, so a float store reaches a float
load exactly and nothing rounds. Which floats are exact constants is
[`crate::floatfold`](../backend/floatfold.md)'s question.

What serves a load is a MemorySSA walk (`avail::forwardable_walk`).
`memoryssa::Accesses` says what each instruction reads and writes,
`alias::PointsTo` what each pointer may point to, and a call's footprint is
its `CallEffects` from `Summaries`; without `Summaries` every callee is
unknown. A call that may write the bytes, or a store that may overlap, ends
the walk, and a walk past `MAX_NEAREST_WRITES` (100) writes gives up.

A value held across a store saves a load but lengthens a live range. Where a
load was served across a store the function is numbered twice, once crossing
stores and once avoiding them, and the crossing one stands unless the
careful
one prices lower (`profit::motion_price`: work weighted by block frequency
plus the spill forecast against the target's registers). Where nothing
spills the crossing one stands. Trip counts are the proven ones
(`profit::proven_trips`). Without `pre` there is no pricing and a load is
served wherever it is redundant on every path.

`EqualCalls` runs the call numbering alone, before `tailrec`, so two equal
calls are one before one of them becomes the loop's next trip (EarlyCSE
precedes tail-recursion elimination in LLVM, FRE in gcc). It asks for no
analysis unless the body has two such calls.

## Runs
`EqualCalls` runs early in the body pipeline, before `tailrec`; `Gvn` after
`dse`, before `promote`. Both are on at every optimizing level and are
turned
off together by `-fno-gcse`. `Gvn`'s PRE steps and pricing (`pre`) are on
from
-O2, the -O1 level being FRE, and off with `-fno-tree-pre`. Each runs once
per
body pass.

## Needs
[`crate::transform`](transform.md), [`crate::loadjoins`](loadjoins.md), `memoryssa::Accesses`,
`alias::PointsTo`, `Summaries`, registers and loop trips for pricing.

## Keeps
Dominators and loops, unless a join split an edge.

## See also
Tests: `gvn_tests.rs`, `loadjoins_tests.rs`. Levels:
`docs/reference/levels.md`. LLVM `GVN.cpp`; gcc `tree-ssa-pre.cc`.
