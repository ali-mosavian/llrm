"""
Correctness on a real machine: link each program, run them all in one DOSBox
launch, read back each one's report stream.

C and Nib link on the host with jwlink. BASIC links in DOSBox with QB 4.5's
LINK and BCOM45.LIB; a `bc` job compiles its source with BC first, which is
how the oracle is checked against BASIC's own compiler.
"""

from __future__ import annotations

import sys
import subprocess
from pathlib import Path
from dataclasses import dataclass

from build import BIN, HERE, ROOT, CompileError

sys.path.insert(0, str(ROOT / "tools" / "dosbatch"))

import dosbatch  # noqa: E402
from dosbatch import QB45, Job  # noqa: E402,F401

NIB_RUNTIME = ROOT / "crates/frontends/llrm-nib/src/runtime"


@dataclass
class Stopped:
    """A program that did not exit: why, and what it reported first."""

    why: str
    partial: list[int]


def _run(command: list[str], cwd: Path | None = None) -> None:
    done = subprocess.run(command, capture_output=True, text=True, cwd=cwd, timeout=300)
    if done.returncode != 0:
        raise CompileError(f"{Path(command[0]).name}: {(done.stdout + done.stderr).strip()[:1500]}")


def assemble(source: Path, obj: Path, *defines: str) -> None:
    try:
        dosbatch.assemble(source, obj, *defines)
    except dosbatch.BuildError as error:
        raise CompileError(str(error))


def check_loads(exe: Path) -> None:
    try:
        dosbatch.check_loads(exe)
    except dosbatch.TooBig as error:
        raise CompileError(str(error))


def link_c(obj: Path, exe: Path, work: Path) -> None:
    try:
        dosbatch.link_c(obj, exe, work)
    except dosbatch.BuildError as error:
        raise CompileError(str(error))


def link_nib(obj: Path, exe: Path, work: Path, opt: str) -> None:
    """As tools/nib-build.sh links, with the corpus's externals beside."""
    link = dosbatch.target_link(dosbatch.REAL_MODE)
    defines = dosbatch.os_defines(dosbatch.REAL_MODE, "nib")
    made = lambda names: [dosbatch.runtime_object(name, work / (Path(name).stem.upper() + ".OBJ"), defines) for name in names]  # noqa: E731
    first, last, final = made(dosbatch.os_start(dosbatch.REAL_MODE, "nib")), made(link["last"]), made(link["final"])
    runtime = obj.with_name(obj.stem + "_RT.OBJ")
    used = [arg for one in (obj, *first, *last, *final) for arg in ("--used-by", str(one))]
    _run([str(BIN / "llrm-nib"), str(NIB_RUNTIME / "runtime.nib"), "-o", str(runtime), opt, "--procedure-segments", *used])
    _run([str(BIN / "jwlink"), "option", "quiet", "option", "eliminate", *dosbatch.linkrecipe.link(dosbatch.REAL_MODE, "format"), "name", str(exe),
          *[word for one in (*first, obj, runtime, *last, *final) for word in ("file", str(one))]])


def run(jobs: list[Job], work: Path, timeout: int = 1800, budget_ms: int = 120_000) -> dict[str, list[int] | str | Stopped]:
    """Every job's report stream, or why there is none."""
    return {stem: reports(one) for stem, one in dosbatch.run(jobs, work, timeout, budget_ms).items()}


def reports(one: dosbatch.Result) -> list[int] | str | Stopped:
    if one.status == "not built":
        return "not built: " + one.detail
    if one.status == "stopped":
        return Stopped(one.detail, [int(w) for w in one.text.split() if w.lstrip("-").isdigit()])
    try:
        return [int(w) for w in one.text.split()]
    except ValueError:
        return "unreadable output: " + one.text.strip()[:300]
