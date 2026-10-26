> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/floatregions.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/floatregions.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/floatregions.rs sha256:39b308063e271dd04277862b368c294f42ec2cc4a979c2ba965c809a3f2da657 -->

# floatregions

Where the x87 stack cannot reach, and the error type the x87 allocator
raises.

## How it works
`boundary` says whether the x87 stack cannot be assumed to survive an
instruction: a call, a barrier, an instruction with no semantics, or one
that names an x87 register by position (`target::positional_place`). The
allocator's passes treat each such instruction as a point where no value may
be live on the stack: [`crate::backend::floatassign`](floatassign.md) ends a register
stretch there, [`crate::backend::floatalloc`](floatalloc.md) flushes the stack there.

`Raised` is the error of those two passes: an operand that cannot be lowered
(`Unlowered`), a frame cell refused (`frame::Refused`), or a bad value.

## Example
No honest example: `boundary` is a predicate and `Raised` an error type, so
neither rewrites LIR. The effect of a boundary is visible in the
`floatassign` example, where the call ends the register stretch of `a`.

## Used by
`floatassign`, `floatalloc`.
