"""Compiles every program of the run tests, the examples and the bench set at each level, for a sweep that watches
the compiler's stderr (an env switch's report). A program it cannot compile is listed in the header with the reason:
a sweep that quietly skipped a language read a class as one program's (the `.nib` rows, 2026-10-30)."""

import dataclasses
import os
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import run_tests

LEVELS = ("-O1", "-O2", "-Omax", "-Os")


def command(program, obj: Path) -> list[str] | None:
    """The compile command for `program`, or None where this sweep has no way to build it."""
    if program.source.suffix == ".c":
        return run_tests.compile_command(program, obj)
    if program.source.suffix == ".nib":
        return [str(run_tests.BIN / "llrm-nib"), str(program.source), *program.flags, "-o", str(obj)]
    return None


def plan(programs) -> tuple[list, dict[str, list[str]]]:
    """The (program, level) jobs, and the programs left out by reason."""
    jobs, skipped = [], {}
    for program in programs:
        base = [one for one in program.flags if not one.startswith("-O")]
        if command(program, Path("x.obj")) is None:
            skipped.setdefault(f"no compile path for {program.source.suffix} in this sweep", []).append(program.name)
            continue
        jobs += [dataclasses.replace(program, flags=[*base, level]) for level in LEVELS]
    return jobs, skipped


def header(built: int, total: int, skipped: dict[str, list[str]]) -> str:
    lines = [f"compiled {built} of {total} jobs"]
    lines += [f"skipped {len(names)} programs: {reason} (first: {names[0]})" for reason, names in skipped.items()]
    return "\n".join(lines)


def run(selected: list[str], env: dict[str, str], workers: int = 8) -> Counter:
    """Every job's stderr lines, counted; the header goes to stdout. Fails when a job that should build did not."""
    jobs, skipped = plan(run_tests.discover(selected))
    obj = Path(os.environ.get("TMPDIR", "/tmp")) / "compile_sweep"
    obj.mkdir(exist_ok=True)

    def one(job):
        target = obj / f"{abs(hash((str(job.source), tuple(job.flags), job.label)))}.obj"
        done = subprocess.run(command(job, target), capture_output=True, text=True, timeout=600, env={**os.environ, **run_tests.compile_env(job), **env})
        return done.returncode, done.stderr

    seen, built = Counter(), 0
    with ThreadPoolExecutor(workers) as pool:
        for code, stderr in pool.map(one, jobs):
            built += code == 0
            seen.update(stderr.splitlines())
    print(header(built, len(jobs), skipped))
    if built != len(jobs):
        sys.exit(f"{len(jobs) - built} jobs did not build")
    return seen
