> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/condtocond.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/condtocond.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/condtocond.rs sha256:e12151984c120b6188f2ccfe510d527b3289cc0901c0f31ae72e2f212bb90fb2 -->

# condtocond

A conditional branch in an empty block that a conditional branch
predecessor shares a destination with, folded into the predecessor: LLVM's
[`SimplifyCondBranchToCondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4773).

In order: a branch on the predecessor's own condition, in a block only it
reaches, is decided; `mergeConditionalStores` (the `MergeCondStores` step);
then the two conditions are joined (`brmerge`) into the predecessor's
branch, which now goes to the common destination or the other one.

| # | DEVIATE |
|---|---|
| D1 | `tryWidenCondBranchToCondBranch` is not ported: it needs `llvm.experimental.widenable.condition` and a deoptimize call, which the MIR has no form for. |
| D2 | No branch weights, and no `unpredictable`/`TTI` predictable-threshold test of the predecessor's probability: the MIR carries none. |

## Example
```text
  b0: br i1 %a, label %b1, label %d         b0: %m = or i1 %a, %b   (a select if %b may be poison)
  b1: br i1 %b, label %d, label %e    ==>       br i1 %m, label %d, label %e
```

## Used by
Named by `simplifycfg`.
