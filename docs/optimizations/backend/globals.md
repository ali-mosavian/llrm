> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/globals.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/globals.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/globals.rs sha256:aa17e3d3a257562df0b0aff46c8e0a35f2d2c03def296eb6919ca68dd6f4c2f7 -->

# globals

A MIR module's globals as masm symbols and data, as LLVM's AsmPrinter
emits a global's initializer: its bytes, and a relocation for each
address in it.
