# What the landing gate runs, and what it does not

Counts are `#[test]` attributes in the crate's sources at feat/parity a0843c575 (lib: under src/, integration: under tests/); not a `--list` of a build.

## Layers of the landing gate (tools/parity_squash.py, tools/parity_smoke.py)

Every layer prints its failures as names and the gate compares them with the head's, cached per head sha and never for the PR's own run. A name the head does not fail refuses the PR; names the head fails and the PR fixes print `FIXED:`.

| layer | PR | runs on a PR that touches | what it protects | measured cost |
|---|---|---|---|---|
| step names | #3503 | any | a new failing test inside an already red step | none |
| tests/run groups | #3522 | llrm-qb, qbfront, llrm-qbruntime, runtime/qb, llrm-x86, llrm-core | the printed output of every BASIC, C and Nib program | 75 s alone, 252 s at load |
| tests/qbrt differential | #3539 | the same | the QB runtime against the MS runtime, 218 results | 67 s at load 7 |
| QCport 65 modules x 4 levels | #3526 | crates/ | a compile failure of valid C | 3 s |
| lib tests of the dependants | #3542 | a crate (closure from `cargo metadata`) | a red lib test in a crate that depends on the changed one | debug profile as the gate builds it: 18 s build after a change in llrm-transforms (76 s cold) + 28 s run, load 18 |
| mapped root integration targets | #3542 | a crate (targets whose imports and compilers include it) | pinned asm and tool tests; a new unmapped target fails its test | 40-47 s run; 109 of 116 targets for a transforms or core change, 22 for llrm-nib |
| fixed steps, 11 ratchets, builds, fmt, docs | | any | | about 7 s of CPU |
| nightly integration (not per PR) | #3542 | all 126 binaries, `tools/gate/nightly_integration.py` | turbo (138 s), cv4 (30 s), run (70 s) and every target, names to the board | 275 s |
| stamps | #3545 | | a measurement from another sha: `llrm-c --version`, `tools/stamp.py --expect-sha` | none |

## Not covered

- Lib tests of crates that do not depend on the changed one.
- turbo, cv4 and run integration targets per PR (nightly only).

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

## Gaps, numbered

| id | what is not run | measured / estimated cost | what covers it now | proposal |
|---|---|---|---|---|
| G1 | integration tests under `crates/*/tests/*.rs` of the changed crates: the layer runs `cargo test -p <crate> --lib` only | `--tests` on the changed crate, debug, load 25-34 (2026-10-23): llrm-core 58 s, llrm-analysis 19 s, llrm-mir 29 s, llrm-transforms 1133 s (queue plus a cold test build; the tests themselves ran in under 4 s) | `tools/gate/nightly_integration.py` runs the workspace targets once a night | map each crate integration target to the files it imports (as #3542 did for the root targets) and run only the targets a PR's files reach; decision: oversight |
| G2 | llrm-mir's scan tests (`no_file_gains_*`) read every crate's sources; a PR that changes no llrm-mir file never ran them (extload.rs, booleans.rs, selectiondag/: three reds on parity) | llrm-mir lib alone: 23 s | the hourly sweep names the red after it lands | closed by this PR: any `.rs` under `crates/` also runs llrm-mir's lib tests |

## Ratchets with no per-test names

`ratchet:docs`, `ratchet:prepush-docs|facts|fmt|scans|test docs_links`, `ratchet:warnings` and `workspace` print no test names; a second failure inside one is hidden. Each needs to print its items or be split.

## Measured (tinytim, 2026-10-22)

- Root crate integration targets: 126 binaries, run 275 s (turbo 138 s, run 70 s, cv4 30 s, the other 123 about 37 s); rebuilding all of them after a transforms change 277 s at load 14. 63 tests in 40 targets are red on parity (names: `tools/gate/nightly_integration.py`).
- Each target is mapped from what it imports and the compilers it runs (`parity_smoke.integration_map`; a new target nobody can map fails its test). A PR runs the targets whose crates include a touched crate, the 11 scanners, and a target whose own file changed. A llrm-transforms or llrm-core change selects 109 targets (every compiler depends on them); a llrm-nib change 22; qbfront 34. turbo, cv4 and run are nightly only (`tools/gate/nightly_integration.py`, report to the board).
- tests/run groups: 75 s alone, 252 s at load; qbrt `--all` 67 s at load 7; QCport 65x4 3 s.
