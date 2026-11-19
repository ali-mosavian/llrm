#!/usr/bin/env python3
"""queue-runner.sh --print-plan [--prs FILE]: what the runner would do, read-only (no fetch, no edit, nothing written).
FILE is `gh pr list --json number,baseRefName,isDraft,labels` output; without it the open PRs are listed with gh."""

import json
import os
import subprocess
import sys

BRANCH = os.environ.get("LAND_BRANCH", "release/next")


def plan(prs: list[dict]) -> list[str]:
    lines = [f"land onto {BRANCH}", f"fetch origin {BRANCH}"]
    position = 0
    for pr in sorted(prs, key=lambda p: p["number"]):
        if pr.get("isDraft") or "ready" not in {label["name"] for label in pr.get("labels", [])}:
            continue
        if pr["baseRefName"] != BRANCH:
            lines.append(f"{pr['number']}: REFUSED: base is not {BRANCH}; the runner retargets it to {BRANCH}")
            continue
        position += 1
        lines.append(f"{pr['number']}: LAND onto {BRANCH}, batch position {position}")
    return lines


def main(argv: list[str]) -> None:
    if "--prs" in argv:
        prs = json.load(open(argv[argv.index("--prs") + 1]))
    else:
        prs = json.loads(subprocess.run(["gh", "pr", "list", "--state", "open", "--limit", "1000", "--json", "number,baseRefName,isDraft,labels"], capture_output=True, text=True, check=True).stdout)
    print("\n".join(plan(prs)))


if __name__ == "__main__":
    main(sys.argv[1:])
