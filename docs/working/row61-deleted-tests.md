# Tests deleted with the non-RFP route (row 61 completion)

`FloatAssign`/`assigned`, `phielim`'s own lowering (`eliminated_priced`), `with_old_lowering` and `LLRM_NO_RFP` are gone; the default route (RFP, translated `PHIElimination`) was unchanged: 0 of 568 objects differ. These tests ran the deleted code.

## floatassign_tests.rs: the old assigner's rounds, spills and folds (module deleted) (13)
- `test_a_compare_crowded_before_its_branches_spills`
- `test_a_round_writes_only_the_blocks_a_new_spill_reaches`
- `test_the_body_liveness_is_solved_at_most_twice`
- `test_spilled_phi_copies_are_not_all_held_at_once`
- `test_a_cycle_of_spilled_phi_copies_is_ordered_not_held_at_once`
- `test_a_blocks_steps_and_liveness_are_worked_out_once`
- `test_a_round_writes_only_the_steps_a_new_spill_reaches`
- `test_a_round_looks_for_the_crowded_block_from_where_the_last_left_off`
- `test_a_body_with_no_float_is_returned_without_rebuilding_a_block`
- `test_pressuresink_builds_no_set_of_live_values_per_block`
- `test_a_cycle_of_float_copies_is_written_one_at_a_time`
- `test_a_float_compare_clobbers_the_status_word_register`
- `test_a_float_reload_defines_its_value`

## phielim.rs: the old lowering, its walks and edge prices (15)
- `test_a_phi_read_by_its_sibling_is_not_renamed_into_the_latch`
- `test_phi_elimination_copies_the_whole_scalar`
- `test_phi_on_a_single_predecessor_exit_does_not_split_the_edge`
- `test_phi_source_live_on_the_other_branch_gets_an_edge_copy`
- `test_splitting_both_edges_of_a_branch_keeps_its_odds`
- `test_critical_edge_copy_uses_the_final_trivial_phi_name`
- `test_a_value_nothing_reads_after_the_edge_asks_no_walk`
- `test_a_read_past_a_loop_is_found_from_the_reads_as_the_scan_finds_it`
- `test_a_read_only_through_the_block_copied_into_is_no_walk`
- `test_a_read_beyond_a_block_is_found_from_either_end_as_the_walk_finds_it`
- `test_phi_observation_on_an_immediate_alternate_edge_is_visible`
- `test_phi_elimination_renames_a_value_a_cell_is_reached_by`
- `test_a_build_for_size_keeps_its_phi_copies_in_the_predecessor`
- `test_an_edge_that_wins_by_less_than_a_tenth_keeps_its_copy_in_the_predecessor`
- `test_a_body_the_translated_lowering_finishes_does_not_take_the_old_stage`

## floatalloc/fpstack: old-assigner decisions (folds, spills, control-word placement) the stackifier no longer sees (34)
- `test_a_copy_between_spilled_values_is_no_instruction`
- `test_a_load_folded_into_x87_memory_arithmetic_retains_its_source_bytes`
- `test_a_load_read_by_two_multiplies_is_their_memory_operand_on_the_486`
- `test_a_loop_of_conversions_switches_the_rounding_mode_on_its_edges_not_every_trip`
- `test_a_loop_that_rounds_outside_a_conversion_keeps_its_switches`
- `test_a_join_that_reads_no_float_takes_none_on_its_other_edges`
- `test_a_load_read_once_by_the_next_arithmetic_is_its_memory_operand`
- `test_an_integer_constant_loads_from_the_pool`
- `test_a_conversion_store_of_a_value_read_again_does_not_copy_it`
- `test_a_quadword_conversion_store_of_a_live_value_keeps_its_copy`
- `test_a_float_live_across_a_call_is_read_again_from_its_cell`
- `test_exact_float_constants_need_no_frame`
- `test_a_value_loaded_before_a_barrier_is_not_the_cell_after_it`
- `test_conversion_result_is_kept_for_non_operand_readers`
- `test_integer_result_waits_before_reading_owned_conversion_storage`
- `test_a_vacated_phi_copy_leaves_its_copy_group`
- `test_a_first_read_moves_past_a_trapping_instruction_only_for_a_quiet_cell`
- `test_independent_x87_regions_select_their_own_complete_candidate`
- `test_integer_conversion_materializes_a_frame_operand`
- `test_runtime_stack_integer_result_uses_memory_without_named_float_values`
- `test_a_volatile_load_is_its_adjacent_readers_operand_only`
- `test_repeated_stable_float_cell_load_is_kept_across_consumers`
- `test_a_value_every_successor_spills_leaves_in_memory`
- `test_a_first_read_moves_past_a_trapping_instruction_where_no_trap_is_unmasked`
- `test_integer_conversion_into_physical_x87_stack_materializes_memory`
- `test_floating_bridge_never_reads_an_unestablished_slot`
- `test_missing_float_is_not_created_by_an_exchange`
- `test_ninth_float_uses_an_owned_spill`
- `test_profitable_multiuse_float_home_is_retained_for_the_selected_cpu`
- `test_region_value_reuses_one_reload_until_an_unknown_effect`
- `test_shared_float_crosses_only_a_unique_straight_line_edge`
- `test_unused_integer_conversion_keeps_checkpoints_without_materializing_result`
- `test_the_arm_that_skips_the_update_takes_no_exchange_in_a_loop`
- `test_x87_memory_operand_matches_each_public_cpu_cost`

## llrm-cc compile.rs: the float argument slot under floatassign (RFP-3 is the route's row) (1)
- `test_truncation_saves_the_control_word_once_per_bodytest_a_float_argument_is_read_from_its_own_cell`

## parallel-copy stress tests whose premise (a loop copy of N moves) the old lowering made; ParallelCopy's own tests pin the cycles (2)
- `test_a_parallel_copy_cycle_through_a_frame_slot_is_scheduled`
- `test_a_reload_stretched_by_a_parallel_copy_is_not_spilled_again`
