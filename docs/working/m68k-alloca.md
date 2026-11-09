# M68K-ALLOCA: `DynamicStackAlloc` at m68k

Owner: target-m68k. Exit: 2026-11-16.

`int a[n]` and `__builtin_alloca` refuse at 68000/68020, every level: `Legalize Unsupported { op: DynamicStackAlloc, ty: I32 }` (w15's sweep, 2026-11-09; reproduced 18:30Z).

## Pin
`M68kISelLowering.cpp` sets `DYNAMIC_STACKALLOC` Custom ([line 175][pin]) and `LowerDYNAMIC_STACKALLOC` ([line 3362][pin]): `CALLSEQ_START 0`, read `SP`, `SP - size`, `AND -align` when `align` exceeds the stack alignment (8, `M68kSubtarget::stackAlignment`), write `SP`, `CALLSEQ_END 0`. `SEG_ALLOCA` is split-stack only: not translated.

`legalize_dag.rs` `ISD::DynamicStackAlloc` is the same node sequence (`ExpandDYNAMIC_STACKALLOC`), run when the target's `stack_pointer()` hook states `(sp, stack_align)`. m68k states none and its `lower_operation` returns `None`: the refusal.

## Plan
Give `M68kLowering` the `stack_pointer()` hook, `(A7, 8)`, and let the shared expansion run (arm64 does the same). Frame: `frame.rs` already takes a frame pointer for `has_var_sized`.

## Other targets (2026-11-09, one line each)
- z80-sdcc, 6502: refuse, `CannotSelect(DynamicStackAlloc)`. wasm32: refuses (`Unsupported`, w15). Not part of this row.
- x86_32, arm64: compile.
- x86_16: not tested.

[pin]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kISelLowering.cpp
