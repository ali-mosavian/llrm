> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/pipeline.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/pipeline.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/pipeline.rs sha256:c2384b059bcdd5729ddcf77fe00293bc18e268d9a7b99599ef3b6136e1b38c10 -->

# pipeline

The MIR fixed point every driver runs: llrm-core's `optimize/transform.rs`
`pipeline`, `applied`, `recorded`, `Applied` and `_Transaction` (with
`_applied` and `_transacted`), on llrm-mir's pass manager. The
transaction is `Fixed`, one function pass; `applied` runs it under a
manager that requires `Summaries` and verifies the module after it,
then the whole-module step, `Interprocedural`, which runs it again on
each body it changes, then `Rotate`.

What changed with the IR:
- A pass reports a change by preserving less than every analysis; the old
  compared bodies. Arenas never shrink, so the repeated-state check compares
  printed bodies instead.
- The stage records are the manager's change log (`recorded`); `watch` is
  `Applied::dump`, a file per changed step of each body.
- `Where`'s segment, BC blocks and object file went with the BC frontend.
  The machine's facts are the program's target's.
- PointerProvenance, SplitPointers and Place have no rich-MIR meaning.
  Hoist's store sinking is loopmotion's pass.
- `flow::optimized`'s rule that an irreducible body is not promoted is here:
  promotion needs dominators.
- The old `_Transaction` verified nothing; `applied`'s manager verifies the
  module after the pipeline.
