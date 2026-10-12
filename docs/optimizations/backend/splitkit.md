> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/splitkit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/splitkit.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/splitkit.rs sha256:e12ff9b10a5df330637005807838df4ffee6997864d9c575c2b6e7e4da053e0a -->

# splitkit

Port of `qbopt/backend/splitkit.py`: splitting a live range instead of
spilling the whole of it.

LLVM's `SplitKit`: `placed`, `local` and `per_block` choose where a piece
holds the value (`tryRegionSplit`, `tryLocalSplit`, `tryBlockSplit`), and
`carved` makes it (`SplitEditor`).
