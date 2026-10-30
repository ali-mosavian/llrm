---
architecture-index: source layout
architecture-order: 20
---
# Source layout

For contributors finding where something lives. Every top-level directory and
every crate in the workspace, grouped by pipeline stage; dependency edges are
in [readme.md](readme.md).

```
 repo
 +-- crates/  support ir opt frontends backend driver target   (Rust workspace)
 +-- src/bin/ the command-line tools, one file each
 +-- runtime/ the per-language runtime sources and descriptions
 +-- tests/ bench/ examples/ tools/ toolchain/ editors/ docs/
```

## crates/

| Path | Holds |
| --- | --- |
| `support/llrm-alloc` | The compilers' global allocator (`Slab`): per-thread free lists by size class over mimalloc; the binaries declare it |
| `support/llrm-env` | The one reading of an `LLRM_*` switch: off when unset, empty, `0`, `false` or `off` |
| `support/llrm-support` | Helpers every crate shares: hashing, JSON and repr helpers, code pages, debug channels and diagnostics |
| `ir/llrm-hir` | The common HIR: model, JSON codec, verifier, escape facts, emission to MIR, `hir-mir` |
| `ir/llrm-mir` | MIR: types, text, verifier, interpreter, dominators, loops, pass manager. No dependencies |
| `ir/llrm-lir` | The machine operand model (`Reg`, `Mem`, `Operation`, `Semantics`) shared by the backend and the BC lift |
| `opt/llrm-analysis` | MIR analyses: alias and points-to, MemorySSA, ranges, induction, summaries, GlobalsAA |
| `opt/llrm-transforms` | MIR to MIR passes and the pipeline |
| `frontends/qbfront` | The QB, QBasic, PDS and VBDOS parser and resolver, generated from `grammar/` |
| `frontends/llrm-qb` | The `llrm-qb` driver: dialects, inline runtime, stage dumps |
| `frontends/llrm-qbruntime` | QB runtime routine contracts (`runtime.toml`) |
| `frontends/nib` | The Nib frontend: lexer, parser, semantics, std, runtime, and the language server (`lsp`) |
| `frontends/llrm-c` | C through Open Watcom's front end (`toolchain/owshim/`): capture stream and translation to HIR |
| `driver/llrm-bcdriver` | Compiles objects BC wrote: lift, optimise, select, write a fresh object |
| `frontends/bcobj` | The BC object frontend: x86 decode, blocks and runtime ABI facts, and the lift of a BC object to MIR |
| `codegen/llrm-codegen` | The compile driver, instruction selection, machine phases (`flow.rs`), layout, `objbuild`, the HIR interpreter, ABIs |
| `driver/llrm-driver` | The one place that names targets and binds each to its selector and rules |
| `backend/llrm-iselgen` | Build-time generator of the instruction selector from `patterns.isel` |
| `backend/llrm-peepgen` | Build-time generator of peephole automata from `.peep` rules |
| `target/llrm-target` | The target-generic layer: what a target description is |
| `target/llrm-x86` | What every x86 target shares: instruction family, selection family, peephole rules |
| `target/llrm-x86-m16` | 16-bit real-mode x86: registers, timings, calling, datalayout, object and DOS descriptions, `Dos` cost model |
| `target/llrm-x86-m16-select` | Selector and peephole rules generated for m16 |
| `target/llrm-x86-m32` | Flat 32-bit x86, the same descriptions |
| `target/llrm-x86-m32-select` | Selector and peephole rules generated for m32 |
| `mc/llrm-object` | The format-neutral `Object` the backend produces ([object model](../formats/object-model.md)) |
| `mc/llrm-omf` | OMF records, writer and reader, CodeView debug info |
| `target/llrm-elf`, `llrm-elf32`, `llrm-elf64` | ELF common part and the 32- and 64-bit writers |
| `target/llrm-coff`, `llrm-coff32`, `llrm-coff64` | COFF common part and the 32- and 64-bit writers |
| `mc/llrm-macho` | Mach-O writer |
| `mc/llrm-dwarf` | DWARF 4 and 5 debug sections |

## Other top-level paths

| Path | Holds |
| --- | --- |
| `src/bin` | `llrm-qb`, `llrm-nib`, `llrm-c`, `llrm-omf`, `llrm-mir`, `llrm-run`, `nibfront`, `nib-lsp` (package `llrm`) |
| `build.rs` | Builds jwasm, jwlink and the headless DOSBox-X beside the binaries (features `jw`, `dosrun`) |
| `runtime/` | `qb` (the QB runtime in C), `c` and `nib` (per-target `x86-m16`, `x86-m32` descriptions and startup), `shared` (DOS facts and OS descriptions) |
| `tests/` | Integration tests (`*.rs`, `test_*.py`), `run` and `check` suites per language, `differential`, `inputs`, `fixtures`, host shims (`hostalloc`, `hostgfx`) |
| `bench/` | Benchmark programs and the reference compilers' counts (`expected.toml`); see `bench/readme.md` |
| `examples/` | Nib, BASIC, Pascal, C interop and `dot` example programs |
| `tools/` | Developer tools: `bench`, `loops`, `e2e`, `dosbatch`, `gate`, `ci`, `callconv`, `torture`, `qcport`, `analysis`, `rfmt-post`, and scripts; see `tools/readme.md` |
| `toolchain/` | Bootstrap scripts and `owshim`, the patched Open Watcom capture |
| `editors/` | The Zed extension and the Nib tree-sitter grammar |
| `docs/` | These documents |
| `.cargo/config.toml` | Sets `LLRM_ROOT`, the repository root for any crate's tests |
| `.github/` | CI workflows |
