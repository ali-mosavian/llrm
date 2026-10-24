# Flipping `--isel` to `dag`: what must be true

Numbers: run 13, 2026-10-24, parity e394947bb plus #4180, `LLRM_ISEL=dag LLRM_ISEL_REAL=1`,
`run_tests.py c alias huge` (464 programs: m16 and m32 variants). Selector proof: 951 functions by
the DAG path, 219 by the old selector. History (passing / total): run 10 309/453, run 11 331/463,
run 12 343/463, run 13 348/464.

| Gate | Now | Needed |
|---|---|---|
| Wrong output | 1 (`wc_mixed`: x87 compare branches on `je`, routed to sdag-d) | 0, and stays 0 across two runs |
| Programs passing | 348 / 464 (75%) | 464 / 464, or each remaining one refused by both selectors |
| Compile refusals | 115, all in `classify_isel.py` rows | none; a refusal in default mode is a build failure |
| Compile time vs old | not measured at corpus scale | measured on the whole corpus, no worse than 1.1x |
| Code quality vs old | not measured | executed instructions per program no worse |
| Pin goldens | green in CI | unchanged |
| Targets | m16, m32 built; m64 refused | m64 keeps the old selector until it has a DAG path |

## Blocking rows (run 13)

| Row | n | Owner |
|---|---|---|
| value read before it is defined on every path (phi 30, mov 8, shl 1) | 39 | sdag-d |
| LD_Fp / ST_FpP long double | 19 | sdag-d |
| frame index N | 12 | sdag-d |
| floating argument stored into the area at m16 | 9 | sdag-d |
| non-constant address part | 9 | sdag-d |
| TargetJumpTable before its definition | 8 | sdag-d |
| string operation on a pointer of no known segment | 7 | sdag-d |
| floating value live across an instruction that empties the x87 stack | 6 | sdag-d |
| lea of a symbol and a lone index, Truncate before definition, outgoing area | 5 | sdag-d |
| memmove of far pointers (the routine is near) | 1 | sdag-a |
| wrong output | 1 | sdag-d, sdag-b |

## Order

1. Wrong output to 0 (the one left is `wc_mixed`).
2. Emit rows: each one is a refusal, none is wrong code, so they can land in any order.
3. Measure compile time and executed instructions on the corpus (new tooling: none yet).
4. Flip the default for m32 first (fewest rows), then m16.
