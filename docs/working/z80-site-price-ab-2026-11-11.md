# z80 per-site shift/multiply price: A/B, 2026-11-11

Head 3b0750448c (llc ba2bc659ec46) vs `z80/pricing` (llc 37aad36b486d). `sweep.py --ours-only`, 12 programs x O1/O2/O3/Os/Oz: 12 of 12 compiled on both sides at every level. Head call_keeps O1 rerun: 1740 B / 55322 T (as recorded).

O2, O3, Os, Oz: no cell changes. O1, cells that moved (bytes, T-states against head):

| cell | bytes | T |
|---|---|---|
| call_keeps | -152 (1588) | +4236 |
| mul_constants | -656 | +661980 |
| div_constants_ulong | -299 | +8836680 |
| shift_int64_variable | -59 | +168960 |
| wide_shift_by_one | +53 | +47446 |
| div_constants_short | +34 | +12238956 |

Fallback tried (multiplies always `__mulsi3`, shifts priced): call_keeps 1490 B / 65966 T; wide_shift_by_one -125 B / +1648710 T. Not taken: more clocks lost, and still above the 1473 B tip.

Cause of the two cells worse on both axes: the model prices the call beside the inline form in isolation; spills around the call in the caller are not in it.

## Parked 2026-11-11 (coordinator: close #8981)

Per-function Pareto of the priced sites (WIP commit 950b70589f, llc 70dd21704270, not pushed): only call_keeps +37 B/-606 T and mul_constants -37 B/-1070 T move at O1; both come from dropping head's whole-function `CALL_MULS` flip, not from the Pareto. The one real candidate (call_keeps `churn`: 1251 -> 1062 B, 7575 -> 8382 T) is rejected by a both-axes rule. A shift call costs T even in isolation (shl 3: -34 B, +37 T), so a strict rule keeps almost no shift calls. call_keeps O1 +267 B stays with the #8911 bisect.
