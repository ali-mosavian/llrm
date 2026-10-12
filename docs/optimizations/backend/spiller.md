> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spiller.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spiller.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spiller.rs sha256:36802d20ed3d00dd36e55caac7e330de8339e5be1adc05e757a9d7c3be7da7b9 -->

# spiller

Port of `qbopt/backend/spiller.py`: a value the allocator would not keep,
kept in memory instead.

LLVM's `InlineSpiller`: every definition of a spilled value becomes a
store into its frame slot, and every use a load into a fresh value that
lives only across that one instruction.
