> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/capturecheck.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/capturecheck.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/capturecheck.rs sha256:3e89c298b7d86573b935ab202cf0dac613e6c4e430fb3c5fbe15540cd50a9814 -->

# capturecheck

`LLRM_CHECK_CAPTURES`: the frame objects the pointer facts say never escape,
checked against LLVM's capture tracking read straight off the uses, with no
fact followed: a pointer is captured when any use of it, or of what is made
of it by a gep, a cast, a phi or a select (every arm), stores it, returns
it, turns it into an integer or passes it where the callee keeps no copy is
not promised. A fact that misses one of these is an under-escape: a store to
the object may be dropped while a later load can still read it.
