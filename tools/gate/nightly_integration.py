#!/usr/bin/env python3
"""The nightly full run of the root crate's integration targets (126 binaries, 275 s to run after a 277 s rebuild at load 14: too slow for the landing gate).

    nightly_integration.py [--cargo-arg ARG...]

Prints every red test as `integration <target>::<test>` (the names the landing gate compares), and the targets with no result. Exit 1 if any red test is not listed in
tools/xfail.toml as `integration <target>::<test>`.
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent))
import xfail  # noqa: E402
from buildslot import slotted  # noqa: E402


from parity_smoke import red_tests  # noqa: E402
import os

BRANCH = os.environ.get("LAND_BRANCH", "release/next")  # the integration branch (tools/landing/branch.sh)


def attribution(
    unexpected: list[str],
    reach: dict[str, set[str]],
    landed: list[tuple[str, list[str]]],
    crate_of,
    pr_and_owner,
) -> list[str]:
    """`NIGHTLY RED integration T::test: #N (owner O), ...` for each red test not in xfail.toml: the PRs landed in the window (`landed`: sha and its files) that touch a crate
    T reaches (`reach`), or T's own file, so the coordinator can route them (the landing runs the scanners and a PR's own tests; the rest of the integration set runs here).
    `crate_of(path)` -> its workspace package or None; `pr_and_owner(sha)` -> (number, owner)."""
    lines = []
    for name in unexpected:
        target = name.split(" ", 1)[1].split("::", 1)[0]
        mine = []
        for sha, files in landed:
            touched = {crate_of(path) for path in files} - {None}
            if touched & reach.get(target, set()) or f"tests/{target}.rs" in files:
                mine.append(pr_and_owner(sha))
        named = ", ".join(f"#{number} (owner {owner})" for number, owner in dict.fromkeys(mine)) or "no PR of the window touches it: older"
        lines.append(f"NIGHTLY RED {name}: {named}")
    return lines


def window(
    since: str,
    run=subprocess.run,
) -> list[tuple[str, list[str]]]:
    """The commits of feat/parity since `since` (a git date), newest first, with the files each changed."""
    shas = run(["git", "log", f"origin/{BRANCH}", "--first-parent", f"--since={since}", "--format=%H"], cwd=ROOT, capture_output=True, text=True).stdout.split()
    return [(sha, run(["git", "diff-tree", "--no-commit-id", "--name-only", "-r", sha], cwd=ROOT, capture_output=True, text=True).stdout.split()) for sha in shas]


def pr_and_owner(
    sha: str,
    run=subprocess.run,
) -> tuple[str, str]:
    """The PR a squash commit came from and its `Owner:` line, from GitHub."""
    found = run(["gh", "api", f"repos/{{owner}}/{{repo}}/commits/{sha}/pulls", "--jq", ".[0].number"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    if not found:
        return sha[:9], "unknown"
    body = run(["gh", "pr", "view", found, "--json", "body", "--jq", ".body"], cwd=ROOT, capture_output=True, text=True).stdout
    owner = re.search(r"^Owner:\s*(\S+)", body, re.M)
    return found, owner.group(1) if owner else "unknown"


def main(argv: list[str]) -> int:
    done = subprocess.run(slotted(["cargo", "test", "--release", "--no-fail-fast", "-p", "llrm", "--tests", *argv]), cwd=ROOT, capture_output=True, text=True)
    red = red_tests(done.stdout + done.stderr)
    unexpected = [one for one in red if one not in xfail.load()]
    print("\n".join(red) or "no red integration test")
    print(f"{len(red)} red, {len(unexpected)} not in tools/xfail.toml")
    if unexpected:
        import parity_smoke  # noqa: PLC0415

        packages, _ = parity_smoke.workspace_graph()
        crate_of = lambda path: next(iter(parity_smoke.touched_packages([path], packages)), None)  # noqa: E731
        for line in attribution(unexpected, parity_smoke.integration_map(), window("26 hours ago"), crate_of, pr_and_owner):
            print(line)
    return 1 if unexpected else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
