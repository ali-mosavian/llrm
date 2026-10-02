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
    _run([str(BIN / "jwasm"), "-q", "-c", "-Cp", "-Zg", "-omf", *(f"-D{one}" for one in defines), f"-Fo{obj}", str(source)])


def check_loads(exe: Path) -> None:
    try:
        dosbatch.check_loads(exe)
    except dosbatch.TooBig as error:
        raise CompileError(str(error))


def link_c(obj: Path, exe: Path, work: Path) -> None:
    crt, ext = work / "CRT.OBJ", work / "EXT.OBJ"
    if not crt.exists():
        assemble(HERE / "runtime" / "crt.asm", crt)
        assemble(HERE / "runtime" / "ext.asm", ext)
    _run([str(BIN / "jwlink"), "option", "quiet", "format", "dos", "name", str(exe), "file", str(crt), "file", str(obj),
          "file", str(ext)])


def link_nib(obj: Path, exe: Path, work: Path, opt: str) -> None:
    """As tools/nib-build.sh links, with the corpus's externals beside."""
    parts = {}
    for part in ("start", "dos"):
        parts[part] = work / f"N{part.upper()}.OBJ"
        if not parts[part].exists():
            assemble(NIB_RUNTIME / f"{part}.asm", parts[part])
    ext = work / "EXT.OBJ"
    if not ext.exists():
        assemble(HERE / "runtime" / "ext.asm", ext)
    runtime = obj.with_name(obj.stem + "_RT.OBJ")
    used = [arg for one in (obj, parts["start"], parts["dos"], ext) for arg in ("--used-by", str(one))]
    _run([str(BIN / "llrm-nib"), str(NIB_RUNTIME / "runtime.nib"), "-o", str(runtime), opt, "--procedure-segments", *used])
    _run([str(BIN / "jwlink"), "option", "quiet", "option", "eliminate", "format", "dos", "name", str(exe),
          "file", str(parts["start"]), "file", str(obj), "file", str(runtime), "file", str(ext), "file", str(parts["dos"])])


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
