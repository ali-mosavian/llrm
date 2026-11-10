# z80 carry ops: sweep against the release/next tip (2026-11-10)

Base: tip 4bc9c84d4c, llc d98266581ecb. Candidate: PR head ae71ee4382, llc 4f34f9587753. Five levels, `sweep.py --ours-only` (ucsim clocks), same pin clang both sides, each side built fresh from its tree; compiles that succeeded per level are in the cells column (a program that failed on either side is not counted).

| level | cells | unchanged | better | worse (either axis) | bytes | clocks |
|---|---|---|---|---|---|---|
| O2 | 99 | 47 | 39 | 13 | -5352 | -1332684 |
| Os | 98 | 40 | 37 | 21 | -4589 | -20003865 |
| O1 | 97 | 40 | 39 | 18 | -3698 | -23915971 |
| Oz | 98 | 42 | 38 | 18 | -5517 | -2416826 |
| O3 | 100 | 49 | 39 | 12 | -5659 | -1380499 |

Cells worse than the tip, (bytes, clocks) deltas. Causes: register assignment around `adc hl, rr` (the sum is tied to `hl`); pricing flips of the shift and multiply routines against inline code (`call_keeps`, `div_constants_short`); loop shape. Row Z80-CARRY-OPS-WORSE in `above-main-rows.md` owns them.

- O2: byte_multiply (+9 B, -13007 T), fill_nest (+4 B, +2809 T), idioms (+166 B, +67235 T), known_loads_walks (+46 B, +1099 T), licm_graph_callee_footprint (+14 B, +57015 T), loop_fusion (+6 B, +175 T), loop_versioning (+37 B, +11102 T), mul_constants (+314 B, +102732 T), recdeep (+20 B, +25539 T), recmany_o1 (+9 B, +56975 T), register_upper_half (+28 B, +1888 T), switch_table_holes (+15 B, +16107 T), wide_shift_by_one (-26 B, +40623 T)
- Os: byte_multiply (-1 B, +46045 T), call_keeps (+52 B, +1811 T), div_constants_short (-364 B, +2001941 T), fill_nest (+28 B, +1848 T), idioms (+11 B, +452 T), known_loads_walks (-9 B, +225 T), licm_graph_callee_footprint (+14 B, +57015 T), local_initializers (+21 B, +382 T), loop_fusion (+17 B, +2812 T), loop_interchange (+3 B, +208983 T), loop_versioning (+11 B, +17740 T), memmove_overlap (+5 B, +471 T), recdeep (+19 B, +51039 T), recmany_o1 (+9 B, +56975 T), register_upper_half (+28 B, +1888 T), shift_int64_variable (+9 B, +9216 T), shrink_wrapped_frame (-5 B, +1 T), switch_peeled_table (+7 B, +12155 T), switch_table_holes (-22 B, +181 T), wide_shift_by_one (+14 B, -6668 T), word_compare_branches (-10 B, +9842 T)
- O1: byte_multiply (+9 B, -13007 T), call_keeps (+267 B, -9091 T), div_constants_ulong (+122 B, -12383932 T), fill_nest (+4 B, +2809 T), idioms (+36 B, +3067 T), licm_graph_callee_footprint (+14 B, +57015 T), local_initializers (+10 B, +205 T), loop_fusion (+21 B, +2002 T), loop_versioning (+11 B, +17740 T), memmove_overlap (+5 B, +471 T), mul_constants (+314 B, +102732 T), phi_slot_unknown_arm (-18 B, +1251 T), register_upper_half (+28 B, +1888 T), shrink_wrapped_frame (+2 B, +3 T), split_loops (-3 B, +492 T), switch_table_holes (+15 B, +16107 T), tail_calls (+4 B, -3015 T), wide_shift_by_one (-26 B, +40623 T)
- Oz: byte_multiply (-9 B, +74548 T), call_keeps (+52 B, +1811 T), div_constants_short (-364 B, +2001941 T), div_constants_ulong (-546 B, +48668 T), fill_nest (+28 B, +1848 T), idioms (+11 B, +452 T), known_loads_walks (-9 B, +225 T), local_initializers (+21 B, +382 T), loop_fusion (+17 B, +2812 T), loop_interchange (+3 B, +208983 T), loop_versioning (+11 B, +17740 T), recdeep (+19 B, +51039 T), recmany_o1 (+9 B, +56975 T), register_upper_half (+28 B, +1888 T), shift_int64_variable (+9 B, +9216 T), switch_peeled_table (+7 B, +12155 T), wide_shift_by_one (+14 B, -6668 T), word_compare_branches (-14 B, +13104 T)
- O3: byte_multiply (+9 B, -13007 T), fill_nest (+4 B, +2809 T), idioms (+166 B, +67235 T), known_loads_walks (+46 B, +1099 T), licm_graph_callee_footprint (+14 B, +57015 T), loop_versioning (+37 B, +11102 T), mul_constants (+314 B, +102732 T), recdeep (+20 B, +25539 T), recmany_o1 (+9 B, +56975 T), register_upper_half (+28 B, +1888 T), switch_table_holes (+15 B, +16107 T), wide_shift_by_one (-26 B, +40623 T)

Suite on the final head (after merging #8906 and #8893): `cargo test -p llrm-z80`, 73 binaries, 410 passed, 0 failed. The sweeps above ran on the merge with 4bc9c84d4c, before those two commits.
