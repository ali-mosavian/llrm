# CI is the gate

A green CI run means what `gate.py run` passing locally means: the same planner, the same `tiers.toml`, the same steps and tiers. No step is skipped for a missing tool; a step that cannot run fails the job by name.

## Rules

1. CI asks `gate.py plan --json` for its jobs. It lists no step itself. A test builds the plan of a diff both ways (what `gate.py run` would run, what the workflow's jobs run) and compares them.
2. `GATE_ALLOW_MISSING` is gone from the workflow. A capability missing on a runner is a failure, not a skip.
3. The run's last job prints `GATE <tier> PASS at <head sha>`, or `FAIL: <groups>`, the line `gate.py run` prints.
4. Per merge candidate CI tests that PR merged with main as it is then. Batch interactions are the periodic full gate on main's job and the merger's premerge against current main.

## The merge candidate

The gate runs in CI, and only to merge into main. A push runs nothing; a pull request into `feat/parity` runs nothing; a pull request into main runs nothing until the merge loop (`cimerge.sh`) puts the label `merge-candidate` on it, one pull request at a time, the head of the merge order. Any other label starts nothing. That run is the full gate. The label comes off when the pull request merges or loses its place; a push after the label makes the run stale, and the label is put on again for a new run. A push to main keeps the full gate (main records the bench counts).

The `ci` job's line for a pull request is `GATE <tier> CANDIDATE PASS at <head12> on <main12>`; `<main12>` is the main the run tested (the merge commit's first parent). It lets a merge only if `tools/ci/candidate_ok.py <head> <main> <line>` exits 0:

