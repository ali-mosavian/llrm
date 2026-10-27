> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/mustexecute.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/mustexecute.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/mustexecute.rs sha256:7045df909c3bc7746e961424ba100df0c3fa395d3eec6651beb0473052355b2b -->

# mustexecute

`SimpleLoopSafetyInfo`.

## How it works
A translation of LLVM `d1106deb71cc` `MustExecute.cpp`'s
`SimpleLoopSafetyInfo`: whether any block of a loop may throw, found by
asking each block whether it is guaranteed to transfer execution to its
successor, stopping at the first that is not.

## Example
A loop that calls a function not marked `nounwind` may throw.
