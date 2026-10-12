> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/ehprepare.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/ehprepare.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/ehprepare.rs sha256:83ddb9d4a01029c627ee1153e7fe68f9c3591136b52d93ce655a52d6e4e4d100 -->

# ehprepare

A QB personality's landing pad made one the runtime can enter, before
instruction selection, as LLVM's SjLjEHPrepare prepares setjmp/longjmp
unwinding.

The runtime keeps one handler address per module (B$OEGA). On an error it
builds a frame of its own over the faulting one and jumps to that address;
B$RESN leaves the frame again, back on the faulting one, and continues at
the first statement-table row at or after the faulting call. So the
address registered is `LANDING`, which keeps ERR in `LANDED` and
calls B$RESN, and the landing pad is its function's
row. The pad is entered with nothing the function left in a register, so
each value live into it goes to the stack, as DemoteRegToStack puts it.
