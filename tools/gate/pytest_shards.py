"""pytest over many files at once: one pytest process per file, a few at a time, longest first.

    pytest_shards.py [-j N] [--ignore F]... [--must-pass F]... [--rerun F]... [--durations FILE] PATH... [-- PYTEST_FLAGS]

The files come from `pytest --collect-only`, so a path pytest would not run is not run here. A file with a failing test, a collection
error, or no test (a file that collected tests and ran none) fails the run; the output of every failed file is printed whole, then one
summary line, which counts the tests passed and skipped. Exit 0 only when every collected test file ran and passed or skipped, and
every --must-pass file passed at least one test and skipped none: a file that skips everything (its inputs missing) is not a pass. A failed
--rerun file is run once more and both verdicts are printed (the run stays red): a red that passes again is a flaky one, one that fails again is not."""
import os
import re
import math
import time
import sys
import json
import argparse
import subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

UNKNOWN_SECONDS = 40.0  # a file with no recorded time starts early (not split): a late long file is the worst case
NO_TESTS = 5
SPLIT_SECONDS = 45.0  # a file in SPLITTABLE recorded slower than this is run in parts
PART_SECONDS = 30.0
MAX_PARTS = 12
# Files whose tests are independent, by name (a path here would name a target: tests/target_facts.rs reads this file). test_qbgfx.py
# builds each probe when its test asks. A file with a module fixture that does all the work is not here: test_qbrt.py's runs every program
# in one DOSBox launch, so five parts ran it five times (185 s each, against 134 s whole).
SPLITTABLE = frozenset({"test_qbgfx.py", "test_qbgorilla.py", "test_scaling_gate.py", "test_scaling.py"})
# Seconds of the files that were slowest when measured, for a run with nothing recorded yet.
SEEDS = {"test_qbgfx.py": 190.0, "test_scaling_gate.py": 300.0, "test_qbrt.py": 130.0, "test_scaling.py": 60.0}


def tally(out: str) -> dict[str, int]:
    """The counts pytest's last line reports: {'passed': 2, 'skipped': 1}."""
    last = out.strip().splitlines()[-1] if out.strip() else ""
    return {word: int(n) for n, word in re.findall(r"(\d+) (passed|skipped|failed|xfailed|xpassed|errors?)\b", last)}


