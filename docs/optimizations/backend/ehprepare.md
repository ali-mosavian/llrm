> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/ehprepare.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/ehprepare.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/ehprepare.rs sha256:32033c6b009ced039089b4635f7fb69f9ffe0238b3bfca01585537e7e907edfa -->

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
