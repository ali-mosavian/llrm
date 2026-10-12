> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/live.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/live.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/live.rs sha256:f4c73a179f7b2eb5d3fa00c269265a6a8b34c3b82bc08903e3bcbc173a65f113 -->

# live

The live intervals the allocator holds of the body it is rewriting.

An interval is worked out once and kept as it is while the instructions that
name its value stay: slots are stable keys (`Indexes::patched`), so a
rewrite moves none of them. What does change with a rewrite is the size of
an interval an added or removed instruction lies inside, and with it the
weight, which is a function of the size. The weight is therefore not stored
but worked out when it is read, from what it is made of: the references the
value has (`total`), what its copies and folds save (`sibling`, `fold`),
whether it may be spilled (`pinned`), and its size in the numbering.
