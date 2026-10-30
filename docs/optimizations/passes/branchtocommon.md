> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/branchtocommon.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/branchtocommon.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/branchtocommon.rs sha256:a21ae900929f68fdf3ee49404552cfe6e4450675fff77dd32e2d11c39c3a74fd -->

# branchtocommon

A block that is only a comparison (and a few instructions that can run
early) and a branch is folded into a predecessor that branches to it and to
one of its successors: SimplifyCFG's `foldBranchToCommonDest`.

## How it works
`folded` takes the block `BB` ending in a conditional branch on a
comparison (or binary operation, select, truncation) of its own that only
the branch reads. A predecessor `P` ending in a conditional branch shares a
successor with it: `BB`'s instructions are copied into `P` before its
branch, and `P` branches on `P`'s condition and `BB`'s joined by `or`
or `and`, to the other target of `BB`. The join is a plain operation where
`BB`'s condition being poison implies `P`'s is, a `select` otherwise. At
most one instruction besides the condition is copied, and the join with
the inversion `P` may need costs two at most.

## Example
The block's comparison runs in the predecessor:

```text
  p:  br %a, bb, x                 p:  %b = icmp ...
  bb: %b = icmp ...          ==>       %c = select %a, %b, false
      br %b, y, x                      br %c, y, x
```

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`foldBranchToCommonDest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4231) | `folded` | MIRROR; D1 |
| [`shouldFoldCondBranchesToCommonDestination`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4099) | `recipe` | MIRROR; D2 |
| [`performBranchToCommonDestFolding`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4132) | `performed` | MIRROR; D2, D3 |
| [`createLogicalOp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4026) | `logical_op` | MIRROR |
| [`safeToMergeTerminators`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L388) | `safe_to_merge_terminators` | MIRROR |
| [`InvertBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1879) | `inverted` | MIRROR |
| `simplifycfg-branch-fold-threshold` (2), `-bonus-inst-threshold` (1), `-branch-fold-common-dest-vector-multiplier` (2) | `FOLD_THRESHOLD`, `BONUS_THRESHOLD`, `VECTOR_MULTIPLIER` | MIRROR |

- D1. No pseudo probes, no `minsize` (the cost kind is size and latency).
- D2. The MIR has no branch weights or `!unpredictable`: the fold is never
  held back for a predictable branch, and no weights are computed.
- D3. No debug locations. The bonus instructions lose the metadata that can
  make a speculated instruction undefined, as in
  [`twoentryphi`](twoentryphi.md) (D4 there).

## Used by
Named by `condtocond`, `simplifycfg`, `switchvalue`.
