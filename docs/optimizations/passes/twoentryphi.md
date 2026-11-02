> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/twoentryphi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/twoentryphi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/twoentryphi.rs sha256:93747a518df14e0a9651ab73d7b4c4d5116065aff12ad6b397d7b320a27d1a84 -->

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

- D5. Where a `select` is a branch (`Tti::has_cmov` false: the i486) the
  fold's selects count against the threshold, which the pin's do not: the
  arms' code then runs on both paths and the branch stays. gcc if-converts
  only where a conditional move exists. x_isqrt -18%, x_mergesort -20%
  clocks, x_crc32 1179 -> 175 B at 1.58x clocks (numbered trade row; exit
  2026-11-20: price the loop-carried arm in the cost model so crc keeps the
  select where the clocks pay). Owner llrm-target-x86-m32. A boolean
  `and`/`or` select (`select x, y, false`) is no branch and is not counted:
  the 6502's `isdigit` range test stayed two compares (28 -> 50 B).

## Example
The phi that a branch chooses between becomes a select in the branch's
block:

```text
entry: br i1 %c, label %then, label %end
then:  br label %end
end:   %p = phi i32 [ 1, %then ], [ 2, %entry ]
; becomes, in entry: %p = select i1 %c, i32 1, i32 2
```

## Used by
Named by `alwaysundef`, `branchtocommon`, `condstores`, `condtocond`, `hoistcommon`, `icmpchain`, `knownpred`, `loopdeletion`, `simplifycfg`, `sinkcommon`, `speculate`, `switcharms`, `switchdead`, `switchlookup`, `switchopt`, `switchphi`, `switchpow2`, `switchrange`, `switchreduce`, `switchselect`, `switchvalue`, `undefpred`.
