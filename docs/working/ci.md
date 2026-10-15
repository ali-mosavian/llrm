# CI is the gate

A green CI run means what `gate.py run` passing locally means: the same planner, the same `tiers.toml`, the same steps and tiers. No step is skipped for a missing tool; a step that cannot run fails the job by name.

## Rules

1. CI asks `gate.py plan --json` for its jobs. It lists no step itself. A test builds the plan of a diff both ways (what `gate.py run` would run, what the workflow's jobs run) and compares them.
2. `GATE_ALLOW_MISSING` is gone from the workflow. A capability missing on a runner is a failure, not a skip.
3. The run's last job prints `GATE <tier> PASS at <head sha>`, or `FAIL: <groups>`, the line `gate.py run` prints.
4. Per pull request CI tests that PR merged with main as it is then. Batch interactions are the periodic full gate on main's job and the merger's premerge against current main.

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
