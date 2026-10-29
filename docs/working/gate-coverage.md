# What the landing gate runs, and what it does not

Counts are `#[test]` attributes in the crate's sources at feat/parity a0843c575 (lib: under src/, integration: under tests/); not a `--list` of a build.

## Layers of the landing gate (tools/parity_squash.py, tools/parity_smoke.py)

Every layer prints its failures as names and the gate compares them with the head's, cached per head sha and never for the PR's own run. A name the head does not fail refuses the PR; names the head fails and the PR fixes print `FIXED:`.

| layer | PR | runs on a PR that touches | what it protects | measured cost |
|---|---|---|---|---|
| step names | #3503 | any | a new failing test inside an already red step | none |
| tests/run groups | #3522 | llrm-qb, qbfront, llrm-qbruntime, runtime/qb, llrm-x86, llrm-codegen | the printed output of every BASIC, C and Nib program | 75 s alone, 252 s at load |
| tests/qbrt differential | #3539 | the same | the QB runtime against the MS runtime, 218 results | 67 s at load 7 |
| QCport 65 modules x 4 levels | #3526 | crates/ | a compile failure of valid C | 3 s |
| lib tests of the dependants | #3542 | a crate (closure from `cargo metadata`) | a red lib test in a crate that depends on the changed one | debug profile as the gate builds it: 18 s build after a change in llrm-transforms (76 s cold) + 28 s run, load 18 |
| mapped root integration targets | #3542 | a crate (targets whose imports and compilers include it) | pinned asm and tool tests; a new unmapped target fails its test | 40-47 s run; 109 of 116 targets for a transforms or core change, 22 for llrm-nib |
| fixed steps, 11 ratchets, builds, fmt, docs | | any | | about 7 s of CPU |
| nightly integration (not per PR) | #3542 | all 126 binaries, `tools/gate/nightly_integration.py` | turbo (138 s), cv4 (30 s), run (70 s) and every target, names to the board | 275 s |
| stamps | #3545 | | a measurement from another sha: `llrm-cc --version`, `tools/stamp.py --expect-sha` | none |

## Not covered

- Lib tests of crates that do not depend on the changed one.
- turbo, cv4 and run integration targets per PR (nightly only).

| crate | lib tests | integration targets | integration tests |
|---|---|---|---|
| llrm | 6857 | 116 | 645 |
| llrm-transforms | 2185 | 6 | 10 |
| llrm-codegen | 1433 | 6 | 10 |
| llrm-analysis | 867 | 6 | 6 |
| llrm-nib | 427 | 0 | 0 |
| llrm-mir | 422 | 2 | 3 |
| qbfront | 211 | 7 | 166 |
| llrm-qb | 363 | 0 | 0 |
| llrm-cc | 181 | 0 | 0 |
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
| llrm-x86-m64 | 2 | 0 | 0 |
| llrm-elf-arm64 | 2 | 0 | 0 |
| llrm-env | 1 | 0 | 0 |

## Gaps, numbered

