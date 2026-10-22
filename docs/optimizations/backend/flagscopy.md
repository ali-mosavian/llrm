> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/flagscopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/flagscopy.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/flagscopy.rs sha256:35087d36b51cf6512b16e73997b1ae5dd4ecac6d557c5cca02b15be85601962b -->

# flagscopy

Flags kept across an instruction that clobbers them: `X86FlagsCopyLowering`.
Translated from
[X86FlagsCopyLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp)
(`runOnMachineFunction`).

## How it works
A copy of the flags is two pseudo instructions: `v := flags_copy` where the
flags are live, and `flags_restore v` where they are wanted back. Each
condition read after the restore (up to the next instruction that writes
the flags) gets a byte, `setcc`, made where the copy was; the reader is
preceded by `test byte, byte` and reads `ne` (`e` for the inverse
condition, which shares the byte). `adc`/`sbb` read the carry: it is saved
as `setb` and made again by `add byte, 0xff`. A reader right after another
of the same byte reuses the `test`. The pseudo instructions go.

Pin functions, in order: [`runOnMachineFunction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L263) is `lowered`;
[`collectCondsInRegs`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L721) and [`promoteCondToReg`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L744) are the
byte made per condition (`bytes`, `setcc` at the copy);
[`getCondOrInverseInReg`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L761) is the shared byte for a condition and
its inverse (`PAIRS`); [`insertTest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L775) is the `test`;
[`rewriteSetCC`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L785) and [`rewriteMI`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L899) (jcc, cmov) are the
`Read::Cond` arm; [`rewriteArithmetic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L824) (adc, sbb) is the
`Read::Carry` arm.

DEVIATE R2 (no number yet): the pin finds `COPY $eflags` that isel and
earlier passes leave; LIR has none, so the copy is the pseudo pair and
`lowered` takes the place of the search. The pin splits blocks
([`splitBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L149)) to follow a copy across them; see R1. Owner w-5,
review 2026-11-19; exit: an isel producer of a flags copy.

DEVIATE R1 (no number yet): a copy and its restore must be in one block (the
pin follows the copy across blocks by dominance); else the pass fails.
Exit: the first isel producer of a cross-block copy. Nothing in isel makes
a copy yet, so the pass does nothing on the corpus.

## Example
```lir
v1 := flags_copy
v2 := add v3, 0x1
flags_restore v1
jl
```
becomes
```lir
v4 := setl
v2 := add v3, 0x1
test v4, v4
jne
```

## Runs
After `FixupSetCC`, before the float phases. Every level (the pin's pass is
not gated), so it is not in `UNOPTIMIZED`.
