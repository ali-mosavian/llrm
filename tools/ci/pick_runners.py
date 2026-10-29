"""Pick each job's `runs-on`: tinytim only for the jobs that need hardware perf counters (PERF), when its heartbeat is fresh and a runner is idle; every other job, and any PERF job tinytim cannot take, on GitHub's hosted runner.

    pick_runners.py GROUPS_JSON HEARTBEAT ELIGIBLE [BACKLOG [COMPILER [NOW]]]    prints {"build": RUNS_ON, "groups": {group: RUNS_ON}, "summary": TEXT}

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
OVERFLOW_ORDER = ("python", "reference", "run", "dos", "integration", "tools", "unit", "bench-base", "bench-1", "bench-2", "bench-3", "bench-4", "bench-5", "bench-6")
MAX_AGE = 180


def idle_runners(heartbeat: str, now: float) -> int:
    try:
        epoch, idle = heartbeat.split()
        epoch, idle = float(epoch), int(idle)
    except ValueError:
        return 0
    return max(idle, 0) if 0 <= now - epoch <= MAX_AGE else 0


def pick(groups: list[str], heartbeat: str, eligible: bool, now: float, backlog: int = 0, compiler: bool = True) -> dict:
    """The jobs' runners. The perf groups judge the compiler: a change to it runs them on tinytim, always (a job queues there until a runner is
    idle, which is the retry a hosted fallback never had), and any other change does not make them. `small` is the runner of the tiny
    serial jobs (bench-merge, ci): hosted, unless hosted jobs are waiting and a tinytim runner is idle after the perf jobs (they queue behind
    cost, which takes 12 minutes)."""
    idle = idle_runners(heartbeat, now) if eligible else 0
    perf = [g for g in sorted(groups) if g in PERF] if compiler else []
    out = {g: (TINYTIM if eligible else HOSTED) for g in perf}
    idle = max(idle - len(perf), 0)
    for group in sorted(g for g in groups if g not in PERF):
        out[group] = HOSTED
    if backlog >= OVERFLOW_AT:
        for group in sorted((g for g in groups if g not in PERF), key=lambda g: OVERFLOW_ORDER.index(g) if g in OVERFLOW_ORDER else len(OVERFLOW_ORDER)):
            if idle:
                out[group], idle = TINYTIM, idle - 1
    small = TINYTIM if backlog >= OVERFLOW_AT and idle else HOSTED  # behind a 12-minute cost job a 5-second job waited 36 minutes
    jobs = {"build": HOSTED, **out}
    names = {kind: " ".join(j for j, r in jobs.items() if r == where) for kind, where in (("tinytim", TINYTIM), ("hosted", HOSTED))}
    return {
        "build": HOSTED,
        "small": small,
        "groups": out,
        "run": sorted(out),
        # Perf groups with no tinytim to run on (a fork's pull request): the caller fails a change to the compiler on this, never passes it.
        "perf_hosted": sorted(g for g in out if g in PERF and out[g] == HOSTED),
        "summary": "; ".join(f"{kind}: {v}" for kind, v in names.items() if v),
    }


if __name__ == "__main__":
    import time

    groups, heartbeat, eligible = json.loads(sys.argv[1]), sys.argv[2], sys.argv[3] == "true"
    backlog = int(sys.argv[4]) if len(sys.argv) > 4 else 0
    compiler = sys.argv[5] != "false" if len(sys.argv) > 5 else True
    print(json.dumps(pick(groups, heartbeat, eligible, float(sys.argv[6]) if len(sys.argv) > 6 else time.time(), backlog, compiler)))
