# Rust port migration ledger

This ledger records the evidence and elapsed verification cost for each port
iteration.  Command durations include compilation triggered by the command.

## Iteration 0: restore the merged baseline

Base: `3aeb1857c3340d0aa5901d4829083b5023aeaf15` on `main`.

The first default Python run took 84.02 seconds: 458 tests passed, two HIR
tests failed, and 31 were deselected.  The first QB Cargo run found two stale
generated-parser golden gates and later exposed three HIR assertions tied to
the pre-source-symbol JSON layout.  Rust also warned about an unused `DATA`
row binding.

The parser snapshots were migrated only after a batched comparison of all 79
accepted programs.  Seventy-eight matched the legacy snapshots after removing
only four representation additions.  `qb45/memorymodel` was the sole semantic
delta: its source had changed in `11c6d519`, while its snapshot still described
the earlier program.  The current gate compares the complete AST exactly and
has no compatibility filter.

Pytest now builds the in-tree QB frontend once per session and uses the
executable path reported by Cargo.  Compatibility-help validation now indexes
the inherited case list once instead of reparsing it for every topic.  Eight
measured end-to-end backend and OMF regressions remain in the `full` tier rather
than the Tier 1 loop.

Final gates:

| Command | Result | Wall time |
| --- | --- | ---: |
| `cargo test --manifest-path frontends/qb/Cargo.toml -q` | 43 unit, 2 generated-lexer, 4 parser-golden, and 130 semantic tests passed | 3.35 s |
| `uv run pytest -q --durations=15` | 459 passed, 39 deselected | 12.42 s |
| `uv run pytest -q tests/test_qbcompat.py` | 20 passed | 3.80 s |

The default Python gate fell from 84.02 to 12.42 seconds.  Recorded verification
execution, including fail-first and mutation runs performed by delegated
agents, stayed below 240 seconds during the first 2,428 seconds of wall time.

The repository-wide `ty` pre-commit gate is not green on the base commit.  The
locked 0.0.75 tool reports 3,016 diagnostics, and the manifest's original
0.0.1a20 constraint reports 2,416.  This is an inherited typing backlog rather
than dependency drift or a regression from the port.  The Python commits ran
the formatting, import, whitespace, merge-marker, and focused behavioral gates;
the known-global `ty` gate was skipped and remains explicit debt.

The first commit attempt also invoked the old 77-second pytest selection before
the separately reviewed tiering commit was staged.  That accidental duplicate
temporarily raised cumulative verification to roughly 328 seconds over 2,757
seconds of wall time (11.9%).  No further broad verification runs are permitted
until implementation and review time bring the cumulative ratio below 10%.

## Iteration 1: freeze the source-frontend contract

The portable Rust IR contract is fixed in `docs/rust-ir.md`.  It defines the
HIR/IR/Machine IR boundaries, verifier and editor rules, object-frontend change
map, and the one sanctioned post-allocation target peephole.  This is the
semantic contract for the port; the Rust implementation will not translate the
Python MIR data structures.

Two provenance-complete external gates were added for later differential use:

- `tools/qrender_gate.py` emits every QB module once, records all link inputs,
  checks the benchmark and four existing qrender oracles, and refuses footprint
  regressions.
- `tools/gorillas_gate.py` derives a pinned deterministic graphics probe from
  Microsoft's original source, compares BC45 and direct-frontend behavior, and
  refuses BASIC-owned or complete linked-code footprint regressions.

The user explicitly deferred qrender because its external project is not yet a
working oracle.  No qrender compilation or runtime result is claimed.  A real
Gorillas run was also stopped after 40.20 seconds during allocation when the
verification-budget instruction was reiterated; its partial receipt is not a
passing result and runtime execution remains deferred.

Stage capture no longer recompiles a diagnostic reconstruction.  The optional
QB compiler observer records HIR, both MIR boundaries, LIR, each machine phase,
final allocated LIR, and the exact assembly model from the same invocation that
emits the OMF object.  Its focused regression proves that observing the pipeline
does not change the emitted object bytes.

Focused verification for the new instruments, including delegated fail-first
and primary-review runs, consumed about 52 seconds.  Together with Iteration 0,
cumulative verification is roughly 380 seconds over 6,213 seconds of elapsed
work (about 6.1%).  No broad suite or external runtime gate was completed in
this iteration.

The qrender runtime criterion is therefore a named deferred acceptance item,
not evidence of correctness.  Per the user's direction, the port proceeds to
the dedicated branch and keeps future checks scoped to the subsystem being
implemented.
