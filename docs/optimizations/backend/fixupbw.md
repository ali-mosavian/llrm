> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/fixupbw.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/fixupbw.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/fixupbw.rs sha256:3815707d3bc0e1aab1355b8a0fffd5e51fff6a5b9ea8e8d310a2ca77a4c69e73 -->

# fixupbw

A byte or word load, copy or extension is the dword form when the rest of
its register is dead: `X86FixupBWInsts`. Translated from
[X86FixupBWInsts.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L165)
(`runOnMachineFunction`), function by function:
[`getSuperRegDestIfDead`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L190)
is `super_reg_dest_if_dead`, [`tryReplaceLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L285),
[`tryReplaceCopy`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L316) and
[`tryReplaceExtend`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L353)
keep their names, [`tryReplaceInstr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L388)
chooses by opcode, and [`processBasicBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L434)
decides every instruction of a block against the liveness of the block as
it was, then replaces them.

## How it works
After allocation, `mov al,[m]` becomes `movzx eax,[m]`, `mov ax,[m]`
`movzx eax,[m]`, `mov al,cl` `mov eax,ecx`, and `movsx`/`movzx` to a word
the dword form, when no other byte of the destination's dword register is
live afterwards. A zero-extending load needs no merge with the old upper
bytes and a dword copy no partial-register write. `ah` and the other high
views are never the destination; a byte load is left alone when the code is
for size (`OptForSize`, L455).

DEVIATE W1 (no number yet): in real mode the dword form takes the 0x66
prefix, so the pin's claim that the size is the same is false there; the
rewrite is made only if it is no longer. Exit: none (a fact of the encoding).
DEVIATE W3 (no number yet): the pin refuses `movsx ax,al` only as the
registers AX and AL; here a `movsx` within one register is refused whatever
the registers. Owner w-5.

## Runs
After `Peephole`, optimizing levels only.
