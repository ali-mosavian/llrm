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

## AM-BT rows under `--isel=dag` (m32 `-O2`, ratios to the pinned clang, from the vsgcc harness)

Closed by the dag selector: `lru`, `nbody`, `x_fir`, `x_iir`, `x_matmul_f` (5). Red under dag: 23 (all the others).
`x_hash` and `x_binsearch` have the pin's form under dag (0 lea, 3 imul) and stay at 1.16 and 1.28 of clang in code,
so they do not close with the flip. `x_ll_arith` does not compile under dag (the row above); `bintree` was wrong output.

## Order

1. Wrong output stays 0 (#4366).
2. Delete `float_across_call` when #4362 lands; that is 157 / 79 functions.
3. Land the emit rows above; each is a refusal, none is wrong code.
4. Measure compile time on the whole corpus (new tooling: none yet), and executed instructions per program.
5. Flip the default for m32 first, then m16.
