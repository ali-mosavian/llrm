> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/layout.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/layout.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/layout.rs sha256:9af902a3892c149ccd353b26396d94c8899aa43306317c515171b50a39865bfe -->

# layout

A whole body, emitted -- and everything that has to move when it does.

Port of `qbopt/backend/layout.py`. Blocks are placed, implicit edges made
explicit and branches threaded; `asm` then measures, shrinks every branch
to a fixed point and emits. What comes back is bytes plus the relocations,
because a relocated displacement is emitted as zero and the fixup that
names it has to be moved to wherever the field ended up.

Refuses the whole body where it cannot emit one op.

Python's `id(op)` is the `Arc` pointer of an LIR occurrence.
