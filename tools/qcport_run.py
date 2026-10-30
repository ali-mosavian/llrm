#!/usr/bin/env python3
"""What the QCport gates share: the module list, the link listing, the headless run and the verdicts. tools/qcport-run-cc.py is the gate.

QCPORT names QCport's src directory (its tools/run.sh is beside it), QCPORT_INC its Borland headers, QCPORT_BORLAND a
build of it by Borland C: the objects, QCPORT.LNK, and the files a run reads (stuff.ini, default.cfg, start.qmp,
QUAKE.QPK, run.conf). A run is headless (`start.qmp -ticks 300`) in a work directory of its own; it is stalled when the emulator's
output stops growing for QCPORT_STALL_SECONDS (90) and capped at QCPORT_RUN_SECONDS (900). The emulated work is pinned
(`cycles=75000`, 300 ticks), so how long it takes on the wall is the host's load, not a property of the build.
"""
from __future__ import annotations

import hashlib
import os
import re
import signal
import resource
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "dosbatch"))

import dosbatch  # noqa: E402
os.environ.setdefault("LLRM_VERIFY", "1")  # the gate checks each pass and phase (llrm_support::debug::verifying)

MODULES = ("host", "render", "model", "game", "sound", "ui")
DATA = ("stuff.ini", "default.cfg", "start.qmp", "QUAKE.QPK", "run.conf")
ARGUMENTS = "start.qmp -ticks 300"
# The attract loop: an empty command line starts `startdemos demo1 demo2 demo3` (src/host/sys.c). run.sh gives an empty argument its default, so one space.
ATTRACT_ARGUMENTS = " "
ATTRACT_DEMOS = ("demo1.qdc", "demo2.qdc", "demo3.qdc")
LOG_READ = 16 << 20  # a looping fault writes without end: the first megabytes name it
# What the shipped QCport draws at -Os, from both builds: a different count is a miscompile or a changed benchmark, not noise.
EXPECTED = {"modules": 65, "frames": "62", "polys": "37637", "md5": "ba64e86d"}
LOGS = ("cstep.txt", "dosbox.out", "dosbox.log", "cport.txt", "CPORT.TXT", "error.log", "ERROR.LOG")
# FP87 first: the order the build that runs uses; CM first leaves a program that links and stops after its arguments.
LIBRARIES = ("FP87", "MATHM", "CM")
TOOLCHAIN_FILES = {"C0M.OBJ", *(f"{name}.LIB" for name in LIBRARIES)}  # what the listing names by the path it had where the Borland build was made


def required(name: str) -> Path:
    value = os.environ.get(name)
    if not value:
        sys.exit(f"{name} is not set; see the top of {Path(__file__).name}")
    return Path(value).expanduser()


def compile_failure(compiler: Path, flags: list[str], source: Path, out: Path, level: str = "-O2") -> str | None:
    """Why `compiler` failed on `source`, or None: its exit status (a negative one is a signal) and what it printed."""
    done = subprocess.run([str(compiler), level, *flags, str(source), "-o", str(out)], capture_output=True, text=True)
    if done.returncode == 0:
        return None
    status = f"exit {done.returncode}"
    if done.returncode < 0:
        status += f" ({signal.Signals(-done.returncode).name})"
    return f"{source.stem}: {status} {(done.stderr or done.stdout).strip()[-300:]}"


def linked(listing: str, objects: Path, extra: list[str], toolchain: Path | None = None) -> str:
    """`listing` (a Borland build's QCPORT.LNK) naming the objects in `objects`, with `extra` added, libraries in the order that runs.
    The listing names the toolchain's start-up and libraries by the path they had where it was made; `toolchain` is where they are here."""
    lines, libraries = [], []
    for line in listing.splitlines():
        if toolchain and (kept := re.match(r"(file|library) '(.*/)?([^/]+\.(?:obj|lib))'$", line, re.I)) and kept.group(3).upper() in TOOLCHAIN_FILES:
            line = f"{kept.group(1)} '{toolchain / kept.group(3)}'"
        found = re.match(r"file '(.*/)?([^/]+)\.obj'$", line, re.I)
        if line.startswith("library"):
            libraries.append(line)
        elif line.startswith(("option", "name")):
            continue
        elif found and found.group(2).upper() != "C0M":
            lines.append(f"file '{objects / (found.group(2) + '.obj')}'")
        else:
            lines.append(line)
    lines = ["format dos", f"option quiet, map={objects / 'QCPORT.MAP'}", f"name {objects / 'QCPORT.EXE'}", *[one for one in lines if not one.startswith("format")]]
    lines += [f"file '{objects / one}'" for one in extra]
    rank = lambda line: LIBRARIES.index(re.search(r"(\w+)\.LIB", line, re.I).group(1).upper())
    return "\n".join(lines + sorted(libraries, key=rank)) + "\n"


def measured(directory: Path) -> dict[str, str]:
    """What a run drew: frames, polys, and the md5 of its picture."""
    text = (directory / "BENCH.TXT").read_text() if (directory / "BENCH.TXT").exists() else ""
    found = dict(re.findall(r"^(frames|polys)\s+(\d+)", text, re.M))
    found["md5"] = hashlib.md5((directory / "BENCH.BMP").read_bytes()).hexdigest() if (directory / "BENCH.BMP").exists() else ""
    return found


