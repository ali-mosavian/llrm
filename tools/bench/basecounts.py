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
import shutil
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
    path.write_text(json.dumps({"rows": {"/".join(k): v for k, v in rows.items()}, "programs": programs(root), "hash_version": HASH_VERSION}))
    return path


HASH_VERSION = 2  # 1: every file of the directory but expected.toml; 2: and not NOT_INPUT


def _is_input(name: str) -> bool:
    return name != "expected.toml" and Path(name).suffix not in NOT_INPUT


def _hash(files: list[tuple[str, bytes]]) -> str:
    digest = hashlib.sha256()
    for name, data in sorted(files):
        digest.update(name.encode() + b"\0" + data)
    return digest.hexdigest()[:12]


def programs(root: Path = ROOT) -> dict[str, str]:
    """Each benchmark's hash: its sources and settings (not its baseline, the waivers or what a run is checked against). A row of a program
    that differs between the base and the head compares two programs, not two compilers, and is not judged."""
    out = {}
    for settings in sorted((root / "bench").glob("**/bench.toml")):
        files = [(path.name, path.read_bytes()) for path in settings.parent.iterdir() if path.is_file() and _is_input(path.name)]
        out[str(settings.parent.relative_to(root / "bench"))] = _hash(files)
    return out


def programs_at(sha: str, root: Path = ROOT) -> dict[str, str]:
    """`programs()` of the tree of commit `sha`, read from git: the hash of a base that was stored under another HASH_VERSION."""
    listing = subprocess.run(["git", "ls-tree", "-r", "--name-only", sha, "--", "bench"], cwd=root, capture_output=True, text=True, check=True).stdout.splitlines()
    by_dir: dict[str, list[str]] = {}
    for path in listing:
        by_dir.setdefault(str(Path(path).parent), []).append(path)
    out = {}
    for directory, paths in sorted(by_dir.items()):
        if f"{directory}/bench.toml" not in paths:
            continue
        files = [(Path(path).name, subprocess.run(["git", "cat-file", "blob", f"{sha}:{path}"], cwd=root, capture_output=True, check=True).stdout) for path in paths if _is_input(Path(path).name)]
        out[str(Path(directory).relative_to("bench"))] = _hash(files)
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
    sha: str, opts: list[str], tree_of, build, measure_with, timed: bool = False, cache: Path | None = None, only: list[str] | None = None, root: Path = ROOT
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
        if stored.get("hash_version") != HASH_VERSION:  # counts do not depend on the hash; the programs' hashes do
            return have, programs_at(sha, root)
        return have, stored["programs"]
    if only:
        rows, hashes = measure_with(build(sha), opts, only)
        return have | {key: value for key, value in rows.items() if key[1] in OURS}, hashes
    rows, hashes = measure_with(build(sha), opts)
    have |= {key: value for key, value in rows.items() if key[1] in OURS}
    cache.mkdir(parents=True, exist_ok=True)
    part = path.with_suffix(".part")
    part.write_text(json.dumps({"rows": {"/".join(key): value for key, value in have.items()}, "programs": hashes, "hash_version": HASH_VERSION}))
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


def waived_rows(measured: dict, base: dict, waived: list[dict], same: set[str] | None = None) -> list[tuple[str, str, str, int, int]]:
    """(file, label, counter, base's, now) of each row a waiver excused: worse than the base and exactly at the waiver's count."""
    rows = []
    for key, got in sorted(measured.items()):
        was = base.get(key)
        if key[1] not in OURS or was is None or (same is not None and key[0] not in same) or any(one in row for row in (got, was) for one in ("error", "skip", "known", "xpass")):
            continue
        for counter in COUNTERS:
            if counter in got and counter in was and got[counter] > was[counter]:
                rows += [(w["file"], label(key), counter, was[counter], got[counter]) for w in waived if w["label"] == label(key) and w["counter"] == counter and w["to"] == got[counter]]
    return rows


def waived_lines(rows: list[tuple], waived: list[dict], stale: list[str]) -> tuple[list[str], list[str]]:
    """(summary lines, problems): how many rows ran under which waiver file, each at its limit. Waivers on file that excused nothing and went
    stale nowhere mean the instrument is not reading them: a judge that printed 0 problems with every waiver unread looks the same as a clean run."""
    per_file: dict[str, int] = {}
    for one in rows:
        per_file[one[0]] = per_file.get(one[0], 0) + 1
    lines = [f"WAIVED {len(rows)} rows of {len(waived)} waivers (limit = measured): " + (", ".join(f"{f} {n}" for f, n in sorted(per_file.items())) or "none")]
    problems = ["WAIVERS: none was applied and none is stale: the waivers are not being read"] if waived and not rows and not stale else []
    return lines, problems


