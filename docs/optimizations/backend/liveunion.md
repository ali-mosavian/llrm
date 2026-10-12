> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/liveunion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/liveunion.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/liveunion.rs sha256:fcb88337b35b4b67686be855c30eed7cb70675f8350535faf13dbb3f4351a7e2 -->

# liveunion

What each register holds, by where: LLVM's `LiveIntervalUnion` and the
`LiveRegMatrix` over it.

A register's holders do not overlap one another, so a query for what meets
an interval is a walk over the segments that start before it ends, back to
the first that ends before it starts: logarithmic and the overlaps found,
not the holders there are. A segment that does meet one already kept (a
rewrite moves every interval, and leaves the clashes for the allocator to
find) is kept apart and looked at one by one, so the answers are the
pairwise ones in every case.
