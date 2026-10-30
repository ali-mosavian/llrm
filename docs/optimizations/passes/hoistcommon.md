> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/hoistcommon.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/hoistcommon.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/hoistcommon.rs sha256:dd6b02fb7089bdc9027cfbaabee20752ea6dc5daddbda7cf0631c6490f3371ba -->

# hoistcommon

The code that opens the arms of a branch in the same way runs before the
branch once: `hoistCommonCodeFromSuccessors` of SimplifyCFG.

## How it works
`hoisted` takes the terminator `branch` of a block whose successors each
have `branch`'s block as their only predecessor. It walks the successors in
step from their first instruction:

1. Instructions that are the same in every successor (the same operation,
   type and operands, a compare with its operands swapped, a commutative
   operation with them swapped) and that may move above the instructions
   skipped so far run once in the branch's block, and the copies go.
2. Any other instruction is skipped, at most `SKIP_LIMIT` of them; what it
   reads, writes or does restricts what may move over it.
3. When the terminators are reached and nothing was skipped, they are the
   same too: the branch is replaced by one terminator, and the phis of the
   successors it reaches take a `select` on the branch's condition for each
   pair of inputs that differ.

With `all_equal_only` (the default slots) the walk starts only if every
successor is the same list of instructions, so nothing is skipped and the
successors empty out.

## Example
The instructions both arms open with run once, before the branch:

```text
  b0: br %c, b1, b2            b0: %t = add %x, 1
  b1: %t1 = add %x, 1                br %j
      br %j              ==>   b1, b2: gone (no predecessor)
  b2: %t2 = add %x, 1
      br %j
```

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`hoistCommonCodeFromSuccessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1873) | `hoisted` | MIRROR; D1, D2, D3 |
| [`hoistSuccIdenticalTerminatorToSwitchOrIf`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2103) | `terminator_hoisted` | MIRROR; D4 |
| [`skippedInstrFlags`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1538) | `skipped_flags` | MIRROR |
| [`isSafeToHoistInstr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1556) | `safe_to_hoist` | MIRROR |
| [`shouldHoistCommonInstructions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1594) | `should_hoist` | MIRROR; D2 |
| [`areIdenticalUpToCommutativity`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1680) | `identical_up_to_commutativity` | MIRROR; D3 |
| `simplifycfg-hoist-common-skip-limit` (20) | `SKIP_LIMIT` | MIRROR |

- D1. No block has its address taken in the MIR, no pseudo probes, no
  debug-record lists to hoist in step (the debug records of a hoisted
  instruction move with it); `!mmra` tags are compared as written.
- D2. `TargetTransformInfo::isProfitableToHoist` is true (the base
  implementation, which the x86 target does not change); `nomerge` and
  `convergent` are not attributes of the MIR.
- D3. Instructions are compared by operation, type and operands (a call by
  its type, convention and tail kind, its attributes only intersecting); the
  flags are `and`ed.
- D4. No `invoke` and no `callbr` in the MIR; no
  `llvm.experimental.deoptimize`.

## Used by
Named by `simplifycfg`, `sinkcommon`.
