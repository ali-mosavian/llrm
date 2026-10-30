# QB float rows above main (#3352)

`llrm-qb` rounds each SINGLE and DOUBLE assignment as BC does (`-fexcess-precision=standard`, #3352), which makes `nbody_single.bas` print 86016 at -O1 and -O2.
That costs one `fstp dword` and a reload per assignment. Oversight ruled the cost a correctness trade (2026-10-21). Each row below is above main (854bb7efc) in at least one counter; owner w-1, exit 2026-10-31: translate main's way of printing 86016 at its low counts (skip the rounding of a value already exactly f32), and hand the feat/parity-versus-main part (the `feat/parity` column) to w-4.

Main's 86016 on `nbody_single.bas` was luck, not a mechanism: main is the plain loop in fast mode, which rounds only at the program's array stores, and the same shape printed 86015 in C (575cecf0d). There is nothing in main to translate; the cost of rounding each assignment is BC's own (one `fstp` per assignment). A reload fold finds no site in these rows: of the 45 `fstp [t]; fld [t]` round trips in nbody_single -O2 none is followed by an arithmetic op that can take the cell as a memory operand (st-form, `fxch`, `fld st(i)` or a store follow).

Measured with `tools/bench/bench.py --lang bas bq16 bq32 --no-judge` (executed instructions and the 486 model, so the build profile does not enter): main (the dist build of 854bb7efc) | feat/parity without #3352 | #3352 (`-` where the base has no counter). 31 rows (jpeg_single, stft_single and every bq32 floats/nbody rows) have no counters in the bench and are not listed.

| row | program/lang/level | instructions | clocks | code bytes |
|---|---|---|---|---|
| QF-1 | floats/bas/O1 | 5044 \| 5042 \| 7042 | 99205 \| 99200 \| 110200 | 208 \| 206 \| 212 |
| QF-2 | floats/bas/O2 | 5044 \| 5042 \| 7042 | 99205 \| 99200 \| 110200 | 208 \| 206 \| 212 |
| QF-3 | floats/bas/O3 | 5044 \| 5042 \| 7042 | 99205 \| 99200 \| 110200 | 208 \| 206 \| 212 |
| QF-4 | floats/bas/Os | 5045 \| 5043 \| 7043 | 99206 \| 99204 \| 110204 | 210 \| 206 \| 212 |
| QF-5 | floats/bq16/O1 | 5044 \| 5042 \| 7042 | 99205 \| 99200 \| 110200 | 213 \| 211 \| 217 |
| QF-6 | floats/bq16/O2 | 5044 \| 5042 \| 7042 | 99205 \| 99200 \| 110200 | 213 \| 211 \| 217 |
| QF-7 | floats/bq16/O3 | 5044 \| 5042 \| 7042 | 99205 \| 99200 \| 110200 | 213 \| 211 \| 217 |
| QF-8 | floats/bq16/Os | 5045 \| 5043 \| 7043 | 99206 \| 99204 \| 110204 | 215 \| 211 \| 217 |
| QF-9 | fpbench/bas/O1 | 21873 \| 24046 \| 32326 | 163138 \| 167806 \| 213046 | 705 \| 815 \| 865 |
| QF-10 | fpbench/bas/O2 | 21873 \| 18831 \| 27611 | 163138 \| 159328 \| 207408 | 705 \| 1516 \| 1822 |
| QF-11 | fpbench/bas/O3 | 16338 \| 17971 \| 27566 | 156097 \| 166143 \| 207183 | 3574 \| 3935 \| 2594 |
| QF-12 | fpbench/bas/Os | 21944 \| 23156 \| 31916 | 163360 \| 166859 \| 213059 | 699 \| 746 \| 804 |
| QF-13 | fpbench/bq16/O1 | 22818 \| 24461 \| 32741 | 165041 \| 169019 \| 214259 | 711 \| 822 \| 872 |
| QF-14 | fpbench/bq16/O2 | 22818 \| 19242 \| 28022 | 165041 \| 160537 \| 208617 | 711 \| 1525 \| 1831 |
| QF-15 | fpbench/bq16/O3 | 17276 \| 18384 \| 27983 | 157993 \| 167353 \| 208397 | 3584 \| 3944 \| 2603 |
| QF-16 | fpbench/bq16/Os | 22887 \| 23573 \| 32333 | 165261 \| 168074 \| 214274 | 705 \| 753 \| 811 |
| QF-17 | fpbench/bq32/O1 | 21403 \| 22318 \| 30598 | 162132 \| 162751 \| 207991 | 797 \| 886 \| 950 |
| QF-18 | fpbench/bq32/O3 | 16673 \| 18232 \| 27196 | 156525 \| 166870 \| 206609 | 4301 \| 4663 \| 3239 |
| QF-19 | fpbench/bq32/Os | 21682 \| 21285 \| 29565 | 162449 \| 161359 \| 206599 | 778 \| 747 \| 801 |
| QF-20 | nbody/bas/O1 | 56396 \| 79791 \| 86991 | 382966 \| 423151 \| 462751 | 868 \| 956 \| 974 |
| QF-21 | nbody/bas/O2 | 56396 \| 75007 \| 86205 | 382966 \| 422246 \| 481435 | 868 \| 1150 \| 1216 |
| QF-22 | nbody/bas/O3 | 52812 \| 45857 \| 66056 | 385455 \| 395299 \| 507694 | 1019 \| 1561 \| 1852 |
| QF-23 | nbody/bas/Os | 58458 \| 79253 \| 86453 | 389279 \| 422060 \| 461660 | 724 \| 808 \| 826 |
| QF-24 | nbody/bq16/O1 | 57380 \| 80212 \| 87412 | 384956 \| 424368 \| 463968 | 870 \| 955 \| 973 |
| QF-25 | nbody/bq16/O2 | 57380 \| 75424 \| 86622 | 384956 \| 423459 \| 482648 | 870 \| 1149 \| 1215 |
| QF-26 | nbody/bq16/O3 | 53792 \| 46276 \| 66471 | 387441 \| 396511 \| 508902 | 1021 \| 1558 \| 1849 |
| QF-27 | nbody/bq16/Os | 59440 \| 79672 \| 86872 | 391267 \| 423275 \| 462875 | 726 \| 807 \| 825 |
| QF-28 | nbody/bq32/O1 | 48537 \| 57845 \| 62645 | 369086 \| 380748 \| 417948 | 823 \| 865 \| 881 |
| QF-29 | nbody/bq32/O2 | 48537 \| 52657 \| 63455 | 369086 \| 383441 \| 442230 | 823 \| 985 \| 1088 |
| QF-30 | nbody/bq32/Os | 52541 \| 53041 \| 60241 | 376218 \| 375275 \| 414875 | 701 \| 691 \| 723 |
| QF-31 | nbody_single/bas/O1 | 1037180 \| 1063203 \| 1477215 | 8236073 \| 8214154 \| 10476220 | 388 \| 415 \| 471 |
| QF-32 | nbody_single/bas/O2 | 1037180 \| - \| 1311142 | 8236073 \| - \| 10345938 | 388 \| - \| 1457 |
| QF-33 | nbody_single/bas/O3 | 744172 \| 871196 \| 1311142 | 7625171 \| 8357227 \| 10345938 | 2389 \| 2831 \| 1457 |
| QF-34 | nbody_single/bas/Os | 1013186 \| 1026239 \| 1440251 | 8104085 \| 8159285 \| 10421351 | 375 \| 406 \| 464 |
| QF-35 | nbody_single/bq16/O1 | 1037180 \| 1063203 \| 1477215 | 8236073 \| 8214154 \| 10476220 | 393 \| 420 \| 476 |
| QF-36 | nbody_single/bq16/O2 | 1037180 \| - \| 1311142 | 8236073 \| - \| 10345938 | 393 \| - \| 1462 |
| QF-37 | nbody_single/bq16/O3 | 744172 \| 871196 \| 1311142 | 7625171 \| 8357227 \| 10345938 | 2394 \| 2836 \| 1462 |
| QF-38 | nbody_single/bq16/Os | 1013186 \| 1026239 \| 1440251 | 8104085 \| 8159285 \| 10421351 | 380 \| 411 \| 469 |
| QF-39 | nbody_single/bq32/O1 | 1013183 \| 1062196 \| 1476208 | 8141081 \| - \| 10435220 | 464 \| 529 \| 601 |
| QF-40 | nbody_single/bq32/O2 | 1013183 \| - \| 1498134 | 8141081 \| - \| 10663882 | 464 \| - \| 1515 |
| QF-41 | nbody_single/bq32/O3 | 744169 \| 871188 \| 1311129 | 7626155 \| 8359188 \| 10347894 | 2750 \| 3533 \| 2228 |
