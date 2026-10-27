> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/looprotate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/looprotate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/looprotate.rs sha256:c3b723d22e7e43ea7576bd1d95fd21dc73cf72de74971009596e72855a9832c1 -->

# looprotate

`LoopRotate`: a loop with its exit test in the header becomes a loop with
its exit test in the latch, behind a copy of the test that guards the entry.

## How it works
`process_loop` first folds the latch into the block that exits before it, if
that is a few speculated instructions (`simplify_loop_latch`), then
`rotate_loop` rotates a loop whose header ends in the exit test.

The header's instructions are copied, in order, into the preheader: one
whose operands are all loop-invariant and that touches no memory is moved
instead of copied; a copy that simplifies (its phi operands now the values
from the preheader) is replaced by what it simplifies to. The preheader then
ends in a copy of the header's branch, so it is a guard: the loop is entered
only if the test passes. The header's phis lose their preheader input, and
every use of a header value the loop no longer dominates is rewritten with
`SsaUpdater` to the preheader's copy or the header's. The header's
successor in the loop is the new header; the old header is merged into the
latch if they are joined by a jump.

```text
  pre -> H(test) -> F -> L -> H            pre(test') -> F' -> L(H, test) -> F'
            \-> X                                  \-> X              \-> X
```

## Deviations
- R1 No `DominatorTree`, `LoopInfo`, `ScalarEvolution`, `MemorySSA` or
  `AssumptionCache` to update (D8 of the plan page): they are derived from
  the function. `process_loop` reads one `LoopInfo` made before the
  rotation; the blocks the rotation adds belong to the loops outside the
  rotated one, which the driver derives again before it reaches them.
- R2 `check-exit-count` is not translated:
  `profitableToRotateLoopExitingLatch` asks ScalarEvolution for exit counts
  only under that option, which no `PassBuilder` pipeline sets.
- R3 New blocks go at the end of the layout: `Function::insert_block` after
  a block scans the layout. LLVM puts them next to their neighbours.
- R4 No `llvm.experimental.noalias.scope.decl` scope cloning and no
  `llvm.coro` check: MIR has no local noalias scope declarations, and no
  coroutines. Debug records stand before an instruction and move with it, so
  only the preheader's old terminator needs `flushTerminatorDbgRecords`.
- R5 `simplifyInstruction` takes no dominator tree or assumptions: the copy
  sits in a preheader whose analyses the rotation is changing.
- R6 A target with no cost model (`Machine::tti`) is priced by the generic
  profile, `llrm_mir::tti::Basic`: a pass names no target crate.

## Example
Before:
```mir
b0:
  br label %b1
b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3
b2:
  %next = add i16 %i, 1
  br label %b1
b3:
  ret i16 %i
```
After, the test copied as the guard and the old header merged into the
latch:
```mir
b0:
  %g = icmp slt i16 0, %n
  br i1 %g, label %ph, label %b3
ph:
  br label %b2
b2:
  %i = phi i16 [ 0, %ph ], [ %next, %b2 ]
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, %n
  br i1 %go, label %b2, label %exit
exit:
  %i.lcssa = phi i16 [ %next, %b2 ]
  br label %b3
b3:
  %r = phi i16 [ %i.lcssa, %exit ], [ 0, %b0 ]
  ret i16 %r
```

## Used by
The `loop-rotate` slot of the pipeline.

## See also
[`LoopRotate::rotateLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopRotationUtils.cpp#L348),
[`simplifyLoopLatch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopRotationUtils.cpp#L908),
[`processLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopRotationUtils.cpp#L946).
