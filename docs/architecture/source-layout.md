# Source layout

The crate root holds pipeline orchestration: `src/flow.rs`, `src/rewrite.rs`
and `src/wholeseg.rs`. Everything else is grouped by responsibility:

| Module | Responsibility |
| --- | --- |
| `src/bin` | The `llrm-*` tools and `nibfront` |
| `src/frontends/qb` | QB-family driver, HIR-to-MIR ABI, inline x87 |
| `src/frontends/nib` | The llrm language: lexer, parser, semantics, HIR |
| `src/frontends/c` | C through Open Watcom's front end (`owshim/`): its code-generator stream, raised to MIR |
| `src/frontends/bc` | BC objects: decode, partition, recognize BC idioms, raise SSA values |
| `src/hir` | The common HIR: model, codec, verifier, lowering to MIR |
| `src/objectfile` | OMF records, module metadata, relocation |
| `src/model` | MIR, LIR, decoded IR, floating semantics, phase interfaces |
| `src/analysis` | SSA, liveness, ranges, loops, induction, memory/value facts |
| `src/optimize` | MIR transformations |
| `src/backend` | Lowering, instruction selection, allocation, frame/layout, peepholes, object writing |
| `src/abi` | Runtime contracts and the adjacent `runtime.toml` data file |
| `src/legacy` | Older lifting and call absorption still shared by raising |
| `src/cycles` | Instruction-cost model |

The QB parser is its own crate in `frontends/qb/` (`qbfront`). `qbopt/` is the
Python predecessor; every Rust module mirrors its path there, and
[the port map](../history/port-map.md) pairs them.

This organization makes ownership visible; it does not claim the architectural
migration is finished. Existing dependency cycles and machine-aware MIR
transformations remain debt documented in `split.md`. Recognition belongs in
the frontend, machine-independent optimization above lowering, and physical
placement in the backend. `model::ir` is the older decoded machine
representation, not the machine-independent MIR contract.
