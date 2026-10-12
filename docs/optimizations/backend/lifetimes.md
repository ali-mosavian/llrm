> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/lifetimes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/lifetimes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/lifetimes.rs sha256:94ef54bd2cbf26ce4ad1b8bf06a1c9ed66a216110bfa523ce87b20a076f5bb05 -->

# lifetimes

Where a block local's bytes are live, from its lifetime markers: from a
`llvm.lifetime.start` along every path to where a `.end` or the return
comes. Two locals live never together may share a frame slot (`slots`).

A local is tracked only if it has a marker and nothing but loads, stores and
pointer arithmetic on it and its markers touch it, and each access is where
it is live; any other local is live throughout and shares with none.
