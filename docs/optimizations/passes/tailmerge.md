> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/tailmerge.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/tailmerge.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/tailmerge.rs sha256:40e4965e069010e2ca988295656cfec5595f419a07598e6dc928a90cb6cccc88 -->

# tailmerge

The blocks that end the function with the same terminator become one block
each: SimplifyCFG's `tailMergeBlocksWithSimilarFunctionTerminators`.

## How it works
`merged` takes every block that ends in a `ret` (or `resume`) and has no
successor, and, when there are two or more of one kind, makes a block
`common.ret` before the first of them. It holds one phi for each operand of
the terminator, and the terminator on the phis. Each of the blocks then
jumps to it, passing its own operand. The pass that follows empties and
removes the blocks that held nothing else.

## Example
Two returns become one block that returns a phi:

```text
  a: ...; ret i16 %x           a: ...; br %common.ret
  b: ...; ret i16 %y     ==>   b: ...; br %common.ret
                               common.ret: %p = phi [%x, a], [%y, b]
                                           ret i16 %p
```

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`tailMergeBlocksWithSimilarFunctionTerminators`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L142) | `merged` | MIRROR; D1 |
| [`performBlockTailMerging`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L86) | `block_merged` | MIRROR; D1 |

- D1. No `llvm.experimental.deoptimize` and no token types in the MIR; debug
  locations are not kept.

## Used by
Named by `jumps`, `simplifycfg`.