| id | what is not run | measured / estimated cost | what covers it now | proposal |
|---|---|---|---|---|
| G1 | integration tests under `crates/*/tests/*.rs` of the changed crates: the layer runs `cargo test -p <crate> --lib` only | `--tests` on the changed crate, debug, load 25-34 (2026-10-23): llrm-codegen 58 s, llrm-analysis 19 s, llrm-mir 29 s, llrm-transforms 1133 s (queue plus a cold test build; the tests themselves ran in under 4 s) | `tools/gate/nightly_integration.py` runs the workspace targets once a night | map each crate integration target to the files it imports (as #3542 did for the root targets) and run only the targets a PR's files reach; decision: oversight; measured 2026-10-23: of the 123 root targets only 3 import a crate directly (llrm-codegen 2, llrm-mir 1); the other ~109 reach a crate through the compilers they run, so a per-crate map selects all of them (275 s warm): the choice is the whole set at landing or the nightly run, oversight's; ruling 2026-10-23 (coordinator, user order: land tempo first): (b), scanners and own-file targets at landing, the rest nightly, and the nightly report names the PR and owner of a red one (`nightly_integration.attribution`); revisit (a), the whole set at landing, when the backend parts have landed: 2026-11-30; the same date decides whether the landing runner also runs the tools pytest (it does not: the `QCPORT SKIPPED` rule, `tools/gate/pytest_qcport.py`, bites in the nightly and gate pytest, and the morning report names it) |
| G2 | llrm-mir's scan tests (`no_file_gains_*`) read every crate's sources; a PR that changes no llrm-mir file never ran them (extload.rs, booleans.rs, selectiondag/: three reds on parity) | llrm-mir lib alone: 23 s | the hourly sweep names the red after it lands | closed by this PR: any `.rs` under `crates/` also runs llrm-mir's lib tests |
| G3 | a translated pass that passes every unit test and never fires in a real compile (#3732's JumpTableToSwitch: x_funcptr asm identical before and after; the compile runs pipeline.rs's per-body path, not the manager the tests used) | one pipeline-level test per slot | nothing | `precheck.sh`: a PR that adds a pass slot (registry entry or phase-table row) must add a `#[test]` in `pipeline*_tests.rs` or `tests/` showing the pass fires on a real input |
| G4 | feature-gated lib tests: llrm-codegen's `fixtures` gates no extra test (1588 lib tests with and without it, 636 s to build cold) | `cargo test --release -p llrm-cc --lib --features toolchain` on a warm target: 12 s (2 s of tests); the cold build is the cost | the hourly sweep compiles tests/fixtures/callconv/c/*.c since #3812 | closed by this PR: llrm-cc's lib run in the layer uses the feature (`parity_smoke.LIB_FEATURES`), and a change under crates/ir/llrm-hir or crates/frontends/llrm-c runs it |
| G5 | root scanner targets (`tests/*.rs` that read sources and docs) at landing: the fast layer ran lib tests only, and `selectiondag_registry` (and later `no_partial_ecx_counts`) were in no map: #3899 landed with SD403 rowless, #3888 with an unmapped target | 13 scanner targets: 89 s cold on a loaded host; red on parity itself today: docs_links 3, no_local_paths 2, descriptions_are_read 1 (cached per head, so only new reds refuse) | `test_every_integration_target_is_mapped` (a pytest, run only when tools/ changes) | closed by this PR: `SCANNERS` lists both, and the layer runs the scanners and any `tests/*.rs` the batch changes (`parity_squash.new_scanner_failures`) |


## Inventory of every test target (2026-10-26, parity 0aea1e46a0, `tools/test_inventory.py`)

`cargo test --release --workspace --no-run --message-format=json` lists 227 test executables. The gate runs, per batch: the 42 lib targets of the packages a batch touches and their
dependants, the 13 root scanners (`SCANNERS`, the always-on ones every batch, the rest with the lib tests) and a root test file the batch changes. It never runs the other
169: 152 `tests/*.rs` targets (123 in the root crate `llrm`, 29 in other packages such as `llrm-qb`'s `parser`, `llrm-codegen`'s and `llrm-mir`'s) and 17 bins' unit tests; three more are
nightly-only (turbo, cv4, run). Nothing schedules `tools/gate/nightly_integration.py`, and it covers the root crate only. So `greedy_inline` sat red from 7ee16741c (10-24) with no
step looking at it.

Running every unrun target once (172 executables, `nice`, cwd as cargo gives it): 320 s in all, 50 s without turbo (161 s), run (79 s) and cv4 (30 s); 48 of the 172 were red,
3 of them listed in `xfail.toml`. `midend` and `nearcalls` were checked with `cargo test` and fail there too. Red: llrm-arm64/midend, llrm-bcdriver/driver, llrm-dagisel-gen/stale, llrm-support/no_uncached_env_checks, llrm/attribute, llrm/called_once, llrm/check, llrm/coalesce_hot_copies, llrm/cv4, llrm/demanded_bits, llrm/differential, llrm/division_multiplier, llrm/division_nonnegative, llrm/dwarf, llrm/env_read_once, llrm/flags, llrm/flat_foreign, llrm/frame_address_flat, llrm/frame_relayout, llrm/greedy_inline, llrm/header_copy, llrm/header_mir, llrm/jump_threading, llrm/listings, llrm/loop_rotation, llrm/loopidioms_name, llrm/lsr_candidates, llrm/masked, llrm/narrow_counter_offset, llrm/nearcalls, llrm/o0_phases, llrm/o1_fill, llrm/object_trips, llrm/phi_copies, llrm/register_convention, llrm/rfp_copies, llrm/seeded_maps, llrm/signed_counters, llrm/stack_alignment, llrm/step_flags, llrm/switch_lowering, llrm/target_facts, llrm/timing, llrm/toolchain, llrm/turbo, llrm/unroll_budget, llrm/unroll_early_exit, llrm/word_ops_promote.

Per batch is not the place: building the test executables after a change in core or transforms is the cost (3 m 21 s for the whole workspace, warm target, load 20-30), more than the
runs. The hourly sweep is: it builds them once per tip and runs the 169 unrun targets minus turbo, cv4 and run (about 50 s), so a red like this is a sweep red within the hour
(`unrun:<package>/<target>` in the sweep line, a new one against the previous run). A target that is cheap and mapped to the touched crates (`integration_map`) can be added to a
batch later, if its rebuild is shared with the lib tests' build.

## Ratchets with no per-test names

`ratchet:docs`, `ratchet:prepush-docs|facts|fmt|scans|test docs_links`, `ratchet:warnings` and `workspace` print no test names; a second failure inside one is hidden. Each needs to print its items or be split.

## Measured (tinytim, 2026-10-22)

- Root crate integration targets: 126 binaries, run 275 s (turbo 138 s, run 70 s, cv4 30 s, the other 123 about 37 s); rebuilding all of them after a transforms change 277 s at load 14. 63 tests in 40 targets are red on parity (names: `tools/gate/nightly_integration.py`).
- Each target is mapped from what it imports and the compilers it runs (`parity_smoke.integration_map`; a new target nobody can map fails its test). A PR runs the targets whose crates include a touched crate, the 11 scanners, and a target whose own file changed. A llrm-transforms or llrm-codegen change selects 109 targets (every compiler depends on them); a llrm-nib change 22; qbfront 34. turbo, cv4 and run are nightly only (`tools/gate/nightly_integration.py`, report to the board).
- tests/run groups: 75 s alone, 252 s at load; qbrt `--all` 67 s at load 7; QCport 65x4 3 s.
