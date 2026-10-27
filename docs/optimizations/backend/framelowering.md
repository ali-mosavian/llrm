> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/framelowering.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/framelowering.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/framelowering.rs sha256:742c9645d2cacd346ce1033f71754756c782514674eda39c41e672e3ce8b9cad -->

# framelowering

The prologue and epilogue a frame needs on i486: LLVM's
`X86FrameLowering::emitPrologue`
([L1677](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L1677)),
`emitEpilogue`
([L2580](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L2580))
and `emitSPUpdate`
([L247](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L247)),
the callee-saved spills of `spillCalleeSavedRegisters` included.

## How it works
With a frame pointer the frame record is `StackSize - SlotSize` bytes, of
which the callee-saved pushes are `callee_saved_size` and the rest,
`NumBytes`, is the `sub` (raised to the largest alignment when the stack
is realigned). Without one `NumBytes` is `StackSize - callee_saved_size`.

The prologue is `push ebp; mov ebp, esp`, the callee-saved pushes in
order, `and esp, -align` if realigned, then the stack adjustment. The
epilogue is the adjustment undone (`mov esp, ebp` or `lea esp, [ebp -
callee_saved_size]` where the stack is realigned or has a variable-sized
object, else `add esp, NumBytes`), the callee-saved pops in reverse, and
`pop ebp`. A stack adjustment of one slot is a `push eax` (a `pop` of a
dead caller-saved register in an epilogue) where one is free.

```text
  stack 24, saved esi edi (8), frame pointer:   NumBytes = 24 - 4 - 8 = 12
  push ebp; mov ebp,esp; push esi; push edi; sub esp,12
  add esp,12; pop edi; pop esi; pop ebp
```

DEVIATE FL-1 (no number, owner w-6): Windows and SEH, funclets, the tail
call return-address reserve, a Swift async context, the red zone, the
stack probe and DWARF CFI are not translated: none applies to i486.

DEVIATE FL-2 (no number, owner w-6): `mergeSPUpdates` and `mergeSPAdd`,
which fold an adjacent stack update into this one, are not translated: a
function of the frame alone sees no neighbour.

DEVIATE CG7 (slot table row 80): `masm::parts` calls this for the flat
m32 frames built at entry (`saved_first`); every other frame (16-bit, the
runtime's, interrupt, inline-code, shrink-wrapped) keeps `parts`' own
sequence, and cells are not yet frame indices (FI-1,
docs/working/pei-plan.md).

## Example
The unit test `a_frame_with_saved_registers_pushes_and_pops_them`.

## Used by
`masm::lowered`, for `saved_first` frames.
