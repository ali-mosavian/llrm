# Rows above main

One table for every row above main that a landing opens, so the tuning phase starts from a complete list. Compile-time losses are rows too (metric `Ir`; there `main` is the pin). Workers add their rows in their PRs; w-6 merges and de-duplicates. `tools/test_above_main_rows.py` refuses a row without an owner and an exit.

Metric: `clocks` (486 model, `tools/rows/`), `bytes` (object or LEDATA), `Ir` (callgrind or `perf instructions:u`). Delta is parity minus main. A date in `exit` is the day the row is re-measured or closed.

| id | program/module | level | metric | main | parity | delta | cause | owner | exit | PR |
|---|---|---|---|--:|--:|--:|---|---|---|---|
| AM-1 | x_mergesort m32 | O1,O2,O3 | clocks | 65146 | 84585 | +19439 (+29.8%) | boolean/select lowered as setcc chains the pin also emits; main branched | w-2 | 2026-10-30, setcc plan | #3571 |
| AM-2 | stft_fixed bq32 | O2,O3 | clocks | 215.0 M | 240.0 M | +25.0 M (+11.6%) | same: string leaves (`str_is_tmp` 3 branches on main, a setcc chain on parity and in the pin's llc) | w-2 | 2026-10-30, setcc plan | #3572 |
| AM-3 | stft_single bq32 | O2,O3 | clocks | 256.8 M | 293.3 M | +36.5 M (+14.2%) | same as AM-2 | w-2 | 2026-10-30, setcc plan | #3572 |
| AM-4 | bintree bq32 | O2,O3 | clocks | 140.0 k | 147.3 k | +7.3 k (+5.2%) | allocation: `depth+1` rematerialised twice and spilled in `WALK&`; fewer instructions than main, more clocks | splitkit | 2026-10-24, ladder check | |
| AM-5 | bintree bq16 | O2,O3 | clocks | 296 k | 309 k | +13 k (+4.4%) | as AM-4 | splitkit | 2026-10-24, ladder check | |
| AM-6 | jpeg_single bq32 (flagged column) | O2 | clocks | 176.5 M | 186.3 M | +9.8 M (+5.6%) | not the runtime (its inlining landed in 5f6abb9234); x87/allocation | splitkit | 2026-10-30, allocation rows | |
| AM-7 | quicksort m32 | O2 | clocks | 190328 | 194727 | +4399 (+2.3%) | inner loop `inc; cmp; jge`: the pin's llc has the same loop; main's count-to-zero form came from its old strength pass; D10 copies reduce it (206294 without) | w-6 | 2026-11-30, D10 review | #3507 |
| AM-8 | quicksort m32 | O3 | clocks | 178429 | 201984 | +23555 (+13.2%) | D10 copies; cause not examined | w-6 | 2026-10-25 | #3507 |
| AM-9 | recfloat m32 | O1 | clocks | 84605 | 86394 | +1789 (+2.1%) | `decay` becomes a loop (good) but the entry gains a table lookup (`G$5[eax*8]`, cmp/jae) for its first calls: +141 B | w-6 | 2026-10-25 | |
| AM-10 | x_hanoi2 m32 | O3 | clocks | 134926 | 164043 | +29117 (+21.6%) | D10 copies at depth 3; no depth reaches main (best 148561 at depth 2); main's -O3 does something else with the recursion | w-6 | 2026-11-30, D10 review | #3507 |
| AM-11 | hanoi m32 | O2,O3 | clocks | 96 | 103 | +7 (+7.3%) | the chain of differences `n-1==0`... in D10 copies; SimplifyCFG's branch-compare to switch fold | w-4 | after #3422 re-measure, 2026-10-24 | #3422 |
| AM-12 | recsum m32 | O1 | clocks | 127296 | 131065 | +3769 (+3.0%) | as AM-11 | w-4 | after #3422 re-measure, 2026-10-24 | #3422 |
| AM-13 | recmany m32 | O1 | clocks | 214706 | 218101 | +3395 (+1.6%) | allocation: the specialised clone leaves 4 spills an iteration | splitkit | after RECMANY-TCE-1 (w-2), allocation freeze | |
| AM-14 | x_popcount m32 | O1,O2,O3 | bytes | 1795 | 1840 | +45 (+2.5%) | isel folds an add into an LEA, then needs a `test` the pin's flags reuse avoids (only with #3497) | w-2 | with #3497, 2026-10-27 | #3497 |
| AM-15 | QCport host/demo | Os | bytes | 5673 | 5852 | +179 (+3.2%) | phi temporary with no register class of its own (QO-1, QO-13b); base already above main | splitkit | 2026-10-30, QO-13b | |
| AM-16 | _ls_animate@3, _sc_bsplit@3 (QCport m16) | O1..Os | allocation | 0 | 7 | 7 functions | four address values live for BX/SI/DI; the greedy fails and the rounds loop places them; the pin has no 8086 model | splitkit | 2026-10-24, ladder check (region split); LU2 parked: docs/working/lsr-address-registers.md | #3585 |
| AM-17 | d_faces compile | O2 | Ir | 508.7 M (pin) | 1298.3 M | +789.6 M (2.55x) | buckets: loop 204.8 vs 86.8, scalar 165.7 vs 81.8, llrm-only 215.2 vs 0, regalloc 172.4 vs 67.2, clean-up 153.2 vs 49.4, module/IPO 51.6 vs 13.3 | w-6 (ledger) | re-take after 5 landings; goal 1.5x | |
| AM-18 | d_faces compile (#3575) | O2 | Ir | 1280.1 M | 1298.3 M | +18.2 M (+1.4%) | a function a pass declares is seen by the passes after it: a design cost the pin shares | w-8 | 2026-10-30 | #3575 |

## To add by their owners

Named by the coordinator and not yet filled: QO-SIZE-1, QO-13 (render/dl -O2), r_bsp, queens -O1/-O2/-O3, x_bubble, nbody, lru m32, CC1 rows, #3582 rows. A row goes in the table with its numbers, cause, owner and exit; nothing is estimated here.
