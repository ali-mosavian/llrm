# Post-parity tuning

Owner: w-6. Where llrm may differ from the pin on purpose after parity, and how.

## Inlining at -Os

The pin prices an instruction at a flat 5 (`inline-instr-cost`) and inlines under `OptSizeThreshold` 50 whatever that does to the bytes. At m32 llrm
follows it: `called_once`'s `twice` (cost 45) is inlined; crc, tile and recchop grow 52, 42 and 15 B (numbered).

In 16-bit code a size row must not grow, so the m16 target description sets `inline_size_guard` (D11): a callee that stays is inlined into an `optsize`
caller only if its code does not outgrow the call. `idioms.bas` -Os stays 4747 B where the pin's decision gives 5809. The way out of the guard is a
cost the pin's algorithm can read: the pin's TTI hooks are free-ness, the call penalty, the threshold multiplier and the last-call bonus, so a 16-bit
tuning is a threshold or a call penalty from the description, not a per-instruction size cost (there is none in `InlineCost.cpp`).

## Constant multiplies are priced by the profile (DEVIATE, number from the coordinator)

Owner: w-1. llrm multiplies by a constant with GCC's `synth_mult` chain (`arithmetic::scale`) when its clocks are below the immediate multiply's under
the CPU profile; the pin's `combineMul` only decomposes 3, 5, 9 and a few two-op shapes, and never at minsize. `x_switch` m32 -O2 (`x * 1103515245`,
vsgcc priced streams, i486): llrm 37207 instructions, 42090 clocks, 619 B; pin's clang 19212 instructions, 83500 clocks, 372 B (gcc 54805 clocks).
Kept: 11 one-clock steps against an `imul` of 13 to 42 clocks. At -Os llrm does not chain (0 `lea`, 2 `imul`; 218 B, 125401 clocks against
clang's 233 B, 129588), as the pin does. Revisit only if the profile's multiply price changes.
