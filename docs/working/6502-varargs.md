# 6502-VARARGS: `va_start`/`va_arg`/`va_end`/`va_copy` at 6502

Owner: 6502. Exit: 2026-11-24. Opened 2026-11-09 (wasm-b's REFUSED table, 9 rows on 6502 and 65c02: the variadic programs and `long_double_variadic`).

A variadic function is refused: `llrm-6502-llc` ends `CannotSelect(VAStart)` (`VAEnd` for the minimal `sum(i16, ...)` that calls `va_start`, `va_arg`, `va_end`, reproduced on release/next 2026-11-09 with the 6502 datalayout). No lowering of the four nodes exists for the target.

## Plan
One general lowering, no per-program case: `va_list` is a pointer into the caller's stack arguments.
- ABI (`calling.toml` `mos`): named arguments go in `a`, `x`, `rc2`..`rc7`, the rest on the soft stack (`rs0`). Decision to take in the first PR: a variadic call puts every variadic argument on the stack (the callee cannot know which registers hold them), as llvm-mos does; the caller knows from the prototype.
- `VASTART`: store the address of the first variadic stack slot (`rs0 + frame size + offset of the first stack argument`, `frame.rs`) into the `va_list`. `VAARG`: the shared expansion (load, advance by the slot size rounded to the stack's argument alignment; `double` is `variadic_float = "double"`); `VAEND`: nothing; `VACOPY`: copy the pointer.
- Reuse `SelectionDAG::expandVAArg` and the pin's `VASTART` custom hook shape (X86/M68k); no private copy.

## Test
Fail-first: the minimal IR above refuses today. Executed on sim65: `sum(int n, ...)` over ints, longs and doubles, `va_copy` of a list read twice, a variadic callee with a register-passed named `long`; output against a host build.
