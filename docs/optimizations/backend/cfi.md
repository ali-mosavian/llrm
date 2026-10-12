> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/cfi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/cfi.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/cfi.rs sha256:0b6b84bec3ac1241c93909330fe9dc49159946932980d59867f82624239ed2e3 -->

# cfi

Call frame information, read back from the code as it was emitted: from
every place in a function, how to find the caller's frame (the canonical
frame address) and the registers the function saved.

The code is decoded and followed from its entry along every path, so the
rule is the code's own, whatever produced it: a frame register, none (a
stack pointer that moves with each argument pushed), registers saved
where first needed and restored before the return that needs it. A rule the
code does not make single-valued (two paths reach a place with different
stack depths) is refused: no information is better than a wrong one.
