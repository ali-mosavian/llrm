> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/twoentryphi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/twoentryphi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/twoentryphi.rs sha256:9fab671c0cbb818310ae17dc0e818821a94488f601e02e9809e703e83fa481f1 -->

# twoentryphi

A two-entry phi that a branch chooses between becomes a `select`, the
arms' cheap code hoisted into the branch's block: the `foldTwoEntryPHINode`
step of SimplifyCFG.

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`foldTwoEntryPHINode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3834) | `folded` | MIRROR; D1, D2 |
| [`GetIfCondition`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1784) | `if_condition` | MIRROR |
| [`dominatesMergePoint`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L456) | `dominates_merge_point` | MIRROR; D3 |
| [`computeSpeculationCost`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L434) | `instruction_cost` at `SizeAndLatency` | MIRROR |
| [`hoistAllInstructionsInto`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3401) | `hoisted_into` | MIRROR; D4 |
| `two-entry-phi-node-folding-threshold` (4), `max-speculation-depth` (10), `speculate-one-expensive-inst` | `THRESHOLD`, `MAX_DEPTH`, on | MIRROR |

The call site is
[`simplifyOnce`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9353):
a block whose first instruction is a phi of two entries.

- D1. No branch weights or `!unpredictable` in the MIR: the test that leaves
  a predictably taken arm alone, and the extra budget of
  `SpeculateUnpredictables` (off at every slot but two later ones), are not
  ported.
- D2. The new select is not named after the phi (`takeName`): MIR has no
  rename.
- D3. The `with.overflow` + `extractvalue` pair is priced as two
  instructions, not one (`ZeroCostInstructions`).
- D4. A hoisted instruction keeps the metadata kinds LLVM keeps
  (`dropUBImplyingAttrsAndMetadata`) and loses the rest; there are no debug
  records to drop.

## See also
[`phiopt`](phiopt.md), which prices a constant-only phi in the
target's units and runs at its own slot.

## Example
The phi that a branch chooses between becomes a select in the branch's
block:

```text
entry: br i1 %c, label %then, label %end
then:  br label %end
end:   %p = phi i32 [ 1, %then ], [ 2, %entry ]
; becomes, in entry: %p = select i1 %c, i32 1, i32 2
```
