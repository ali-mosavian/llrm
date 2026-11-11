> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/profswap.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/profswap.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/profswap.rs sha256:8bfe9a5ff6ccda78eb9391b7e9dc1244453356688e9fd8efc4bc042ecd383fb7 -->

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
