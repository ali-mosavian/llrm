# Source layout

| Path | Holds |
| --- | --- |
| `crates/support/llrm-support` | Helpers every crate shares: Python-compatible repr and JSON, hashing, code pages, diagnostics |
| `crates/ir/llrm-hir` | The common HIR: model, JSON codec, verifier, escape facts |
| `crates/ir/llrm-mir` | The rich portable MIR of `rich-mir.md`: types, verifier, text, interpreter |
| `crates/opt/llrm-analysis` | MIR analyses, and `graph`: dominance and loops over any IR's blocks |
| `crates/opt/llrm-transforms` | MIR to MIR passes and the pipeline |
| `crates/target/llrm-x86-code16` | The 16-bit x86 target: the `Dos` cost model passes see as `llrm_mir::target::Machine`, and the machine description (`machine`) |
| `crates/target/llrm-omf` | OMF records, the code segment as a module, CodeView debug info |
| `crates/backend/llrm-core` | The compile driver, instruction selection, the machine phases and object writing; the HIR interpreter |
| `crates/bc/llrm-bcmachine` | BC objects: x86 decode, the instruction model and its classification (`model::ir::lift`), object reading, flags |
| `crates/bc/llrm-bc` | The BC object frontend: machine code raised onto MIR |
| `crates/bc/llrm-bcdriver` | The rich route for BC objects |
| `crates/frontends/llrm-qbruntime` | The QB-family runtime's `B$` routine contracts and `runtime.toml` |
| `crates/frontends/llrm-nib` | The Nib frontend and language server; its runtime, `std` and `abi` modules |
| `crates/frontends/llrm-qb` | The QB-family frontend: driver, inline x87, stage dumps |
| `crates/frontends/llrm-c` | C through Open Watcom's front end (`toolchain/owshim/`) |
| `crates/frontends/qbfront` | The QB parser, run by `llrm-qb` as a program, and its compatibility corpus |
| `src/bin` | The `llrm-*` tools, `nibfront` and `nib-lsp` (package `llrm`) |
| `tests` | Integration tests, fixtures, and the BASIC suite |
| `bench` | Benchmark programs |
| `examples` | Nib, BASIC, Pascal and C interop programs |
| `editors` | The Zed extension and tree-sitter grammar |
| `toolchain` | What the build scripts bootstrap: `wccq` (`owshim`), jwasm, jwlink, DOSBox-X |
| `tools` | Developer tools; see `tools/readme.md` |

`llrm-core` re-exports `llrm-support`, `llrm-omf` and `llrm-hir` as
`support`, `objectfile` and `hir::{model, codec, verify, escape}`. The frontends depend on `llrm-core`
and never on each other. `LLRM_ROOT` (`.cargo/config.toml`) is the
repository root for any crate's tests.

In `llrm-core`, `driver` runs a compile and `flow.rs` orders the machine
phases. The rest is grouped by responsibility:

| Module | Responsibility |
| --- | --- |
| `hir` | The HIR interpreter, and `llrm-hir` re-exported |
| `model` | LIR, decoded IR, floating semantics, phase interfaces, and what the backend still reads of the old MIR |
| `analysis` | Loops, intervals, regions and alias facts the backend reads |
| `optimize` | What the backend still reads of the old MIR passes |
| `backend` | Instruction selection, allocation, frame/layout, peepholes, object writing |
| `abi` | Runtime contracts (`llrm-qbruntime`, as `abi::runtime`) and the QB runtime ABI (`abi::qb`) |
| `frontends::bc` | `llrm-bcmachine`'s BC decoding, re-exported |
| `legacy` | `llrm-bcmachine`'s BC long-call shapes, read by selection |

This organization makes ownership visible; it does not claim the architectural
migration is finished. Existing dependency cycles and machine-aware MIR
transformations remain debt documented in `split.md`. Recognition belongs in
the frontend, machine-independent optimization above lowering, and physical
placement in the backend. `model::ir` is the older decoded machine
representation, not the machine-independent MIR contract.
