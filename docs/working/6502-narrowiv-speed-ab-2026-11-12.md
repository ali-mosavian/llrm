# 6502 narrow counters at speed + LU19/LU22, A/B, 2026-11-12

Base 0e1e83e22f (tip with #9280) vs this branch, `6502-sim65`, `ours_cr.py`, 81 programs PASS per side, mean of 4 code paddings (0, 64, 128, 192), executed sim65, whole-program off. Os: 0 cells change.
Clock deltas under the pad spread are noise (div_constants_ulong, huffman_decode: instructions +46K of 22M, +6 of 35K).

## O1: 30 cells, -8438 B

| program | bytes | clocks |
|---|---|---|
| copy_runs | 2320 -> 2259 | 73997 -> 71012 (-4.0%) |
| div_constants_long | 27389 -> 25445 | 123820677 -> 123421566 (-0.3%) |
| div_constants_short | 14119 -> 12088 | 57546436 -> 57491173 (-0.1%) |
| div_constants_ulong | 19550 -> 18147 | 78883156 -> 79017483 (+0.2%) |
| double_to_uint64 | 2667 -> 2646 | 18826 -> 18701 (-0.7%) |
| extension_chain_fused | 122 -> 96 | 1018 -> 927 (-8.9%) |
| fill_nest | 523 -> 505 | 262592 -> 260995 (-0.6%) |
| frameless | 2544 -> 2462 | 73318 -> 74306 (+1.3%) |
| huffman_decode | 1371 -> 1264 | 113412 -> 112717 (-0.6%) |
| idioms | 3304 -> 3140 | 14494365 -> 13009591 (-10.2%) |
| int64_division | 4076 -> 4044 | 995966 -> 994153 (-0.2%) |
| local_initializers | 390 -> 380 | 3156 -> 3014 (-4.5%) |
| long_double_constant_store | 108 -> 88 | 10878 -> 6452 (-40.7%) |
| loop_fusion | 1422 -> 1360 | 132657 -> 129798 (-2.2%) |
| loop_interchange | 1437 -> 1315 | 3743906 -> 3562972 (-4.8%) |
| loop_versioning | 945 -> 943 | 807528 -> 793872 (-1.7%) |
| memcpy_libcall | 627 -> 578 | 68864 -> 56673 (-17.7%) |
| memmove_overlap | 848 -> 727 | 17080 -> 12863 (-24.7%) |
| mul_constants | 14399 -> 12540 | 16882748 -> 16572915 (-1.8%) |
| overflow_builtins | 8069 -> 8045 | 427061 -> 425936 (-0.3%) |
| pattern_fill | 845 -> 820 | 125039 -> 121374 (-2.9%) |
| rep_count | 372 -> 362 | 133230 -> 133092 (-0.1%) |
| sat_builtins | 2115 -> 2089 | 33853 -> 33638 (-0.6%) |
| split_loops | 632 -> 630 | 40128 -> 38235 (-4.7%) |
| stack_arrays_copy | 290 -> 263 | 12066 -> 7435 (-38.4%) |
| switch_empty_cases | 106 -> 92 | 884 -> 791 (-10.5%) |
| switch_jump_table | 901 -> 825 | 7088 -> 6399 (-9.7%) |
| wide_shift_by_one | 1237 -> 1204 | 411715 -> 405957 (-1.4%) |
| word_compare_branches | 992 -> 961 | 219077 -> 217258 (-0.8%) |
| word_compares | 521 -> 485 | 94566 -> 95014 (+0.5%) |

## O2: 27 cells, 3624 B

| program | bytes | clocks |
|---|---|---|
| copy_runs | 2426 -> 2374 | 73571 -> 70691 (-3.9%) |
| div_constants_long | 27389 -> 25445 | 123820677 -> 123421566 (-0.3%) |
| div_constants_short | 14100 -> 24393 | 57650404 -> 57264034 (-0.7%) |
| div_constants_ulong | 19550 -> 18147 | 78883156 -> 79017483 (+0.2%) |
| double_to_uint64 | 2757 -> 2736 | 17059 -> 16928 (-0.8%) |
| extension_chain_fused | 122 -> 96 | 1018 -> 927 (-8.9%) |
| fill_nest | 916 -> 873 | 262502 -> 260670 (-0.7%) |
| frameless | 3819 -> 3819 | 65920 -> 68531 (+4.0%) |
| huffman_decode | 1356 -> 1355 | 111794 -> 112018 (+0.2%) |
| idioms | 12559 -> 12199 | 14288767 -> 12749302 (-10.8%) |
| int64_division | 4076 -> 4044 | 995966 -> 994153 (-0.2%) |
| loop_interchange | 1985 -> 1826 | 3744195 -> 3562438 (-4.9%) |
| loop_versioning | 945 -> 943 | 807528 -> 793872 (-1.7%) |
| memcpy_libcall | 607 -> 476 | 72018 -> 55169 (-23.4%) |
| memmove_overlap | 1070 -> 1229 | 17228 -> 11313 (-34.3%) |
| mul_constants | 14399 -> 12540 | 16882748 -> 16572915 (-1.8%) |
| overflow_builtins | 8069 -> 8045 | 427061 -> 425936 (-0.3%) |
| pattern_fill | 845 -> 820 | 125039 -> 121374 (-2.9%) |
| rep_count | 3467 -> 3454 | 97567 -> 97350 (-0.2%) |
| sat_builtins | 2115 -> 2089 | 33853 -> 33638 (-0.6%) |
| split_loops | 627 -> 613 | 25009 -> 23241 (-7.1%) |
| stack_arrays_copy | 693 -> 193 | 3792 -> 4953 (+30.6%) |
| switch_empty_cases | 106 -> 92 | 884 -> 791 (-10.5%) |
| switch_jump_table | 901 -> 825 | 7088 -> 6399 (-9.7%) |
| wide_shift_by_one | 1237 -> 1204 | 411715 -> 405957 (-1.4%) |
| word_compare_branches | 874 -> 840 | 187088 -> 185224 (-1.0%) |
| word_compares | 521 -> 485 | 94566 -> 95014 (+0.5%) |

