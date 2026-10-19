> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/speculate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/speculate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/speculate.rs sha256:19d89ca3818ef069d6ce894effd23af45b3c4f70eeae8bd86cecf57b14948a20 -->

# speculate

A block that only a branch reaches and that jumps back to the branch's
other target runs before the branch, its phis in the target becoming
selects: the `speculativelyExecuteBB` step of SimplifyCFG.

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`speculativelyExecuteBB`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3244) | `executed` | MIRROR; D1, D2, D3 |
| [`validateAndCostRequiredSelects`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3130) | `selects_priced` | MIRROR; D4 |
| [`passingValueIsAlwaysUndefined`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9094) | [`alwaysundef::always_undefined`](alwaysundef.md) | MIRROR |
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

## Example
The arm's one cheap instruction runs before the branch and the phi becomes a
select:

```text
entry: br i1 %c, label %then, label %end
then:  %t = add i32 %x, 1 ; br label %end
end:   %p = phi i32 [ %t, %then ], [ %x, %entry ]
; becomes, in entry: %t = add i32 %x, 1 ; %p = select i1 %c, i32 %t, i32 %x
```
