> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/assemble.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/assemble.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/assemble.rs sha256:75f28dffc83f4fc111526a15021355a72202dbcc194ad2449d5ef01cea070a57 -->

# assemble

A MIR module to masm, as LLVM's `llc` takes a module to an object:
each defined function selected and run through the machine phases, each
defined variable's data, and the symbols both name.
