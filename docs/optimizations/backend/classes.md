> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/classes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/classes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/classes.rs sha256:f0d6a40252489426572a0d0a74b18881d2ccdc8700965c8f0e6b842044538c5f -->

# classes

What a target says about its registers that the allocator and the passes
around it read: which operand of which instruction form lives in which
register. Built from the selected target (`RegisterClasses::of`) and handed
down beside `Segments`; no pass reads another target's.
