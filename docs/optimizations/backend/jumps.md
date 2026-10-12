> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/jumps.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/jumps.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/jumps.rs sha256:8ecd54c410c19acfe553b4be616981c259428dbc1bfdecfe97f96fb33f410935 -->

# jumps

Port of `qbopt/backend/jumps.py`: block order, and the jumps it makes
redundant.

Runs on the allocated body, after the last phase that adds or empties
blocks: edge splits and their undoing leave jumps to the next block, and the
raise's `if false goto` beside a `goto` leaves a `jcc` over a block that
only jumps. `placed` orders the blocks; `threaded` keeps that order and
drops the jumps and the blocks nothing reaches.
