> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/deadargs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/deadargs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/deadargs.rs sha256:386ed7e1a082703fb364e8cf677e09728ace5cb114fe656071836723b4257e65 -->

# deadargs

LLVM's DeadArgumentElimination for a function only direct calls reach: a
parameter nothing reads (but its own recursive calls, which pass it on to
itself) goes, and with it the argument of every call. Promotion and the
constants propagated into a body leave them: a length every caller passes as
12, folded into the body, is still pushed at every call.
