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
OURS = ("bas", "c", "nib", "bq16", "bq32")
COUNTERS = ("instructions", "memory_operands", "code_bytes", "data_bytes", "bss_bytes")
# Not under bench/: every tool that lists the benchmarks took a directory there for a program (a "waivers" with no waivers.c).
WAIVERS = "tools/bench/waivers"


# Files in a benchmark directory no compiler reads: the output a run is checked against (`.out`, `.ci.out`, BC's `.bcout`) and a listing.
NOT_INPUT = {".out", ".bcout", ".asm"}


# What changes nothing a count is made of: a commit that touches only these has its parent's counts.
INERT = {"docs", "editors", ".github", ".zed", ".gitignore", "license"}  # and every top-level .md


def compiler_key(sha: str, root: Path = ROOT) -> str:
    """What `sha`'s counts depend on, as one name: the hash of its top-level entries but the inert ones (docs, editor files, workflows). The
    key of the cache of a base's counts, so a docs-only commit finds its parent's, where its tree is new."""
    listing = subprocess.run(["git", "ls-tree", sha], cwd=root, capture_output=True, text=True, check=True).stdout.splitlines()
    names = [line.split("\t", 1)[1].lower() for line in listing]
    kept = [line for line, name in zip(listing, names) if name not in INERT and not name.endswith(".md")]
    return hashlib.sha256("\n".join(kept).encode()).hexdigest()[:20]


def record(rows: dict, cache: Path, key: str, root: Path = ROOT) -> Path:
    """Store the rows of ours (bench.py's measured rows), with the programs' hashes, as the counts of the tree `key` names: the file
    `base_counts` reads for a base. A main push does this for its own commit, so the next pull requests find their base measured."""
    rows = {k: v for k, v in rows.items() if k[1] in OURS}
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / f"{key}-own.json"
    path.write_text(json.dumps({"rows": {"/".join(k): v for k, v in rows.items()}, "programs": programs(root)}))
    return path


def programs(root: Path = ROOT) -> dict[str, str]:
    """Each benchmark's hash: its sources and settings (not its baseline, the waivers or what a run is checked against). A row of a program
    that differs between the base and the head compares two programs, not two compilers, and is not judged."""
    out = {}
    for settings in sorted((root / "bench").glob("**/bench.toml")):
        digest = hashlib.sha256()
        for one in sorted(path for path in settings.parent.iterdir() if path.is_file() and path.name != "expected.toml" and path.suffix not in NOT_INPUT):
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


def base_counts(
    sha: str, opts: list[str], tree_of, build, measure_with, timed: bool = False, cache: Path | None = None, only: list[str] | None = None
) -> tuple[dict[tuple[str, str, str], dict], dict[str, str]]:
    """(`sha`'s rows (ours only), its programs' hashes): stored by its tree, else measured with the compiler `build(sha)` names (a release
    directory) by `measure_with(directory, opts)`, which returns (bench.py's rows, programs()) of the base's own tree: its harness and its
    programs, as it was. Nothing of the head enters the key, so a base is measured once. `only` (benchmark names; a shard of the bench step):
    a miss measures just those, with `measure_with(directory, opts, only)`, and stores nothing: a part is not the base."""
    cache = cache or CACHE
    tree = tree_of(sha) or sha
    path = cache / f"{tree}-own{'-timed' if timed else ''}.json"
    stored = json.loads(path.read_text()) if path.is_file() else {"rows": {}, "programs": {}}
    have = rows_of(stored["rows"])
    if all(any(key[2] == opt for key in have) for opt in opts):
        return have, stored["programs"]
    if only:
        rows, hashes = measure_with(build(sha), opts, only)
        return have | {key: value for key, value in rows.items() if key[1] in OURS}, hashes
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


def stale_summary(stale: list[str]) -> list[str]:
    """The stale waivers (judge's third value) as one line naming their files: 29 rows of them buried the failures under them."""
    files = sorted({one.split(":", 1)[0] for one in stale})
    return [f"BASE-STALE {len(stale)} waivers in {len(files)} files, delete them: {' '.join(files)}"] if stale else []


def measure_in_subprocess(directory: Path, opts: list[str], work: Path, timed: bool = False, only: list[str] | None = None) -> tuple[dict, dict[str, str]]:
    """(bench.py's rows, programs()) of the tree `directory` (a release directory of measure.py's build tree, `<build>/target/release`) was
    built from, by that tree's own bench.py with its compiler: the harness and the programs as the base had them (the head's harness may
    pass flags the base's compiler does not know). A subprocess: the harness binds its compiler's directory at import."""
    tree = directory.parents[1] / "tree"
    out = work / "base.json"
    work.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "LLRM_BIN": str(directory)}
    env.pop("LLRM_BENCH_BASE", None)
    command = [sys.executable, str(tree / "tools/bench/bench.py"), "--opt", *opts, "--json", str(out), "--work", str(work / "run")]
    if timed:
        command.append("--time")
    command += only or []
    if (tree / "tools/bench/bench.py").is_file() and "--no-judge" in (tree / "tools/bench/bench.py").read_text():
        command.append("--no-judge")
    done = subprocess.run(command, cwd=tree, env=env, capture_output=True, text=True)
    if not out.is_file():
        raise SystemExit("the base did not measure: " + " | ".join(line.strip() for line in (done.stdout + done.stderr).splitlines()[-4:] if line.strip()))
    return rows_of(json.loads(out.read_text())), programs(tree)


if __name__ == "__main__":
    if sys.argv[1:2] == ["key"]:
        print(compiler_key(sys.argv[2] if len(sys.argv) > 2 else "HEAD"))
    elif sys.argv[1:2] == ["record"]:  # record ROWS.json DIR: the counts of HEAD
        print(record(rows_of(json.loads(Path(sys.argv[2]).read_text())), Path(sys.argv[3]), compiler_key("HEAD")))
    else:
        sys.exit("usage: basecounts.py key [REV] | record ROWS.json DIR")
