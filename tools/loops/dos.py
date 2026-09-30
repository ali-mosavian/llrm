"""
Correctness on a real machine: link each program, run them all in one DOSBox
launch, read back each one's report stream.

C and Nib link on the host with jwlink. BASIC links in DOSBox with QB 4.5's
LINK and BCOM45.LIB; a `bc` job compiles its source with BC first, which is
how the oracle is checked against BASIC's own compiler.
"""

from __future__ import annotations

import os
import re
import sys
import json
import shutil
import subprocess
from pathlib import Path
from dataclasses import dataclass

from build import BIN, HERE, ROOT, CompileError

sys.path.insert(0, str(ROOT / "tools" / "e2e"))

from dosbox import read_dos  # noqa: E402

QB45 = Path.home() / "work/42-labs/mini-qb/dosbox/qb45"
NIB_RUNTIME = ROOT / "crates/frontends/llrm-nib/src/runtime"
CONF = """\
[sdl]
priority=higher,normal
output=surface
[dosbox]
memsize=32
startquiet=true
startbanner=false
quit warning=false
[cpu]
core=normal
cputype=pentium
cycles=max
[dos]
xms=true
ems=true
[mixer]
nosound=true
"""


@dataclass
class Stopped:
    """A program that did not exit: why, and what it reported first."""

    why: str
    partial: list[int]


@dataclass
class Job:
    """One program. kind: exe (linked here), obj (a BASIC object to LINK
    there), bas (a BASIC source for BC, then LINK)."""

    stem: str  # at most 8 characters
    kind: str
    path: Path


def _run(command: list[str], cwd: Path | None = None) -> None:
    done = subprocess.run(command, capture_output=True, text=True, cwd=cwd, timeout=300)
    if done.returncode != 0:
        raise CompileError(f"{Path(command[0]).name}: {(done.stdout + done.stderr).strip()[:1500]}")


def assemble(source: Path, obj: Path, *defines: str) -> None:
    _run([str(BIN / "jwasm"), "-q", "-c", "-Cp", "-Zg", "-omf", *(f"-D{one}" for one in defines), f"-Fo{obj}", str(source)])


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


def run(jobs: list[Job], work: Path, timeout: int = 1800, budget_ms: int = 120_000) -> dict[str, list[int] | str]:
    """Every job's report stream, or why there is none. One dosrun launch: a
    first job builds (BC, LINK), then each program runs as its own job with
    `budget_ms` of emulated time, so a hang ends that program alone."""
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    head = [f"mount c {work}", f"mount v {QB45}", r"set LIB=V:\LIB", "c:"]
    building = []
    for job in jobs:
        u = job.stem.upper()
        if job.kind == "exe":
            shutil.copy(job.path, work / f"{u}.EXE")
        elif job.kind == "obj":
            shutil.copy(job.path, work / f"{u}.OBJ")
            building.append(f"V:\\LINK /NOE {u}.OBJ,{u}.EXE,,V:\\LIB\\BCOM45.LIB; > {u}.LNK")
        else:
            shutil.copy(job.path, work / f"{u}.BAS")
            building.append(f"V:\\BC /O /FPi {u}.BAS,{u}.OBJ; > {u}.BCO")
            building.append(f"V:\\LINK /NOE {u}.OBJ,{u}.EXE,,V:\\LIB\\BCOM45.LIB; > {u}.LNK")
    script = [":ms 1200000", *head, *building, "."]
    for job in jobs:
        u = job.stem.upper()
        script += [f":ms {budget_ms}", *head, f"if exist {u}.EXE {u}.EXE > {u}.TXT", "."]
    (work / "job.conf").write_text(CONF)
    (work / "jobs.txt").write_text("\n".join(script) + "\n")
    events = work / "events.txt"
    with open(work / "jobs.txt") as stdin, open(events, "w") as sink:
        try:
            subprocess.run([str(BIN / "dosbox-x"), "-nolog", "-conf", str(work / "job.conf")], stdin=stdin,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, pass_fds=(sink.fileno(),),
                           env={**os.environ, "SDL_VIDEODRIVER": "dummy", "DOSRUN_FD": str(sink.fileno())},
                           timeout=timeout)
        except subprocess.TimeoutExpired:
            raise RuntimeError(f"DOSBox did not finish in {timeout}s ({work})")
    return collect(jobs, work, events)


def collect(jobs: list[Job], work: Path, events: Path) -> dict[str, list[int] | str | Stopped]:
    """Each job's reports, from a launch's events and files."""
    lines = [json.loads(line) for line in events.read_text().splitlines() if line.startswith('{"ev":')]
    ends = [one for one in lines if one["ev"] == "end"]
    # A stopped program never closes its output, so DOS leaves the file
    # empty; what it wrote is in the events.
    written: dict[str, str] = {}
    for one in lines:
        if one["ev"] == "out" and one.get("file"):
            written[one["file"].upper()] = written.get(one["file"].upper(), "") + one["text"]
    if len(ends) != len(jobs) + 1:
        raise RuntimeError(f"DOSBox ran {len(ends)} of {len(jobs) + 1} jobs ({events})")
    out = {}
    for job, end in zip(jobs, ends[1:]):
        u = job.stem.upper()
        if not (work / f"{u}.EXE").exists():
            out[job.stem] = "not built: " + (read_dos(work, f"{u}.BCO") + read_dos(work, f"{u}.LNK")).strip()[-600:]
            continue
        text = read_dos(work, f"{u}.TXT")
        if end.get("reason") != "exit":
            partial = [int(one) for one in written.get(f"{u}.TXT", "").split() if one.lstrip("-").isdigit()]
            out[job.stem] = Stopped(f"{end.get('reason')} after {end.get('ms')} ms ({events})", partial)
            continue
        try:
            out[job.stem] = [int(one) for one in text.split()]
        except ValueError:
            out[job.stem] = "unreadable output: " + text.strip()[:300]
    return out
