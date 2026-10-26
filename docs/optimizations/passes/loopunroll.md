> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopunroll.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopunroll.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopunroll.rs sha256:07152607784afa097ec6f94792f449543d79f6a15710fc099223d699ae489772 -->

# loopunroll

Complete unrolling of a loop over any exiting block: LLVM's `UnrollLoop`
(`LoopUnroll.cpp`) for the count that is the loop's maximum trip count.

## How it works
The caller says how many times the loop's blocks run (`Options::count`,
the maximum trip count) and, for each exiting block that ends in a
conditional `br`, how many times that block runs when it is the one that
leaves (`Exit::trip_count`, ScalarEvolution's `getSmallConstantTripCount`;
0 where unknown). The loop needs one preheader, one latch that is either
unconditional or an exiting `br`, and closed SSA form (`lcssa::closed`):
a value defined in the loop is read outside it only by a phi of an exit.

Iteration 0 is the original blocks. Each later iteration is a copy of every
block of the loop, made in reverse postorder so that the value an exit
phi takes from a block is already copied: the header's phis are not copied
but read, in iteration `n`, what the latch gave them in iteration `n - 1`;
every other operand is renamed to its copy. Each exit's phis gain an arm
from each copy of the exiting block. Then the original header's phis are
replaced by their preheader values, each latch jumps to the next
iteration's header, and a branch whose way an iteration knows becomes a
jump: the last iteration's latch leaves, an iteration before the exiting
block's `trip_count` stays in. An exiting block that is not the latch is
not folded where it leaves, so every block stays reachable. When the latch
is unconditional the last copy's jump back is `unreachable`. Finally a
block a latch jumps to, if only that latch reaches it, is merged into it.

The loop may exit from any of its blocks and may hold loops of its own. A
loop that is the loop of a rotated function (test in the latch) and one
that is not (test in the header) are both this function's input: neither
shape is a guard.

## Deviations
- D2 No partial, runtime or epilog unrolling: `options.count` must be the
  maximum trip count (`unroll_loop` refuses otherwise). The pinned `opt`
  leaves `UP.Partial` and `UP.Runtime` off at `-mcpu=i486` and `pentiumpro`
  ([the
  plan](https://github.com/ali-mosavian/llrm/blob/main/docs/architecture/loop-passes-port.md)).
- D8 No `LoopInfo`, `DominatorTree`, `ScalarEvolution` or `AssumptionCache`
  to update: llrm derives its loop shape and dominators from the function
  each time they are asked, so none is kept up to date here, and nothing
  holds one across this call.
- D9 `simplifyLoopAfterUnroll` (`simplifyInstruction`, dead code) is the
  `fold` and `dead` that follow in the pipeline's round, until the
  `InstructionSimplify` translation has an entry that takes blocks. LCSSA is
  closed over the function (`lcssa::closed`) after the unroll, where LLVM
  closes the outer loop only when `needToInsertPhisForLCSSA`. `simplifyLoop`
  of the outer loop is the next `LoopSimplify`.

## Runs
Not a pass: a library, called by the full-unroll pass when it is wired in.

## Example
A loop of two trips, `count` 2 and `b2` leaving on trip 2:
```mir
b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  br label %b2
b2:
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 2
  br i1 %go, label %b1, label %b3
```
becomes straight-line code, the second copy reading `%next` for `%i`:
```mir
b1:
  %next = add i16 0, 1
  br label %b1.1
b1.1:
  %next.1 = add i16 %next, 1
  br label %b3
```

## Needs
`lcssa`, `cfg`, `edges`.

## See also
[`UnrollLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUnroll.cpp#L929).
