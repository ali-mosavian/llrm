> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spiller.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spiller.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spiller.rs sha256:8e2b2cb9cbf7e974d87e1fcaf351b14fb307b44612574d9c09cb7caa6c696bde -->

# spiller

Port of `qbopt/backend/spiller.py`: a value the allocator would not keep,
kept in memory instead.

LLVM's `InlineSpiller`: every definition of a spilled value becomes a
store into its frame slot, and every use a load into a fresh value that
lives only across that one instruction.