- the line is a candidate PASS (a main push's PASS, a FAIL or a STALE line never merges);
- `<head12>` is the pull request's head now;
- `<main12>` is main now, or every commit since touches only `[inert]` paths (`tiers.toml`: docs, Markdown, editors, `.github`).

A pull request run reads its workflow from the PR's merge ref (`refs/pull/N/merge`), which GitHub does not remake when main moves or a label is added: a ref made before a change to `ci.yml` keeps the old triggers, and the label starts nothing (#2340 after #2358). Before it labels, the loop runs `tools/ci/merge_ref_fresh.py N`; on exit 1 it merges main into the PR's branch (the candidate is the head with main merged anyway), which remakes the ref, and checks again.

The loop cancels a candidate run in flight when main moves by a non-inert commit, so the runner does not spend 55 minutes on a stale main. `tools/ci/test_candidate_ok.py` holds the cases: not a candidate line, another head, moved main, docs-only move.

**No gate runs anywhere else.** Sessions and loops do not run `gate.py`, a full premerge check, the ratchets, `identical.py`, rows, bench or the oracles on tinytim or any host of theirs; those run only in this gate. Before a merge to `feat/parity` a loop builds once per batch and runs the unit tests and a few fast run tests of the touched crates, 10 s of CPU in all (`tools/parity_smoke.py`).

## Runners

Only the groups that need hardware perf counters (`perf`: the scaling tests, `cost`: `measure`) prefer tinytim; every other job, and anything tinytim cannot take, runs on `ubuntu-24.04` (tinytim is shared with every session's builds: the same build took 82-103 s hosted and 243-669 s there). `tools/ci/workflow-version` is bumped by a commit that changes what a run checks or how it judges (the bench judge and waivers, the steps or capabilities a tier runs, the `ci` job's pass logic, the tests of the gate), not by one that only makes it cheaper (jobs, shards, runners, caching): a pull request run whose merge parent lacks the latest bump prints `STALE WORKFLOW (not a pass)`. A pull request that changes the compiler's source and whose `perf` or `cost` job fell back to hosted (no idle tinytim) prints `GATE <tier> PERF SKIPPED at <head12>` and fails: the counter tests judge the compiler, so it is re-run when tinytim is idle. Other changes may skip them. GitHub has no "prefer A, else B", so `plan` chooses: `tools/ci/pick_runners.py` reads the repository variable `TINYTIM_HEARTBEAT` (`<epoch> <idle runners>`, written each minute by the supervisor). Missing, older than 3 minutes or zero idle means hosted, so a dead tinytim never blocks a run. Only a push to main and a pull request from this repository can get tinytim; a fork's pull request, a push to any other branch and a manual run elsewhere are hosted. The idle count can be a minute stale: another run may take the runner before the job starts, and the job then waits for a free one. That wait is not bounded by `timeout-minutes`, and there is no cancel-and-rerun-hosted timeout: accepted. The run summary and the `ci` line (`GATE <tier> PASS at <head12>  [runners: tinytim: ...; hosted: ...]`) say which runner took each job.

The pin's tools other than clang come from one cached tree, `~/.cache/llrm/pintools-<pin sha12>` (`toolchain/pintoolsbuild.sh`, built by the `pintools` job beside `toolchain`, so the clang cache keys are unchanged): `llc` and `opt` for `tools/pairs/pin.py` and `tests/common/pin.rs`, and the source with `llvm-tblgen` for the DAG tables' staleness test (`llrm-dagisel-gen/tests/stale.rs`, which runs `tools/dagisel/regen.sh`). The setup action exports `LLVM_BIN`, `LLVM_SRC` and `LLVM_BUILD` to the unit, integration and python groups (LLVM_BIN also holds the pin's clang, linked in; `LLRM_REQUIRE_PINTOOLS=1` makes the difftest tests that need `opt` red, not skipped); a missing tool is red. Assertions do not change the tables, so the build is Release. Cold, 7 min on a loaded 20-thread host, about 20-25 min estimated hosted (the evidence run gives the number), beside the 1 h clang job.

The two runners are not the same gate on one point: tinytim has `perf`, so `test_scaling*.py` and `measure` run there with `GATE_ALLOW_MISSING` empty; hosted has none and drops them (`GATE_ALLOW_MISSING=perf`). One PR can pass on one and fail on the other until those tests have a counter hosted can read (#1844 and the sharding after it).

tinytim (`tools/ci/selfhosted`, `install.sh`): user `ghrunner`, which cannot read the secrets in `~alim`, runs six slot units (`ghrunner@1..6`, one runner each) and `ghrunner-heartbeat`. Per job the supervisor gets a single-use JIT config from the API and starts a rootless podman container (crun) of the runner image; one job, then the container is gone. The PAT (`/etc/ghrunner/pat`, Administration and Variables on this repository) stays outside the containers. No Docker. Limits (`AllowedCPUs` and `MemoryMax` of each slot unit, since rootless podman ignores `--cpuset-cpus` in the unit's cgroup): 6 slots of 4 threads, each the two SMT threads of two physical cores inside one CCD (tinytim clocks about twice a hosted VM, so 4 threads match 8 hosted); cores 0, 1, 8, 9 (8 threads) stay free; 16 GB each, 24 threads in total, low CPU weight. `perf` works there (host `kernel.perf_event_paranoid=-1`, a seccomp profile that allows `perf_event_open`), so `GATE_ALLOW_MISSING` is empty on tinytim and `perf` only on hosted. The private tools are cloned inside the container (see below) and die with it. "Require approval for all outside contributors" is on.

## Public and private steps

Hosted runners hold the public tools only. What the repository must not hold is in a private repository, `ali-mosavian/llrm-private-tools`, read by a deploy key kept as the secret `LLRM_PRIVATE_KEY`:

| capability (`tiers.toml`) | directory in the private repository | env the gate reads |
|---|---|---|
| turbo | `tcpp30/`, `td/` | `TCPP30_DIR`, `TD_DIR` |
| codeview | `vbdos/`, `tcpp30/` | `VBDOS_DIR`, `TCPP30_DIR` |
| qcport | `qcport/src/`, `qcbcc/`, the d_faces sources | `QCPORT`, `QCPORT_BORLAND` |
| qb45, qb45demos | `qb45/`, `gor/` | `QB45_DIR`, `QB45_DEMOS_DIR` |

A job that needs one checks the repository out into its workspace outside `target/`, only where `github.event.pull_request.head.repo.full_name == github.repository` (and on pushes to main). It never saves it to a cache or an artifact, never prints a file of it (`set +x`, no `ls -R` into a log), and removes it at the end. A job that needs one on a fork PR runs a step that fails with `group X needs the private tools; fork PRs cannot have them`.

A step is wholly public or wholly private, so the private jobs are as fine as the public ones. Steps that are half of each are split in `tiers.toml`, locally too: `pytest` (the files that `REQUIRES` nothing private) and `pytest-qb`, `run` and `run-qb`, `integration` and `integration-qb` (the BC differential binary). Private steps: `turbo`, `cv4`, `qcport`, `dos-slow`, `bench`, `pytest-qb`, `run-qb`, `integration-qb`.

## Instruction counters

Hosted VMs have no `perf` instruction counter. Steps that read one: `measure`, and the scaling tests (`test_scaling_gate.py`, `test_scaling.py`). Until a counter exists on hosted runners they cannot be a CI job; the options, in order of preference:

- **qemu-user with the instruction-count plugin** (`-plugin libinsn`): exact guest instruction counts, at about 5 to 10 times native. `measure` is about 10^12 instructions: ~80 CPU-minutes, split over its axes as a matrix of jobs (each axis and level one job): a few minutes of wall time. The counter becomes a backend of `scaling.sample` (`LLRM_COUNTER=qemu`), chosen for base and head alike; counts differ from `perf`'s by a few per cent, so the baselines are per backend.
- **valgrind `--tool=callgrind`**: exact too, 30 to 100 times native: too slow for `measure`, usable for one cell of the scaling tests.
- Keep them off CI, run by the merger's premerge and the periodic gate on main: a skipped step, which rule 2 forbids.

## Order

1. Main's CI green (#1837).
2. `gate.py plan --json` lists jobs with the capabilities each needs; the workflow runs them; the plan-equality test; the summary line.
3. The step splits in `tiers.toml`.
4. The private repository, the secret, the private jobs (needs the owner of the repository: it holds files whose licences the repository does not).
5. The counter backend and `measure` as a matrix.
6. Owners merge main into their open PRs.

## Debt

A check or waiver that is knowingly missing is a line here, not a silence. Format, one line each, parsed by `tools/ci/test_debt.py` (red on a malformed line or a passed deadline):

`- DEBT <id> | limit <what is allowed to stay> | deadline YYYY-MM-DD | owner <name> | <what and why>`

On the deadline the owner pays it (the check goes red or the test goes) or moves the date in a commit that says why.

- DEBT skip-llrm-pass | limit 2 tests | deadline 2026-11-05 | owner w11 | `tools/difftest/test_difftest.py` skips `llrm-pass is not built`: the python group's pytest step builds no binaries.
- DEBT skip-pindiff-llc | limit 1 test | deadline 2026-11-05 | owner w11 | `tools/pairs/test_pindiff.py` skips `needs LLRM_LLC`: nothing sets it in CI.
- DEBT bench-tile-os-m16-insns | limit tile c -Os insns <= 4182, clocks <= 10224, code_bytes <= 94 | deadline 2026-11-05 | owner w6 | `tile` c -Os is 4182 insns / 10224 clocks / 94 B against main's 4155 / 10169 / 97 (+55 clocks, +0.54%, for -3 B; same harness); cause 7ee16741c1, the LSR default. The pin does not reassociate `((y+61)&63)<<7` (the same `phi i6` either order); main's step-128 IV came from the deleted gcc-style LSR. Fix: a SCEV rule plus LSR formula sharing, clocks back to <= 10169 (days, post-FF). bq16, bq32 and nib `tile` rows keep their own numbers.
- DEBT bench-clocks-lsr-default | limit sieve c/nib/bas and the other class-1 rows of clock_limits.toml at their parity clocks | deadline 2026-11-05 | owner w6 | `-fno-lsr` brings sieve c/nib back to main (24762 and 24724 against 24556 and 24538); ablation candidate 7ee16741c1, first-bad TBD.
- DEBT bench-clocks-loop-header-copy | limit lru c/nib, ring bas, shellsort c at their parity clocks (class 2 of clock_limits.toml) | deadline 2026-11-05 | owner w3 | `-fno-tree-ch` brings lru to main (293 and 572 against 279 and 571); ablation candidate, first-bad TBD.
- DEBT bench-clocks-pre | limit sieve bas, recfloat c at their parity clocks (class 3 of clock_limits.toml) | deadline 2026-11-05 | owner w3 | `-fno-tree-pre` and `-fno-gcse` recover part of sieve/bas (43379 against 46421, main 33601); ablation candidate, first-bad TBD.
- DEBT bench-clocks-quicksort-nib | limit quicksort nib -Os clocks <= 447484 | deadline 2026-11-05 | owner w3 | main 316122: (1) the bounds check on a[i] is not a gap against the pin (pin opt keeps 2-3 per swap; main's none came from analyses the pin lacks); (2) 11 against 4 spill cells in the loop and (3) no pointer strength reduction come from LSR dropping the solution that would spill (LU4, w9 notes); the allocator (w13) is not the cause; amplifiers tree-ch -7% and tree-pre -14% clocks, both pin-faithful.
- DEBT bench-clocks-float | limit the class-4 float rows of clock_limits.toml at their parity clocks (all fast, as main) | deadline 2026-11-05 | owner w2 | float rows: the QB default is now C's fast mode (user ruling; `-fexcess-precision=standard` brings back BC rounding), so fpbench/bas is -0.8% and nbody_single/bas -2.9% against main. What remains above main: mandel/bas +2.9%, nbody/bas +2.6%, nbody/nib +4.0% (the fast nib loop recomputes a scaled index, `mov di,dx; shl di,2`, a missed LICM); post-FF.
- DEBT bench-clocks-unattributed | limit the class-0 rows of clock_limits.toml (4 rows: parity/algebra bas, parity/sum_three nib, particle c, recsum c) at their parity clocks | deadline 2026-11-05 | owner w6 | more clocks than main (854bb7efc4), no ablation yet.
- DEBT bench-clocks-dag-isel | limit the class-5 rows of clock_limits.toml at their parity clocks | deadline 2026-11-05 | owner w12 | (A) pin-faithful, deadline = the greedy switch-over: fib bas, hanoi bas, crc c, crc nib; the old allocator picks callee-saved regs across calls under DAG-shaped code (greedy runs only in shadow), the pin llc on the same IR gives the same push/pop esi/edi (hanoi: no shrink-wrap, the shared ret join gives no restore point), old isel spilled to frame slots; crc: the branchless step is what the pin emits. (B) follows the switch-over: histo c/nib (IV in si, push/pop si), regalloc rows matmul c/nib, parity/qmove bas; parity/branch c/bas, parity/memory bas, parity/sum_three bas are DAG form/layout, not yet checked against the pin. Evidence: w12-abl/fib-plain.ll, hanoi-plain.ll, isel/*.{dag,old}.s
- DEBT bench-clocks-masked-iv | limit tile bas/c/nib, ring c, textfill c/nib, particle bas/nib -Os clocks at their parity values (class 6 of clock_limits.toml) | deadline 2026-11-05 | owner w6 | pin SCEV does not move `shl` through the mask: ring c `and si,1023; shl si,2` (+6000 insns = the 12000-clock gap) where main steps the IV pre-scaled (`and si,4092`); tile the same with `<<7`; textfill c is the same LSR formula family (main counts the IV to zero, `add;jne`). One fix after FF: SCEV rule + LSR formula sharing. Insns/bytes of tile: bench-tile-os-m16-insns. Pin check (ring c, textfill c): the pin's loop-reduce leaves the same `add;cmp;jne` / unreduced `ult` exit, so count-to-zero is pin-faithful debt; the scale-under-mask half is not in the pin (POC pass on w6/masked-iv)
- DEBT bench-clocks-fill-idiom-stosw | limit scroll c -Os clocks <= 687174 | deadline 2026-11-05 | owner w12 | main 668279; the 80-word fill is `rep stosw` in main, a counted loop here (pin makes memset only for byte patterns); the last loop also keeps a second IV (LSR) Pin-faithful; waived at parity clocks; deadline tied to the user's decision on non-pin extensions (rep stosw loop idiom).
- DEBT bench-clocks-call-result-add-order | limit parity/qlight c and bas -Os clocks at their parity values (class 8) | deadline 2026-11-05 | owner w12 | the 4th call result goes to a second stack slot ([bp-8], +4 B, +1 memop) instead of being added into [bp-4]; clamp laid out jl-over-mov Allocator/CSR family; follows the greedy switch-over.
- DEBT bench-clocks-imul-const-les | limit bintree bas -Os clocks <= its parity value (class 9) | deadline 2026-11-05 | owner w12 | main fewer clocks with more insns: here 3 imul (x6, x6, x3) + setl/movzx and 2 `les` where main has 2 imul + lea, a branch and 2 movs; a clock-model question, not a count Waived at parity clocks: size over clocks (bytes_beat_clocks); bintree bas -57 B vs main.
- DEBT bench-clocks-gcse | limit parity/qmove c and nib -Os clocks at their parity values (485 for c; class 10) | deadline 2026-11-05 | owner w3 | `-fno-gcse` recovers the qmove rows; first-bad TBD
- DEBT bench-clocks-sound-alias | limit textfill bas -Os clocks <= 64169 | deadline 2027-03-31 | owner w13 | main 58189; +3997 memory operands: BYREF `ch`/`at` cells are reloaded each trip because a far POKE store cannot be told apart from near BYREF cells by alias analysis (the pin's LICM does the same). Main assumed no alias, which is unsound for BASIC (DEF SEG into DGROUP); we keep the sound behavior (user ruling). Post-main option, the user's call: a frontend alias fact for a constant non-DGROUP segment.
