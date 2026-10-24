#!/usr/bin/env python3
"""qbrt_run.py WORK [--level -Ox] [--all | --only NAME...]: tests/qbrt against BCOM45 on both targets, at one level, from the command line.

The program selection of test_qbrt.py's differential, without pytest: by default the programs that use error handling (ON ERROR, RESUME, ERL, ERR),
`--all` every program of test_qbrt.SOURCES (tests/qbrt, the milestones and the examples; both runtimes, m16 and m32), `--only` the ones named.
A program that does not compile for the oracle (the qb45 runtime) is reported by name and left out of the run, never skipped silently. Exit
code: the number of programs that did not compile and of (program, target) runs that differ."""

from __future__ import annotations

import argparse
import re
import sys
from collections.abc import Callable
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

ERROR_HANDLING = re.compile(r"on error|resume|\berl\b|\berr\b", re.I)


def select(
    sources: dict[str, Path],
    everything: bool = False,
    only: tuple[str, ...] = (),
) -> list[str]:
    """The programs to run: `only` as named, else every one (`everything`), else those that handle errors."""
    if only:
        return list(only)
    if everything:
        return sorted(sources)
    return sorted(name for name, path in sources.items() if ERROR_HANDLING.search(path.read_text(errors="replace")))


def compiling(
    names: list[str],
    sources: dict[str, Path],
    compile_for_oracle: Callable[[Path, Path], str | None],
    work: Path,
) -> tuple[list[str], dict[str, str]]:
    """(the programs the oracle's route compiles, the reason for each that it does not)."""
    kept, failed = [], {}
    for name in names:
        reason = compile_for_oracle(sources[name], work / f"pre_{name}.obj")
        if reason:
            failed[name] = reason
        else:
            kept.append(name)
    return kept, failed


def differs(one) -> bool:
    """Whether a (program, target) run is not byte for byte BCOM45's."""
    return not (one.reference.status == "ok" and one.candidate.status == "ok" and one.difference == "" and one.screen_difference == "")


def unexpected(
    known: dict[tuple[str, str], str],
    name: str,
    target: str,
    one,
) -> str | None:
    """Why a run counts against the result, or None: a difference not in `known`, or a `known` one that now matches (the entry goes)."""
    if (name, target) in known:
        return None if differs(one) else f"now matches BCOM45: remove it from KNOWN ({known[name, target]})"
    return (one.candidate.detail or one.difference or one.screen_difference or "differs")[:150] if differs(one) else None


def level(text: str) -> str:
    """`-O1`, `O1`, `1` or `s` as the compiler's `-O1` / `-Os`."""
    return text if text.startswith("-") else "-O" + text.removeprefix("O")


def parse(argv: list[str]) -> argparse.Namespace:
    """The arguments, `--level -O1` as well as `--level=-O1` (argparse takes the first for an option of its own and refuses it)."""
    joined: list[str] = []
    for one in argv:
        if joined and joined[-1] == "--level":
            joined[-1] = f"--level={one}"
        else:
            joined.append(one)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("work", type=Path)
    parser.add_argument("--level", default="-O2", type=level, help="-O1, -O2, -O3 or -Os, with the dash or without")
    parser.add_argument("--all", action="store_true", help="every program, not those that handle errors")
    parser.add_argument("--only", nargs="+", default=[])
    return parser.parse_args(joined)


def load():
    """The modules the run needs: the programs of tests/qbrt and the differential live in qbrt_programs, which does not import pytest."""
    import dosbatch
    import qbbuild
    import qbrt_programs
    import qbruntime

    return dosbatch, qbbuild, qbruntime, qbrt_programs


def main(argv: list[str]) -> int:
    args = parse(argv)
    dosbatch, qbbuild, qbruntime, test_qbrt = load()

    built = qbbuild.build
    qbbuild.build = lambda source, work, objects, flags=("-O2",), tolerant=False: built(source, work, objects, (args.level,), tolerant)
    ran = dosbatch.run
    dosbatch.run = lambda *a, **k: ran(*a, **{**k, "mount_tools": True})  # a subset has no screen job, so V: was left unmounted on our side only

    def oracle(source: Path, obj: Path) -> str | None:
        done = dosbatch.subprocess.run(
            [str(dosbatch.BIN / "llrm-qb"), str(source), "--dialect", "qb45", "-fqb-runtime=qb45", args.level, *qbruntime.frontend_flags(source), "-o", str(obj)],
            capture_output=True,
            text=True,
        )
        return None if done.returncode == 0 and obj.exists() else (done.stderr or done.stdout).strip()[-200:]

    args.work.mkdir(parents=True, exist_ok=True)
    names = select(test_qbrt.SOURCES, args.all, tuple(args.only))
    print(f"PROGRAMS {len(names)} {' '.join(names)}", flush=True)
    names, failed = compiling(names, test_qbrt.SOURCES, oracle, args.work)
    for name, why in failed.items():
        print(f"COMPILE-FAIL {name} {args.level}: {why.replace(chr(10), ' ')[:150]}", flush=True)
    results = test_qbrt.differential({name: test_qbrt.SOURCES[name] for name in names}, args.work) if names else {}
    differing = 0
    for name in names:
        for target, one in results[name].items():
            if (why := unexpected(test_qbrt.KNOWN, name, target, one)) is not None:
                differing += 1
                print(f"NOTPASS {name} {target} {one.reference.status} {one.candidate.status} {why}", flush=True)
    for name in names:
        stored = test_qbrt.real_bc_text(test_qbrt.SOURCES[name])
        one = results[name].get(qbbuild.REAL)
        if stored is not None and one is not None and name not in test_qbrt.BC_KNOWN and (why := test_qbrt.not_real_bc(stored, one.reference.text, one.candidate.text)):
            differing += 1
            print(f"NOTPASS {name} {qbbuild.REAL} real-bc {why}", flush=True)
    print(f"RESULT {2 * len(names) - differing} of {2 * len(names)} pass (program x target); {len(failed)} did not compile")
    return len(failed) + differing


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
