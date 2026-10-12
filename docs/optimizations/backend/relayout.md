> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/relayout.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/relayout.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/relayout.rs sha256:7cc450c9706107690f9174b3283eb0ba9228cb14b1c215a7c688e02f5dc75480 -->

# relayout

The frame laid out again: a function whose frame reaches past `[bp-128]` is
also tried with a hole above its allocas for the spill slots; this makes
that layout from the first one, by the slot each frame cell names, where it
used to run the whole backend a second time
(docs/optimizations/second-frame.md).

A cell is moved by its slot: the incoming arguments stay, a slot instruction
selection laid out moves down by the hole, a spill slot goes where
`Frame::slot` would hand it out in a frame that has the hole. Anything this
does not know how to move (bytes of inline code that name a displacement, a
cell in no known slot, a set of call-private ranges that is not what
`outside` makes) makes it give up, and the caller runs the backend again as
before. `LLRM_CHECK_FRAME=1` runs it again either way and compares.
