"""
Bench counts against the base's, not against counts committed per program.

`bench/NAME/expected.toml` held every language's and level's count, and each PR blessed its own rows there, so two PRs that touched one
benchmark conflicted (all five of a train did, on jpeg_fixed). The counts are derived data: the baseline is what the base commit's compiler
measures with this tree's harness and programs, kept by the base's git tree like measure.py's (a landed tree is measured once). A row
worse than the base's fails; one better passes, and becomes the next base. A deliberate trade is a waiver, one file per PR in
tools/bench/waivers/ (unique names, so none conflict): the row, the counter and the count it is allowed to reach. It holds only while the row
is that count and worse than the base's, so it expires with the PR.
"""

from __future__ import annotations

import os
import re
import sys
import json
import hashlib
import tomllib
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
CACHE = Path(os.environ.get("LLRM_BENCH_BASE_DIR") or Path.home() / ".cache/llrm/bench-base")
# The rows the compiler under test makes: the reference compilers' (bas, ow, bcc, ...) do not move with it.
OURS = ("c", "nib", "bq16", "bq32")
COUNTERS = ("instructions", "memory_operands", "code_bytes", "data_bytes", "bss_bytes")
# Not under bench/: every tool that lists the benchmarks took a directory there for a program (a "waivers" with no waivers.c).
WAIVERS = "tools/bench/waivers"


def programs(root: Path = ROOT) -> dict[str, str]:
    """Each benchmark's hash: its sources and settings (not its baseline or the waivers). A row of a program that differs between the base
    and the head compares two programs, not two compilers, and is not judged."""
    out = {}
    for settings in sorted((root / "bench").glob("**/bench.toml")):
        digest = hashlib.sha256()
        for one in sorted(path for path in settings.parent.iterdir() if path.is_file() and path.name != "expected.toml"):
            digest.update(one.name.encode() + b"\0" + one.read_bytes())
        out[str(settings.parent.relative_to(root / "bench"))] = digest.hexdigest()[:12]
    return out


def rows_of(raw: dict) -> dict[tuple[str, str, str], dict]:
    """bench.py's --json keys, "name/language/opt" (the name may hold a slash), as (name, language, opt)."""
    out = {}
    for key, value in raw.items():
        name, language, opt = key.rsplit("/", 2)
        out[(name, language, opt)] = value
    return out


def label(key: tuple[str, str, str]) -> str:
    return f"{key[0]} {key[1]} -{key[2]}"


def base_counts(sha: str, opts: list[str], tree_of, build, measure_with, cache: Path | None = None) -> tuple[dict[tuple[str, str, str], dict], dict[str, str]]:
    """(`sha`'s rows (ours only), its programs' hashes): stored by its tree, else measured with the compiler `build(sha)` names (a release
    directory) by `measure_with(directory, opts)`, which returns (bench.py's rows, programs()) of the base's own tree: its harness and its
    programs, as it was. Nothing of the head enters the key, so a base is measured once."""
    cache = cache or CACHE
    tree = tree_of(sha) or sha
    path = cache / f"{tree}-own.json"
    stored = json.loads(path.read_text()) if path.is_file() else {"rows": {}, "programs": {}}
    have = rows_of(stored["rows"])
    if all(any(key[2] == opt for key in have) for opt in opts):
        return have, stored["programs"]
    rows, hashes = measure_with(build(sha), opts)
    have |= {key: value for key, value in rows.items() if key[1] in OURS}
    cache.mkdir(parents=True, exist_ok=True)
    part = path.with_suffix(".part")
    part.write_text(json.dumps({"rows": {"/".join(key): value for key, value in have.items()}, "programs": hashes}))
    os.replace(part, path)
    return have, hashes


def waivers(root: Path = ROOT) -> list[dict]:
    """tools/bench/waivers/*.toml: `reason`, and `[[row]]` entries of `label` ("fib bq32 -O2"), `counter` and `to`."""
    out = []
    for path in sorted((root / WAIVERS).glob("*.toml")):
        data = tomllib.loads(path.read_text())
        for row in data.get("row", []):
            out.append({**row, "reason": data.get("reason", ""), "file": path.name})
    return out


