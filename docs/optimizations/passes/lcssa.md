> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lcssa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lcssa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lcssa.rs sha256:920c7cda5fac3f227c99e4d4cddaaf7eccf5bf092b3200af236472723b8f9297 -->

# lcssa

Put every natural loop in loop-closed SSA form.

## How it works
A value defined in a loop and read outside it is read through a phi in the
loop's exit block. Later loop passes can then change the loop's body, copy
it or move its exits without rewriting readers outside it.

Loops are taken innermost first, so an inner exit phi is an ordinary
definition of the enclosing loop. For one loop:

- With a single exit block whose predecessors are exactly the exiting
  blocks, a value is closed when every block that reads it is dominated by
  the exit and its definition dominates every exiting block. Each such value
  gets a phi in the exit with one input per exiting block, and every outside
  reader the exit dominates is rewritten to read it. A phi that reads the
  value on an edge counts at that edge's source block. If the exit has other
  predecessors, the loop is left for `loopsimplify`.
- With several exit blocks, [`crate::lcssamerges`](lcssamerges.md) makes a phi at each and
  merges them where the exits' paths join.

```text
 b1: %c = phi ..                    b1: %c = phi ..
     br %more, b2, b3                   br %more, b2, b3
 b3: %a = add %c, 7        =>       b3: %c.lcssa = phi [ %c, %b1 ]
                                        %a = add %c.lcssa, 7
```

First, a phi outside any loop exit whose inputs are all one value is
replaced by that value: no loop closes there any more. The new phi is named
after its value with `.lcssa`. The pass is idempotent.

The helpers `arms`, `from_arms`, `operations`, `definitions`, `exit_phi` and
`place_phi` read and build phis for many other loop passes.

## Runs
In every body's pipeline after `loopsimplify`, unless `lcssa` is turned off,
and again after header copying. `unswitch` and `peel` call `closed` on
their candidates.

## Keeps
Dominators and loops stay valid; all other analyses are invalidated when
something changed.

## See also
LLVM's LCSSA (LCSSA.cpp, `formLCSSA`).
