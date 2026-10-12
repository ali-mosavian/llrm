> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/allocate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/allocate.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/allocate.rs sha256:466d744d49add97aa6bed2c1762459c1105ee960716c3647938db3831540da07 -->

# allocate

Port of `qbopt/backend/allocate.py`: registers for a lowered body, using
greedy allocation with eviction.

LLVM's `RegAllocGreedy`: spilling is priced by `intervals::weights`, and
fixed intervals go first.