def consume_waivers(root: Path, measured: dict) -> list[str]:
    """Delete the waiver files whose every row the blessed counts now are: the baseline carries them, so the waiver is spent."""
    spent = []
    for path in sorted((root / WAIVERS).glob("*.toml")):
        rows = tomllib.loads(path.read_text()).get("row", [])
        parts = [row["label"].split(" ") for row in rows]
        if rows and all(measured.get((p[0], p[1], p[2].lstrip("-")), {}).get(row["counter"]) == row["to"] for p, row in zip(parts, rows)):
            path.unlink()
            spent.append(path.name)
    return spent


def judge(measured: dict, base: dict, waived: list[dict], same: set[str] | None = None) -> tuple[list[tuple], list[tuple], list[str]]:
    """(worse, better, stale waivers): each as (label, counter, base's, now). A row worse than the base's fails unless a waiver names
    its row and counter and the count it reaches; a waiver that excused nothing is stale. Rows the base lacks (a new program) and rows
    that errored or were skipped on either side are not judged; nor are rows of a program not in `same` (the programs both trees have unchanged)."""
    worse, better, used = [], [], set()
    for key, got in sorted(measured.items()):
        was = base.get(key)
        if key[1] not in OURS or was is None or (same is not None and key[0] not in same) or any(one in row for row in (got, was) for one in ("error", "skip", "known", "xpass")):
            continue
        for counter in COUNTERS:
            if counter not in got or counter not in was:
                continue
            now, then = got[counter], was[counter]
            if now > then:
                excused = [w for w in waived if w["label"] == label(key) and w["counter"] == counter and w["to"] == now]
                if excused:
                    used.update((w["file"], w["label"], w["counter"]) for w in excused)
                else:
                    worse.append((label(key), counter, then, now))
            elif now < then:
                better.append((label(key), counter, then, now))
    stale = [f"{w['file']}: {w['label']} {w['counter']} to {w['to']}" for w in waived if (w["file"], w["label"], w["counter"]) not in used]
    return worse, better, stale


OLD = re.compile(r"^(?P<label>.+): (?P<counter>[a-z_]+) (?P<now>\d+) (?P<op>[<>]) (?P<then>\d+) \((?P<kind>worse|better)")


def differences(old: list[str], worse: list[tuple], better: list[tuple]) -> list[str]:
    """Where the committed baselines and the base disagree: rows one judge fails or improves that the other does not."""
    seen_old = {}
    for line in old:
        found = OLD.match(line)
        if found and found["label"].split(" ")[1] in OURS:
            seen_old[(found["label"], found["counter"])] = found["kind"]
    seen_new = {(one[0], one[1]): "worse" for one in worse} | {(one[0], one[1]): "better" for one in better}
    out = []
    for key in sorted(seen_old.keys() | seen_new.keys()):
        if seen_old.get(key) != seen_new.get(key):
            out.append(f"{key[0]} {key[1]}: expected.toml says {seen_old.get(key, 'same')}, the base says {seen_new.get(key, 'same')}")
    return out


def measure_in_subprocess(directory: Path, opts: list[str], work: Path) -> tuple[dict, dict[str, str]]:
    """(bench.py's rows, programs()) of the tree `directory` (a release directory of measure.py's build tree, `<build>/target/release`) was
    built from, by that tree's own bench.py with its compiler: the harness and the programs as the base had them (the head's harness may
    pass flags the base's compiler does not know). A subprocess: the harness binds its compiler's directory at import."""
    tree = directory.parents[1] / "tree"
    out = work / "base.json"
    work.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "LLRM_BIN": str(directory)}
    env.pop("LLRM_BENCH_BASE", None)
    command = [sys.executable, str(tree / "tools/bench/bench.py"), "--opt", *opts, "--json", str(out), "--work", str(work / "run")]
    if (tree / "tools/bench/bench.py").is_file() and "--no-judge" in (tree / "tools/bench/bench.py").read_text():
        command.append("--no-judge")
    done = subprocess.run(command, cwd=tree, env=env, capture_output=True, text=True)
    if not out.is_file():
        raise SystemExit("the base did not measure: " + " | ".join(line.strip() for line in (done.stdout + done.stderr).splitlines()[-4:] if line.strip()))
    return rows_of(json.loads(out.read_text())), programs(tree)
