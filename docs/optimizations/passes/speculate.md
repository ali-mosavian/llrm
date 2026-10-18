> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/speculate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/speculate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/speculate.rs sha256:ddfbe17867c913a21cbc18a4b29273854488628f7d2750dceceddcf1fcda4b19 -->

# speculate

A block that only a branch reaches and that jumps back to the branch's
other target runs before the branch, its phis in the target becoming
selects: the `speculativelyExecuteBB` step of SimplifyCFG.

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`speculativelyExecuteBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3244) | `executed` | MIRROR; D1, D2, D3 |
| [`validateAndCostRequiredSelects`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3130) | `selects_priced` | MIRROR; D4 |
| [`passingValueIsAlwaysUndefined`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9094) | `passing_value_always_undefined` | partly; D5 |
| `phi-node-folding-threshold` (2) | `THRESHOLD` | MIRROR |

The call sites are in
[`simplifyCondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9060),
behind `SpeculateBlocks`. One instruction at most runs early, and a
value of the branch's block that only that instruction reads counts as one
more.

- D1. Stores, loads that fault conditionally and `HoistCondStores` are not
  ported: a block with a store or a load that may trap is left alone.
- D2. No branch weights or `!unpredictable` (`isProfitableToSpeculate` is
  true), no ephemeral values (no assumption cache), no pseudo probes, no
  `minsize` (costs are `SizeAndLatency`).
- D3. A hoisted instruction keeps the metadata kinds LLVM keeps and loses
  the rest, as in [`twoentryphi`](twoentryphi.md) (D4 there).
- D4. No constant expressions in function bodies: LLVM's price of one is not
  needed.
- D5. Only the uses LLVM lists that need no attribute are tested: a load or
  store through the value, a call of it, a division by it. The look through
  a `getelementptr`, the `noundef`/`nonnull` return and argument rules and
  `llvm.assume` are not ported; the MIR has no `NullPointerIsDefined`, so a
  null pointer is undefined. What stands between the phi and the use must be
  no call (`isGuaranteedToTransferExecutionToSuccessor`).
