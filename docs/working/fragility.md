# Fragility

For contributors changing the backend, the loop passes or the measuring tools: places where a change breaks quietly, and the test that guards each. Every guard named here exists in the tree.

## Measurement

**DOSBox cannot see memory operands.** Its fixed-cycle core charges per instruction, so turning `add ax,[bp-6Ah]` into `add ax,bp` leaves the time unchanged. Unchanged time is not evidence of no gain, and a gain in time is not evidence of fewer memory operands. Judge such changes by `tools/innerloops.py` and the emulator counts of `tools/bench/bench.py`.

**Decoding data as code hides loops.** `tools/innerloops.py` decodes only what control reaches from the entries (`_decoded`), not the segment from byte 0. Guard: `tests/test_innerloops.py`.

**A loop's estimated trips depend on where it is tested.** `executed` (`crates/codegen/llrm-codegen/src/backend/executed.rs`) gives a loop tested at its header N/(N+1) to stay, so an unrotated loop's body is not counted one trip short of a rotated one; a loop with no proven count goes round nine times in ten whatever its number of exits. Guards: `test_a_loop_tested_at_its_header_runs_its_body_every_trip`, `test_an_uncounted_loop_runs_as_often_as_its_exits_odds_say`.

**A test that passes before the fix proves nothing.** Check the listing holds the instruction under test, and watch the test fail with the fix reverted.

## Contracts

**A port is not an unknown callee.** An OUT to a port that reaches memory writes anything, not only what escaped; a DMA write may touch a global whose address was never taken. Guard: `test_a_port_call_is_narrowed_only_where_the_target_says_its_device_reaches_no_memory` (`ports_tests.rs`).

**Facts only the frontend knows must be said.** A slice descriptor is `noalias readonly`, so its length loads leave the loop and bounds checks are counted. Guard: `test_a_load_from_a_readonly_noalias_parameter_leaves_past_a_store` (`licm/tests.rs`).

**A pass can ask for a form the selector does not build.** A far pointer is never compared for the exit: the selector has no compare for it. Guard: `test_a_far_pointer_is_never_compared_for_the_exit` (`lsr_tests.rs`).

**A loop that "fits" must be allocatable.** A far or indexed access reads base, index and selector back from spilled cells. Guard: `test_loops_over_many_arrays_build_on_a_p5` (`tests/toolchain.rs`).

## Machine form

**The allocator knows values the body no longer names.** A reload numbered from the body alone reuses such a name. Guard: `test_a_spill_numbers_its_reloads_above_the_floor_it_is_given` (`spiller.rs`).

**A rewrite narrows a value's class.** An assignment made before a split can fall outside the class recomputed after it. Guard: `test_an_assignment_outside_its_rewritten_class_is_evicted` (`allocate.rs`).

**An unspillable range is not refused by a rule for spillable ones.** It evicts at any stage. Guard: `test_an_unspillable_range_evicts_a_spillable_holder_at_any_stage` (`tests/toolchain.rs`).

**Tail merging keeps the kept tail's values.** The other tail's readers must be renamed to them. Guard: `test_merged_tails_share_their_values` (`jumps_tests.rs`).

**A parallel copy reads before it writes.** A copy group's sources interfere before any destination is written. Guards: `test_parallel_copy_sources_interfere_before_any_destination_is_written` (`coalesce.rs`), `test_a_parallel_copy_reads_its_sources_before_any_destination_is_written`.

**A constant copied into a slot is still remadeable.** Guard: `test_a_constant_copied_into_a_slot_by_a_parallel_copy_is_remade` (`spiller.rs`).
