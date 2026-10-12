> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopmotion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopmotion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopmotion.rs sha256:dcfd524030d408447c58ea1ebad9842e304ee4829b00c9c29780c8fd203786b6 -->

# loopmotion

Loop stores nothing inside the loop observes, sunk to its single exit:
LICM's store sinking, as `promoteLoopAccessesToScalars` leaves one store
after the loop. Adapted from llrm-core's `optimize/loopmotion.rs`, the
port of `qbopt/optimize/loopmotion.py`.

Whether two accesses meet is `regions::overlapping` of the references
and call writes `memoryssa::Accesses` holds, asked again after each loop
that changed: a moved store's narrowing was its old block's.

What changed with the IR:
- A store is moved, not copied; one that goes with another value has it as
  its operand. An address is an operand too, and must reach the exit.
- What refuses a loop is what `llrm_mir::memory` says is more than a value,
  a store aside: a volatile access (the old barrier), a call that may touch
  memory, an `invoke` (a raise a handler here observes). The old refused
  every call; one with no effect that returns is a value.
- A stored cell is in an object (`MemRef::object`), as the old `Segment` and
  `Frame` spaces were, with no selector.
- `root` followed copies, which have no instruction.

Dropped, no rich MIR analogue: the `excludes` an indexed store had to
carry (a hole of the x86 region lattice), and the per-block intervals
`sunk_stores` built, which alias's `annotated` now applies.

llrm-mir's `licm` hoists invariant instructions and loads to the
preheader; it sinks no store.
