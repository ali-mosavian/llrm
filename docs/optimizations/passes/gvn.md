> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/gvn.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/gvn.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/gvn.rs sha256:37ce3d88a704ee4e7ff2e884a1f0b93bdd2fce887aa90311528a6bdb37c6ab37 -->

# gvn

Reuses a value the function already has, so the same operation is not
computed twice and the same bytes are not read again: LLVM's GVN and gcc's
FRE/PRE. One pass owns reuse of a value, whether it came from arithmetic, a
load or a store. `Gvn` is the pass; `EqualCalls` is its call part alone.

## How it works
`Gvn` runs these steps on a body, in order.

1. The scalar pass (`pass::run_impl`, `table`: LLVM's `GVNPass` and
   `ValueTable` without the load and PRE parts). Blocks with one way in are
   merged into it, then each block is met in reverse postorder, its
   instructions simplified or numbered by opcode, type and operand numbers
   (a commutative pair ordered, a compare's predicate swapped with its
   operands, a GEP by the bytes it adds, flags in no key): one that a
   dominating leader has is replaced, keeping the flags both have. In what
   an edge of a branch, a switch case or an assume dominates, the condition
   is that edge's constant (`propagateEquality`), and the arm of a branch on
   a constant is dead: its join's phis read poison from it. Two calls with
   the same callee and arguments that read and write no memory are one.
2. Load numbering (`transform::forwarded`, `transform::subexpressions`,
   `transform::equal_calls`). A load a known value serves becomes that
   value: a dominating store of the same bytes and type, or an earlier load
   of them, with nothing between that may write them. The blocks are met
   dominators first, not in layout order, which a removed join leaves behind
   the blocks it came to dominate.
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

A volatile access is never numbered. Floats are numbered like integers; a
value serves a read only of its own type, so a float store reaches a float
load exactly and nothing rounds. Which floats are exact constants is
[`crate::floatfold`](../backend/floatfold.md)'s question.

What serves a load is a MemorySSA walk (`avail::forwardable_walk`).
`memoryssa::Accesses` says what each instruction reads and writes,
`alias::PointValues` what each pointer may point to, and a call's footprint
is its `CallEffects` from `Summaries`; without `Summaries` every callee is
unknown. A call that may write the bytes, or a store that may overlap, ends
the walk, and a walk past `MAX_NEAREST_WRITES` (100) writes gives up.

A value held across a store saves a load but lengthens a live range. Where a
load was served across a store the function is numbered twice, once crossing
stores and once avoiding them, and the crossing one stands unless the
careful one prices lower (`profit::motion_price`: work weighted by block
frequency plus the spill forecast against the target's registers). Where
nothing spills the crossing one stands. Trip counts are the proven ones
(`profit::proven_trips`). Without `pre` there is no pricing and a load is
served wherever it is redundant on every path.

`EqualCalls` runs the call numbering alone, before `tailrec`, so two equal
calls are one before one of them becomes the loop's next trip (EarlyCSE
precedes tail-recursion elimination in LLVM, FRE in gcc). It asks for no
analysis unless the body has two such calls.

## Example
A store's value reaches the load after it, and the load below a join takes
the value each arm stored. Before:

```mir
@g = global i16 0

define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  store i16 %x, ptr @g
  %r = load i16, ptr @g
  ret i16 %r
}
```

After:

```mir
define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  store i16 %x, ptr @g
  ret i16 %x
}
```

From unit test `test_gvn_forwards_a_stored_value_to_its_load` in
`gvn_tests.rs`. Before:

```mir
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %x, ptr @g
  br label %b3

b2:
  store i16 %y, ptr @g
  br label %b3

b3:
  %r = load i16, ptr @g
  ret i16 %r
```

After:

```mir
b3:
  %r1 = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  ret i16 %r1
```

`b0` to `b2` are unchanged. From unit test
`test_gvn_joins_the_values_each_arm_stored` in `gvn_tests.rs`.

## Runs
`EqualCalls` runs early in the body pipeline, before `tailrec`; `Gvn` after
`dse`, before `promote`. Both are on at every optimizing level and are
turned off together by `-fno-gcse`. `Gvn`'s PRE steps and pricing (`pre`)
are on from -O2, the -O1 level being FRE, and off with `-fno-tree-pre`. Each
runs once per body pass.

## Needs
[`crate::transform`](transform.md), [`crate::loadjoins`](loadjoins.md), `memoryssa::Accesses`,
`alias::PointValues`, `Summaries`, registers and loop trips for pricing.

## Keeps
Dominators and loops, unless a join split an edge.

## See also
Tests: `gvn_tests.rs`, `loadjoins_tests.rs`. Levels:
`docs/reference/levels.md`. LLVM `GVN.cpp`; gcc `tree-ssa-pre.cc`.
