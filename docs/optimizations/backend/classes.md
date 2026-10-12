> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/classes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/classes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/classes.rs sha256:67608d2546469f188d28e4711da37c794a32713f2a0e8352ec0854ba9a32a68b -->

# classes

What a target says about its registers that the allocator and the passes
around it read: which operand of which instruction form lives in which
register. Built from the selected target (`RegisterClasses::of`) and handed
down beside `Segments`; no pass reads another target's.
