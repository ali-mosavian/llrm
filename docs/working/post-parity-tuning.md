# Post-parity tuning

Owner: w-6. Where llrm may differ from the pin on purpose after parity, and how.

## Inlining at -Os

The pin prices an instruction at a flat 5 (`inline-instr-cost`) and inlines under `OptSizeThreshold` 50 whatever that does to the bytes. At m32 llrm
follows it: `called_once`'s `twice` (cost 45) is inlined; crc, tile and recchop grow 52, 42 and 15 B (numbered).

In 16-bit code a size row must not grow, so the m16 target description sets `inline_size_guard` (D11): a callee that stays is inlined into an `optsize`
caller only if its code does not outgrow the call. `idioms.bas` -Os stays 4747 B where the pin's decision gives 5809. The way out of the guard is a
cost the pin's algorithm can read: the pin's TTI hooks are free-ness, the call penalty, the threshold multiplier and the last-call bonus, so a 16-bit
tuning is a threshold or a call penalty from the description, not a per-instruction size cost (there is none in `InlineCost.cpp`).

## Constant multiplies are priced by the profile (DEVIATE D35)

Owner: w-1. llrm multiplies by a constant with GCC's `synth_mult` chain (`arithmetic::scale`) when its clocks are below the immediate multiply's under
the CPU profile; the pin's `combineMul` only decomposes 3, 5, 9 and a few two-op shapes, and never at minsize. `x_switch` m32 -O2 (`x * 1103515245`,
vsgcc priced streams, i486): llrm 37207 instructions, 42090 clocks, 619 B; pin's clang 19212 instructions, 83500 clocks, 372 B (gcc 54805 clocks).
Kept: 11 one-clock steps against an `imul` of 13 to 42 clocks. At -Os llrm does not chain (0 `lea`, 2 `imul`; 218 B, 125401 clocks against
clang's 233 B, 129588), as the pin does. Revisit only if the profile's multiply price changes.

## x_switch keeps 3 more copies than the pin (parked, owner w-13)

`x_switch` m32 -O2 with a single imul (no chain): copies pin/ours, PhiElimination 72/68, TwoAddress 102/104, Coalescer 14/17. Ours carries the switch
state in 12 copies against the pin's 8, at the switch head blocks; every copy left is interference by our liveness. The lea chain is not the cause.
Method if it comes back: a probe printing each refused join's two values and the first overlapping segment, or our TwoAddress state fed to the pin's
coalescer as MIR. Parked: on this kernel llrm is 2x faster than the pin in clocks.

## CodeView register variables have no location ranges

Owner: TBD (low priority, after the parity items). CodeView register variables are one whole-function `S_REGISTER`, so a pin-faithful register choice
makes a variable invisible where it had a home before: `p.nib` -O0 `add`'s `b` was in EDX to the end before b74cf6bf4, and now EDX takes `t` at `b`'s
last use. Fix: `S_DEFRANGE_REGISTER` ranges per live interval. `cv4::codeview_debugs_a_nib_program_as_it_does_a_c_one` expects no `reg long b`; it
documents today's behaviour.

## Huge-pointer carry: `sbb`+`and` for `select(carry, 2^n, 0)`

Owner: w12. Do after switch-over. A huge pointer's step moves the selector by `select(carry, 4096, 0)` of the offset add (`expand_huge_add`). The pin lowers
that select to `setb al; shl eax, 12` (its `llc -O2`, i486, `uadd.with.overflow.i16` + `select i1 %c, i16 4096, i16 0`: `xorl %eax,%eax; addw; setb %al; shll $12,%eax`);
`SETCC_CARRY` only fires for an all-ones select. The old selector's `sbb r, r; and r, 4096` prices 2 clocks against `setb`+`shl` 5 on a 486.
`tests/run/huge/pairs.c` m16 -Os loop: dag 16 instructions (`setb dl; shl dx, 12; add dx, cx` twice), old 17 (`sbb dx, dx; and dx, 4096; add dx, cx` twice).
Fix: a documented deviation for 486-class targets, priced from the CPU profile, with a fail-first isel test and the m16 -Os sweep in clocks.

## Redo handler inlining with the runaway fixed

Owner: w1. Do after switch-over. `94e1feee07` (#4851) let a QB error handler that only its registering owner calls be inlined into the owner's landing pad
(`range_on_error.bas` m16 -Os -70 B). With the llrm runtime it made `RESUME NEXT` run forever: `B$RESN` and `B$FERR` called each other until the stack
ended (gate run 2, `tools/dosbatch/test_qbstrblocks.py` at m16: "limit after 60000 ms"). Reverted. Redo: find why the inlined re-arm lands in the wrong
frame, fix it, bring the rule back with `test_qbstrblocks` as its guard and the `range_on_error.bas` size as its price. `tests/run` does not show it: it
runs the QB45 runtime, the hang needs `-fqb-runtime=llrm`.