def pytest(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-m", "pytest", "--rootdir=.", *args], capture_output=True, text=True)


def collect(paths: list[str], ignores: list[str], flags: list[str]) -> tuple[dict[str, list[str]], str | None]:
    """({test file: its test ids, in collection order}, or the collector's output when it failed)."""
    done = pytest(*paths, *(f"--ignore={i}" for i in ignores), *(f for f in flags if f != "-q"), "--collect-only", "-q")
    if done.returncode not in (0, NO_TESTS):
        return {}, done.stdout + done.stderr
    tests: dict[str, list[str]] = {}
    for line in done.stdout.splitlines():
        if "::" in line and re.match(r"^\S+\.py::", line):
            tests.setdefault(line.partition("::")[0], []).append(line)
    return tests, None


def units(tests: dict[str, list[str]], known: dict[str, float]) -> list[tuple[str, list[str]]]:
    """(label, pytest arguments), longest first. A SPLITTABLE file recorded over SPLIT_SECONDS is cut into parts of its tests, dealt round-robin: one
    file held the whole run to its 430 s."""
    out: list[tuple[str, list[str], float]] = []
    for name, ids in tests.items():
        base = Path(name).name
        took = known.get(name, SEEDS.get(base, UNKNOWN_SECONDS))
        parts = min(MAX_PARTS, len(ids), math.ceil(took / PART_SECONDS)) if took > SPLIT_SECONDS and base in SPLITTABLE else 1
        if parts == 1:
            out.append((name, [name], took))
        else:
            out += [(f"{name} [{i + 1}/{parts}]", ids[i::parts], took / parts) for i in range(parts)]
    return [(label, args) for label, args, _ in sorted(out, key=lambda u: -u[2])]


def run_all(work: list[tuple[str, list[str]]], flags: list[str], jobs: int) -> dict[str, tuple[int, float, str]]:
    def one(item: tuple[str, list[str]]) -> tuple[str, int, float, str]:
        start = time.time()
        done = pytest(*item[1], *flags)
        return item[0], done.returncode, time.time() - start, done.stdout + done.stderr

    with ThreadPoolExecutor(jobs) as pool:
        return {n: (code, took, out) for n, code, took, out in pool.map(one, work)}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("-j", type=int, default=int(os.environ.get("PYTEST_SHARDS") or min(16, os.cpu_count() or 4)))
    ap.add_argument("--ignore", action="append", default=[])
    ap.add_argument("--must-pass", action="append", default=[], help="a test file that must pass a test and skip none")
    ap.add_argument("--rerun", action="append", default=[], help="a test file run once more when it fails, to tell flaky from deterministic")
    ap.add_argument("--durations", help="JSON {file: seconds}, read for the order (longest first) and rewritten after the run")
    ap.add_argument("paths", nargs="+")
    argv = sys.argv[1:]
    at = argv.index("--") if "--" in argv else len(argv)
    args, flags = ap.parse_args(argv[:at]), argv[at + 1 :]  # what follows `--` is pytest's, whatever it looks like
    tests, broken = collect(args.paths, args.ignore, flags)
    if broken is not None:
        print(broken)
        print("pytest shards: collection failed")
        return 1
    if not tests:
        print("pytest shards: no test file collected")
        return 1
    known = json.loads(Path(args.durations).read_text()) if args.durations and Path(args.durations).exists() else {}
    work = units(tests, known)
    results = run_all(work, flags, args.j)
    bad = {n: r for n, r in results.items() if r[0] != 0}
    counts = {n: tally(out) for n, (_, _, out) in results.items()}
    ran = sum(sum(c.values()) for c in counts.values())
    wanted = sum(map(len, tests.values()))
    if not bad and ran != wanted:
        print(f"pytest shards: {wanted} tests collected, {ran} reported")
        return 1
    vacuous = [n for n, c in counts.items() if any(n.partition(" [")[0].endswith(f) for f in args.must_pass) and (c.get("skipped") or not c.get("passed"))]
    for name in vacuous:
        print(f"pytest shards: {name} is a must-pass file and ran {counts[name].get('passed', 0)} passed, {counts[name].get('skipped', 0)} skipped")
    for name in sorted(bad):
        if any(name.partition(" [")[0].endswith(f) for f in args.rerun):
            again = pytest(*dict(work)[name], *flags)
            print(f"pytest shards: {name} failed, rerun {'passed' if again.returncode == 0 else 'failed again'}: {(again.stdout.strip().splitlines() or [''])[-1]}")
    for name, (code, _, out) in sorted(bad.items()):
        print(f"\n######## {name}: exit {code}\n{out}")
    if args.durations and not bad:
        per_file: dict[str, float] = {}
        for label, (_, took, _) in results.items():
            per_file[label.partition(" [")[0]] = per_file.get(label.partition(" [")[0], 0) + took
        Path(args.durations).write_text(json.dumps({n: round(t, 1) for n, t in sorted(per_file.items())}, indent=1) + "\n")
    slowest = sorted(results.items(), key=lambda kv: -kv[1][1])[:3]
    passed, skipped = (sum(c.get(w, 0) for c in counts.values()) for w in ("passed", "skipped"))
    print(f"pytest shards: {len(tests)} files, {len(results)} runs, {len(bad)} failed; {passed} passed, {skipped} skipped; slowest " + ", ".join(f"{n} {r[1]:.0f}s" for n, r in slowest))
    if bad:
        print("FAILED files: " + " ".join(sorted(bad)))
    return 1 if bad or vacuous else 0


if __name__ == "__main__":
    sys.exit(main())
