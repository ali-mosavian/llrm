> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/allocate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/allocate.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/allocate.rs sha256:51ded4009d32c8c66403422a7595766331f46fe31fd60196248f87f1848c279c -->

# allocate

Port of `qbopt/backend/allocate.py`: registers for a lowered body, using
greedy allocation with eviction.

LLVM's `RegAllocGreedy`: spilling is priced by `intervals::weights`, and
fixed intervals go first.
