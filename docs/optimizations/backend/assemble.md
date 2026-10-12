> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/assemble.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/assemble.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/assemble.rs sha256:efb08ebd91e48530c570820c5174fc37c18d0f7bd7b0254beaea71c3308008ae -->

# assemble

A MIR module to masm, as LLVM's `llc` takes a module to an object:
each defined function selected and run through the machine phases, each
defined variable's data, and the symbols both name.
