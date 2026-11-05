# QB float rows above main (#3352)

`llrm-qb` rounds each SINGLE and DOUBLE assignment as BC does (`-fexcess-precision=standard`, #3352), which makes `nbody_single.bas` print 86016 at -O1 and -O2.
That costs one `fstp dword` and a reload per assignment. Oversight ruled the cost a correctness trade (2026-10-21). Main (65098fd950) already rounds each assignment, so against main the `bas` and `bq16` rows are level (floats: +1 clock, -2 B on bq16). What is left above main is `bq32` at -O1/-O2/-O3/-Os: owner qb-metadata, exit 2026-11-08.

Run 2026-11-05: main column = the 2026-11-02 run of 65098fd950 (unchanged), head = release/next 44eb8ed1ef, the dist and the tree's own `tools/bench/bench.py` of that commit (`floats fpbench nbody nbody_single --lang bas bq16 bq32 --opt O1 O2 O3 Os`; counts are emulated and exact). Closed (level or better in every counter): QF-10, QF-14, QF-15, QF-16, QF-18. Still above main, owner qb-metadata for the numbering, exit 2026-11-08:

| row | program/lang/level | instructions main \| head | clocks main \| head | code bytes main \| head |
|---|---|---|---|---|
| QF-1 | floats/bas/O1-O3 | 5041 \| 5041 | 99197 \| 99201 | 202 \| 205 |
| QF-2 | floats/bas/Os | 5041 \| 5039 | 99197 \| 99212 | 202 \| 203 |
| QF-3 | floats/bq16/O1-O3 | 5041 \| 5041 | 99197 \| 99201 | 209 \| 210 |
| QF-4 | floats/bq16/Os | 5041 \| 5039 | 99197 \| 99212 | 209 \| 208 |
| QF-9 | fpbench/bq32/O2 | 17711 \| 17653 | 155673 \| 158294 | 1451 \| 1564 |
| QF-11 | nbody/bq32/O1 | 50215 \| 50281 | 365465 \| 365568 | 672 \| 730 |
| QF-12 | nbody/bq32/O3 | 43460 \| 51743 | 368560 \| 426491 | 1451 \| 1695 |
| QF-13 | nbody/bq32/Os | 46034 \| 46110 | 362400 \| 362513 | 613 \| 655 |
| QF-17 | nbody_single/bq32/O2 | 859134 \| 853134 | 7923824 \| 8029859 | 1681 \| 1588 |

Cause: QF-1..4 are 4 to 15 clocks and 1 to 3 B on a 99,000-clock program (noise; QF-4 is also 2 instructions fewer). QF-9, 11, 12, 13, 17 are the bq32 loop decisions on the float stack (the rounding is main's too): nbody -O3 is 19% more instructions and 15.7% more clocks, the largest. They are x86-m32's (U3 float-stack decline, exit 2026-11-06); this file keeps the numbers until their post-fix run replaces them.
