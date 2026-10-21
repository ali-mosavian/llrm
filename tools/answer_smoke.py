#!/usr/bin/env python3
"""answer_smoke.py [--bin DIR] [--work DIR] [PROGRAM...]: do the programs print what they should?

Each PROGRAM (default crc, nbody, NBODY_SINGLE, NBODY_FIXED and JPEG_FIXED) is a bench/NAME/NAME.bas with its expected output NAME.out.
It is compiled with its header's flags (-O2) by DIR's llrm-qb (default $LLRM_BIN or this tree's) for both targets (m16 real mode, m32 under DOS/32A), run in
DOSBox, and its output compared line by line (blanks and trailing spaces apart). Prints the binary's md5 and one line per run; the exit
code is the number of wrong answers, builds that failed and runs that did not end.

Not the bench: no counters, no emulator. A compiler that miscompiles `crc` or `nbody` prints another number, whatever it counts."""

from __future__ import annotations

import argparse
import hashlib
import os
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "dosbatch"))

import dosbatch  # noqa: E402
import qb32demos  # noqa: E402
import qbbuild  # noqa: E402
import qbruntime  # noqa: E402
import run_tests  # noqa: E402

DEFAULT = ("crc", "nbody", "nbody_single", "nbody_fixed", "jpeg_fixed")
BUDGET_MS = 120_000
BENCH = dosbatch.ROOT / "bench"


def same(got: str, want: str) -> bool:
    """The printed text and the expected, as lines: BASIC's PRINT of a number leads with a blank."""
    return [one.strip() for one in run_tests.lines(got)] == [one.strip() for one in run_tests.lines(want)]


def check(
    names: tuple[str, ...],
    run: Callable[[str, str], tuple[str, str]],
    bench: Path = BENCH,
) -> list[str]:
    """One line per (program, target); `run(source, target)` is (status, output). A line starting with ok is a right answer."""
    out = []
    for name in names:
        want = (bench / name / f"{name}.out").read_text(errors="replace")
        for target in qbbuild.TARGETS:
            label = f"{name} m{dosbatch.m_flag(target)[2:]}"
            status, output = run(str(bench / name / f"{name}.bas"), target)
            if status != "ok":
                out.append(f"{label}: {status}")
            elif same(output, want):
                out.append(f"ok {label}")
            else:
                out.append(f"WRONG {label}: want {run_tests.lines(want)[:2]}, got {run_tests.lines(output)[:2]}")
    return out


def demo_lines(results: dict) -> list[str]:
    """One line per (demo, target) of `demos`' results, {demo: {target: differing checkpoints | the reason it was not played}} or a demo's own
    exception: ok where it was played and shows what BCOM45 shows where nothing is random."""
    out = []
    for name in sorted(results):
        if isinstance(results[name], Exception):
            out.append(f"{name}: could not be played: {str(results[name]).strip()[-200:]}")
            continue
        for target, wrong in results[name].items():
            label = f"{name} m{dosbatch.m_flag(target)[2:]}"
            if isinstance(wrong, str):
                out.append(f"{label}: did not build: {wrong[-200:]}")
            else:
                out.append(f"ok {label} (played)" if not wrong else f"WRONG {label}: the screens differ at {wrong}")
    return out


def played(sources: dict[str, Path], work: Path) -> dict:
    """qb32demos.run, but a target that does not build is that target's result and the other is still played (a refusal at m32 hid m16)."""
    objects = qbbuild.runtimes(work)

    def one(name: str) -> dict:
        built = qbbuild.build(sources[name], work / name / "build", objects, tolerant=True)
        ours = {target: shown for target, shown in built.items() if not isinstance(shown, str)}
        out: dict = {target: shown for target, shown in built.items() if isinstance(shown, str)}
        if ours:
            want = qb32demos.reference_shots(name, sources[name], work / name / "ref")
            got = qb32demos.candidate_shots(name, ours, work / name / "cand")
            out |= {target: qb32demos.differences(name, want, shots, target) for target, shots in got.items()}
        return out

    from concurrent.futures import ThreadPoolExecutor

    with ThreadPoolExecutor(len(sources)) as pool:
        futures = {name: pool.submit(one, name) for name in sources}
        return {name: qb32demos.outcome(future) for name, future in futures.items()}


def demos(work: Path) -> list[str]:
    """NIBBLES and GORILLA (QB45_DEMOS_DIR, else ~/scratch/gor) played with typed keys at m16 and m32 and compared with BCOM45's screens
    (tools/dosbatch/qb32demos.py): the user's check of the Microsoft demos."""
    found, reason = qbruntime.demo_sources(Path(os.environ.get("QB45_DEMOS_DIR", Path.home() / "scratch" / "gor")))
    if not found:
        return [f"demos: not played: {reason}"]
    return demo_lines(played({path.stem.upper(): path for path in found.values()}, work / "demos"))


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("programs", nargs="*", default=list(DEFAULT))
    parser.add_argument("--work", type=Path)
    parser.add_argument("--no-demos", action="store_true", help="leave out NIBBLES and GORILLA, played at m16 and m32 against BCOM45's screens")
    args = parser.parse_args(argv)
    binary = dosbatch.BIN / "llrm-qb"
    print(f"llrm-qb {hashlib.md5(binary.read_bytes()).hexdigest()[:12]} ({dosbatch.BIN})", flush=True)
    with tempfile.TemporaryDirectory() as scratch:
        work = args.work or Path(scratch)
        objects = qbbuild.runtimes(work / "runtime")
        built: dict[str, dict] = {}

        def run(source: str, target: str) -> tuple[str, str]:
            name = Path(source).stem
            settings = run_tests.header(Path(source))
            if name not in built:
                flags = tuple(settings["flags"].split("|")[0].split()) if "flags" in settings else ("-O2",)
                built[name] = qbbuild.build(Path(source), work / name, objects, flags, tolerant=True)
            one = built[name][target]
            if isinstance(one, str):
                return f"did not build: {one}", ""
            data = tuple(Path(source).parent / one for one in settings.get("data", "").split())  # a program that reads a file: its `' data:` header
            ran = qbbuild.run(one, work / name / f"run-{dosbatch.m_flag(target)}", data, budget_ms=BUDGET_MS)
            return ran.status, ran.output.decode("latin1")

        lines = check(tuple(args.programs), run)
        if not args.no_demos:
            lines += demos(work)
    print("\n".join(lines))
    wrong = sum(not one.startswith("ok ") for one in lines)
    print(f"{wrong} wrong of {len(lines)}")
    return wrong


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
