> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/floatregions.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/floatregions.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/floatregions.rs sha256:6d8160ead4279f8e0ff751e1b0330f1a585467a74ea3a21d72cf752f1a6b7adb -->

# floatregions

Where the x87 stack cannot reach, and the error type the x87 allocator
raises.

## How it works
`boundary` says whether the x87 stack cannot be assumed to survive an
instruction: a call, a barrier, an instruction with no semantics, or one
that names an x87 register by position (`target::positional_place`). The
allocator's passes treat each such instruction as a point where no value may
be live on the stack: [`crate::backend::floatassign`](floatassign.md) ends a register
stretch there, [`crate::backend::floatalloc`](floatalloc.md) flushes the stack there, and
`pressuresink` does not move a load across it.

`Raised` is the error of those two passes: an operand that cannot be lowered
(`Unlowered`), a frame cell refused (`frame::Refused`), or a bad value.

## Used by
`floatassign`, `floatalloc`, `pressuresink`.
