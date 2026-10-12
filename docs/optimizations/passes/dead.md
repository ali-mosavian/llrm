> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dead.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dead.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dead.rs sha256:56d48571d165c7f8bd0dc4036419ff5720ae21edf6dbfb7ca64d22a55319961b -->

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
