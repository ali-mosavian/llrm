# M68K-VA-DOUBLE: a variadic double after an odd-size named argument

Owner: m68k. Review: 2026-12-20.

TL;DR: the pin's m68k callee rounds a variadic `double` to 8; gcc callers pack it at 4. Ours is pin-faithful, so `long_double_variadic` prints 0 on m68k as the pin does. The corpus program is `// targets: !m68k`.

## Evidence
`~/scratch/handoff/m68k-va-double-hazard/`: `sum(int n, ...)` over three doubles. gcc 15.2 prints 13; pin (clang IR, `llc` M68020) prints 0 at -O1 and -O2.

## Cause
`M68kISelLowering.cpp:169` sets `VAARG` to Expand; `SelectionDAG::expandVAArg` rounds the pointer to the IR alignment (8, the datalayout has no f64 entry). gcc's m68k ABI aligns stack arguments to 4.

## Frame (DEVIATE, rowed here)
`frame.rs` rounds the frame to 4 (`SHIPPED_STACK_ALIGN`), not the pin's `stackAlignment = 8` (`M68kSubtarget.h:69`). It came with #7199 (2026-11-05, `68f8f290a7`): a function that pushes two words and calls paid `lea (-4,%sp),%sp` and `addq.l #4`. Reason still holds (bytes and clocks); the exit stays none, a target ABI fact.
It is not pin-faithful in all cases: sp is 4 mod 8 at some calls, so a callee that rounds (the pin's `va_arg`) reads a different address there. That is why `v.c` call_a printed 13 on ours and 0 on the pin.

## Option, not now
If a user program needs gcc-caller to our-callee: change the CALLEE. `expandVAArg` with the platform ABI alignment (4, as gcc) from the target description's f64 alignment. A caller-side fix leaves gcc callers broken. DEVIATE with the permalink to `M68kISelLowering.cpp:169` and the datalayout string, plus an executed gcc-caller/our-callee test. Decided at the review or earlier.
