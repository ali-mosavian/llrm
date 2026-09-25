# Source layout

| Path | Holds |
| --- | --- |
| `crates/llrm-support` | Helpers every crate shares: Python-compatible repr and JSON, hashing, code pages, diagnostics |
| `crates/llrm-cycles` | The instruction-cost model |
| `crates/llrm-omf` | OMF records, the code segment as a module, CodeView debug info |
| `crates/llrm-core` | HIR, MIR, the optimizer, the x86 backend, and BC raising |
| `crates/llrm-nib` | The Nib frontend and language server; its runtime, `std` and `abi` modules |
| `crates/llrm-qb` | The QB-family frontend: driver, inline x87, stage dumps |
| `crates/llrm-c` | C through Open Watcom's front end (`toolchain/owshim/`) |
| `crates/qbfront` | The QB parser, run by `llrm-qb` as a program, and its compatibility corpus |
| `src/bin` | The `llrm-*` tools, `nibfront` and `nib-lsp` (package `llrm`) |
| `tests` | Integration tests, fixtures, and the BASIC suite |
| `bench` | Benchmark programs |
| `examples` | Nib, BASIC, Pascal and C interop programs |
| `editors` | The Zed extension and tree-sitter grammar |
| `toolchain` | What the build scripts bootstrap: `wccq` (`owshim`), jwasm, jwlink, DOSBox-X |
| `tools` | Developer tools; see `tools/readme.md` |

`llrm-core` depends on `llrm-support`, `llrm-cycles` and `llrm-omf`, and
re-exports them as `support`, `cycles` and `objectfile`. The frontends
depend on `llrm-core` and never on each other. `LLRM_ROOT`
(`.cargo/config.toml`) is the repository root for any crate's tests.

In `llrm-core`, `flow.rs` is the pipeline; `rewrite.rs` and `wholeseg.rs`
rewrite BC objects. The rest is grouped by responsibility:

| Module | Responsibility |
| --- | --- |
| `hir` | The common HIR: model, codec, verifier, lowering to MIR |
| `model` | MIR, LIR, decoded IR, floating semantics, phase interfaces |
| `analysis` | SSA, liveness, ranges, loops, induction, memory/value facts |
| `optimize` | MIR transformations |
| `backend` | Lowering, instruction selection, allocation, frame/layout, peepholes, object writing |
| `abi` | Runtime contracts, `runtime.toml`, and the QB runtime ABI (`abi::qb`) |
| `frontends::bc` | BC objects: decode, partition, recognize BC idioms, raise SSA values |
| `legacy` | Older lifting and call absorption still shared by raising |

This organization makes ownership visible; it does not claim the architectural
migration is finished. Existing dependency cycles and machine-aware MIR
transformations remain debt documented in `split.md`. Recognition belongs in
the frontend, machine-independent optimization above lowering, and physical
placement in the backend. `model::ir` is the older decoded machine
representation, not the machine-independent MIR contract.
