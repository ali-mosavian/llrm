> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/coalesce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/coalesce.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/coalesce.rs sha256:7afcc79388304fdb2924cc47acfa57aeb548e867ba9c1a7390a0ef35bc775f40 -->

# coalesce

Port of `qbopt/backend/coalesce.py`: a copy whose two values can share a
register is not a copy.

LLVM's `RegisterCoalescer`, conservative: Briggs and George before the
join, since there is no undo.
