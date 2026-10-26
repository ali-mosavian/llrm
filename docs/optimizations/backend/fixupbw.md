> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/fixupbw.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/fixupbw.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/fixupbw.rs sha256:c67303ef508b76510f6f5c87a32a1e9b2f1d3543a04723728f06ef08e111d892 -->

# fixupbw

A byte or word load, copy or extension is the dword form when the rest of
its register is dead: `X86FixupBWInsts`. Translated from
[X86FixupBWInsts.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L165)
(`runOnMachineFunction`), function by function:
[`getSuperRegDestIfDead`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L190)
is `super_reg_dest_if_dead`,
[`tryReplaceLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L285),
[`tryReplaceCopy`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L316)
and
[`tryReplaceExtend`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L353)
keep their names,
[`tryReplaceInstr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L388)
chooses by opcode, and
[`processBasicBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L434)
decides every instruction of a block against the liveness of the block as it
was, then replaces them.

## How it works
After allocation, `mov al,[m]` becomes `movzx eax,[m]`, `mov ax,[m]`
`movzx eax,[m]`, `mov al,cl` `mov eax,ecx`, and `movsx`/`movzx` to a word
the dword form, when no other byte of the destination's dword register is
live afterwards. A zero-extending load needs no merge with the old upper
bytes and a dword copy no partial-register write. `ah` and the other high
views are never the destination; a byte load is left alone when the code is
for size (`OptForSize`, L455).

The second half of `getSuperRegDestIfDead` (an implicit-def of the super
register on a `MOV8rm`, `MOV16rm`, `MOV8rr` or `MOV16rr` whose upper bits
were undefined, where the pin's liveness has no sub-register lanes) is N/A:
`dead_after` is per lane, so a dead upper half is already dead.

DEVIATE W1 (no number yet): in real mode the dword form takes the 0x66
prefix, so the pin's claim that the size is the same is false there; the
rewrite is made only if it is no longer. Exit: none (a fact of the
encoding). `movsx ax,al` stays `cbw`, as the pin leaves it.

## Example
`unsigned char c = *p;` at `-m32 -O2`, where the rest of `eax` is dead:

```text
mov al, byte ptr [ebx]        ->    movzx eax, byte ptr [ebx]
mov cl, dl                    ->    mov ecx, edx
```

From `test_a_byte_load_into_a_dead_register_is_the_zero_extending_load` and
`test_a_byte_or_word_copy_is_the_dword_copy_when_the_rest_is_dead` in
`tests/fixup_bw.rs`.

## Runs
After `Peephole`, optimizing levels only.
