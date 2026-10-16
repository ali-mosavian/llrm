"""Pick each job's `runs-on`: tinytim when its heartbeat is fresh and a runner is idle, else GitHub's hosted runner.

    pick_runners.py GROUPS_JSON HEARTBEAT ELIGIBLE [NOW]    prints {"build": RUNS_ON, "groups": {group: RUNS_ON}, "summary": TEXT}

ELIGIBLE is true only for a push to main and a pull request from this repository: every other event (a fork's pull request, a push to
another branch, a manual run elsewhere) is hosted.

HEARTBEAT is the repository variable TINYTIM_HEARTBEAT, "<epoch> <idle runners>", written every minute by the supervisor.
Missing, malformed, stale or zero-idle means hosted: a dead tinytim never queues a job.
"""

import json
import sys

HOSTED = "ubuntu-24.04"
TINYTIM = ["self-hosted", "tinytim"]
HEAVY = ("python", "tools", "reference", "cost")  # and every bench shard (heavy())
MAX_AGE = 180


def heavy(group: str) -> bool:
    return group in HEAVY or group.startswith("bench-")


def idle_runners(heartbeat: str, now: float) -> int:
    try:
        epoch, idle = heartbeat.split()
        epoch, idle = float(epoch), int(idle)
    except ValueError:
        return 0
    return max(idle, 0) if 0 <= now - epoch <= MAX_AGE else 0


def pick(groups: list[str], heartbeat: str, eligible: bool, now: float) -> dict:
    idle = idle_runners(heartbeat, now) if eligible else 0
    # The build job runs alone first, so it can take the one runner the gate groups then reuse.
    build = TINYTIM if idle else HOSTED
    out = {}
    # The long groups first: python is 12 minutes, a bench shard 4.
    for group in sorted(groups, key=lambda g: (not heavy(g), g.startswith("bench-"))):
        if heavy(group) and idle:
            out[group], idle = TINYTIM, idle - 1
        else:
            out[group] = HOSTED
    jobs = {"build": build, **out}
    names = {kind: " ".join(j for j, r in jobs.items() if r == where) for kind, where in (("tinytim", TINYTIM), ("hosted", HOSTED))}
    return {"build": build, "groups": out, "summary": "; ".join(f"{kind}: {v}" for kind, v in names.items() if v)}


if __name__ == "__main__":
    import time

    groups, heartbeat, eligible = json.loads(sys.argv[1]), sys.argv[2], sys.argv[3] == "true"
    print(json.dumps(pick(groups, heartbeat, eligible, float(sys.argv[4]) if len(sys.argv) > 4 else time.time())))
