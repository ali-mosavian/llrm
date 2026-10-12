> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loadjoins.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loadjoins.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loadjoins.rs sha256:2f3c44da21040d788daea0ed90dc0f264017de0b098b2d224bf805c251d7b80f -->

# loadjoins

LLVM's GVN load PRE at joins: llrm-core's `optimize/loadjoins.rs`, the
port of `qbopt/optimize/loadjoins.py`, adapted to the rich MIR. A load at
a join whose bytes every predecessor already holds becomes a phi of those
values; with `insert`, a predecessor holding none gets its own load, on
its edge, where that speculates nothing.

What changed with the IR:
- A provider is a load or a store, what it holds a value or a constant, of
  the load's type; the old width check is the type.
- A missing load goes on either edge of a conditional branch. The old MIR
  could split only the explicit (taken) one.
- The join's prefix may hold what `llrm_mir::memory::only_value` allows,
  loads aside, where the old one allowed moves. Division is C's, so the old
  guard against a trapping one has no MIR meaning.
- An address is translated on an edge when the load's pointer is a phi of
  the join, as when the old pointer reference's base was.
- Dropped: stack slots, x87 operations, `merges`, and the `symbol`,
  `source_backed` and `raised` marks.

A call's footprint is its `CallEffects`: without `Summaries` required,
an unknown callee's. An address translated onto an edge carries
alias's provenance, as every access does.
