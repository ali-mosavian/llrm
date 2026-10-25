# DAG default flip: packet

Owner w10 (assembler). One page for the user's decision on #4512. Updated as rows close; every status names its evidence.
Last update 2026-10-25 18:10 CET, parity `f529f1e7c`+.

| # | condition | status | evidence |
|---|---|---|---|
| 1 | Correctness, DOSBox under `--isel=dag` | **Met on the corpus; examples unconfirmed.** Runtime equality, 1042 programs, m16+m32: `-O1..-Os` 1042/1042 dag and old; `-O0` dag 1038 (old 1037), the same forced-level recursion programs fail on both (recdeep, recmany, recmany_o1, recsum). Forced `-Os` corpus in DOSBox: dag 1045/1045 (#4795). Open since 09:45: planets x3, speaker x2 (w9): not re-checked here. | w11 runtime sweep in a private note (bench rows at 69b60cafc3) 16:16; #4795 body; w9 for planets/speaker |
| 2 | Compile time: dag − old ≤ 80 M Ir, d_faces `-O2`, one binary | **Met.** `81f154e16` (#4614): m16 old 788,800,409 / dag 832,692,447 = **+43.9 M**; m32 781,564,612 / 822,262,995 = **+40.7 M**. Parent `0aea1e46a`: +63.6 M / +61.4 M. Control: old differs 46 K between the dists. Re-take at the final tip before landing. | dists built by `tools/pairs/pair_build.sh` at 81f154e16 and 0aea1e46a (md5 = `ok`); method `tools/pairs/pair_build.sh`, `valgrind --tool=callgrind`, `d_faces` flattened to one source |
| 3a | Size, QCport m32 within numbered rows | **Open, shrinking.** `-Os` dag/old on 64 common modules: +2.89% at `81f154e16` (+7,542 B) → **+1.37% (+3,583 B)** at `8df7ebf2e` (#4649 in). Bisect: first bad `67cc69178` (#4620 piece 1), cause CF5 ebp-keep at `-Os` (weapons `leave` 17 vs pin 0). #4795 recovers weapons 16,285 → 16,266 (pin 15,126); the rest is the call-frame lane (pin pushes arguments at minsize). | a private script summing the QCport stub tree's emitted bytes, a private bisect log; w12 #4795, #4649 |
| 3b | Size, m16 against the pin | **Met on the last measure, to re-take on the corrected pin (#4780).** At `c11bc98dd`: `-Os` pin 21,675 / old 17,513 / dag 18,168 (dag closer to the pin than old); `-O2` pin 31,296 / old 24,453 / dag 24,479. | w12 15:20 in `bench-rows-69b60cafc3.md`; `tools/dagisel/m16_bytes.py` |
| 3c | AM-BT clocks, dag/old > 1.02 get a row | **Table: 17/22 PIN-FAITHFUL on the old pin instrument.** Closed since: x_list (1.125 → 1.000, #4763), x_funcptr (#4787), x_crc32 faithful (loop idiom is IR level). Open: x_hanoi2 1.23 (pass order, w12/w2), x_dct 1.05 and x_cordic 1.05 (#4745, w13). The pin column is inflated for call-heavy code until #4780 lands (llc used stack align 16): rerun then. | a private note (AM-BT table) (#4754), `flip-ambt-rows-8-14.md` (w11); `a private AM-BT harness` |
| 3d | Static instructions vs the pin, m32 `-O2`, 71 programs | dag +0.4% vs pin (old +5.9%) at `c11bc98dd`; the pin column needs the #4780 rerun. Residual causes are not selection: return-block merge and callee-saved count (w1 row 80, #4729), call frames (#4649), zext-load `xor;mov8` (recorded 486 win). | `tools/pairs/pincount.py` (#4739); first-divergence table a private note (first divergence, m32) (#4699) |
| 4 | No wrong-output risk open | **Met by the named items:** volatile #4570/#4581, returns_twice #4587, float setcc #4593 all merged. The gate-hole table is the remaining risk: group (a) #4797 14/15 green, 1 real regression left (nearcalls queens bounds checks, bisect pending). | w8/w4 gate-hole table; #4797 |
| 5 | The user is told before the landing | Open; coordinator tells. | |

## Open rows that can still move the decision
1. 3a: CF5 frame removal conditions (oversight 17:11): esp-relative locals with a runtime fail-first test, full `-Os` sweep of 1042 programs, trace the host IR rejection by llc. Owner w12.
2. 3c/3d: reruns after #4780 and #4745.
3. 4: the nearcalls regression and the 8 remaining stale gate fixtures (w4/w8).
4. 2: dist pair at the final tip.
