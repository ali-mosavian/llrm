"""Pick each job's `runs-on`: tinytim only for the jobs that need hardware perf counters (PERF), when its heartbeat is fresh and a runner is idle; every other job, and any PERF job tinytim cannot take, on GitHub's hosted runner.

    pick_runners.py GROUPS_JSON HEARTBEAT ELIGIBLE [BACKLOG [NOW]]    prints {"build": RUNS_ON, "groups": {group: RUNS_ON}, "summary": TEXT}

ELIGIBLE is true only for a push to main and a pull request from this repository: every other event (a fork's pull request, a push to
another branch, a manual run elsewhere) is hosted.

HEARTBEAT is the repository variable TINYTIM_HEARTBEAT, "<epoch> <idle runners>", written every minute by the supervisor.
Missing, malformed, stale or zero-idle means hosted: a dead tinytim never queues a job.
"""

import json
import sys

HOSTED = "ubuntu-24.04"
TINYTIM = ["self-hosted", "tinytim"]
# The jobs that need hardware instruction counters: only tinytim has them. Everything else is hosted (tinytim is shared with every session's
# builds: the same build ran 82-103 s hosted and 243-669 s there at the same cache state).
PERF = ("perf", "cost")
# GitHub runs 20 hosted jobs of this account at once. When this many hosted jobs are already waiting (tools/ci/hosted_backlog.py), a job
# would wait for a slot longer than tinytim's slowness (2-4x) costs it, so the other jobs overflow to idle tinytim runners.
OVERFLOW_AT = 8
OVERFLOW_ORDER = ("python", "reference", "run", "dos", "integration", "tools", "unit", "bench-1", "bench-2", "bench-3", "bench-4", "bench-5", "bench-6")
MAX_AGE = 180


def idle_runners(heartbeat: str, now: float) -> int:
    try:
        epoch, idle = heartbeat.split()
        epoch, idle = float(epoch), int(idle)
    except ValueError:
        return 0
    return max(idle, 0) if 0 <= now - epoch <= MAX_AGE else 0


def pick(groups: list[str], heartbeat: str, eligible: bool, now: float, backlog: int = 0) -> dict:
    idle = idle_runners(heartbeat, now) if eligible else 0
    out = {}
    for group in sorted(groups):
        if group in PERF and idle:
            out[group], idle = TINYTIM, idle - 1
        else:
            out[group] = HOSTED
    if backlog >= OVERFLOW_AT:
        for group in sorted((g for g in groups if g not in PERF), key=lambda g: OVERFLOW_ORDER.index(g) if g in OVERFLOW_ORDER else len(OVERFLOW_ORDER)):
            if idle:
                out[group], idle = TINYTIM, idle - 1
    jobs = {"build": HOSTED, **out}
    names = {kind: " ".join(j for j, r in jobs.items() if r == where) for kind, where in (("tinytim", TINYTIM), ("hosted", HOSTED))}
    # A perf job on a hosted runner skips its counter steps: the caller decides whether that passes (a change to the compiler's source must not).
    # ...and a job that would only skip them is not made (its set-up alone is 70-140 s of a runner out of the 20 the account has).
    return {"build": HOSTED, "groups": out, "run": [g for g in sorted(groups) if not (g in PERF and out[g] == HOSTED)], "perf_hosted": sorted(g for g in out if g in PERF and out[g] == HOSTED), "summary": "; ".join(f"{kind}: {v}" for kind, v in names.items() if v)}


if __name__ == "__main__":
    import time

    groups, heartbeat, eligible = json.loads(sys.argv[1]), sys.argv[2], sys.argv[3] == "true"
    backlog = int(sys.argv[4]) if len(sys.argv) > 4 else 0
    print(json.dumps(pick(groups, heartbeat, eligible, float(sys.argv[5]) if len(sys.argv) > 5 else time.time(), backlog)))
