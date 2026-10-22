# What the landing gate runs, and what it does not

Counts are `#[test]` attributes in the crate's sources at feat/parity a0843c575 (lib: under src/, integration: under tests/); not a `--list` of a build.

## Layers of the landing gate (tools/parity_squash.py, tools/parity_smoke.py)

- Step names (#3503): every red step is compared with the head by failing test or program name, not by step.
- tests/run (#3522): a PR touching llrm-qb, qbfront, llrm-qbruntime, runtime/qb, llrm-x86 or llrm-core runs all 8 groups.
- QCport (#3526): a PR touching crates/ compiles all 65 modules at 4 levels.
- Fixed steps (programs, sc, d_faces, qc_d_faces, qc_modules, run), 11 ratchets, workspace and warnings builds, fmt, docs: every PR.
- Unit tests: only the lib tests of the crates the PR touches, cut at a CPU allowance (parity_smoke.toml has no ranking, so all of them, cut).

## Never run by the gate

- The lib tests of every crate the PR does not touch: a change in llrm-transforms never runs llrm-core's 1433 (five isel tests were red on parity unseen).
- Integration tests (`tests/*.rs` targets) except the 8 named ratchets: the root crate has 116 targets with 645 tests; the 11 ratchets run 8 of them.

| crate | lib tests | integration targets | integration tests |
|---|---|---|---|
| llrm | 6857 | 116 | 645 |
| llrm-transforms | 2185 | 6 | 10 |
| llrm-core | 1433 | 6 | 10 |
| llrm-analysis | 867 | 6 | 6 |
| llrm-nib | 427 | 0 | 0 |
| llrm-mir | 422 | 2 | 3 |
| qbfront | 211 | 7 | 166 |
| llrm-qb | 363 | 0 | 0 |
| llrm-c | 181 | 0 | 0 |
| llrm-x86 | 141 | 0 | 0 |
| llrm-x86-bcmachine | 129 | 0 | 0 |
| buildprs | 121 | 0 | 0 |
| llrm-hir | 81 | 0 | 0 |
| llrm-omf | 81 | 0 | 0 |
| llrm-arm64 | 42 | 3 | 32 |
| llrm-qbruntime | 63 | 1 | 1 |
| llrm-target | 56 | 0 | 0 |
| llrm-x86-bc | 1 | 3 | 32 |
| llrm-driver | 31 | 0 | 0 |
| llrm-x86-m16 | 29 | 0 | 0 |
| llrm-support | 17 | 2 | 2 |
| llrm-lir | 14 | 3 | 4 |
| llrm-coff32 | 10 | 1 | 6 |
| llrm-x86-m32 | 15 | 0 | 0 |
| llrm-dwarf | 14 | 0 | 0 |
| llrm-bcdriver | 0 | 1 | 14 |
| llrm-alloc | 0 | 1 | 8 |
| llrm-macho | 7 | 0 | 0 |
| llrm-peepgen | 7 | 0 | 0 |
| llrm-elf32 | 6 | 0 | 0 |
| llrm-iselgen | 6 | 0 | 0 |
| llrm-elf64 | 5 | 0 | 0 |
| llrm-object | 3 | 0 | 0 |
| llrm-coff64 | 3 | 0 | 0 |
| llrm-instr | 2 | 0 | 0 |
| llrm-x86-m64 | 2 | 0 | 0 |
| llrm-elf-arm64 | 2 | 0 | 0 |
| llrm-env | 1 | 0 | 0 |

## Smallest addition inside the budget

Run the lib tests of every crate that depends on a touched crate, names vs the head (measured: llrm-core, llrm-hir, llrm-mir and llrm-x86 together run in 5 s once built; the cost is building their test binaries). Adding integration targets is a separate decision for oversight.

## Ratchets with no per-test names

`ratchet:docs`, `ratchet:prepush-docs|facts|fmt|scans|test docs_links`, `ratchet:warnings` and `workspace` print no test names; a second failure inside one is hidden. Each needs to print its items or be split.
