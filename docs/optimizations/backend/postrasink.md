> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/postrasink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/postrasink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/postrasink.rs sha256:ec48ff58d46e2d8e4b12ac9aaa25dcf3231a79848af19afe20f320356c5a5912 -->

# postrasink

LLVM's PostRAMachineSinking: a copy whose result only one successor reads
moves into that successor. A spill store does the same: a slot only one
successor reads is stored there (hanoi's `n == 0` exit stored a slot it
never read, and so built the frame).

`mov edi, edx` ahead of `test eax, eax / jne body` is dead on the path that
returns at once, where nothing reads EDI (a saved register's incoming value
is on the stack): the copy belongs to the block that reads it.
Shrink wrapping can then place the prologue after the test, as gcc and LLVM
do. Sound while the successor is entered only from here, nothing between the
copy and the branch reads its result or writes its source, and no other
successor has the result live.