def verdict(reference: dict[str, str], built: dict[str, str]) -> list[str]:
    """What differs between the two runs: empty where they drew the same. A run that drew nothing differs."""
    if not reference.get("polys") or not built.get("polys"):
        return [f"a run drew nothing: Borland {reference}, llrm {built}"]
    return [f"{key}: Borland {reference.get(key)}, llrm {built.get(key)}" for key in ("frames", "polys", "md5") if reference.get(key) != built.get(key)]


def pinned(modules: int, built: dict[str, str]) -> list[str]:
    """Where the llrm build's drawing is not the pinned one (EXPECTED): empty where it is."""
    seen = {"modules": modules, "frames": built.get("frames"), "polys": built.get("polys"), "md5": (built.get("md5") or "")[:8]}
    return [f"{key}: pinned {EXPECTED[key]}, llrm {seen[key]}" for key in EXPECTED if seen[key] != EXPECTED[key]]


def tails(directory: Path, lines: int = 12) -> list[str]:
    """What the emulator and the program last said, for a run that drew nothing: the work directory is gone with a CI runner."""
    out = []
    for name in ("dosbox.out", "dosbox.log", "error.log", "cport.txt"):
        one = directory / name
        if one.is_file() and (text := one.read_text(errors="replace").strip()):
            out += [f"  {directory.name}/{name}:"] + [f"    {line}" for line in text.splitlines()[-lines:]]
    return out or [f"  {directory.name}: no output at all ({sorted(p.name for p in directory.iterdir())[:12]})"]


def faults(directory: Path) -> list[str]:
    """The CPU exceptions the emulator logged in `directory`, each once: a miscompiled program that executes garbage ends in one, then loops on it."""
    seen = []
    for name in LOGS:
        one = directory / name
        if one.is_file():
            with one.open(errors="replace") as log:
                seen += re.findall(r"CPU_Exception: Calling CPU_Interrupt for exception (\d+)", log.read(LOG_READ))
    return [f"CPU exception {number}" for number in dict.fromkeys(seen)]


def attract_problems(directory: Path) -> list[str]:
    """What the attract loop run in `directory` did wrong: a CPU exception, a demo it never started, and every line that says it ran out of memory. Empty where all three played."""
    def text(name: str) -> str:
        one = directory / name
        return one.read_text(errors="replace") if one.is_file() else ""

    # run.sh writes cstep.txt from the emulator's log only when the emulator exits: a run stopped while it plays has the lines in dosbox.log and dosbox.out.
    started = re.findall(r"attract demo (\S+) map", "\n".join(text(name) for name in LOGS).replace("\r", ""))
    memory = [line.strip() for name in LOGS for line in text(name).replace("\r", "").splitlines() if "out of memory" in line.lower()]
    missing = [demo for demo in ATTRACT_DEMOS if demo not in started]
    return faults(directory) + ([f"never started: {' '.join(missing)}"] if missing else []) + list(dict.fromkeys(memory))[:3]


# tools/xfail.toml id of the attract run at -O2: it runs out of memory (code 329,553 bytes against Borland's 296,085); the shipped -Os build (342,560) plays all three demos.
def attract_verdict(wrong: dict[str, list[str]]) -> list[str]:
    """What each attract run (`borland`, `llrm`) did wrong: both must play all three demos."""
    return [f"{name}: {'; '.join(problems)}" for name, problems in wrong.items() if problems]


def attract_over(directory: Path) -> bool:
    """The attract run has said all it will: all three demos started, or it ran out of memory."""
    problems = attract_problems(directory)
    return not problems or any("out of memory" in one.lower() or one.startswith("CPU exception") for one in problems)


def stop(child: subprocess.Popen) -> None:
    """Kill `child` and everything under it: the emulator runs under `timeout`, in a process group of its own, which a kill of the
    script's group misses."""
    sys.path.insert(0, str(HERE / "gate"))
    import proctree

    below = proctree.descendants(child.pid)
    os.killpg(child.pid, signal.SIGKILL)
    for one in below:
        try:
            os.kill(one, signal.SIGKILL)
        except ProcessLookupError:
            pass
    child.wait()


def progress(directory: Path) -> int:
    """How much the run has written: the emulator's log grows as the program steps, so a stalled run's does not."""
    return sum(one.stat().st_size for one in directory.iterdir() if one.is_file())


# A run that faults loops on the CPU exception and logs it without end: one dosbox.log reached 57 GB.
FILE_LIMIT = 1 << 30


def run(directory: Path, qcport: Path, stall: float, cap: float, tick: float = 1.0, arguments: str = ARGUMENTS, over=None, file_limit: int = FILE_LIMIT) -> str | None:
    """Run QCport in `directory`; None when it finished (or `over(directory)` said so), else why it was stopped: no progress for `stall` s, or `cap` s in all. No file it writes passes `file_limit` bytes."""
    environment = {**os.environ, "DOSBOX_BIN": os.environ.get("DOSBOX_BIN", str(dosbatch.DOSBOX)), "TIMEOUT": str(int(cap))}
    child = subprocess.Popen([str(qcport.parent / "tools" / "run.sh"), str(directory), arguments], env=environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True,
                             preexec_fn=lambda: resource.setrlimit(resource.RLIMIT_FSIZE, (file_limit, file_limit)))
    start = moved = time.monotonic()
    seen = -1
    while child.poll() is None:
        time.sleep(tick)
        now = time.monotonic()
        if (size := progress(directory)) != seen:
            seen, moved = size, now
        if over and over(directory):
            stop(child)
            return None
        why = f"no progress for {stall:g}s" if now - moved > stall else f"still running after {cap:g}s" if now - start > cap else None
        if why:
            stop(child)
            return why
    return None
