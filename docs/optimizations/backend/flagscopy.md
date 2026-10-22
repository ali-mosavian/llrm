> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/flagscopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/flagscopy.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/flagscopy.rs sha256:7c2801b787f2f216254e540374345b2276638f663d3c76b4cb4f6ef23632ac38 -->

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
