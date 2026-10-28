> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/sinkcommon.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/sinkcommon.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/sinkcommon.rs sha256:a92fd3ab16da6d47926a639a5f20402e2580d791e7d7a2c582d451a3e3864142 -->

# sinkcommon

The code that ends every unconditional way into a block in the same way
runs once in the block: SimplifyCFG's `sinkCommonCodeFromPredecessors`.

## How it works
`sunk` takes a block with two or more predecessors that end in a jump. It
scans their instructions from the end in step, as far as each column
(one instruction of each predecessor) is the same operation with operands
that are the same, or that a phi can merge (`can_sink`). Of those the
columns whose sinking would need more than one new phi stay, from the first
that does. If some predecessor does not end in a jump the predecessors that
do are first given a block of their own to sink into (`.sink.split`), and
that only when a column holds something that cannot be run speculatively.
Each column is then one instruction at the start of the block, its
differing operands phis (`sink_last_instruction`).

## Example
The add both arms end with runs once in the join:

```text
  a: %x = add %p, 1 ; br e         a: br e
  b: %y = add %q, 1 ; br e   ==>   b: br e
  e: %z = phi [%x, a], [%y, b]     e: %s = phi [%p, a], [%q, b]
                                      %z = add %s, 1
```

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`sinkCommonCodeFromPredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2425) | `sunk` | MIRROR; D1, D2 |
| [`canSinkInstructions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2236) | `can_sink` | MIRROR; D1 |
| [`sinkLastInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2340) | `sink_last_instruction` | MIRROR; D1 |
| [`canReplaceOperandWithVariable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3907) | `can_replace_operand_with_variable` | MIRROR; D3 |
| [`replacingOperandWithVariableIsCheap`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2219) | `replacing_operand_is_cheap` | MIRROR |
| [`IsBlockFollowedByDeoptOrUnreachable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L653) | `followed_by_unreachable` | MIRROR; D4 |
| [`simplifyDuplicatePredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8573) and `mergeIdenticalBBs` | `duplicate_predecessors_merged` | MIRROR |

- D1. No pseudo probes, inline assembly, `nomerge` or `convergent`
  attributes, `swifterror` pointers or `!mmra`-different pairs beyond the
  metadata ids being compared as written; debug records go with the
  instruction that holds them.
- D2. The split block has no name (`.sink.split`) and `invoke`s are not
  merged (`mergeCompatibleInvokes`: none in the MIR).
- D3. A call's `immarg` parameter is its `Flag("immarg")` argument
  attribute; the intrinsic cases for variadic parameters, `gcroot` and
  `threadlocal_address` are not in the MIR.
- D4. No `llvm.experimental.deoptimize`.

## Used by
Named by `simplifycfg`, `switcharms`.
