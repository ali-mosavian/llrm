> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/mustexecute.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/mustexecute.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/mustexecute.rs sha256:78e33b126b3caa354df1fd9b2fe8c95f2f7dd2b63e345f5f78fc20660dae21b1 -->

# mustexecute

`SimpleLoopSafetyInfo`.

## How it works
A translation of LLVM `d1106deb71cc` `MustExecute.cpp`'s
`SimpleLoopSafetyInfo`: whether any block of a loop may throw, found by
asking each block whether it is guaranteed to transfer execution to its
successor, stopping at the first that is not.

## Example
A loop that calls a function not marked `nounwind` may throw.
