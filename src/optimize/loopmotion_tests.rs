//! Port of tests/test_loopmotion.py.
//!
//! Every test loads a corpus fixture through modules not yet ported
//! (corpus, `mir.bodies`, wholeseg, transform, promote), so all are skipped:
//! `test_indexed_record_accumulators_store_only_after_loop`,
//! `test_indexed_exit_store_requires_a_dominating_invariant_address`,
//! `test_harr_constant_column_exit_is_stored_once_only_after_a_nonempty_loop`,
//! `test_nbody_conditional_accumulator_stores_sink`,
//! `test_lngmxx_invariant_temporaries_sink_only_when_loop_executes`,
//! `test_nested_accumulator_seed_follows_outer_phi`,
//! `test_nested_accumulator_is_stored_only_after_the_outer_loop`,
//! `test_addrm_exit_store_requires_complete_initial_memory`,
//! `test_counter_is_written_once_at_the_exit_not_every_iteration`,
//! `test_an_observer_in_the_loop_keeps_the_store`,
//! `test_an_exit_reachable_without_the_store_gets_no_new_write`,
//! `test_an_accumulator_without_zero_trip_initialization_stays_in_the_loop`,
//! `test_rotated_accumulator_store_uses_exit_phi`,
//! `test_a_float_loop_sinks_its_counter_store_without_an_error_handler`.
