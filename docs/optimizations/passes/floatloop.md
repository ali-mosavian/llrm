> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/floatloop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/floatloop.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/floatloop.rs sha256:44ee4b2d5912730ab106648c8e4046d14f657bb4cfe4c6d35f880ec6aa1f2ee7 -->

# floatloop

A counted loop of exact float work in memory, run once: its last trip,
from the memory the trips before leave, which floatfacts proves. Adapted
from llrm-core's `optimize/floatloop.rs`, the port of
`qbopt/optimize/floatloop.py`. LLVM has no one counterpart; its
IndVarSimplify rewrites exit values and its SCCP folds a computable
loop's effects.

What changed with the IR:
- The seeds go first in the latch, where the old ones followed its first
  float load, the x87 checkpoint.
- The counter's value after the loop is its final number, placed by
  `SsaUpdater` for every use outside the loop, a phi's included.
- Header phis lose the latch's arm and, left with one, are that value. What
  the loop no longer reads is Dead's.
- A seed's pointer must be there before the latch: a constant or a value
  from outside it.

Dropped, no rich MIR analogue: the checkpoint and `floatfacts::checkpoint`
(the rich MIR observes no FP exception), `strength::_made` (the final
counter is a constant, no copy), `transform::_leaving` (every use is an
operand). With no exception to observe the last trip could go too; it
stays, as the old pass kept it.

floatbounds has no consumer here, nor elsewhere on the rich MIR.

Tests, in `floatloop_tests.rs`:
`test_exact_loop_retains_checkpoint_and_final_iteration`,
`test_emitted_final_answer_has_the_correct_symbol` and
`test_unproved_or_observable_iterations_remain`, as MIR. Skipped:
`test_checkpoint_with_additional_effects_is_not_ignored` (`Fcheck`),
`test_floatloop_can_be_disabled_for_stage_bisection` (the pipeline's
options) and
`test_checkpoint_keeps_initial_memory_and_counter_stores_for_an_error_handler`
(loopmotion's ON ERROR stores).
