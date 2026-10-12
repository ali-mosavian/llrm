> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/splitkit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/splitkit.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/splitkit.rs sha256:1f4fbf6b03537458f359963d689959ad15fdd450d3551fc4785d15d973d01e46 -->

# splitkit

Port of `qbopt/backend/splitkit.py`: splitting a live range instead of
spilling the whole of it.

LLVM's `SplitKit`: `placed`, `local` and `per_block` choose where a piece
holds the value (`tryRegionSplit`, `tryLocalSplit`, `tryBlockSplit`), and
`carved` makes it (`SplitEditor`).
