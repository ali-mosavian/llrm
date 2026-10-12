> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dead.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dead.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dead.rs sha256:12d58094e5498c153c7d278a1670a394ee6521e5982c138fa2fb469c73a91ca3 -->

# dead

Instructions nothing needs, removed: LLVM's ADCE and DCE. Adapted from
llrm-core's `optimize/transform.rs` `Dead` (`dead`, `_kept`,
`_removable`).

What changed with the IR:
- What stays whatever reads it (`_kept`: the old observed kinds, stores and
  barriers) is what `llrm_mir::memory::only_value` says is more than a
  value. A call with no effect that returns goes; the old MIR kept every
  call.
- A removed operation left an empty byte-owning marker; here it goes.
- Phis go after the operations, so a phi only dead work read goes in the
  same run rather than the next.

Dropped, no rich MIR analogue: `_overwritten_locally` and the limited
mode it served (an opaque or barrier operation reading registers its
operands do not name; every read here is an operand), and the cleanup of
merged halves.

Tests, in `dead_tests.rs`:
`test_dead_code_goes_and_the_bytes_are_still_accounted_for` is ported.
Skipped: `test_dead_code_leaves_a_body_it_cannot_read_alone` (the limited
mode).
