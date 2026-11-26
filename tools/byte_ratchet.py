#!/usr/bin/env python3
"""Per-program, per-level code byte ceilings of a target's corpus.

    tools/byte_ratchet.py TARGET [--update] [--jobs N]

A target is a table of tools/byte_ratchet.toml: its corpus, its levels, the commands that build one program and where
the bytes are read. Ceilings are in tools/byte_ratchet_TARGET.tsv: a number of bytes, or `no:STEP` for a cell that
does not build. Exit 1 on a cell over its ceiling, a ceiling cell that does not build, an unbuilt cell that does, or a
program and a row without each other. A smaller cell passes; --update writes what was measured.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import corpus_targets  # noqa: E402
import llrmbin  # noqa: E402
import pin_clang  # noqa: E402

TABLE = HERE / "byte_ratchet.toml"


@dataclass
class Unbuilt:
    step: str
    why: str


def judge(target, levels, ceilings, measured):
    """(failure lines, cells below their ceiling). `ceilings` and `measured` are {program: [cell per level]}."""
    failures, below = [], 0
    for program in sorted(ceilings.keys() | measured.keys()):
        if program not in measured:
            failures.append(f"{target} {program}: listed, but not in the corpus")
            continue
        if program not in ceilings:
            failures.append(f"{target} {program}: in the corpus, with no ceiling")
            continue
        for level, ceiling, got in zip(levels, ceilings[program], measured[program]):
            cell = f"{target} {program} -{level}:"
            if isinstance(ceiling, str):
                if not isinstance(got, Unbuilt):
                    failures.append(f"{cell} builds now ({got} B) but is listed {ceiling}")
            elif isinstance(got, Unbuilt):
                failures.append(f"{cell} did not build ({got.step}: {got.why}), ceiling {ceiling} B")
            elif got > ceiling:
                failures.append(f"{cell} {got} B, ceiling {ceiling} B (+{got - ceiling})")
            elif got < ceiling:
                below += 1
    return failures, below


def read(path, levels):
    lines = [line.split("\t") for line in Path(path).read_text().splitlines() if line and not line.startswith("#")]
    if not lines or lines[0] != ["program", *levels]:
        sys.exit(f"byte_ratchet: {path} does not have the columns program, {', '.join(levels)}: run --update")
    return {row[0]: [one if one.startswith("no:") else int(one) for one in row[1:]] for row in lines[1:]}


def write(path, levels, rows):
    out = ["# code bytes ceilings; written by tools/byte_ratchet.py --update", "\t".join(["program", *levels])]
    out += ["\t".join([program, *(f"no:{one.step}" if isinstance(one, Unbuilt) else str(one) for one in rows[program])]) for program in sorted(rows)]
    Path(path).write_text("\n".join(out) + "\n")


def programs(config, root=ROOT):
    """The corpus: sources with an expected output that are for the target, relative to the root and without suffix."""
    out = []
    for pattern in config["corpus"]:
        for path in sorted(root.glob(pattern)):
            if path.with_suffix(".out").exists() and corpus_targets.applies(config["corpus_target"], path.read_text(), path):
                out.append(path.relative_to(root).with_suffix("").as_posix())
    return out


def tools(config, bins):
    """The placeholders that name tools; a missing one stops the run, so it is never read as a program that does not build."""
    compiler = Path(bins) / config["compiler"]
    if not os.access(compiler, os.X_OK):
        sys.exit(f"byte_ratchet: no compiler {compiler}: build it, or set LLRM_BIN")
    if missing := [one for one in config["needs"] if not shutil.which(one)]:
        sys.exit(f"byte_ratchet: not on PATH: {' '.join(missing)}")
    return {"compiler": str(compiler), "root": str(ROOT)}


def cell(config, names, program, level, work):
    """The bytes of one program at one level, or why there are none."""
    work.mkdir(parents=True)
    names = {**names, "level": config["levels"][level], "source": f"{program}.c", "work": str(work)}
    for step in config["step"]:
        command = [one.format(**names) for one in step["command"]]
        limit = ["prlimit", f"--cpu={config['cpu_seconds']}", f"--as={16 << 30}"]
        done = subprocess.run([*limit, *command], cwd=ROOT, capture_output=True, text=True, errors="replace")
        if done.returncode:
            lines = (done.stderr + done.stdout).strip().splitlines()
            why = next((line for line in lines if "error:" in line or "REFUSED" in line), lines[-1] if lines else f"exit {done.returncode}")
            return Unbuilt(step["name"], why.replace(str(ROOT) + "/", "")[:160])
    sized = Path(config["sized"].format(**names)).read_text()
    found = re.findall(config["bytes_pattern"], sized, re.M)
    if not found:
        return Unbuilt("size", "no size record")
    total = sum(int(one, config["radix"]) for one in found)
    return total or Unbuilt("size", "0 bytes")


def measure(config, names, jobs):
    corpus = programs(config)
    if not corpus:
        sys.exit("byte_ratchet: the corpus is empty")
    with tempfile.TemporaryDirectory(prefix="byte-ratchet-") as scratch, ThreadPoolExecutor(jobs) as pool:
        cells = {(program, level): pool.submit(cell, config, names, program, level, Path(scratch) / program / level) for program in corpus for level in config["levels"]}
        return {program: [cells[program, level].result() for level in config["levels"]] for program in corpus}


def main():
    targets = tomllib.loads(TABLE.read_text())
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("target", choices=targets)
    ap.add_argument("--update", action="store_true")
    ap.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS") or max(1, (os.cpu_count() or 4) // 4)))
    args = ap.parse_args()
    config = targets[args.target]
    names = tools(config, llrmbin.bin_dir())
    names["clang"] = pin_clang.path()
    ceilings_file = HERE / f"byte_ratchet_{args.target}.tsv"
    levels = list(config["levels"])
    start = time.time()
    measured = measure(config, names, args.jobs)
    unbuilt = [(program, level, one) for program, row in sorted(measured.items()) for level, one in zip(levels, row) if isinstance(one, Unbuilt)]
    for program, level, one in unbuilt:
        print(f"unbuilt {program} -{level}: {one.step}: {one.why}")
    print(f"{args.target}: {len(measured)} programs, {len(levels) * len(measured)} cells, {len(unbuilt)} unbuilt, {time.time() - start:.0f}s")
    if args.update:
        write(ceilings_file, levels, measured)
        print(f"wrote {ceilings_file}")
        return 0
    failures, below = judge(args.target, levels, read(ceilings_file, levels), measured)
    if below:
        print(f"{below} cells are below their ceiling: --update locks them in")
    if failures:
        print("\n".join(failures))
        print(f"{len(failures)} failures. If intended: python3 tools/byte_ratchet.py {args.target} --update, commit {ceilings_file.relative_to(ROOT)} and say why")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
