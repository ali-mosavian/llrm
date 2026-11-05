> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lcssa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lcssa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lcssa.rs sha256:356e0d1aa559606dba9129aabdf4fa3e33a92139b1fc42c78206ec2d2f50bbfe -->

# lcssa

Puts every natural loop in loop-closed SSA form, so loop passes can change a
loop without rewriting the readers of its values elsewhere.

## How it works
A value defined in a loop and read outside it is read through a phi in the
loop's exit block. Later loop passes can then change the loop's body, copy
it or move its exits without rewriting readers outside it.

This is `LCSSA.cpp`. Loops are taken innermost first (`formLCSSARecursively`),
so an inner exit phi is an ordinary definition of the enclosing loop. For one
loop (`formLCSSA`):

- The blocks that dominate an exit and sit directly in the loop are found
  by walking immediate dominators up from the exits.
- Each value they define, with a use but not a lone one in its own block, is
  a candidate.
- A candidate's uses outside the loop (a phi's counting at its incoming
  block) are rewritten. A use in an unreachable block becomes poison.
- Each exit block its definition dominates gets a phi, named after the value
  with `.lcssa`, placed first in the block, with one input per predecessor.
  An input from a block outside the loop is itself rewritten.
- A use in an exit block reads that block's phi, a lone new phi is read
  directly, and any other use goes through `SsaUpdater`, which places a phi
  where the exits' paths join.
- A phi that nothing reads is erased again.

Phis are only added, as `formLCSSA` does: a phi whose inputs are all one
value stays for CodeGenPrepare's phi fold. The pass is idempotent.

The helpers `arms`, `from_arms`, `operations`, `definitions`, `exit_phi` and
`place_phi` read and build phis for many other loop passes.

## Example
Before:

```mir
define i16 @f(i16 %seed, i16 %n) {
b0:
  br label %b1

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
  %more = icmp ult i16 %carried, %n
  br i1 %more, label %b2, label %b3

b2:
  %stepped = add i16 %carried, 1
  br label %b1

b3:
  %answer = add i16 %carried, 7
  ret i16 %answer
}
```

After:

```mir
b3:
  %carried.lcssa = phi i16 [ %carried, %b1 ]
  %answer = add i16 %carried.lcssa, 7
  ret i16 %answer
; ... b0 to b2 unchanged
```

`%carried` is read in `b3`, outside the loop, so the exit block gets a phi
of it and the reader reads the phi.

From unit test `test_a_loop_value_used_after_the_exit_gets_an_exit_phi` in
`lcssa_tests.rs`.

## Runs
In every body's pipeline after `loopsimplify`, unless `lcssa` is turned off,
and again after header copying. The unroller calls it first.

## Keeps
Dominators and loops stay valid; all other analyses are invalidated when
something changed.

## Deviations
- L1 Lifetime markers are ordinary users (2026-11-04, w2, exit 2026-11-12):
  the pin drops them when one crosses the loop boundary, but a pass has no
  callee table to know them by. A marker names an alloca, which sits in the
  entry block, never in a loop.
- L2 No `ScalarEvolution` and no debug records: the pin caches the SCEV of a
  new phi and moves `dbg.value`s; neither is kept here (S1 of `ssaupdater`).
- L3 A phi's inputs come in predecessor id order, not use-list order.

## See also
LLVM's
[`LCSSA.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LCSSA.cpp).
