# Shared enablers cut from the m68k spike

Each is a goal PR of its own on `release/next` (draft, WIP). Dependencies come from `Cargo.toml` and `use` greps at
`feat/m68k-spike`; none has yet been built alone on a clean `release/next` (that is each PR's gate).
Everything below `release/next` 974d839ee0 is absent there: `llrm-spike-mf`, `llrm-sdag`, `target_calling.rs`,
`tools/dagisel/{regen-target.sh,asmtable.py,enctable.py}`.

| # | Enabler | Paths | Commits (spike branch) | Needs | Used by | Behaviour risk |
|---|---|---|---|---|---|---|
| 1 | `llrm-spike-mf` (MFunction, colouring allocator, placement, sink, spill, reschedule) | `crates/backend/llrm-spike-mf` (new, 19 files) | 25, `git log -- crates/backend/llrm-spike-mf` | `llrm-lir`, `llrm-mir` only | m68k, 6502 (its own edits, ~40 conflicts), wasm via m68k | none: a new crate nothing links yet |
| 2 | `llrm-sdag` (DAG type and action tables out of `llrm-x86`) | `crates/codegen/llrm-sdag`, `llrm-x86/src/dag`, `llrm-codegen` `*_with` hooks | b1033b4312, 3b5a80face, 3f41951400 | `llrm-support` | x86 (moves its code), arm64, m68k, wasm | x86 code moves: byte-identity sweep with control, then the batch judge |
| 3 | `TargetCalling` and the narrow `BuilderArch` | `selectiondag/target_calling.rs`, `builder.rs` | a2bcbe44f6, 809b82b2ff, 78c9ae6ae6 | 2 (the builder uses the sdag tables) | m68k `call.rs`, arm64 `dag/call.rs` | x86 builder paths refuse without a full Target: sweep x86 |
| 4 | tooling: `.td` target regeneration | `tools/dagisel/*`, `llrm-dagisel-gen` | 4c8c9eba56, 993d75da8a, ea39714cbc, 282c5d6d13, db128ea23b (+ 24b5dc89ef, aaf0314a31 for asm/enc tables) | none | wasm, any `.td` target, arm64's move to the pin's `.td` | none for x86: generator only; regenerated x86 tables must be byte-identical |

Order: 1 first (independent, 6502 conflicts there), then 2, then 3 right after, 4 with whichever first needs a `.td`
target. Items 1 and 4 can go in parallel with 2.

Entangled and not an enabler: the `feat(arm64)` commits on the spike branch (b5a265ae3d, b2a6e60ba4, 5c1234f8fe,
afd2749328, ...) belong to the arm64 goal; `dagisel` predicates and hooks for AArch64 (3a43e2eae8, 3054183fe1,
df365a2c92) likewise. A series that needs them is not clean.

After 1 to 4 land, the m68k goal PR is the remainder: `llrm-m68k`, `llrm-dagisel` m68k tables, `llrm-cc` rows, tests.
Re-measure its commit count then.
