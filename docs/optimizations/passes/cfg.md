> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cfg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cfg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cfg.rs sha256:e2b04e37de83f3e92f8682673be8d7fb0320e75a1486b02fb576af79fe1258fa -->

# cfg

Adapted from llrm-core's `optimize/cfg.rs`, the port of
`qbopt/optimize/cfg.py`: exact MIR control-flow cleanup.

What the old `merged` checked only for the old representation is gone:
- Byte ownership: the erased jump's emptied operation, the dead
  ownership-only blocks moved along (`_empty` and transform's
  `_empty_operation`), and the forward-only walk that kept blocks placed by
  their source bytes. Layout is free here, so any chain merges and the
  merged block keeps the first block's place.
- Unrolling provenance (`repetitions`) and floating sequences
  (`floating_origin`): the rich MIR has neither.
- The substitution error: `replace_all_uses_with` cannot fail, and a swap
  cycle is still refused before anything changes.

Tests that stay behind, each of the old byte ownership or the BC raise:
- test_cloned_chain_without_source_bytes_merges and
  test_transferred_byte_ownership_does_not_block_chain_merge: the chain
  differs from the first test only in owned bytes.
- test_unreachable_ownership_between_blocks_moves_without_losing_spans and
  test_empty_accepts_zero_stack_depth: `_empty` is not ported.
- test_merge_preserves_alternate_entries_and_layout's entry, repetition and
  intervening-block cases: the entry has no predecessors (the verifier), and
  there are no repetitions or source placement; its other predecessor case
  is ported.
- test_end_guards_have_no_return_edge_in_raised_control_flow,
  test_udtrng_bounds_compare_explicit_values,
  test_udtrng_guards_constrain_subsequent_reads_of_slot,
  test_only_established_terminal_contracts_remove_return_edges,
  test_bools_constant_program_is_one_live_block and
  test_localp_keeps_termination_after_interleaved_procedure: they read BC
  fixtures through the raise and the old pipeline, not `merged`.
