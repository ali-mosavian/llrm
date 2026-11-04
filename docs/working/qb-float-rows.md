# QB float rows above main (#3352)

`llrm-qb` rounds each SINGLE and DOUBLE assignment as BC does (`-fexcess-precision=standard`, #3352), which makes `nbody_single.bas` print 86016 at -O1 and -O2.
That costs one `fstp dword` and a reload per assignment. Oversight ruled the cost a correctness trade (2026-10-21). Main (65098fd950) already rounds each assignment, so against main the `bas` and `bq16` rows are level (floats: +1 clock, -2 B on bq16). What is left above main is `bq32` at -O1/-O2/-O3/-Os: owner qb-metadata, exit 2026-11-08.

Run 2026-11-02: main = the build of 65098fd950, head = release/next 17c5b8bec4 + 54445f41fb (no effect on the corpus). `tools/bench/bench.py floats fpbench nbody nbody_single --lang bas bq16 bq32 --opt O1 O2 O3 Os --no-judge`, load 146 at the start of main's run and 37 at the head's; counts are emulated and exact. Each side ran its own tree's bench.py (the head's with main's binaries does not build): the m16 judge change (293ad80408) moves clocks by under 0.3% on these programs. The 30 other cells are level or better.

| row | program/lang/level | instructions main \| head | clocks main \| head | code bytes main \| head |
|---|---|---|---|---|
| QF-1 | floats/bas/O1 | 5041 \| 5041 | 99197 \| 99198 | 202 \| 202 |
| QF-2 | floats/bas/O2 | 5041 \| 5041 | 99197 \| 99198 | 202 \| 202 |
| QF-3 | floats/bas/O3 | 5041 \| 5041 | 99197 \| 99198 | 202 \| 202 |
| QF-4 | floats/bas/Os | 5041 \| 5041 | 99197 \| 99198 | 202 \| 202 |
| QF-5 | floats/bq16/O1 | 5041 \| 5041 | 99197 \| 99198 | 209 \| 207 |
| QF-6 | floats/bq16/O2 | 5041 \| 5041 | 99197 \| 99198 | 209 \| 207 |
| QF-7 | floats/bq16/O3 | 5041 \| 5041 | 99197 \| 99198 | 209 \| 207 |
| QF-8 | floats/bq16/Os | 5041 \| 5041 | 99197 \| 99198 | 209 \| 207 |
| QF-9 | fpbench/bq32/O2 | 17711 \| 21674 | 155673 \| 159982 | 1451 \| 838 |
| QF-10 | fpbench/bq32/O3 | 18058 \| 21725 | 160233 \| 162125 | 2965 \| 2020 |
| QF-11 | nbody/bq32/O1 | 50215 \| 50200 | 365465 \| 365355 | 672 \| 752 |
| QF-12 | nbody/bq32/O3 | 43460 \| 51662 | 368560 \| 409467 | 1451 \| 1706 |
| QF-13 | nbody/bq32/Os | 46034 \| 46029 | 362400 \| 362300 | 613 \| 655 |
| QF-14 | nbody_single/bas/Os | 990195 \| 990195 | 7865094 \| 7865095 | 392 \| 392 |
| QF-15 | nbody_single/bq16/Os | 990195 \| 990195 | 7865094 \| 7865095 | 399 \| 397 |
| QF-16 | nbody_single/bq32/O1 | 972201 \| 972205 | 7817098 \| 7817102 | 624 \| 528 |
| QF-17 | nbody_single/bq32/O2 | 859134 \| 972205 | 7923824 \| 7817102 | 1681 \| 528 |
| QF-18 | nbody_single/bq32/O3 | 902132 \| 1020139 | 8309816 \| 8126832 | 2542 \| 1390 |

Cause: the `bas` and `bq16` rows are noise (1 clock, 2 B). The `bq32` rows are the loop decisions at -O2/-O3 (code 1681 -> 528 B on nbody_single -O2 with +13% instructions and -1.3% clocks; fpbench -O2 1451 -> 838 B, +22% instructions, +2.8% clocks): not the rounding, which main has. Landing (x86-m32, 2026-11-02): bf74ea331c (unroll size override, removed by #6404 d0eb0cc5b3) and the U3 float-stack decline, which keeps fpbench/nbody/nbody_single rolled; fix by x86-m32, exit 2026-11-06.
