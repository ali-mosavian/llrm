> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/floatfold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/floatfold.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/floatfold.rs sha256:0b216263445b7fbdd3b35012132501c2d8231f653835924c6f701abc5f6c1389 -->

# floatfold

Float values floatfacts proves exact, replaced by their constants: LLVM's
InstSimplify and InstCombine for floats. Adapted from llrm-core's
`optimize/floatfold.rs`, the port of `qbopt/optimize/floatfold.py`.

What changed with the IR:
- `stored` wrote an exact x87 value's bits where it was stored, x87 having
  no immediate operand. Here every read of the value reads the constant, a
  store's included, and Dead takes the definition, as Fold's integers
  (`_dead_values` is Dead's).
- `discarded` dropped an unread exact conversion. Here every read of the
  conversion's integer reads the number, and Dead takes the conversion.

Dropped, no rich MIR analogue: `checks` and `_checked`, the `Fcheck`
(FWAIT) kept for each removed operation's exceptions: the rich MIR
observes no FP exception, so gvn has nothing to call.

Tests, in `floatfold_tests.rs`: `test_storage_requires_exact_bits` and
`test_exact_pair_keeps_checks_and_refuses_observable_results`, less
their checks. Skipped: the `checks` tests, and BC object corpora
(`test_fpdeep_exact_double_stores_do_not_execute_floating_arithmetic`,
`test_qb_fpcse_preserves_entry_when_first_load_disappears`,
`test_collapsed_fpcse_has_no_empty_jump_trampoline`,
`test_original_wait_is_an_explicit_checkpoint_with_encoding_provenance`).