def coverage(measured: dict, base: dict, same: set[str]) -> tuple[int, int, list[str], list[str]]:
    """(rows judged, programs judged, programs left out as edited, problems): what `judge` looked at. A program the base and the head share
    that yields no judged row is a problem, and so is a run that judged none: 424 measurements once passed with `same` empty, 218 waivers
    stale and nothing compared. The left-out programs are named so that a growing list is seen."""
    rows: dict[str, int] = {}
    skipped: dict[str, set[str]] = {}
    for key, got in measured.items():
        if key[1] not in OURS:
            continue
        was = base.get(key)
        why = "no base" if was is None else next((one for row in (got, was) for one in ("error", "skip", "known", "xpass") if one in row), None)
        if key[0] in same and why is None:
            rows[key[0]] = rows.get(key[0], 0) + 1
        elif key[0] in same:
            skipped.setdefault(key[0], set()).add(why)
    programs = sorted({key[0] for key in measured if key[1] in OURS})
    left_out = [name for name in programs if name not in same]
    problems = [f"COVERAGE {name}: no row was judged (rows skipped: {', '.join(sorted(skipped.get(name, {'none measured'})))})" for name in programs if name in same and name not in rows]
    if not same and programs:
        problems = ["COVERAGE: no row was judged: no program is the same in the base and the head"]
    return sum(rows.values()), len(rows), left_out, problems


def coverage_lines(judged: int, programs: int, left_out: list[str]) -> list[str]:
    return [f"JUDGED {judged} rows of {programs} programs" + (f"; left out, edited since the base: {' '.join(left_out)}" if left_out else "")]


def stale_summary(stale: list[str]) -> list[str]:
    """The stale waivers (judge's third value) as one line naming their files: 29 rows of them buried the failures under them."""
    files = sorted({one.split(":", 1)[0] for one in stale})
    return [f"BASE-STALE {len(stale)} waivers in {len(files)} files, delete them: {' '.join(files)}"] if stale else []


def measure_in_subprocess(directory: Path, opts: list[str], work: Path, timed: bool = False, only: list[str] | None = None, tree: Path | None = None) -> tuple[dict, dict[str, str]]:
    """(bench.py's rows, programs()) of the tree `directory` (a release directory of measure.py's build tree, `<build>/target/release`) was
    built from, by that tree's own bench.py with its compiler: the harness and the programs as the base had them (the head's harness may
    pass flags the base's compiler does not know). A subprocess: the harness binds its compiler's directory at import."""
    tree = tree or directory.parents[1] / "tree"
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


def measure_head_sources(directory: Path, opts: list[str], work: Path, names: list[str], head: Path = ROOT) -> dict:
    """The base compiler's rows for `names` built from the head's sources: a copy of the base's tree whose bench/NAME directories are the
    head's, measured by that tree's own bench.py."""
    tree = directory.parents[1] / "tree"
    overlay = work / "overlay"
    shutil.rmtree(overlay, ignore_errors=True)
    shutil.copytree(tree, overlay, symlinks=True, ignore=lambda directory, names: [n for n in names if n == ".git" or (n == "target" and Path(directory) == tree)])  # crates/target is source
    for name in names:
        shutil.rmtree(overlay / "bench" / name, ignore_errors=True)
        shutil.copytree(head / "bench" / name, overlay / "bench" / name)
    return measure_in_subprocess(directory, opts, work, False, names, overlay)[0]


def with_edited(base: dict, was: dict[str, str], now: dict[str, str], opts: list[str], measure_with, cache: Path | None = None, tree: str = "") -> tuple[dict, set[str], list[str]]:
    """(the base's rows with those of the programs edited since the base, the programs the base and the head have alike, the edited rows the
    base compiler could not build): an edited program is judged too, its base being the base compiler's counts on the head's sources.
    `measure_with(names)` returns those rows; stored by the base's tree and the edited programs' hashes, so measured once."""
    same = {name for name in now if was.get(name) == now[name]}
    edited = sorted(name for name in now if name in was and was[name] != now[name])
    if not edited:
        return base, same, []
    cache = cache or CACHE
    key = hashlib.sha256(json.dumps([tree, opts, [(name, now[name]) for name in edited]]).encode()).hexdigest()[:20]
    path = cache / f"{tree}-edited-{key}.json"
    if path.is_file():
        rows = rows_of(json.loads(path.read_text()))
    else:
        rows = {k: v for k, v in measure_with(edited).items() if k[1] in OURS}
        cache.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"/".join(k): v for k, v in rows.items()}))
    unbuilt = sorted(label(k) for k, v in rows.items() if "error" in v or "skip" in v)
    return base | rows, same | {name for name in edited if any(k[0] == name for k in rows)}, unbuilt


if __name__ == "__main__":
    if sys.argv[1:2] == ["key"]:
        print(compiler_key(sys.argv[2] if len(sys.argv) > 2 else "HEAD"))
    elif sys.argv[1:2] == ["record"]:  # record ROWS.json DIR: the counts of HEAD
        print(record(rows_of(json.loads(Path(sys.argv[2]).read_text())), Path(sys.argv[3]), compiler_key("HEAD")))
    else:
        sys.exit("usage: basecounts.py key [REV] | record ROWS.json DIR")
