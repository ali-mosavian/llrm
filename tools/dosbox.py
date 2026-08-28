"""
Run a batch file under DOSBox-X, headless.

Two things here were paid for once already and are not incidental. DOSBox
ignores SIGTERM, so a run that overstays has to be killed outright. And
completion is decided by a marker file the batch writes last, never by looking
for the output: with several programs in one launch the first one's output
appears while the second is still running, and reading it early makes a passing
suite look like a missing one.
"""

import os
import shutil
import subprocess
from pathlib import Path
from time import monotonic
from dataclasses import dataclass

MARKER = "FINISHED.TXT"

# core=dynamic and cycles=max: fast, and not a measurement. Anything that
# produces a number uses the pinned profile instead -- see docs/measurement.md.
FAST = """\
[sdl]
priority=higher,normal
output=surface
showdetails=false
showbasic=false
[dosbox]
memsize=32
startquiet=true
startbanner=false
[cpu]
core=dynamic
cputype=pentium_iii
cycles=max
[dos]
xms=true
ems=true
dpmi=false
log console=false
[mixer]
nosound=true
"""


@dataclass(frozen=True, slots=True)
class Run:
    finished: bool
    timed_out: bool
    seconds: float


def dosbox_bin() -> str | None:
    return os.environ.get("DOSBOX_BIN") or shutil.which("dosbox-x")


def dos_file(workdir: Path, name: str) -> Path | None:
    """DOS writes 8.3 names in whatever case it likes; find one either way."""
    for candidate in (name, name.upper(), name.lower()):
        p = workdir / candidate
        if p.is_file():
            return p
    return None


def read_dos(workdir: Path, name: str) -> str:
    p = dos_file(workdir, name)
    return p.read_text(encoding="latin1") if p else ""


def _write_batch(path: Path, lines: list[str]) -> None:
    body = ["@echo off", f"if exist {MARKER} del {MARKER}", *lines, f"echo DONE > {MARKER}"]
    path.write_bytes("".join(line + "\r\n" for line in body).encode("latin1"))


def launch(
    workdir: Path,
    mount_v: Path,
    lines: list[str],
    *,
    timeout: int = 300,
    conf: str = FAST,
    env: dict[str, str] | None = None,
) -> Run:
    binary = dosbox_bin()
    if binary is None:
        raise RuntimeError("no dosbox-x on PATH; set DOSBOX_BIN")

    workdir.mkdir(parents=True, exist_ok=True)
    for stale in (MARKER, MARKER.lower()):
        (workdir / stale).unlink(missing_ok=True)

    _write_batch(workdir / "RUN.BAT", lines)
    autoexec = [
        "[autoexec]",
        "@echo off",
        f"mount c {workdir}",
        f"mount v {mount_v}",
        *(f"set {k}={v}" for k, v in (env or {}).items()),
        "c:",
        "RUN.BAT",
        "exit",
    ]
    (workdir / "dosbox.conf").write_text(conf + "\n".join(autoexec) + "\n")

    started = monotonic()
    proc = subprocess.Popen(
        [binary, "-nolog", "-exit", "-conf", str(workdir / "dosbox.conf")],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env={**os.environ, "SDL_VIDEODRIVER": "dummy"},
    )
    timed_out = False
    try:
        proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        proc.kill()  # SIGTERM is ignored
        proc.wait()

    return Run(dos_file(workdir, MARKER) is not None, timed_out, monotonic() - started)
