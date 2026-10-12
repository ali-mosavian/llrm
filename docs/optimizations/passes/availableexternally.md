> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/availableexternally.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/availableexternally.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/availableexternally.rs sha256:b7ca47425a71bb93117c57039e17a071b8a26d24206f1e9f7b3d8e371816a080 -->

# availableexternally

LLVM's EliminateAvailableExternally: a body the program holds only to
inline from is a declaration again once inlining is done, so no copy of
it is emitted and what calls it still reaches the runtime's own routine.
