> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/deadmi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/deadmi.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/deadmi.rs sha256:b8a561015784842f5be0b2eced4bf78170bfe69b09dd511f307dfb545e9c6ec0 -->

# deadmi

An instruction nothing reads is gone: `DeadMachineInstructionElim` after
selection. Translated from
[DeadMachineInstructionElim.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/DeadMachineInstructionElim.cpp)
(`isDead`, `runOnMachineFunction`).

## How it works
Backwards over each block, an instruction goes when it only computes: every
value it defines has no reader, it touches no register but its own results,
its memory access is not volatile, and the flags it sets are read by no
one. Its operands lose a reader, so a chain goes in one walk; the blocks are
walked again while anything went (the pin does one walk and has the
liveness of a loop's back edge).

Selection leaves these where it builds a value its user then takes in
halves: a far pointer joined to a dword (`offset | selector << 16`) for the
string registers, whose offset and selector the emitter reads from the
halves. Left in, the join reads the selector at four bytes, and the
allocator holds the word in a dword register: `mov esi, ecx`, a dword spill.

## Example
`memcpy` of a far struct at `-m16 -Os`: `v34 := v33 | v26 << 16`, read by no
one, is gone, and `v26` is a word again.

## Used by
Named by `flow`.
