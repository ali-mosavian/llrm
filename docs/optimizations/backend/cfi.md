> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/cfi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/cfi.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/cfi.rs sha256:4b9f959cc32095c99915a4a504ba0a159d068b761352e99f823a71423cade1e2 -->

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
