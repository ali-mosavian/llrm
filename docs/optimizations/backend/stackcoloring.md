> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/stackcoloring.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/stackcoloring.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/stackcoloring.rs sha256:cfe664ecfcaee6420f5a6bf95fd6f8f5a998928b28a0fe68e287c96b34a74b5d -->

# stackcoloring

StackColoring
([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/StackColoring.cpp#L1)),
shared by every target: the static allocas whose lifetimes never overlap
share one frame object.

## Example
Two `[64 x i32]` allocas, each live between its own `llvm.lifetime.start`
and `.end`, were a 512-byte frame; they are one 256-byte object (jpeg's
frame went from 400 to 336 bytes, the pin's). Without markers each alloca
keeps its own.

## Used by
- [`crate::backend::selectiondag::dag_function`](selectiondag.md), which groups the allocas
  before the DAG is built (`share_frame`).
- `llrm-wasm`, which applies the grouping to its machine code after
  selection (`color_frame`).
