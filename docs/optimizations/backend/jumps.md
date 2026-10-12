> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/jumps.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/jumps.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/jumps.rs sha256:8387b25d171b6016054c98c71341f4b794ead31478de4546bf0b440016a80861 -->

# jumps

Port of `qbopt/backend/jumps.py`: block order, and the jumps it makes
redundant.

Runs on the allocated body, after the last phase that adds or empties
blocks: edge splits and their undoing leave jumps to the next block, and the
raise's `if false goto` beside a `goto` leaves a `jcc` over a block that
only jumps. `placed` orders the blocks; `threaded` keeps that order and
drops the jumps and the blocks nothing reaches.
