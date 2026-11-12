# 6502 flag motion audit, 2026-11-13

Cause: `compare_before_step` moved a compare above `clc; adc #1` (#9508). Guard: `flagcheck`, run by `tools/6502/ours_cr.py` (`LLRM_6502_CHECK_FLAGS=1`).

Corpus, O1/O2/Os, 81 programs each: the verifier reports 0 violations on the fixed tree; on the tree with the fix removed it reports `c bb17: instruction 3 reads C, cleared by instruction 2 (clc; adc #1)` (`counter_compare_after_step`).

Method: every text pass in `peephole.rs`, `peephole_cells.rs`, `cmpzero.rs`, `copyprop.rs`, `sharecmp.rs` and the MIR motions in `alloc.rs` read line by line by two reading agents; each UNSOUND/UNCLEAR re-derived by hand and, where real, given a test that fails first. The verifier checks the MIR motions (`compare_before_step`, `forward_old_copies`, `keep_carry_in_c`, copy removal); it has no view of the text passes.

| rewrite | differs after it | verdict |
|---|---|---|
| `compare_before_step` | N/Z/V of the compare's run | fixed #9508; V/Z/N after the branch unchecked (no V branch emitted) |
| `forward_old_copies`, copy removal | N/Z | sound; verifier clean |
| `keep_carry_in_c` | A, N/Z | sound at removal; verifier guards what later passes add |
| `load_straight_into_a` | N/Z of the deleted load | **unsound**: a zero/sign branch read them; fixed, #9518 |
| `carry_stays_in_the_flag` | A/X/Y copies of the carry byte, Z/N of the `lsr` | **unsound**: fixed, #9518 |
| `arithmetic_chain_in_a` | A after the chain | **unsound**: fixed, #9518 |
| `dead_from(A)` | `bit` read as no read | **unsound**: fixed, #9518 |
| `flagcheck` writes table | `plx`/`ply` missing | instrument gap: fixed in #9511 |
| `drop_dead_stores`, `store_then_index_load`, `fold_temp_operand`, `fold_spill_copy` | cells, X | sound |
| `chain_ends_in_a`, `shift_loop_in_a`, `forward_cell_copy`, `coalesce_copy`, `carry_holds_the_bit` | cells, A | sound |
| `copyprop`, `drop_reloads`, `drop_known_loads`, `fold_known_cells`, `compare_through_an_index` | N/Z of removed loads | sound; `flags_unread_after` guards each |
| `drop_zero_compares`, `cmpzero` | C of a dropped `cmp #0` | sound: no flag lives across blocks (`redo_compare`) |
| `hoist_common_successor_code`, `drop_flag_reload`, `drop_dead_repeat_load` | code across a branch | sound |
| `hoist_loop_constant` | a constant hoisted out of a loop with a second entry | latent: needs an irreducible loop; none seen in the corpus |
| `step_in_place` | V of `clc; adc #1` | latent: no V branch is emitted |
| `sharecmp` | A, N/Z, V | dead code: nothing builds `CMOV` after `expand_selects` |

Guard notes, none reachable from the compiler's output today: `flags_unread_after` assumes a `jmp` leaves the block; `is_cell` matches by prefix; `fold_temp_operand` compares pointer names only.

The run-output corpus passes at all three levels with every text pass on (81 programs, O1/O2/Os; the carry-byte and load fixes change no cell).
