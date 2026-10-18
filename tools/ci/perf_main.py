"""The scheduled perf run on main: when to speak, and what to say.

    perf_main.py decide STATE RESULT        STATE and RESULT are green|red; prints comment-red, comment-green or nothing
    perf_main.py report LAST_GREEN HEAD LOG   the comment body: the failing tests of LOG and the commits in LAST_GREEN..HEAD

The workflow (.github/workflows/perf-main.yml) runs tools/test_budgets.py and the other perf tests on main every 3 hours on tinytim, because
hosted CI cannot run them (no perf counters) and a red there is unseen. It comments on one issue only when the state changes.
"""

from __future__ import annotations

import re
import sys
import subprocess

FAILED = re.compile(r"^FAILED (\S+?)(?: - (.*))?$", re.M)


def decide(previous: str, now: str) -> str:
    """comment-red on green -> red, comment-green on red -> green, else nothing. No state yet counts as green: a first red is news."""
    previous = previous if previous in ("green", "red") else "green"
    if previous == "green" and now == "red":
        return "comment-red"
    if previous == "red" and now == "green":
        return "comment-green"
    return ""


def failures(log: str) -> list[str]:
    """The failing tests of a pytest summary, with the first line of each assertion where it is there."""
    return [f"{name}" + (f": {why[:200]}" if why else "") for name, why in FAILED.findall(log)]


def merges(last_green: str, head: str) -> list[str]:
    """The commits since the last green run, oldest first, one line each."""
    if not last_green:
        return []
    done = subprocess.run(["git", "log", "--reverse", "--format=%h %s", f"{last_green}..{head}"], capture_output=True, text=True)
    return done.stdout.splitlines()


def report(last_green: str, head: str, log: str) -> str:
    lines = failures(log)
    out = [f"perf on main is red at {head[:9]}: {len(lines)} failing test(s)", ""] + [f"- `{one}`" for one in lines[:30]]
    commits = merges(last_green, head)
    out += ["", f"merged since the last green ({last_green[:9] or 'none recorded'}): {len(commits)}"] + [f"- {one}" for one in commits[:60]]
    return "\n".join(out)


def main() -> int:
    if len(sys.argv) == 4 and sys.argv[1] == "decide":
        print(decide(sys.argv[2], sys.argv[3]))
        return 0
    if len(sys.argv) == 5 and sys.argv[1] == "report":
        print(report(sys.argv[2], sys.argv[3], open(sys.argv[4], errors="replace").read()))
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
