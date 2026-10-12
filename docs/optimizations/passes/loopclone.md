> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopclone.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopclone.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopclone.rs sha256:777813192e5fdc66028f2f0ef446101e8209e684ad2cc942798b2fba87e9b1b6 -->

# loopclone

Adapted from llrm-core's `optimize/loopclone.rs`, the port of
`qbopt/optimize/loopclone.py`: clone loop iterations as CFGs, retaining a
residual loop and every exit.

This is the MIR building block for peeling and bounded full unrolling,
not a profitability decision. Loop live-outs must already be in LCSSA.
Python's `ValueError`s are the `Err` text.

The old body's clones were found again by their fresh labels; here
`peeled` returns each iteration's block map. Its callers were peel and
unswitch. Not ported, meaning nothing here:
- `_block_local_floating`, whether cloning can join x87 stack values (a
  float is an ordinary value); its caller was `peeled`.
- Copying the old body's pointer and integer-range side tables and the
  `cloned` flag: the rewrite ledger records each `Cloned` instruction.

Tests skipped:
`test_peeling_refuses_floating_work_behind_an_internal_branch`
and `test_peeling_accepts_block_local_floating_values_behind_a_branch`
(x87 regions), and the byte-ownership half of
`test_clones_read_their_own_values_and_do_not_duplicate_byte_ownership`.
