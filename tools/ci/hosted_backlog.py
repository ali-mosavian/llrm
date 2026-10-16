"""How many hosted jobs are waiting for a runner: the queued jobs of this repository's runs that are not for a self-hosted runner.

    hosted_backlog.py    needs `gh` and GITHUB_REPOSITORY; prints the number (0 when it cannot be read)

Runs ahead is the wrong measure: with ready marks held, 12 queued runs left hosted idle (a job started in 3 s) and the overflow to tinytim
made a pull request slower. The account runs 20 hosted jobs at once, so the jobs that wait are what a new job queues behind.
"""

import os
import json
import subprocess
import sys


def gh(path: str) -> dict:
    done = subprocess.run(["gh", "api", path], capture_output=True, text=True)
    return json.loads(done.stdout) if done.returncode == 0 and done.stdout else {}


def waiting(jobs: list[dict]) -> int:
    """The queued jobs that wait for a hosted runner."""
    return sum(1 for j in jobs if j.get("status") == "queued" and "self-hosted" not in j.get("labels", []))


def backlog(repo: str) -> int:
    total = 0
    for status in ("queued", "in_progress"):
        for run in gh(f"repos/{repo}/actions/runs?status={status}&per_page=30").get("workflow_runs", []):
            total += waiting(gh(f"repos/{repo}/actions/runs/{run['id']}/jobs?per_page=100").get("jobs", []))
    return total


if __name__ == "__main__":
    try:
        print(backlog(os.environ["GITHUB_REPOSITORY"]))
    except Exception:  # noqa: BLE001: a number that cannot be read is no backlog, never a failed plan
        print(0)
    sys.exit(0)
