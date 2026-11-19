# Z80-ALLOCA: `DynamicStackAlloc` at z80

Owner: z80. Exit: 2026-11-30. Opened 2026-11-09 (coordinator's survey).

`int a[n]` and `__builtin_alloca(n)` refuse at every level: `CannotSelect(DynamicStackAlloc)`. Reproduced on release/next 2026-11-09:

```llvm
declare void @use(ptr)
define void @f(i16 %n) { %a = alloca i8, i16 %n  call void @use(ptr %a)  ret void }
```

#8576 handles constant allocas only (frame size, refusal past 8 bytes); no row covered the dynamic one.

## Plan
Reuse the shared expansion (`legalize_dag.rs` `ISD::DynamicStackAlloc`, run when the target's `stack_pointer()` hook states `(sp, stack_align)`), as m68k does ([m68k-alloca.md](m68k-alloca.md)); no private copy. z80: SP is read with `ld hl,#0 / add hl,sp`, written with `ld sp,hl`; the frame keeps `ix`, so locals stay addressable after SP moves. Size rounds up to the stack alignment (1 byte: none).

## Test
The IR above must compile and its `sp` update must precede the call: fail-first today (refused).
