> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/fold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/fold.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/fold.rs sha256:8211eeb7df66784f684aef44d49f101be12a9461429e01d25fba01ca742ca42e -->

# fold

Replaces every integer value [`llrm_analysis::consts`](../analyses/consts.md) knows by its
constant, folds a pure operation on each edge of a join, and puts compares
and identities in canonical form. A constant operand is free to encode and
lets the later passes see through the computation; the definition it
replaces is left dead.

## How it works
Facts come from `consts`: "the low `width` bits of this value are `n`",
solved through registers, phis and memory (a load of a cell every reaching
store fills with one number is that number). What each call writes is
`manager::writes`, so a constant stored before a `readonly` call survives
it. A fact replaces its value only when it is as wide as the value. A
division or remainder of two known numbers is computed here too.

Each use of a known value becomes the constant at once, so a store's value,
a call argument, a fill's byte or an address index need no case of their
own, and the definition is left unread for [`crate::dead`](dead.md) to take. A
commutative operation that ends up with the constant on the left gets it
back on the right; an ordered one (`sub`, shifts) keeps its operand order.

A call kills only the cells its writes may overlap
(`alias::calls_by_attributes`, from the call's `memory(...)`, `readonly` and
`readnone` attributes and `GlobalsModRef`); one that states writes without
an effect list clears every cell. A fact about reaching an edge says nothing
about a callee, so edge facts clear all cells at a call.

```text
 store i16 9, ptr @g          store i16 9, ptr @g
 %a = load i16, ptr @g   ==>  call void @clobber()   ; writes @g
 call void @clobber()         %b = load i16, ptr @g  ; unknown again
 %b = load i16, ptr @g        (%a is 9; with @clobber readonly, %b too)
```

One edge fold runs per call. At a block with two or more predecessors, a
pure `Binary` or `Cast` in the block (or in the single-predecessor chain
below it) that reads one of its phis, and whose answer is a number on every
incoming edge, becomes a phi of those numbers. An operation that also reads
an unknown value stays.

A phi in one arm of a `br i1` that takes the branch's own condition from the
branch's block reads that arm's constant (GVN's `propagateEquality`, over
the edge's phi uses): loop rotation and `lcssa` make `phi i1 [%c, %latch]`
for a test after the loop, and no GVN runs after them. The branch on the phi
then decides itself, with no `setcc` and `test` for the flag.

Then [`crate::floatfold`](../backend/floatfold.md) runs on the same body (exact floats, and the
integers of exact conversions), and [`crate::canonical`](canonical.md) swaps a compare
whose constant is on the left and simplifies identities. The pass reports
every analysis invalid when anything changed.

## Example
Before:

```mir
define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  %y = add i16 %p, %x
  %s = add i16 %p, 10
  %t = add i16 %s, %y
  ret i16 %t
}
```

After:

```mir
b3:
  %p = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  %s1 = phi i16 [ 11, %b1 ], [ 12, %b2 ]
  %y = add i16 %p, %x
  %t = add i16 %s1, %y
  ret i16 %t
```

`%s` is a number on both edges, so it becomes the phi `%s1`; `%y` reads
the unknown `%x` and stays. Blocks `b0` to `b2` are unchanged.

From unit test `test_an_operation_on_a_join_is_folded_on_its_edges` in
`fold_tests.rs`.

## Runs
Second in the per-body pipeline, after `sroa`, and again after the header
copy of `rotate` (to clean the test it leaves in the latch). Part of the
fixed point; on at -O1 and up, off at -O0. `Options::fold` turns it off
for bisection. See `docs/reference/levels.md`.

## Needs
[`llrm_analysis::consts`](../analyses/consts.md), [`llrm_analysis::floatfacts`](../analyses/floatfacts.md),
`manager::writes` (`Summaries`).

## See also
LLVM's
[`SCCP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp)
and `InstSimplify`; GCC's `tree-ssa-ccp.cc`.
