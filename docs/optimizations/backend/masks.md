> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/masks.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/masks.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/masks.rs sha256:11b98967887215ef0b04fc695889c880b497499de4f61fd35a7351f13b3960f0 -->

# masks

Points that destroy a register without naming it, and the question "may
this value sit in that register over its interval" they answer.

## How it works
A call (its clobbers), a 16-bit callee (the upper half only) and a point
that needs the data segment register destroy registers no instruction
operand names. `Masks` holds those points of a body; `_clobbered` asks
whether one of them lies inside an interval, comparing registers by root so
`AX` and `EAX` are one register. A mask on the upper half matters only to a
value wider than a word.

## Example
A value live across `call f`, which clobbers `eax`, `ecx` and `edx`: the call
is a mask point inside its interval, so `_clobbered` is true for those three
and the value takes `ebx`, `esi` or `edi`.

## Used by
`backend::allocate`, the greedy allocator's inputs, and the passes that
place values after it.
