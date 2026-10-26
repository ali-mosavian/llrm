#!/usr/bin/env python3
"""Does the runtime still link? A QB program and C programs built and linked (no run) for the flat target, against the binaries in LLRM_BIN.

A target fact (c.toml `libcalls`) made the QB runtime's own C sources call `memcpy_`, which the QB link does not define: qbrt went 183/218 on
50387f114 and the gate was green, because nothing linked the QB runtime. The landing layer runs this for a batch that touches `runtime/`, a
`c.toml` or a target's `machines/` description.

    tools/link_check.py WORK      exit 1 and one line per program that does not link
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent

QB_PROGRAMS = ("arena_type", "color_masks", "bc_draw")  # the three the memcpy_ fact broke: fin.c, varptr.c
C_PROGRAMS = ("memcpy_runtime", "memfuncs_runtime")  # the C runtime's ext.asm, in both conventions

TOUCHING = ("runtime/", "runtime/qb/", "crates/target/")


def touches_runtime(files: list[str]) -> bool:
    """A change to what a program links against, or to a fact that decides what the compiler calls: not docs, not a crate's code."""
    return any(f.startswith("runtime/") or f.endswith("c.toml") or ("/machines/" in f and f.endswith(".toml")) or f.startswith("runtime/qb/") for f in files)


def link_failures(work: Path, qb=None, c=None) -> list[str]:
    """One line per program that does not build and link. `qb(name, work)` and `c(name, work)` -> the reason or None; by default the real ones."""
    sys.path.insert(0, str(HERE))
    sys.path.insert(0, str(HERE / "dosbatch"))
    qb = qb or _qb
    c = c or _c
    failures = []
    for name in QB_PROGRAMS:
        if why := qb(name, work / "qb"):
            failures.append(f"qb {name}: {why}")
    for name in C_PROGRAMS:
        if why := c(name, work / "c"):
            failures.append(f"c {name}: {why}")
    return failures


def _qb(name: str, work: Path) -> str | None:
    import dosbatch  # noqa: PLC0415
    import qbbuild  # noqa: PLC0415

    work.mkdir(parents=True, exist_ok=True)
    try:
        qbbuild.one_target(ROOT / "tests/qbrt" / f"{name}.bas", work, qbbuild.runtimes(work), ("-O2",), qbbuild.FLAT)
    except dosbatch.BuildError as error:
        return str(error)[-300:]
    return None


def _c(name: str, work: Path) -> str | None:
    import run_tests  # noqa: PLC0415

    work.mkdir(parents=True, exist_ok=True)
    for program in run_tests.discover([name]):
        built = run_tests.build(program, work, f"{program.source.stem}")
        if isinstance(built, str):
            return built[-300:]
    return None


def main() -> int:
    work = Path(sys.argv[1] if len(sys.argv) > 1 else os.environ.get("CARGO_TARGET_DIR", "/tmp") + "/link-check")
    failures = link_failures(work)
    print(f"link: {len(QB_PROGRAMS)} QB + {len(C_PROGRAMS)} C programs, {len(failures)} do not link")
    for line in failures:
        print(line)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
