# Flipping `--isel` to `dag`: what must be true

Numbers: sdag-d, 2026-10-25, parity plus #4366 #4354 #4344 #4338 #4359, `LLRM_VERIFY=1 LLRM_ISEL=dag LLRM_ISEL_REAL=1`,
`tools/dosbatch/run_tests.py c` (426 variants: m16 and m32), `tools/dagisel/qcport_shadow.py` (QCport `-Os`; m32 on the
stubbed tree `scaling.py qcport_tree` builds, not the raw sources), and the vsgcc harness (30 AM-BT programs, one tree,
old against dag). History of the c corpus (passing / total): 348/464 (run 13), 381/418, 406/425, 413/426.

| Gate | Now | Needed |
|---|---|---|
| Wrong output | 0 once #4366 lands (`bintree`: the encoder dropped a lea displacement) | 0 across two runs |
| c programs | 413 / 426 under dag; 426 / 426 under the old selector | all, or each remaining one refused by both |
| Compile refusals, c | 13: float live across a float-returning call 11 (w-2, #4362), `byval_struct` (sdag-a #4346), `huge_runs` | none |
| QCport m16 `-Os` | 523 / 701 selected, 65 / 65 modules compile | all selected |
| QCport m32 `-Os` | 603 / 699 selected, 65 / 65 compile | all selected |
| Compile time vs old | not measured | no worse than 1.1x over the corpus |
| Code quality vs old | AM-BT, below | no row worse than old by more than 2% |
| Pin goldens | green in CI | unchanged |
| Targets | m16, m32; m64 keeps the old selector | until it has a DAG path |

## Remaining refusals

| Row | n (m16 / m32) | Owner |
|---|---|---|
| float live across a call that returns a float (RFP-5), kept by `float_across_call` | 157 / 79 | w-2 #4362, then sdag-d deletes the net |
| opcode 30013 (LLRM_CODE), 30010 (LLRM_ASM) | 3, 1 / 2 | sdag-a #4352 |
| `BTR`/`BTS` forms, memory and immediate `BT` | 0 seen | sdag-d on sight |
| `flags overwritten between a producer and a consumer` (a 64-bit add's carry, a select between `add` and `adc`) | 0 / 1 (`x_ll_arith`) | sdag-d: SD30 `BacktrackBottomUp` / `CopyAndMoveSuccessors`, review 2026-12-03 |
| lea of a symbol and a lone index, `fill_nest` | 1 | sdag-c |

## AM-BT rows, old against dag on one tree (m32 `-O2`, 2026-10-25, parity ebfe5d2ac)

Ratios of the vsgcc harness's clocks/code: old and dag to the pinned clang, and dag to old (above 1.02 = the flip makes it worse). Red = dag more than 2% above clang on the row's metric. 3 closed (nbody, x_fir, x_matmul_f), 27 red, 15 worse under dag than under old; all 30 run right (ok=true).

| row | program | metric | old/clang | dag/clang | dag/old | owner | state |
|---|---|---|---|---|---|---|---|
| AM-BT-1 | bintree | clk+code | 1.05/1.04 | 1.03/1.08 | 0.98/1.04 | w-6 | red, worse than old |
| AM-BT-2 | crc | code | 1.04 | 1.47 | 1.41 | sdag-b | red, worse than old |
| AM-BT-3 | lru | clk | 1.46 | 1.06 | 0.72 | w-6 | red |
| AM-BT-4 | matmul | code | 1.20 | 1.07 | 0.89 | w-3 | red |
| AM-BT-5 | nbody | code | 1.19 | 0.91 | 0.76 | w-6 | closed |
| AM-BT-6 | queens | code | 1.04 | 1.26 | 1.21 | splitkit | red, worse than old |
| AM-BT-7 | switch | code | 1.03 | 1.07 | 1.03 | sdag-b | red, worse than old |
| AM-BT-8 | x_binsearch | code | 1.21 | 1.28 | 1.05 | splitkit | red, worse than old |
| AM-BT-9 | x_bubble | clk+code | 1.06/1.26 | 1.45/1.11 | 1.38/0.89 | w-3 | red, worse than old |
| AM-BT-10 | x_cordic | code | 1.18 | 1.16 | 0.98 | w-6 | red |
| AM-BT-11 | x_crc32 | code | 1.87 | 1.96 | 1.05 | sdag-b | red, worse than old |
| AM-BT-12 | x_dct | code | 1.05 | 1.03 | 0.98 | splitkit | red |
| AM-BT-13 | x_fir | code | 1.37 | 0.94 | 0.69 | splitkit | closed |
| AM-BT-14 | x_fixmul | clk+code | 1.00/0.96 | 1.02/1.08 | 1.02/1.12 | splitkit | red, worse than old |
| AM-BT-15 | x_funcptr | clk+code | 0.97/0.97 | 1.30/1.07 | 1.34/1.11 | splitkit | red, worse than old |
| AM-BT-16 | x_hanoi2 | clk | 1.24 | 1.28 | 1.03 | w-6 | red, worse than old |
| AM-BT-17 | x_hash | code | 1.17 | 1.10 | 0.94 | splitkit | red |
| AM-BT-18 | x_heapsort | code | 1.03 | 1.06 | 1.03 | splitkit | red, worse than old |
| AM-BT-19 | x_iir | code | 1.35 | 1.07 | 0.80 | splitkit | red |
| AM-BT-20 | x_isqrt | code | 1.15 | 1.05 | 0.91 | w-3 | red |
| AM-BT-21 | x_list | clk+code | 1.13/1.08 | 1.23/0.96 | 1.08/0.89 | w-6 | red, worse than old |
| AM-BT-22 | x_ll_arith | code | 1.13 | 1.33 | 1.18 | splitkit | red, worse than old |
| AM-BT-23 | x_matmul_f | code | 1.03 | 0.97 | 0.94 | w-6 | closed |
| AM-BT-24 | x_nbody_f | code | 1.14 | 1.11 | 0.98 | w-6 | red |
| AM-BT-25 | x_popcount | code | 1.43 | 1.23 | 0.86 | sdag-b | red |
| AM-BT-26 | x_reverse | clk+code | 1.17/1.18 | 1.25/1.26 | 1.07/1.07 | w-3 | red, worse than old |
| AM-BT-27 | x_strlen | code | 1.17 | 1.22 | 1.04 | w-3 | red, worse than old |
| AM-BT-28 | x_switch_dense | code | 1.08 | 1.06 | 0.98 | sdag-b | red |
| AM-BT-29 | x_switch_ranges | code | 1.22 | 1.07 | 0.88 | sdag-b | red |
| AM-BT-30 | x_transpose | code | 1.08 | 1.03 | 0.95 | w-3 | red |

Causes read: crc (x1.41, `c ? POLY : 0` stays a branch diamond; pin: `neg; sbb; and`: sdag-b combineCMov), queens (allocation in the inner loop: spill/reload, counter in memory, invariant `n` re-read, IV copied twice; the conditions are fixed by #4416: splitkit), x_bubble (clocks: a cross-block `shl j,2` address the pin's CGP sinks into the use: w-4), x_funcptr (jump-table range check kept: w9), x_ll_arith (3 scheduler fallbacks left in SD30, CopyAndMoveSuccessors: w12).

## Order

1. Wrong output stays 0 (#4366).
2. Delete `float_across_call` when #4362 lands; that is 157 / 79 functions.
3. Land the emit rows above; each is a refusal, none is wrong code.
4. Measure compile time on the whole corpus (new tooling: none yet), and executed instructions per program.
5. Flip the default for m32 first, then m16.
