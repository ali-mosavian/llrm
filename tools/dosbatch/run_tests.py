"""
tests/run: compile each program with llrm, run them all in one DOSBox launch,
diff stdout with NAME.out.

    python3 tools/dosbatch/run_tests.py [LANG|NAME]... [--work DIR]

A header comment holds a program's settings:

    ' flags: -Os --cpu P5      extra compiler flags (default: -O2 --cpu 486)
    ' known: #123              fails today, tracked by issue 123

A known program that passes fails the run: remove its mark.
"""

from __future__ import annotations

import re
import sys
import argparse
import subprocess
from pathlib import Path
from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
from dosbatch import BIN, ROOT, Job  # noqa: E402

RUN = ROOT / "tests" / "run"
DEFAULT_FLAGS = ["-O2", "--cpu", "486"]
HEADER = re.compile(r"^\s*(?:'|//|#)\s*(flags|known):\s*(.*?)\s*$")
COMPILERS = {
    ".bas": ["llrm-qb", "--dialect", "qb45", "--runtime", "qb45"],
}


@dataclass
class Program:
    source: Path
    flags: list[str]
    known: str | None

    @property
    def name(self) -> str:
        return f"{self.source.parent.name}/{self.source.stem}"


def header(source: Path) -> tuple[list[str], str | None]:
    flags, known = DEFAULT_FLAGS, None
    for line in source.read_text(encoding="latin1").splitlines():
        found = HEADER.match(line)
        if not found:
            if line.strip() and not re.match(r"^\s*(?:'|//|#)", line):
                break
            continue
        if found[1] == "flags":
            flags = found[2].split()
        else:
            known = found[2]
    return flags, known


def discover(selected: list[str]) -> list[Program]:
    programs = []
    for source in sorted(RUN.glob("*/*")):
        if source.suffix in COMPILERS and source.with_suffix(".out").exists():
            flags, known = header(source)
            programs.append(Program(source, flags, known))
    if selected:
        programs = [p for p in programs if p.source.parent.name in selected or p.source.stem in selected or p.name in selected]
    return programs


def lines(text: str) -> list[str]:
    return [one.rstrip() for one in text.replace("\r\n", "\n").split("\n") if one.strip()]


def first_difference(want: list[str], got: list[str]) -> str:
    for at in range(max(len(want), len(got))):
        w = want[at] if at < len(want) else "<end>"
        g = got[at] if at < len(got) else "<end>"
        if w != g:
            return f"line {at + 1}: want {w!r}, got {g!r}"
    return ""


def compile_one(program: Program, obj: Path) -> str | None:
    tool, *rest = COMPILERS[program.source.suffix]
    done = subprocess.run([str(BIN / tool), str(program.source), *rest, *program.flags, "-o", str(obj)],
                          capture_output=True, text=True, timeout=300)
    if done.returncode != 0 or not obj.exists():
        return "compile: " + (done.stderr or done.stdout).strip()[-600:]
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("select", nargs="*")
    parser.add_argument("--work", type=Path, default=ROOT / "target" / "tests-run")
    args = parser.parse_args()
    programs = discover(args.select)
    if not programs:
        print("no programs selected")
        return 1
    work = args.work
    objs = work.with_name(work.name + "-obj")
    objs.mkdir(parents=True, exist_ok=True)
    stems = {p.name: f"T{at:03d}" for at, p in enumerate(programs)}
    with ThreadPoolExecutor() as pool:
        failed = dict(zip((p.name for p in programs), pool.map(lambda p: compile_one(p, objs / f"{stems[p.name]}.obj"), programs)))
    jobs = [Job(stems[p.name], "obj", objs / f"{stems[p.name]}.obj") for p in programs if not failed[p.name]]
    ran = dosbatch.run(jobs, work) if jobs else {}
    bad = passed = known = 0
    for program in programs:
        problem = failed[program.name]
        if not problem:
            result = ran[stems[program.name]]
            if result.status != "ok":
                problem = f"{result.status}: {result.detail}"
            else:
                problem = first_difference(lines(program.source.with_suffix(".out").read_text()), lines(result.text))
        if problem and program.known:
            known += 1
            print(f"KNOWN {program.name} ({program.known}): {problem}")
        elif problem:
            bad += 1
            print(f"FAIL  {program.name}: {problem}")
        elif program.known:
            bad += 1
            print(f"XPASS {program.name}: passes; remove its known mark ({program.known})")
        else:
            passed += 1
    print(f"{len(programs)} programs: {passed} pass, {known} known, {bad} fail")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
