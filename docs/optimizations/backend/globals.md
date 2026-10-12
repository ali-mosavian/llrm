> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/globals.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/globals.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/globals.rs sha256:888ddfbb2f0c8427d71e6f10765652ffdc5da80c1219edf54a9e52862042b8ed -->

# globals

A MIR module's globals as masm symbols and data, as LLVM's AsmPrinter
emits a global's initializer: its bytes, and a relocation for each
address in it.
