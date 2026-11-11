# 6502-ALLOCA: `DynamicStackAlloc` at 6502

Owner: 6502. Exit: 2026-11-30. Opened 2026-11-09 (coordinator's survey, same refusal as [z80](z80-alloca.md) and [m68k](m68k-alloca.md)).

`int a[n]` and `__builtin_alloca(n)` refuse at every level: `CannotSelect(DynamicStackAlloc)`. Reproduced on release/next 2026-11-09: the IR of z80-alloca.md with the 6502 datalayout (`e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8`) through `llrm-6502-llc -S -O2` ends `CannotSelect("DynamicStackAlloc")`. `frame.rs` lists dynamic allocas as not translated (DEVIATE, "when a program needs them").

## Plan
Reuse the shared expansion (`legalize_dag.rs` `ISD::DynamicStackAlloc`, run when the target's `stack_pointer()` hook states `(sp, stack_align)`); no private copy. The software stack pointer is `rs0` (`rc0:rc1`), the stack grows down, alignment 1, so the hook is `(rs0, 1)`.

The question the row answers is the frame: fixed objects are addressed from `rs0` at `offset + size` (`(zp),Y`, at most 255), so an `rs0` that moves breaks every local. Two ways: (a) a frame pointer in a callee-saved pair (`rs4`..) for a function with a dynamic alloca: the prologue copies `rs0` after `ADJSP -size`, objects are addressed from it, the epilogue puts `rs0` back from it; (b) no frame pointer, the dynamic area below a fixed frame and popped at exit, locals addressed from `rs0 + dynamic` (not known at compile time: rejected). (a), by `has_var_sized` in `frame.rs`, which already adjusts `rs0` around each call for variable-sized objects.

## Test
The IR above must compile, the `rs0` update must precede the call and the epilogue must restore it; fail-first today (refused). Executed on sim65: a loop that fills `a[n]` for n = 1, 7, 200 and reports a checksum.
