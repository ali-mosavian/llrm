"""pytest over many files at once: one pytest process per file, a few at a time, longest first.

    pytest_shards.py [-j N] [--ignore F]... [--must-pass F]... [--rerun F]... [--durations FILE] [--shard I/N --json DIR] PATH... [-- PYTEST_FLAGS]
    pytest_shards.py --merge DIR N

The files come from `pytest --collect-only`, so a path pytest would not run is not run here. A file with a failing test, a collection
error, or no test (a file that collected tests and ran none) fails the run; the output of every failed file is printed whole, then one
summary line, which counts the tests passed and skipped. Exit 0 only when every collected test file ran and passed or skipped, and
every --must-pass file passed at least one test and skipped none: a file that skips everything (its inputs missing) is not a pass. A failed
--rerun file is run once more and both verdicts are printed (the run stays red): a red that passes again is a flaky one, one that fails again is not."""
import os
import re
import math
import hashlib
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
HOSTED = Path(__file__).with_name("pytest_hosted.json")  # seconds of each file on a hosted runner: the split of a sharded run reads only this
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


def units(tests: dict[str, list[str]], known: dict[str, float], with_seconds: bool = False) -> list:
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
    ordered = sorted(out, key=lambda u: -u[2])
    return ordered if with_seconds else [(label, args) for label, args, _ in ordered]


HOSTED_UNKNOWN = 7.0  # run 8: 1653 CPU-seconds over 242 files; the 40 s a file has when nothing is known would make the long files light


def hosted_seconds(tests: dict[str, list[str]], path: Path = HOSTED) -> dict[str, float]:
    """Seconds of every collected file for a sharded deal: the table's, else the seed of a file known to be slow, else an average file."""
    table = json.loads(path.read_text()) if path.is_file() else {}
    return {name: table.get(name, SEEDS.get(Path(name).name, HOSTED_UNKNOWN)) for name in tests}


def deal(work: list[tuple[str, list[str], float]], index: int, count: int) -> list[tuple[str, list[str]]]:
    """Shard `index` (1-based) of `count`: the units dealt, heaviest first, onto the lightest shard. Every shard computes the same deal from
    the same collection and the same table, so each unit is in exactly one shard."""
    if not 1 <= index <= count:
        raise SystemExit(f"shard {index} of {count}")
    load = [0.0] * count
    mine: list[tuple[str, list[str]]] = []
    for label, args, seconds in sorted(work, key=lambda u: (-u[2], u[0])):
        at = min(range(count), key=lambda i: (load[i], i))
        load[at] += seconds
        if at == index - 1:
            mine.append((label, args))
    return mine


def digest(ids) -> str:
    return hashlib.sha256("\n".join(sorted(ids)).encode()).hexdigest()[:16]


def ids_of(args: list[str], tests: dict[str, list[str]]) -> list[str]:
    """The test ids a unit's pytest arguments run: a file's all, or the ids themselves (a part of a split file)."""
    return [i for a in args for i in (tests[a] if a in tests else [a])]


def merge(folder: Path, count: int) -> int:
    """The verdict of the `count` shard files in `folder`: every shard is there and green, saw the same collection, and the shards' tests are that
    collection, each once. A shard that crashed left no file; a missing one is a failure, never a pass over its tests."""
    files = {i: folder / f"pytest-shard-{i}.json" for i in range(1, count + 1)}
    if missing := [i for i, f in files.items() if not f.is_file()]:
        print(f"FAIL pytest shard {' '.join(map(str, missing))} of {count} left no result: its tests were not run")
        return 1
    data = {i: json.loads(f.read_text()) for i, f in files.items()}
    problems = []
    whole = {d["whole"] for d in data.values()}
    if len(whole) != 1 or {d["count"] for d in data.values()} != {count}:
        problems.append("the shards collected different tests: " + " ".join(sorted(whole)))
    ids = [i for d in data.values() for i in d["ids"]]
    seen = {i for i in ids}
    if len(ids) != len(seen):
        problems.append(f"{len(ids) - len(seen)} tests ran in more than one shard")
    if len(whole) == 1 and digest(seen) != next(iter(whole)):
        problems.append(f"the shards' tests are not the collection: {len(seen)} tests, digest {digest(seen)} against {next(iter(whole))}")
    for i, d in sorted(data.items()):
        if d["ran"] != len(d["ids"]) or d["failed"]:
            problems.append(f"shard {i}: {d['ran']} reported of {len(d['ids'])}" + (f"; failed {' '.join(d['failed'])}" if d["failed"] else ""))
    passed, skipped = (sum(d[w] for d in data.values()) for w in ("passed", "skipped"))
    print(f"pytest shards merged: {count} shards, {len(seen)} tests, {passed} passed, {skipped} skipped, {len(problems)} problems")
    for one in problems:
        print("FAIL", one)
    return 1 if problems else 0


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
    ap.add_argument("--shard", metavar="I/N", help="only the units the hosted table deals to shard I of N; --json DIR then gets pytest-shard-I.json for --merge")
    ap.add_argument("--json", type=Path)
    ap.add_argument("--merge", nargs=2, metavar=("DIR", "N"), help="judge nothing: the verdict of the N pytest-shard-*.json in DIR")
    ap.add_argument("paths", nargs="*")
    argv = sys.argv[1:]
    at = argv.index("--") if "--" in argv else len(argv)
    args, flags = ap.parse_args(argv[:at]), argv[at + 1 :]  # what follows `--` is pytest's, whatever it looks like
    if args.merge:
        return merge(Path(args.merge[0]), int(args.merge[1]))
    if not args.paths:
        ap.error("no path")
    tests, broken = collect(args.paths, args.ignore, flags)
    if broken is not None:
        print(broken)
        print("pytest shards: collection failed")
        return 1
    if not tests:
        print("pytest shards: no test file collected")
        return 1
    known = json.loads(Path(args.durations).read_text()) if args.durations and Path(args.durations).exists() else {}
    if args.shard:  # the split must be the same in every shard: the checked-in table, never a runner's own durations
        index, count = map(int, args.shard.split("/"))
        work = deal(units(tests, hosted_seconds(tests), with_seconds=True), index, count)
    else:
        work = units(tests, known)
    results = run_all(work, flags, args.j)
    bad = {n: r for n, r in results.items() if r[0] != 0}
    counts = {n: tally(out) for n, (_, _, out) in results.items()}
    ran = sum(sum(c.values()) for c in counts.values())
    assigned = ids_of([a for _, args in work for a in args], tests)
    wanted = len(assigned)
    if args.shard and args.json:
        args.json.mkdir(parents=True, exist_ok=True)
        (args.json / f"pytest-shard-{index}.json").write_text(json.dumps({
            "shard": index, "count": count, "whole": digest(i for ids in tests.values() for i in ids), "ids": assigned, "ran": ran,
            "passed": sum(c.get("passed", 0) for c in counts.values()), "skipped": sum(c.get("skipped", 0) for c in counts.values()), "failed": sorted(bad),
        }))
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
    if args.shard:
        print(f"pytest shards: shard {index} of {count} ran {wanted} of {sum(map(len, tests.values()))} tests")
    if bad:
        print("FAILED files: " + " ".join(sorted(bad)))
    return 1 if bad or vacuous else 0


if __name__ == "__main__":
    sys.exit(main())
