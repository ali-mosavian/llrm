#!/usr/bin/env python3
"""clocks.py DIST LEVEL OUT.json [FILE...]: user-mode instructions of tests/run/c programs, ours (DIST/llrm-cc) against gcc and clang.

One run of the whole program, minus the same link's empty main. The output must equal the .out file.

A count is read the one way for every column and every run (`count`): the binary is copied to a path of one fixed length, run
under `env -i` with ASLR off, and the least of three runs is taken. The libc start-up scales with the length of argv[0] (+56
instructions for 8 more characters) and of the environment (+235 for one 300-byte variable, -25788 under `env -i`), so a binary
called `tag.ours` beside one called `tag.clang`, or the same one run from another shell, read differently: a cell that tied
with the best of gcc and clang flipped from counted to not on no code change (6 O2 and 39 O3 cells, 2026-11-15).
`verdicts` reports a cell as gained or lost only where its count moved by more than 1%.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRATCH = Path(tempfile.gettempdir()) / "llrm-clocks"
FLAGS = ["-fPIE"]
REPEATS = 3
MOVED = 0.01


def sh(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, timeout=300, **kw)


def count(exe: Path, repeats: int = REPEATS) -> int | None:
    """The least `instructions:u` of `repeats` runs of `exe` copied to a canonical path, in an empty environment."""
    SCRATCH.mkdir(parents=True, exist_ok=True)
    home = Path(tempfile.mkdtemp(prefix="c", dir=SCRATCH))  # mkdtemp's names are one length
    try:
        copy = home / "p"
        shutil.copy2(exe, copy)
        runs = []
        for _ in range(repeats):
            done = sh(["env", "-i", "setarch", "-R", "perf", "stat", "-x,", "-e", "instructions:u", "./p"], cwd=home)
            for line in done.stderr.splitlines():
                field = line.split(",")
                if len(field) > 2 and field[2].startswith("instructions") and field[0].isdigit():
                    runs.append(int(field[0]))
        return min(runs) if runs else None
    finally:
        shutil.rmtree(home, ignore_errors=True)


def moved(was: int, now: int, tolerance: float = MOVED) -> bool:
    """A count that changed by more than `tolerance` of the old one."""
    return abs(now - was) > tolerance * max(was, 1)


def verdicts(base: dict, new: dict) -> dict[str, list[str]]:
    """The cells of two runs (`rows` of a result) whose count moved, split by whether `ours` now beats the best of gcc and clang.

    gained: moved and ours is at or under the best now, was not before. lost: the reverse. A cell that did not move by more
    than 1% is neither, whatever side of a tie it landed on. gcc and clang are the same binaries in both runs, so a change in
    either column is drift of the instrument (merge_icmps_fields: both fell 4-5% on no code change): `ours` must move by more
    than the larger of 1% and that drift.
    """
    def best(cell):
        return min(cell["gcc"]["instr"], cell["clang"]["instr"])

    out: dict[str, list[str]] = {"gained": [], "lost": [], "moved": []}
    for name in sorted(base.keys() & new.keys()):
        was, now = base[name], new[name]
        try:
            before, after = was["ours"]["instr"], now["ours"]["instr"]
            old_best, new_best = best(was), best(now)
        except (KeyError, TypeError):
            continue
        if before is None or after is None:
            continue
        drift = max(abs(now[column]["instr"] - was[column]["instr"]) / max(was[column]["instr"], 1) for column in ("gcc", "clang"))
        if not moved(before, after, max(MOVED, drift)):
            continue
        out["moved"].append(name)
        if after <= new_best and before > old_best:
            out["gained"].append(name)
        elif after > new_best and before <= old_best:
            out["lost"].append(name)
    return out


def main(argv: list[str]) -> int:
    dist, level, out = argv[0], argv[1], argv[2]
    files = [Path(f) for f in argv[3:]] or sorted((ROOT / "tests/run/c").glob("*.c"))
    work = Path(tempfile.mkdtemp(prefix="clocks-"))

    def build(column: str, source: Path, tag: str):
        obj, exe = work / f"{tag}.{column}.o", work / f"{tag}.{column}"
        if column == "ours":
            env = {**os.environ, "LLRM_CC_TARGET": "x86_64-linux"}
            done = sh([f"{dist}/llrm-cc", "-m64", f"-{level}", "-w", "-c", str(source), "-o", str(obj)], env=env)
        else:
            done = sh([column, *FLAGS, f"-{level}", "-w", "-c", str(source), "-o", str(obj)])
        if done.returncode:
            return None, "compile"
        link = sh(["gcc", "-static", "-no-pie", "-o", str(exe), str(obj), str(Path(__file__).with_name("clocks_report.c")), "-lm"])
        return (exe, None) if link.returncode == 0 else (None, "link")

    empty = work / "empty.c"
    empty.write_text("int main(void){return 0;}\n")
    base = {}
    for column in ("ours", "gcc", "clang"):
        exe, _ = build(column, empty, "empty")
        base[column] = count(exe) if exe else None

    def one(source: Path):
        tag, row = source.stem, {}
        want = source.with_suffix(".out").read_text() if source.with_suffix(".out").exists() else None
        for column in ("ours", "gcc", "clang"):
            exe, why = build(column, source, tag)
            if not exe:
                row[column] = {"error": why}
                continue
            try:
                ran = sh([str(exe)], cwd=source.parent)
            except subprocess.TimeoutExpired:
                row[column] = {"error": "timeout"}
                continue
            if ran.returncode != 0:
                row[column] = {"error": "exit"}
                continue
            n = count(exe)
            row[column] = {"instr": None if n is None else n - base[column], "out": ran.stdout, "ok": want is None or ran.stdout == want}
        return tag, row

    with ThreadPoolExecutor(int(os.environ.get("CLOCKS_JOBS", "6"))) as pool:
        rows = dict(pool.map(one, files))
    shutil.rmtree(work, ignore_errors=True)
    json.dump({"level": level, "base": base, "rows": rows}, open(out, "w"), indent=1)
    print("CLOCKS-DONE", len(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
