#!/usr/bin/env python3
"""Tip-red monitor: tests release/next between gate runs and names the commit that broke a test.

    tools/tipred.py [--repo DIR] [--state FILE] [--full]   one tick (cron or a systemd timer, hourly)

A tick fetches origin/release/next, tests the crates changed since the last green tip (and what depends on
them; `--full`: every llrm-* crate), and for each test that fails and is not a known host failure bisects
last_green..head down to one commit. Each new (test, commit) is sent once; a tick that cannot run says why.
Silence is never green: a refused tick, a build failure and a dead allow-list name are reported too.
"""
from __future__ import annotations

import argparse
import datetime
import fcntl
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

LOAD_LIMIT = 35.0
FREE_FLOOR = 150 * 2**30
BISECT_STEPS = 8
# the lane locks the gate holds; looked at, never kept
LANE_LOCKS = ("/tmp/land.lock",)
ENV_LIST = Path(__file__).with_name("tipred_env.txt")
RECIPIENTS = ("llrm-oversight@tinytim", "llrm-coordinator@tinytim")


def run(cmd, cwd=None, env=None, timeout=None):
    return subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True, timeout=timeout)


def refusal(load: float, free: int, locked: bool) -> str | None:
    """Why this tick does not run, or None."""
    if load > LOAD_LIMIT:
        return f"load1m {load:.0f} > {LOAD_LIMIT:.0f}"
    if free < FREE_FLOOR:
        return f"/home free {free >> 30} GB < {FREE_FLOOR >> 30} GB"
    if locked:
        return "a gate lane holds its lock"
    return None


def lane_locked(paths=LANE_LOCKS) -> bool:
    """A lock someone else holds, found by asking for it without waiting and giving it back."""
    for path in paths:
        try:
            with open(path, "a") as handle:
                try:
                    fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    return True
                fcntl.flock(handle, fcntl.LOCK_UN)
        except OSError:
            continue
    return False


def affected_crates(changed: list[str], packages: list[dict], everything: bool = False) -> list[str]:
    """The llrm packages whose directory holds a changed path, and every package that depends on one."""
    names = sorted(p["name"] for p in packages if p["name"].startswith("llrm"))
    if everything:
        return names
    hit = set()
    for p in packages:
        directory = os.path.relpath(Path(p["manifest_path"]).parent, p["root"])
        if any(path == directory or path.startswith(directory + "/") for path in changed):
            hit.add(p["name"])
    depends = {p["name"]: {d["name"] for d in p["dependencies"]} for p in packages}
    grown = True
    while grown:
        grown = False
        for name, wants in depends.items():
            if name not in hit and wants & hit:
                hit.add(name)
                grown = True
    return sorted(name for name in hit if name in names)


def failed_tests(output: str) -> tuple[list[str], bool]:
    """(the tests that failed, whether the build broke)."""
    tests = sorted(set(re.findall(r"^test (\S+) \.\.\. FAILED$", output, re.M)))
    return tests, bool(re.search(r"^error(\[E\d+\])?: could not compile", output, re.M))


def check_env_list(entries: list[tuple[str, str]], listed: dict[str, set[str]]) -> list[str]:
    """Allow-list entries `(crate, test)` that match no test the crate lists."""
    return [f"{crate} {test}" for crate, test in entries if test not in listed.get(crate, set())]


def read_env_list(path: Path = ENV_LIST) -> list[tuple[str, str]]:
    entries = []
    for line in path.read_text().splitlines() if path.exists() else []:
        line = line.split("#", 1)[0].strip()
        if line:
            crate, test = line.split()[:2]
            entries.append((crate, test))
    return entries


def bisect(commits: list[str], red) -> str | None:
    """The first commit of `commits` (oldest first) for which `red(commit)` holds, at most BISECT_STEPS
    probes, on the assumption that red stays red once it starts; None where the last one is green."""
    if not commits:
        return None
    low, high = 0, len(commits) - 1
    steps = 0
    while low < high and steps < BISECT_STEPS:
        middle = (low + high) // 2
        steps += 1
        if red(commits[middle]):
            high = middle
        else:
            low = middle + 1
    return commits[high] if red(commits[high]) else None


def red_at(repo: str, commit: str, command: list[str], env=None) -> bool:
    """Whether `command` fails on `commit`, checked out in a scratch worktree."""
    with tempfile.TemporaryDirectory(prefix="tipred-") as scratch:
        tree = Path(scratch) / "tree"
        if run(["git", "worktree", "add", "--detach", str(tree), commit], cwd=repo).returncode:
            return True
        try:
            return run(command, cwd=tree, env=env).returncode != 0
        finally:
            run(["git", "worktree", "remove", "--force", str(tree)], cwd=repo)


