> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/edges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/edges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/edges.rs sha256:005a9a99d2a3e8e3a40ed3ccf11d381a875ac2c37b7712503173bcb842e4d1c4 -->

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

`retarget` redirects every operand of a terminator that names one block to
another, and `phis` lists a block's leading phis.

## Runs
Not a pass: a library. `split` is called by `loadjoins`, `rotate` and
`unswitch`; `phis` and `retarget` by most of the CFG-rewriting passes.
