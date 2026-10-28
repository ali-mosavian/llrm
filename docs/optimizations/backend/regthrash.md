> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/regthrash.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/regthrash.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/regthrash.rs sha256:8746558cc7852cc7f49bafca28fbbb7a39b802212be25d69789ef34a3124a441 -->

# regthrash

Which register lanes are dead after each instruction of a block, from the
lanes dead at its exit.

## How it works
One walk from the last instruction to the first. The lanes dead after an
instruction are those dead before the one that follows it: what that
instruction leaves unread and unwritten-before-read. `_unread_after` keeps
the high half of a register a call may clobber (`clobbers_high`) as read.

## Example
`mov ax, 1; mov bx, ax` with `ax` dead at the exit: after the second
instruction `ax` and `bx` are dead; after the first only `bx` is.

## Used by
`peephole`, `exactaddress`, `sharedstores`, `fixupleas`, `fixupbw`
and `parcopy`.
