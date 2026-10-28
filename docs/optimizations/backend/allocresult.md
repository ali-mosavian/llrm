> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/allocresult.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/allocresult.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/allocresult.rs sha256:f14d1fa11a9668be63846ee9379232fbadd89b677fa9398e8165169ea84e8b0e -->

# allocresult

What an allocation returns: the error kinds and the assignment.

## How it works
`Unplaced` (a value no register can hold) and `Spilled` (a spill nothing
writes) are the two ways an allocation fails; `Error` adds a refused body
and carries them out of `RegAlloc`. `Assignment` says where each value
lives, what it cost and whether that is the best.

## Used by
`backend::allocate`, the passes that run after it and the tests that
allocate a body.

## Example
A body that names `eax` for three values live at once gives
`Error::Unplaced("no register for v3")`; one that fits gives an `Assignment`
with `spilled` empty.
