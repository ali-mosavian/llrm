# 6502 -fwhole-program A/B, 2026-11-11

Same llrm-cc (tip 8ce215fe37 + this PR), 6502-sim65, `tools/6502/ours_cr.py`, one layout (pad 0), 81 programs PASS per run, output checked.
Only the sweep differs (`OURS_WHOLE_PROGRAM=0` vs default). Bytes = program object + linked runtime members; clocks = sim65 executed, single layout (differences of a few clocks are layout drift, not the pass).
Worse cells: none at O2, none at Os. Cells not listed are identical.

## O2: 19 cells, -6854 B

| program | bytes off -> on | clocks off -> on |
|---|---|---|
| bitfield_long_long | 193 -> 118 | 782 -> 782 |
| builtins_table | 174 -> 104 | 638 -> 638 |
| data_int64 | 1243 -> 1235 | 5326 -> 5326 |
| double_to_uint64 | 2765 -> 2638 | 17040 -> 17056 |
| fill_nest | 916 -> 468 | 262486 -> 262456 |
| float_in_memory_loop | 363 -> 21 | 664 -> 319 |
| frame_address_stack_args | 1276 -> 88 | 1500 -> 540 |
| idioms | 12559 -> 12549 | 14253611 -> 14254593 |
| idiv_loop_os | 841 -> 470 | 1789441 -> 1792176 |
| pair_return | 83 -> 30 | 399 -> 398 |
| pattern_fill | 845 -> 651 | 124890 -> 125023 |
| recmany_o1 | 1579 -> 851 | 5108934 -> 5108575 |
| shuttle_value | 1412 -> 140 | 1798 -> 928 |
| strlen_idiom | 124 -> 76 | 600 -> 601 |
| struct_odd_sizes | 565 -> 36 | 434 -> 404 |
| struct_with_i64 | 850 -> 68 | 982 -> 577 |
| switch_i64 | 522 -> 123 | 857 -> 857 |
| switch_peeled_table | 339 -> 169 | 18470 -> 18470 |
| value_of_valueless | 59 -> 19 | 317 -> 317 |

## Os: 16 cells, -4109 B

| program | bytes off -> on | clocks off -> on |
|---|---|---|
| bitfield_long_long | 193 -> 118 | 782 -> 782 |
| builtins_table | 174 -> 104 | 638 -> 638 |
| data_int64 | 1234 -> 1226 | 5368 -> 5368 |
| double_to_uint64 | 2535 -> 2408 | 23250 -> 23429 |
| float_in_memory_loop | 386 -> 21 | 724 -> 319 |
| frame_address_stack_args | 1108 -> 909 | 12201 -> 12200 |
| idioms | 3239 -> 3229 | 30892194 -> 30892067 |
| memmove_overlap | 726 -> 720 | 17286 -> 17279 |
| pair_return | 83 -> 30 | 399 -> 398 |
| pattern_fill | 863 -> 669 | 148514 -> 148420 |
| shuttle_value | 1342 -> 140 | 1798 -> 928 |
| strlen_idiom | 124 -> 76 | 600 -> 601 |
| struct_odd_sizes | 567 -> 36 | 434 -> 404 |
| struct_with_i64 | 850 -> 68 | 982 -> 577 |
| switch_i64 | 522 -> 123 | 857 -> 857 |
| value_of_valueless | 59 -> 19 | 317 -> 317 |
