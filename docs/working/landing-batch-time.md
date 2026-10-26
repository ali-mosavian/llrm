# Where a landing batch's wall clock goes

2026-10-25, from the `sqb-*.log` of the last 40 hours (97 batches) and `queue-runner3.log`; `tools/parity_squash.py` `batch`, `passes_fast`, `merge_batch`. The logs time only the lib tests and the behavioural stage; nothing else is stamped.

The 11:13 batch (16 PRs) took 58 min. Its log shows 3 lib-test runs (363 + 341 + 332 s = 1036 s, 30%), 5 behavioural runs (235 s, 7%), 11 squashes and 6 refusals. The other 37 min are unmeasured: builds, `head_failures`, per-PR offender checks, squashes, waits for `buildslot`.

| Fact | Evidence |
|---|---|
| A pass ends at its first red unit, so every refusal costs another full pass | `batch()` halves or drops offenders and calls `passes` again; 4611: build red, then docs_links red (3 PRs, after 363 s of lib tests), then behavioural red |
| The two always-on scanners (`docs_links`, `no_local_paths`, milliseconds) run after the lib tests | 4611 prints `lib tests ... 363s` then `scanners ... 1 red`: the 363 s bought nothing |
| A lib run is the longest stage and grew 4x | 70-100 s on 10-24 morning, 300-570 s now (more tests, and `buildslot` contention from other sessions) |
| A change under `crates/` re-runs every crate | `dependants(touched)` plus `llrm-mir`'s scan tests: 28 of 28 crates in 4611 |

Long pole: lib tests, multiplied by the passes refusals cause.

Changes that do not weaken the gate (same checks, same PRs, other order or fewer repeats). Agree with w8 before editing `parity_squash.py`:
1. Stamp every stage in the batch log (start, seconds). Without it 37 of 58 min are a guess.
2. Run `docs_links` and `no_local_paths` first, and `cargo check` per PR (`precheck.sh`), before the lib tests. A scanner or build red then costs seconds, not a pass.
3. Run the behavioural stage beside the lib tests (separate checkout of the same tree); it needs the release binaries, the lib tests the debug ones.
4. On a retry after dropping a PR, reuse a crate's lib result when the hash of its sources and its dependencies' equals that of a run that passed on the same base.
