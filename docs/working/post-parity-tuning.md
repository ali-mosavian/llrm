# Post-parity tuning

Owner: w-6. Where llrm may differ from the pin on purpose after parity, and how.

## Inlining at -Os

The pin prices an instruction at a flat 5 (`inline-instr-cost`) and inlines under `OptSizeThreshold` 50 whatever that does to the bytes. At m32 llrm
follows it: `called_once`'s `twice` (cost 45) is inlined; crc, tile and recchop grow 52, 42 and 15 B (numbered).

In 16-bit code a size row must not grow, so the m16 target description sets `inline_size_guard` (D11): a callee that stays is inlined into an `optsize`
caller only if its code does not outgrow the call. `idioms.bas` -Os stays 4747 B where the pin's decision gives 5809. The way out of the guard is a
cost the pin's algorithm can read: the pin's TTI hooks are free-ness, the call penalty, the threshold multiplier and the last-call bonus, so a 16-bit
tuning is a threshold or a call penalty from the description, not a per-instruction size cost (there is none in `InlineCost.cpp`).

## CodeView register variables have no location ranges

Owner: TBD (low priority, after the parity items). CodeView register variables are one whole-function `S_REGISTER`, so a pin-faithful register choice
makes a variable invisible where it had a home before: `p.nib` -O0 `add`'s `b` was in EDX to the end before b74cf6bf4, and now EDX takes `t` at `b`'s
last use. Fix: `S_DEFRANGE_REGISTER` ranges per live interval. `cv4::codeview_debugs_a_nib_program_as_it_does_a_c_one` expects no `reg long b`; it
documents today's behaviour.
