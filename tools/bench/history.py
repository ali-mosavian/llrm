"""
The benchmark history: one line per main commit x benchmark x language x level in results.jsonl on the
`bench-history` branch, which holds nothing else.

    uv run --project tools python tools/bench/history.py record [--commit SHA] [--push]
    uv run --project tools python tools/bench/history.py backfill N [--push]

`record` builds SHA (default: origin/main, else main) in a scratch worktree, runs that commit's own
tools/bench with timing and the reference compilers, and appends its lines. A commit already in the file
is skipped, so a scheduled run can call it every time. `backfill` records the last N first-parent commits
that are not there yet, oldest first; a commit with no tools/bench is skipped, and says so.

It installs nothing and schedules nothing: run it by hand, or from whatever runs your jobs.
"""

from __future__ import annotations

import os
import sys
import json
import shutil
import argparse
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BRANCH = "bench-history"
RESULTS = "results.jsonl"


def git(*arguments: str, cwd: Path | None = None, check: bool = True) -> str:
    done = subprocess.run(["git", *arguments], cwd=cwd or ROOT, capture_output=True, text=True)
    if check and done.returncode != 0:
        raise RuntimeError(f"git {' '.join(arguments)}: {done.stderr.strip()}")
    return done.stdout.strip()


def recorded(path: Path) -> set[str]:
    """The commits that already have lines in `path`."""
    if not path.exists():
        return set()
    return {json.loads(line)["commit"] for line in path.read_text().splitlines() if line.strip()}


def lines_for(commit: str, date: str, subject: str, measured: dict[str, dict]) -> list[str]:
    """One JSON line per measurement: benchmark/language/opt keys as bench.py's --json writes them."""
    out = []
    for key, value in sorted(measured.items()):
        *name, language, opt = key.split("/")
        line = {"commit": commit, "date": date, "subject": subject, "benchmark": "/".join(name), "language": language, "opt": opt}
        line |= {one: value[one] for one in ("instructions", "memory_operands", "cycles", "ms", "known", "error") if one in value}
        out.append(json.dumps(line, sort_keys=True))
    return out


def history_worktree(directory: Path) -> Path:
    """A worktree of the bench-history branch in `directory`, creating the branch as an orphan if there is none."""
    if git("rev-parse", "--verify", "--quiet", f"refs/heads/{BRANCH}", check=False):
        git("worktree", "add", "--force", str(directory), BRANCH)
    else:
        remote = git("rev-parse", "--verify", "--quiet", f"refs/remotes/origin/{BRANCH}", check=False)
        if remote:
            git("worktree", "add", "--force", "-B", BRANCH, str(directory), f"origin/{BRANCH}")
        else:
            git("worktree", "add", "--detach", str(directory))
            git("checkout", "--orphan", BRANCH, cwd=directory)
            git("rm", "-rfq", ".", cwd=directory, check=False)
            for leftover in directory.iterdir():
                if leftover.name != ".git":
                    shutil.rmtree(leftover) if leftover.is_dir() else leftover.unlink()
    return directory


def append(directory: Path, lines: list[str], message: str) -> None:
    results = directory / RESULTS
    with results.open("a") as handle:
        handle.write("".join(line + "\n" for line in lines))
    git("add", RESULTS, cwd=directory)
    git("-c", "core.hooksPath=/dev/null", "commit", "-q", "-m", message, cwd=directory)


def fresh_tree(tree: Path, commit: str) -> None:
    """A detached worktree of `commit` at `tree`, replacing what an interrupted run left there."""
    git("worktree", "remove", "--force", str(tree), check=False)
    shutil.rmtree(tree, ignore_errors=True)
    git("worktree", "prune")
    git("worktree", "add", "--detach", str(tree), commit)


def measure(commit: str, scratch: Path) -> dict | str:
    """bench.py's measurements of `commit`, built and run in a scratch worktree; or why there are none."""
    tree = scratch / f"tree-{commit[:8]}"
    fresh_tree(tree, commit)
    try:
        tool = tree / "tools" / "bench" / "bench.py"
        if not tool.exists():
            return "no tools/bench at that commit"
        target = scratch / "target"
        env = {**os.environ, "CARGO_TARGET_DIR": str(target)}
        built = subprocess.run(["cargo", "build", "--release", "--bins", "--manifest-path", str(tree / "Cargo.toml")], env=env, capture_output=True, text=True)
        if built.returncode != 0:
            return "does not build: " + built.stderr.strip()[-300:]
        out = scratch / f"{commit[:8]}.json"
        base = ["uv", "run", "--project", str(tree / "tools"), "python", str(tool)]
        # an older commit's bench.py may not know --time or --references: ask for what it has
        for extra in (["--time", "--references"], []):
            subprocess.run([*base, *extra, "--json", str(out), "--work", str(scratch / "work")], env={**env, "LLRM_BIN": str(target / "release")}, capture_output=True, text=True)  # a failed gate still has numbers
            if out.exists():
                return json.loads(out.read_text())
        return "bench.py wrote nothing"
    finally:
        git("worktree", "remove", "--force", str(tree), check=False)


def record(commits: list[str], push: bool) -> int:
    scratch = Path(os.environ.get("BENCH_SCRATCH", Path.home() / "scratch" / "bench-history"))
    scratch.mkdir(parents=True, exist_ok=True)
    keeper = Path(tempfile.mkdtemp(dir=scratch, prefix="history-"))
    try:
        history = history_worktree(keeper / "history")
        seen = recorded(history / RESULTS)
        for commit in commits:
            full = git("rev-parse", commit)
            if full in seen:
                print(f"{full[:8]}: already recorded")
                continue
            measured = measure(full, scratch)
            if isinstance(measured, str):
                print(f"{full[:8]}: skipped: {measured}")
                continue
            date, subject = git("show", "-s", "--format=%cI%n%s", full).split("\n", 1)
            append(history, lines_for(full, date, subject, measured), f"bench: {full[:8]} {subject[:60]}")
            print(f"{full[:8]}: {len(measured)} measurements")
        if push:
            git("push", "origin", f"{BRANCH}", cwd=history)
    finally:
        git("worktree", "remove", "--force", str(keeper / "history"), check=False)
        shutil.rmtree(keeper, ignore_errors=True)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("action", choices=("record", "backfill"))
    parser.add_argument("count", nargs="?", type=int, default=1)
    parser.add_argument("--commit")
    parser.add_argument("--push", action="store_true")
    args = parser.parse_args()
    main_ref = "origin/main" if git("rev-parse", "--verify", "--quiet", "origin/main", check=False) else "main"
    if args.action == "record":
        return record([args.commit or main_ref], args.push)
    commits = git("rev-list", "--first-parent", f"-n{args.count}", main_ref).splitlines()
    return record(list(reversed(commits)), args.push)


if __name__ == "__main__":
    sys.exit(main())
