> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/coalesce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/coalesce.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/coalesce.rs sha256:861761ccacc187729c3f18891a22f70e4132995ab4d1d76f24ccca6f389876b0 -->

# coalesce

Port of `qbopt/backend/coalesce.py`: a copy whose two values can share a
register is not a copy.

LLVM's `RegisterCoalescer`, conservative: Briggs and George before the
join, since there is no undo.
