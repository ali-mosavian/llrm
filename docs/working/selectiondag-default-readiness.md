# Flipping `--isel` to `dag`: what must be true

Numbers: run 12, 2026-10-23, parity with #4130/#4131/#4135/#4136, `LLRM_ISEL=dag LLRM_ISEL_REAL=1`,
`run_tests.py c alias huge` (463 programs: m16 and m32 variants). Selector proof: 1083 functions by
the DAG path, 219 by the old selector.

| Gate | Now | Needed |
|---|---|---|
| Wrong output | 2 (run 11: 25; run 10: 2) | 0, and stays 0 across two runs |
| Programs passing | 343 / 463 (74%) | 463 / 463, or each remaining one refused by both selectors |
| Compile refusals | 118, all in `classify_isel.py` rows | none; a refusal in default mode is a build failure |
| Compile time vs old | not measured at corpus scale (6 programs, one run each: within noise) | measured on the whole corpus, no worse than 1.1x |
| Code quality vs old | not measured | executed instructions per program no worse (cost channel is MIR: use executed counts) |
| Pin goldens | green in CI | unchanged |
| Targets | m16, m32 built; m64 refused | m64 keeps the old selector until it has a DAG path |

## Blocking rows (run 12)

| Rows | n | Owner |
|---|---|---|
| emit: opcode N, frame index N, LD_Fp/ILD_Fp long double, LESNrm, REP_STOSB_N, I64 constant pool, jump table | ~110 | sdag-d |
| Far `CopyToReg` legalize | 8 | sdag-b |
| `LLRM_FARCALL` has no id and no emitter arm (a call through a far pointer is dropped if only the id is added) | 7 | sdag-b + sdag-d |
| `ExtractElement` unselected | 3 | sdag-c |
| allocator could not place every range | 2 | regalloc |

## Order

1. Wrong output to 0 (the 2 left are `int_compared_with_float -O0`: #4141).
2. Emit rows: each one is a refusal, none is wrong code, so they can land in any order.
3. Measure compile time and executed instructions on the corpus (new tooling: none yet).
4. Flip the default for m32 first (fewest rows), then m16.
