"""Pick each job's `runs-on`: tinytim when its heartbeat is fresh and a runner is idle, else GitHub's hosted runner.

    pick_runners.py GROUPS_JSON HEARTBEAT EVENT_IS_FORK [NOW]    prints {"build": RUNS_ON, "groups": {group: RUNS_ON}}

HEARTBEAT is the repository variable TINYTIM_HEARTBEAT, "<epoch> <idle runners>", written every minute by the supervisor.
Missing, malformed, stale or zero-idle means hosted: a dead tinytim never queues a job.
"""

import json
import sys

HOSTED = "ubuntu-24.04"
TINYTIM = ["self-hosted", "tinytim"]
HEAVY = ("python", "tools", "reference", "cost")
MAX_AGE = 180


def idle_runners(heartbeat: str, now: float) -> int:
    try:
        epoch, idle = heartbeat.split()
        epoch, idle = float(epoch), int(idle)
    except ValueError:
        return 0
    return max(idle, 0) if 0 <= now - epoch <= MAX_AGE else 0


def pick(groups: list[str], heartbeat: str, fork: bool, now: float) -> dict:
    idle = 0 if fork else idle_runners(heartbeat, now)
    # The build job runs alone first, so it can take the one runner the gate groups then reuse.
    build = TINYTIM if idle else HOSTED
    out = {}
    for group in sorted(groups, key=lambda g: g not in HEAVY):
        if group in HEAVY and idle:
            out[group], idle = TINYTIM, idle - 1
        else:
            out[group] = HOSTED
    return {"build": build, "groups": out}


if __name__ == "__main__":
    import time

    groups, heartbeat, fork = json.loads(sys.argv[1]), sys.argv[2], sys.argv[3] == "true"
    print(json.dumps(pick(groups, heartbeat, fork, float(sys.argv[4]) if len(sys.argv) > 4 else time.time())))