def required_env(name: str) -> str:
    """A setting the unit file provides; unset is an error that says which, never a guess."""
    value = os.environ.get(name)
    if not value:
        sys.exit(f"tipred: {name} is not set (the systemd unit sets it; see tools/tipred/tipred.service)")
    return value


def outbox() -> Path:
    return Path(required_env("TIPRED_OUTBOX"))


def send(to: str, text: str) -> bool:
    """Files only (no bus a script can reach): one file per message in the outbox, which a session
    forwards; the name carries the recipient so each one is told."""
    box = outbox()
    box.mkdir(parents=True, exist_ok=True)
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S%f")
    (box / f"{stamp}.{to.split('@')[0]}.txt").write_text(f"to: {to}\n{text}\n")
    return True


class Reporter:
    """One message per new key to every recipient; a key already sent is not sent again."""

    def __init__(self, state: dict, deliver=send):
        self.state, self.deliver = state, deliver
        self.state.setdefault("sent", [])

    def report(self, key: str, text: str) -> None:
        if key in self.state["sent"]:
            return
        if all([self.deliver(to, text) for to in RECIPIENTS]):
            self.state["sent"].append(key)
        else:
            # the bus is down or refused it: the text is kept where a person finds it, and sent next tick
            self.state.setdefault("unsent", {})[key] = text


def load_state(path: Path) -> dict:
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return {}


def save_state(path: Path, state: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, indent=1, sort_keys=True))


def tick(repo: str, state_path: Path, everything: bool, reporter_for=Reporter) -> int:
    state = load_state(state_path)
    reporter = reporter_for(state)
    why = refusal(os.getloadavg()[0], shutil.disk_usage("/home").free, lane_locked())
    if why:
        reporter.report(f"refused:{why.split()[0]}:{state.get('head', '')}", f"tipred: tick refused ({why}); last green {state.get('last_green', 'none')}")
        save_state(state_path, state)
        return 0
    run(["git", "fetch", "-q", "origin", "release/next"], cwd=repo)
    head = run(["git", "rev-parse", "origin/release/next"], cwd=repo).stdout.strip()
    last = state.get("last_green")
    changed = run(["git", "diff", "--name-only", f"{last}..{head}"], cwd=repo).stdout.split() if last else []
    meta = json.loads(run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=repo).stdout or "{}")
    packages = [{**p, "root": repo} for p in meta.get("packages", [])]
    crates = affected_crates(changed, packages, everything or not last)
    env = {**os.environ, "CARGO_INCREMENTAL": "0", "CARGO_BUILD_JOBS": "1", "CARGO_TARGET_DIR": str(state_path.parent / "target")}
    env["GCC_M68K"] = required_env("GCC_M68K")  # without it the m68k tests fail on every tick
    entries = read_env_list()
    tree = Path(state_path.parent / "tree")
    if not tree.exists():
        run(["git", "worktree", "add", "--detach", str(tree), head], cwd=repo)
    run(["git", "checkout", "-q", "--detach", head], cwd=tree)
    reds, broken = {}, []
    for crate in crates:
        done = run(["nice", "-n", "19", "cargo", "test", "-p", crate, "--no-fail-fast"], cwd=tree, env=env)
        failed, build = failed_tests(done.stdout + done.stderr)
        if build:
            broken.append(crate)
        reds[crate] = [t for t in failed if (crate, t.rsplit("::", 1)[-1]) not in entries and (crate, t) not in entries]
    for crate in broken:
        reporter.report(f"build:{crate}:{head}", f"tipred: {crate} does not build on release/next {head[:10]}")
    for crate, tests in reds.items():
        for test in tests:
            first = None
            if last:
                commits = run(["git", "rev-list", "--reverse", f"{last}..{head}"], cwd=repo).stdout.split()
                first = bisect(commits, lambda c: red_at(repo, c, ["cargo", "test", "-p", crate, test.rsplit("::", 1)[-1]], env))
            where = f"first red {first[:10]}" if first else f"red at {head[:10]} (no earlier green to bisect from)"
            subject = run(["git", "log", "-1", "--format=%an: %s", first], cwd=repo).stdout.strip() if first else ""
            reporter.report(f"red:{crate}:{test}:{first or head}", f"tipred: {crate} {test} is red on release/next, {where} {subject}")
    if not any(reds.values()) and not broken:
        state["last_green"] = head
    state["head"] = head
    save_state(state_path, state)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=str(Path(__file__).resolve().parent.parent))
    parser.add_argument("--state", default=os.environ.get("TIPRED_STATE"), required="TIPRED_STATE" not in os.environ)
    parser.add_argument("--full", action="store_true")
    args = parser.parse_args()
    return tick(args.repo, Path(args.state), args.full)


if __name__ == "__main__":
    sys.exit(main())
