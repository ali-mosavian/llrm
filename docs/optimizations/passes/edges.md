> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/edges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/edges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/edges.rs sha256:105b474e143b4087c37fe2783d7fec3378ace5959110defe06507765c0a916e1 -->

# edges

Edge surgery on conditional branches: a new block on one arm of a `br i1`,
holding the operations that belong to that edge and not to either arm's
block.

## How it works
A conditional edge is one of the two arms of a block's `br i1`.
`conditional` tests that `block` ends in a two-way `br` with one arm to
`target`; `explicit` adds that the arm is the branch's taken (condition
true) operand.

`split(function, source, target, instructions)` creates a block, places the
given instructions in it followed by a `br target`, redirects the `source`
branch's operands that name `target` to the new block, and renames `source`
to the new block in the incoming blocks of `target`'s phis. It refuses an
edge that is not a conditional one. Block ids are never reused, so
`create_block` is a fresh label.

```text
  source: br i1 %c, target, other        source: br i1 %c, bridge, other
  target: %p = phi [%v, source], ..  ==> bridge: <instructions>
                                                 br target
                                         target: %p = phi [%v, bridge], ..
```

`conditions_decided` makes a phi operand that is a branch's own condition,
read on the edge that branch's arm is, that arm's constant: `phi i1 [%c,
%latch]` in an exit is `false` (GVN's `propagateEquality` for the edge's own
phi uses, which `fold` runs after rotation and `gvn` before).

`retarget` redirects every operand of a terminator that names one block to
another, and `phis` lists a block's leading phis.

## Example
Before, `%x` is loaded at the join of `b1` and `b2`, where only `b1` has the
cell's value:

```mir
b2:
  br i1 %d, label %b3, label %b4

b3:
  %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
; ...
```

After load PRE inserts the missing load on the `b2`-to-`b3` edge:

```mir
b2:
  br i1 %d, label %0, label %b4

0:
  %1 = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b3:
  %x1 = phi i16 [ %y, %b1 ], [ %1, %0 ]
; ...
```

`b2` has another successor, so the load goes in a new block `0` on the edge,
not in `b2`.

From unit test `test_missing_conditional_edge_gets_its_own_load_block` in
`gvn_load_tests.rs`.

## Runs
Not a pass: a library. `split` is called by `rotate` and
`simpleloopunswitch`; `phis` and `retarget` by most of the CFG-rewriting
passes.
