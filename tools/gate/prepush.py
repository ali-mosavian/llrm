#!/usr/bin/env python3
"""The gate's cheap checks, before a push: what failed CI most often today (fmt, generated docs, no_local_paths, target_facts) caught locally.

    tools/prepush.sh [--only STEP ...]     prints `PREPUSH PASS` or `PREPUSH FAIL: step names` first, then the output of the failing steps

Not a gate: it does NOT run the build, the unit/integration/python tests, benches, the oracle sweep, perf or cost. It runs the commands of the
gate itself (tools/gate/gate.py, the smoke tier's), for the steps and root tests named in tools/gate/tiers.toml [prepush]: a scan added to
[[scan]] is run by the next prepush with no change here.
"""

from __future__ import annotations

import os
import sys
import time
import argparse
import subprocess
import concurrent.futures
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))

import gate  # noqa: E402

sys.path.insert(0, str(HERE.parent))
from buildslot import slotted  # noqa: E402


def selected(cfg: dict, only: list[str] | None = None) -> dict[str, str]:
    """name -> the shell command of every prepush step (the gate's own) and root test, in tiers.toml's order."""
    base = subprocess.run("git merge-base HEAD origin/feat/parity 2>/dev/null || git merge-base HEAD origin/main", shell=True, capture_output=True, text=True, cwd=ROOT).stdout.strip() or "origin/main"
    files = subprocess.run(["git", "diff", "--name-only", base], capture_output=True, text=True, cwd=ROOT).stdout.split()
    plan = gate.plan(files or ["README.md"], "smoke")
    commands = gate.commands(plan, cfg, gate.packages())
    chosen = {step: commands[step] for step in cfg["prepush"]["steps"] if step in commands}
    cargo = "cargo test --release -q --no-fail-fast"
    chosen.update({f"test {name}": f"{cargo} --test {name}" for name in cfg["prepush"]["tests"]})
    chosen.update(cfg["prepush"].get("checks", {}))
    return {name: command for name, command in chosen.items() if not only or name in only}


def run_all(commands: dict[str, str], env: dict[str, str], jobs: int = 4) -> tuple[list[tuple[str, int, float, str]], float]:
    """(name, exit code, seconds, output) of each command, run in parallel but cargo's: the build lock serialises those anyway."""

    def one(item: tuple[str, str]) -> tuple[str, int, float, str]:
        name, command = item
        started = time.perf_counter()
        done = subprocess.run(slotted(command), shell=True, cwd=ROOT, env=env, capture_output=True, text=True, errors="replace")
        return name, done.returncode, time.perf_counter() - started, done.stdout + done.stderr

    started = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(jobs) as pool:
        results = list(pool.map(one, commands.items()))
    return results, time.perf_counter() - started


def verdict(results: list[tuple[str, int, float, str]]) -> str:
    failed = [name for name, code, _, _ in results if code]
    return "PREPUSH PASS" if not failed else "PREPUSH FAIL: " + ", ".join(failed)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--only", nargs="+", metavar="STEP")
    args = parser.parse_args()
    cfg = gate.load()
    env = {**os.environ, **gate.STEP_ENV, "PYTHONDONTWRITEBYTECODE": "1"}
    env.setdefault("CARGO_TARGET_DIR", subprocess.run([sys.executable, str(ROOT / "tools/llrmbin.py"), "target"], capture_output=True, text=True).stdout.strip())
    results, wall = run_all(selected(cfg, args.only), env)
    print(verdict(results))
    print("covers: " + ", ".join(name for name, *_ in results) + f" ({wall:.0f} s). NOT covered: the build, unit/integration/python tests, benches, the oracle sweep, perf, cost: the full gate.")
    for name, code, seconds, output in results:
        print(f"  {name}: {'FAIL' if code else 'ok'} {seconds:.0f} s")
        if code:
            print("\n".join("    " + line for line in output.strip().splitlines()[-30:]))
    return 1 if any(code for _, code, _, _ in results) else 0


if __name__ == "__main__":
    sys.exit(main())
