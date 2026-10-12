> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/unswitch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/unswitch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/unswitch.rs sha256:5723b1e23feed432501b3c8341db854248c796533ab40f4594328ffdca07c320 -->

# unswitch

Adapted from llrm-core's `optimize/unswitch.rs`, the port of
`qbopt/optimize/unswitch.py`: specialize a loop around a pure invariant
condition, entirely in MIR.

What changed with the IR:
- The old candidate was re-optimized by `transform::recorded`, the whole
  pipeline with unswitching off and the machine's tuning forwarded. That
  pipeline is not ported, so the re-optimization is the passes the pass is
  given; its price is `profit::weighted` at the target's costs, each loop
  weighted by the trips induction proves.
- The old stage records and `watch` hook are the pass manager's dump and
  change log.
- A condition's purity was checked on the old operations' memory, flag,
  stack and x87 fields; an `icmp` has none, so what is left is that its
  operands are defined before the loop.
- The guard and dispatch dropped the old operations' byte provenance; a
  clone's is its `Cloned` change.

Python's `ValueError`s are the `Err` text.

Tests, in `unswitch_tests.rs`:
`test_condition_must_be_pure_and_loop_invariant` is ported with a written
loop. The three monkeypatching tests that replaced the re-optimization are
ported by passing it:
`test_unswitch_rejects_a_candidate_without_loop_removal`,
`test_unswitch_rejects_lower_count_but_higher_target_cost` and
`test_unswitch_rejects_semantic_work_without_a_target_price`;
`test_unswitch_reoptimization_preserves_mir_target_costs` records what
the candidate's passes read of the target. Stay behind, reading BC
corpora (`ivarm-*.obj`, `ivproc-*.obj`):
`test_production_ivarm_has_no_loop_and_stores_last_value`,
`test_specialized_main_and_legacy_procedure_emit_together`,
`test_invariant_branch_specialization_exposes_loop_deletion`,
`test_cloning_provenance_survives_ssa_reconstruction` and
`test_implicit_edge_bridge_does_not_retarget_the_taken_arm`.
