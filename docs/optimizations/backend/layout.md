> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/layout.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/layout.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/layout.rs sha256:5de53af45dbab45af05fcba430b4d0ddeff20704a5db43008409dcd4ccd64c7b -->

# layout

A whole body, emitted -- and everything that has to move when it does.

Port of `qbopt/backend/layout.py`. Blocks are placed, implicit edges made
explicit and branches threaded; `asm` then measures, shrinks every branch
to a fixed point and emits. What comes back is bytes plus the relocations,
because a relocated displacement is emitted as zero and the fixup that
names it has to be moved to wherever the field ended up.

Refuses the whole body where it cannot emit one op.

Python's `id(op)` is the `Arc` pointer of an LIR occurrence.
