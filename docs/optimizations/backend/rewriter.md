> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/rewriter.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/rewriter.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/rewriter.rs sha256:ac56265324b8411c09d2d0793427b5a1dc14fc7ec221e4f7f8ee07b05781db94 -->

# rewriter

`VirtRegRewriter`: the allocation put into the function. Translated from
llvm/lib/CodeGen/VirtRegMap.cpp (`VirtRegRewriter::rewrite`,
`handleIdentityCopy`) at d1106deb71cc.

## How it works
`rewrite` replaces every operand's value by the register the allocator
gave it, at the operand's width (`MO.setReg(PhysReg)`), puts selector and
address registers in their cells, and removes the copies that became
identities (`handleIdentityCopy`), keeping the bytes such a copy owned.
`_dead_insertions` first drops the allocator's own copies nothing reads.

DEVIATE W1 (no number): no sub-register operands. A value is one lane
(coalescer C1), so `SuperKills`, `SuperDefs`, `SuperDeads` and
`readsUndefSubreg` have nothing to do; x86's AL/AH are registers of the
class table.

DEVIATE W2 (no number): no kill, dead or `renamable` flags and no block
live-ins (`LIS->addKillFlags`, `addMBBLiveIns`): LIR keeps no such flags;
the passes after allocation recompute liveness from the instructions.

DEVIATE W3 (no number): a parallel copy is not expanded here
(`expandCopyBundle`); `parcopy` sequences it after allocation, where which
moves conflict is a question about locations.

DEVIATE W4 (no number): an x87 value placed in the stack registers loses
its value names; the stackifier numbers it again (`fpregs`).

## Example
The allocator placed `v1` in AX and `v2` in BX; before and after:
```text
v1 := mov 5          AX := mov 5
v2 := mov v1         BX := mov AX
v1 := mov v1         (an identity move: dropped, its bytes kept)
ret v2               ret BX
```

## Used by
`allocate::RegAlloc`.
