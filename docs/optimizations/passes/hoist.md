> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/hoist.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/hoist.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/hoist.rs sha256:004e9cb8d9253bdd772a5cddad746547faaf2b0d7ffc880d3b618da506af5d94 -->

# hoist

Loop-invariant work done once, before the loop: LLVM's LICM hoisting.

Adapted from llrm-core's `optimize/transform.rs` `Hoist`: `hoisted`,
`_invariant_run`, `_preheader`, `_crossed_values`, `_cannot_fault` and
`_guaranteed_float_work`. Loads ask memoryssa's `Accesses`, from the
manager, whether the loop writes them (`transform::_undisturbed`), so the
pipeline requires `Summaries`; induction says whether the loop makes a
trip, and noreturn which calls end one.

What changed with the IR:
- A value is SSA, and an operand defined outside the loop dominates the
  preheader's end, so the run goes before its terminator. `_placement`,
  `_reparented`, `_pruned`, `_effective`, `_starts` and `_rewritten` placed,
  renamed and thinned variables and copies there are none of.
- A call keeps only the loads it may write in the loop. The old
  `motion_blocked` refused a whole loop holding a call, which clobbered the
  machine's registers.
- What may fault -- a load of what is not known dereferenceable and the
  target says may trap at its alignment (`Machine::load_may_trap`), a
  division by other than a constant neither 0 nor -1 -- moves only from
  where the loop certainly runs it: `_guaranteed`, the old
  `_guaranteed_float_work` asked of faults rather than of the x87
  environment. Floating arithmetic is pure here.
- Dropped: flags (`_crossing`'s refusal), volatile published reads (a
  volatile access never moves), and `loopmotion::sunk_stores`, which the old
  `Hoist` ran after this and loopmotion's port owns.
