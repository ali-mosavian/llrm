> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/availableexternally.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/availableexternally.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/availableexternally.rs sha256:6877eac894ec9839b92e1fb7a4af848604592e2fdc7aab4df9c1ee83003424ec -->

# availableexternally

LLVM's EliminateAvailableExternally: a body the program holds only to
inline from is a declaration again once inlining is done, so no copy of
it is emitted and what calls it still reaches the runtime's own routine.
