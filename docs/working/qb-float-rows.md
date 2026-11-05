# QB float rows above main (#3352)

`llrm-qb` rounds each SINGLE and DOUBLE assignment as BC does (`-fexcess-precision=standard`, #3352), which makes `nbody_single.bas` print 86016 at -O1 and -O2.
That costs one `fstp dword` and a reload per assignment. Oversight ruled the cost a correctness trade (2026-10-21). Main (65098fd950) already rounds each assignment, so against main the `bas` and `bq16` rows are level (floats: +1 clock, -2 B on bq16). What is left above main is `bq32` at -O1/-O2/-O3/-Os: owner qb-metadata, exit 2026-11-08.

Run 2026-11-05: main column = the 2026-11-02 run of 65098fd950 (unchanged), head = release/next tip 43e6b5bba4 (includes #7189, QF-19), the clean dist stamped 43e6b5bba4 (provenance in the run's `.provenance`) and the tree's own `tools/bench/bench.py` of that commit (`floats fpbench nbody nbody_single --lang bas bq16 bq32 --opt O1 O2 O3 Os`; counts are emulated and exact). Closed (level or better in every counter): QF-10, QF-14, QF-15, QF-16, QF-18. Still above main, owner qb-metadata for the numbering, exit 2026-11-08:

| row | program/lang/level | instructions main \| head | clocks main \| head | code bytes main \| head |
|---|---|---|---|---|
| QF-1 | floats/bas/O1-O3 | 5041 \| 5041 | 99197 \| 99201 | 202 \| 205 |
| QF-2 | floats/bas/Os | 5041 \| 5039 | 99197 \| 99212 | 202 \| 203 |
| QF-3 | floats/bq16/O1-O3 | 5041 \| 5041 | 99197 \| 99201 | 209 \| 210 |
| QF-4 | floats/bq16/Os | 5041 \| 5039 | 99197 \| 99212 | 209 \| 208 |
| QF-9 | fpbench/bq32/O2 | 17711 \| 17541 | 155673 \| 157870 | 1451 \| 1562 |
| QF-11 | nbody/bq32/O1 | 50215 \| 50269 | 365465 \| 365544 | 672 \| 730 |
| QF-12 | nbody/bq32/O3 | 43460 \| 44730 | 368560 \| 395266 | 1451 \| 1546 |
| QF-13 | nbody/bq32/Os | 46034 \| 46098 | 362400 \| 362489 | 613 \| 655 |
| QF-17 | nbody_single/bq32/O2 | 859134 \| 853134 | 7923824 \| 8029859 | 1681 \| 1588 |

Cause: QF-1..4 are 4 to 15 clocks and 1 to 3 B on a 99,000-clock program (noise; QF-4 is also 2 instructions fewer). QF-9, 11, 12, 13, 17 are the bq32 loop decisions on the float stack (the rounding is main's too): nbody -O3 is 2.9% more instructions and 7.2% more clocks (was 19% and 15.7% before #7189), still the largest. After #7189 (2026-11-05 run): QF-12 improved (clocks 426491 -> 395266, bytes 1695 -> 1546); QF-9 instructions are now below main (17541 < 17711) but clocks +1.4% and bytes +111; QF-17 is smaller than main (-93 B, fewer instructions) at +1.3% clocks; QF-11 and QF-13 stay +58 and +42 B at under 0.1% clocks. All five stay above main in at least one counter and are x86-m32's (exit 2026-11-06).
