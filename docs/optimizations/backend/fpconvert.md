> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/fpconvert.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/fpconvert.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/fpconvert.rs sha256:7dab66e50c0e5c572622dd618dc850260f443e8f149e9f31c6852e51c389d386 -->

# fpconvert

A float converted to an unsigned 64-bit integer, as signed conversions and a
select, before instruction selection: the x87 stores only signed qwords.
LLVM's legalizer (`expandFP_TO_UINT`) and GCC's `fixuns` expansion do the
same: where `x < 2^63` the conversion is `fptosi x`; else it is `fptosi (x -
2^63)` with the top bit set. The select is the target's general one
(`selects`: a branch on a 486, which has no conditional move), so there is
one answer for select.
