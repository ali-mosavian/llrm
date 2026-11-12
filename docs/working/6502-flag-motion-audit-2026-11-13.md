# 6502 flag motion audit, 2026-11-13

Cause: `compare_before_step` moved a compare above `clc; adc #1` (#9508). Guard: `flagcheck`, run by `tools/6502/ours_cr.py` (`LLRM_6502_CHECK_FLAGS=1`).

Corpus, O1/O2/Os, 81 programs each: the verifier reports 0 violations on the fixed tree; on the tree with the fix removed it reports `c bb17: instruction 3 reads C, cleared by instruction 2 (clc; adc #1)` (`counter_compare_after_step`).

What the verifier covers: the MIR motions before expansion (`compare_before_step`, `forward_old_copies`, copy removal, reload forwarding, `keep_carry_in_c`). What it does not: the text passes after it, which have no MIR flag edge to check.

| rewrite | moves or drops across flags | guard read | verdict |
|---|---|---|---|
| `compare_before_step` | compare above a step | the step must leave the carry | fixed |
| `forward_old_copies`, copy removal, `forward_reloads` | copies (write N/Z only) | verifier | no violation |
| `keep_carry_in_c` | `SAVEC`..`LOADC` | verifier | no violation |
| peephole `drop_reloads`, `drop_known_loads`, `fold_known_cells` | loads | `flags_unread_after` | guard read, rewrite not re-derived |
| `zero_test_through_an_index`, `step_in_place`, `shift_in_place` | `cmp #0`, `clc; adc #1`, shifts | `carry_dead_from` | guard read, rewrite not re-derived |
| `drop_zero_compares`, `compare_through_an_index`, `cmpzero`, `sharecmp` | compares | branch kind (zero/sign only, carry select only) | guard read, rewrite not re-derived |
| `branch_on_shift_carry`, `carry_holds_the_bit`, `carry_stays_in_the_flag` | carry producer | `sets_carry` | guard read, rewrite not re-derived |
| `hoist_common_successor_code`, `hoist_loop_constant`, `drop_flag_reload`, `drop_dead_repeat_load` | code across a branch | not read | open |

The run-output corpus passes at all three levels with every text pass on. A line-by-line check of the text passes, and the `hoist_*` rows, is open.
