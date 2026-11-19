# M68K-ALLOCA: `DynamicStackAlloc` at m68k

Owner: target-m68k. Exit: 2026-11-16.

`int a[n]` and `__builtin_alloca` refuse at 68000/68020, every level: `Legalize Unsupported { op: DynamicStackAlloc, ty: I32 }` (w15's sweep, 2026-11-09; reproduced 18:30Z).

## Pin
`M68kISelLowering.cpp` sets `DYNAMIC_STACKALLOC` Custom ([line 175][pin]) and `LowerDYNAMIC_STACKALLOC` ([line 3362][pin]): `CALLSEQ_START 0`, read `SP`, `SP - size`, `AND -align` when `align` exceeds the stack alignment (8, `M68kSubtarget::stackAlignment`), write `SP`, `CALLSEQ_END 0`. `SEG_ALLOCA` is split-stack only: not translated.

`legalize_dag.rs` `ISD::DynamicStackAlloc` is the same node sequence (`ExpandDYNAMIC_STACKALLOC`), run when the target's `stack_pointer()` hook states `(sp, stack_align)`. m68k states none and its `lower_operation` returns `None`: the refusal.

## Done
- `M68kLowering::stack_pointer()` = `(A7, 4)`: the shared expansion runs (arm64 does the same).
- A body that writes the stack pointer gets a frame pointer and unreserved call frames (`has_var_sized`). The frame-pointer path had never run: `Layout::address` indexed past the saved-`%a6` slot, so a local sat at `(0,%a6)`, over the saved `%a6`.
- A dynamic alloca keeps the alignment it asks for (`Machine::honors_dynamic_alloca_align`, m68k true): the address is masked, so neither the clamp to 4 nor the refusal above 8 applies. The refusal in the builder is for static allocas only.

## DEVIATE M68K-ALLOCA-ALIGN4
The pin states the stack alignment 8 and its frames keep 8; ours keep 4 (`frame::SHIPPED_STACK_ALIGN`, `kept_stack_align`). `stack_pointer()` states 4, so an align-8 dynamic alloca is masked here and not in the pin. Owner m68k. Exit: when the frames keep 8, or 2026-11-30.

## Not covered
A static (constant-size, entry-block) alloca asking more than 4 is still clamped to 4 up to 8 and refused above (`max_alloca_align`): the realignment the pin does for fixed objects is not translated (`over_aligned_local_after_even_pushes`).

## Other targets (2026-11-09, one line each)
- z80-sdcc, 6502: refuse, `CannotSelect(DynamicStackAlloc)`. wasm32: refuses (`Unsupported`, w15). Not part of this row.
- x86_32, arm64: compile.
- x86_16: not tested.

[pin]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kISelLowering.cpp
