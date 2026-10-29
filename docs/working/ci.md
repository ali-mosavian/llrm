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

The pin's tools other than clang come from one cached tree, `~/.cache/llrm/pintools-<pin sha12>` (`toolchain/pintoolsbuild.sh`, built by the `pintools` job beside `toolchain`, so the clang cache keys are unchanged): `llc` and `opt` for `tools/pairs/pin.py` and `tests/common/pin.rs`, and the source with `llvm-tblgen` for the DAG tables' staleness test (`llrm-dagisel-gen/tests/stale.rs`, which runs `tools/dagisel/regen.sh`). The setup action exports `LLVM_BIN`, `LLVM_SRC` and `LLVM_BUILD` to the unit and integration groups; a missing tool is red. Assertions do not change the tables, so the build is Release. PRICE

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
