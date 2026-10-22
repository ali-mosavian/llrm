# FPRound is priced as one instruction

`CastOp::FPRound` is a float rounded to its own type's precision, what a store to a variable does to a value x87 holds wider. LLVM has no such op: the pin never sees it.

`instructioncost.rs` prices it as one instruction (`cast_of` returns none, so `Cost::BASIC`). In a loop with many (QB's `nbody_single`: six per iteration in `%b20`) it counts in the loop size and in the full unroll's cost:

| input to the cost | unrolled | rolled | `%b20` at -O2 |
|---|---|---|---|
| ours, FPRound = 1 | 168 | 186 | not unrolled (boost 110, threshold 165) |
| ours, FPRound = store + reload (tti) | 204 | 222 | not unrolled (boost 108, threshold 162) |
| pin, FPRound written as `bitcast` | 122 | ~127 | unrolled |
| pin, FPRound written as `fpext`+`fptrunc` | 168 | ~220 | unrolled |
| ours and pin, FPRound removed | | | both unroll (size 22 x 6 < 150) |

`shouldFullUnroll`, `getFullUnrollBoostingFactor` and `analyzeLoopUnrollCost` are the pin's; the decision differs only because the op cannot be written to the pin. Pricing it higher did not change one unroll decision in `nbody` or `nbody_single` at -O2 and -O3. Closed as representation, not translation (w-3, 2026-10-22).

Not this row: `nbody` -O2/-O3 clocks are +6.2% / +10.6% against main (355,537 -> 377,442; 336,281 -> 372,067). The first bad commit is 7761d2b672 (LoopFullUnroll cost half, x87 spill traffic: 9 live doubles against 8 slots). Owner: w-1, with w-4.

Method: `tools/pairs/pin.py opt -m 32 --passes loop-unroll` with `-unroll-threshold` and `-unroll-max-percent-threshold-boost` swept to read the pin's costs (the pinned `opt` has no `-debug-only`); `LLRM_DEBUG=unroll`.
