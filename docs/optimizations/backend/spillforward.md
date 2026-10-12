> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spillforward.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spillforward.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spillforward.rs sha256:37219e63ce55a4b284638cf3fdd4dff2d46b16dbaf16760784e8ef5aa0a29500 -->

# spillforward

Port of `qbopt/backend/spillforward.py`: remove reloads of a slot a
register already holds on every path here, and stores of such a register
back into that slot.

Within a block a register holds the bytes of every slot it has been read out
of or written into since. Across blocks that fact has to be met at every
incoming edge, which is an availability problem and not something the first
instruction of a block and the last of its predecessor can answer.
