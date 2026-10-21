> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/emptyblocks.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/emptyblocks.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/emptyblocks.rs sha256:c8b6c966ed564fbcb903d346890e9f1822a7d67cba18052a63fe7b64481939e2 -->

# emptyblocks

A block of nothing but a jump merged into its successor, so a phi's copy
lands in the predecessor that fed it. Translated from CodeGenPrepare's
`eliminateMostlyEmptyBlocks`
([CodeGenPrepare.cpp#L939](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L939)).

## How it works
Passes before selection leave blocks that only jump (`br b6`) between a
branch and a block with phis. Left alone, phi elimination puts the copy of
a phi's input in that block, and the block stays as a block of its own with
the code that follows it duplicated (`xor ebx, ebx; cmp ebx, eax; ja ...`).
Each such block `BB` with the jump to `DestBB` is removed when
`can_merge` (`canMergeBlocks`: no predecessor of `BB` that is already a
predecessor of `DestBB` feeds the phis a different value) and
`profitable` (`isMergingEmptyBlockProfitable`: not a loop preheader that
would become a critical edge) say so. A `DestBB` whose only predecessor is
`BB` takes `BB`'s place; otherwise the phis of `DestBB` take one input for
each predecessor of `BB` and the predecessors jump to `DestBB`:

```text
 p:  br c, a, e              p:  br c, a, d
 e:  br d                    d:  %x = phi [0, p], [%y, a]
 d:  %x = phi [0, e], [%y, a]
```

DEVIATE B1 (no number): a block with phis of its own is kept (the pin merges
it), `DeleteDeadPHIs` is not run, and a predecessor ending in a `switch`
keeps the block whenever `DestBB` has phis (the pin weighs block
frequencies there). Exit: those three pieces translated.

## Example
`b4: br %5, label %21, label %25`, `25: br label %b6`, `b6: %6 = phi [0, %25],
...`: `25` is gone, `b4` branches to `b6`, and the phi reads `0` from `b4`.

## Runs
Once per program in `driver::optimized`, after `legalize` and before
`selects`, at every -O level.
