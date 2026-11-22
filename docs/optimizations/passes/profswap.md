> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/profswap.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/profswap.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/profswap.rs sha256:2954fbf50db7871715c17cf017860c9e12f8602d872228619977b2397af52d6f -->

# profswap

`swapProfMetadata`: a branch or select whose two arms change places
takes its weights with them.

Translation of
[`Instruction::swapProfMetadata`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/Instruction.cpp)
at d1106deb71cc, as `CondBrInst::swapSuccessors` and
`SelectInst::swapValues` call it. Weights that are not `branch_weights` with
one per arm stay.

## Used by
Every site that swaps the two arms of a `br` or `select` in place.

## Example
When the two arms of a conditional branch change places, its weights change places too.

```text
br i1 %c, label %a, label %b, !prof !{"branch_weights", i32 90, i32 10}
=> swapSuccessors
br i1 %c, label %b, label %a, !prof !{"branch_weights", i32 10, i32 90}
```
