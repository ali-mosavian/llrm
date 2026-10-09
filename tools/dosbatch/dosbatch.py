"""
Many programs, one DOSBox launch: build each, run each under its own time
budget, read back what it printed. Serves tests/run, tests/differential and
bench; tools/loops reads its report streams through it too.

A job is an EXE (linked on the host), an OBJ (LINK'd in DOS against the QB 4.5
runtime) or a BAS (BC'd, then LINK'd -- the reference route). A program's stdout
goes to NAME.TXT; a stopped program never closes it, so what it wrote before
stopping comes from the launch's events.
"""

from __future__ import annotations

import os
import re
import sys
import json
import threading
import time
import shutil
import tomllib
import subprocess
from collections.abc import Iterable
from pathlib import Path
from dataclasses import dataclass

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import linkrecipe  # noqa: E402
import llrmbin  # noqa: E402

BIN = llrmbin.bin_dir()
QB45 = Path(os.environ.get("QB45_DIR", Path.home() / "work/42-labs/mini-qb/dosbox/qb45"))
PDS71 = Path(os.environ.get("PDS71_DIR", Path.home() / "work/other/d32x/toolchains/pds71"))
VBDOS = Path(os.environ.get("VBDOS_DIR", Path.home() / "work/other/d32x/toolchains/vbdos"))
DOSBOX = Path(os.environ.get("DOSBOX_BIN", BIN / "dosbox-x"))
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

# What DOSBox leaves a program of its 640K: the rest is DOS, the shell and the
# environment. A bigger one stops with "Unable to run program (errcode=8)".
LOAD_LIMIT = 560_000


def place(source: Path, target: Path) -> None:
    """`source` as `target` in a job's directory: once for the whole launch, and a hard link where the volume allows
    (a 10 MB corpus per job was 250 MB copied per run)."""
    if target.exists():
        return
    try:
        os.link(source, target)
    except OSError:
        shutil.copy(source, target)


@dataclass(frozen=True)
class Toolchain:
    """A Microsoft BASIC install, mounted as V:, and the runtime library its programs link."""

    mount: Path
    bc: str
    link: str
    library: str


QB45_TOOLS = Toolchain(QB45, r"V:\BC", r"V:\LINK", r"V:\LIB\BCOM45.LIB")
PDS71_TOOLS = Toolchain(PDS71, r"V:\BINB\BC", r"V:\BINB\LINK", r"V:\LIB\BCL71ENR.LIB")
VBDOS_TOOLS = Toolchain(VBDOS, r"V:\BIN\BC", r"V:\BIN\LINK", r"V:\LIB\VBDCL10E.LIB")


class TooBig(Exception):
    pass


class BuildError(Exception):
    pass


_RUNTIME_LOCK = threading.Lock()


def _host(command: list[str]) -> None:
    done = subprocess.run(command, capture_output=True, text=True, timeout=300)
    if done.returncode != 0:
        raise BuildError(f"{Path(command[0]).name}: {(done.stdout + done.stderr).strip()[:1500]}")


def assemble(source: Path, obj: Path, *defines: str) -> None:
    _host([str(BIN / "jwasm"), "-q", "-c", "-Cp", "-Zg", linkrecipe.assembler(REAL_MODE), *(f"-D{one}" for one in defines), f"-Fo{obj}", str(source)])


def ow_root() -> Path:
    """The Open Watcom tree wccq is built from (toolchain/owshim/build.sh): OWROOT, else the cached one."""
    if os.environ.get("OWROOT"):
        return Path(os.environ["OWROOT"])
    commit = (ROOT / "toolchain" / "owshim" / "ow-commit").read_text().strip()
    return Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "llrm" / f"open-watcom-v2-{commit}"


def watcom_cc(target: str) -> Path:
    """Open Watcom's own compiler for `target`, built in the same tree as wccq (`bwcc386` for 32 bits, `bwcc` for 16): the ground
    truth of its register convention."""
    return ow_root() / "build" / "binbuild" / ("bwcc386" if target_bits(target) == 32 else "bwcc")


def watcom_compile(source: Path, obj: Path, target: str) -> None:
    """`source` compiled by Open Watcom's own compiler for `target`: its default convention, no library calls, the target's
    struct packing (byte-packed medium model in 16 bits, -zp4 flat)."""
    flags = ["-zq", "-3r", "-zp4"] if target_bits(target) == 32 else ["-zq", "-mm", "-ecw", "-zp1"]
    _host([str(watcom_cc(target)), *flags, "-s", "-zl", "-ox", f"-fo={obj}", str(source)])


def target_modes() -> dict[str, int]:
    """Each target's gcc `-m` number (tools/linkrecipe.py)."""
    return linkrecipe.modes()


def target_bits(target: str) -> int:
    """The bits of `target`'s code: its `-m` number."""
    return target_modes()[target]


def m_flag(target: str) -> str:
    """The flag that names `target` to the compilers."""
    return f"-m{target_modes()[target]}"


def target_of(flags: list[str], default: str) -> str:
    """The target a compiler's `flags` name by `-m<N>`, else `default`."""
    for flag in flags:
        for name, mode in target_modes().items():
            if flag == f"-m{mode}":
                return name
    return default


def target_link(target: str) -> dict:
    """How `target` links a C program: the `[link]` of its `object.toml` (tools/linkrecipe.py)."""
    return linkrecipe.recipe(target)["link"]


_ASSEMBLED: set[tuple[Path, tuple[str, ...]]] = set()


def runtime_object(name: str, path: Path, defines: tuple[str, ...] = ()) -> Path:
    """`path`, the object of the runtime file `name`, made once for this process however many builds ask: one
    made with the description's defines is not dated by its source, so it is made on the first ask, and an
    object built by another build is never rewritten under a linker reading it (it is made beside and renamed).
    Builds run in threads; the others wait for the one that assembles."""
    with _RUNTIME_LOCK:
        if (path, defines) not in _ASSEMBLED:
            if defines or not path.exists() or path.stat().st_mtime < (ROOT / name).stat().st_mtime:
                made = path.with_name(f"{path.stem}.{os.getpid()}.tmp")
                assemble(ROOT / name, made, *defines)
                os.replace(made, path)
            _ASSEMBLED.add((path, defines))
    return path


C_RUNTIME = "@c-runtime"


def link_files(source: Path, names: Iterable[str], target: str) -> list[Path]:
    """The files a `link:` header names for `target`: paths beside `source`, and `@c-runtime`, the target's own file of the
    routines a program calls and does not define (its `[link] last`), which no program names a path of. Every reader of
    a `link:` header resolves it here."""
    out: list[Path] = []
    for name in names:
        out += [ROOT / one for one in target_link(target)["last"]] if name == C_RUNTIME else [source.parent / name]
    return out


def link_target(target: str, obj: Path, exe: Path, work: Path, listing: Path | None = None, after: tuple[str, ...] = (), before: tuple[str, ...] = (), runtime: tuple[list[str], list[str]] | None = None, objects_after: tuple[Path, ...] = (), defines: tuple[str, ...] | None = None) -> tuple[Path, ...]:
    """A C object with its start-up and `report(long)`, which prints a signed decimal and a newline, linked as
    `target` says; the files its executable needs beside it (an extender's loader)."""
    link = target_link(target)
    if defines is None:
        defines = os_defines(target, "c")
    fill = lambda text: text.replace("{ow}", str(ow_root()))  # noqa: E731
    made = work / target
    made.mkdir(exist_ok=True)

    def objects(names: list[str]) -> list[Path]:
        return [runtime_object(name, made / (Path(name).stem.upper() + ".OBJ"), defines) for name in names]

    first, last, final = objects(runtime[0] if runtime else link["first"]), objects(runtime[1] if runtime else link["last"]), objects(link["final"])
    mapping = ["option", f"map={listing}"] if listing else []
    files = lambda paths: [word for one in paths for word in ("file", str(one))]  # noqa: E731
    _host([str(BIN / "jwlink"), "option", "quiet", *mapping, *before, *link["format"], "name", str(exe), *map(fill, link.get("options", [])), *files(first), "file", str(obj), *files(objects_after), *files(last), *after, *files(final)])
    return (Path(fill(link["loader"])),) if "loader" in link else ()


# The 16-bit target the loop corpus and the C helpers build for.
REAL_MODE = linkrecipe.named(16)

_COMPILERS = {"c": "llrm-c", "nib": "llrm-nib"}


def os_layer(target: str, field: str, language: str) -> str:
    """What `language`'s compiler says of `target`'s OS layer (`--os-layer FIELD`)."""
    return subprocess.run([str(BIN / _COMPILERS[language]), m_flag(target), "--os-layer", field], capture_output=True, text=True, check=True).stdout.strip()


def os_defines(target: str, language: str) -> tuple[str, ...]:
    """What the assembler is told of `target`'s OS layer and `language`'s description: `SYMBOL=value` each."""
    return tuple(os_layer(target, "defines", language).split())


def os_start(target: str, language: str) -> list[str]:
    """The files that start a `language` program on `target`: the layer's start-up, then the language's own hook."""
    directory = Path(os_layer(target, "directory", language))
    hook = os_layer(target, "language_file", language)
    return [str((directory / os_layer(target, "start", language)).relative_to(ROOT))] + ([hook] if hook else [])


def c_include(target: str, work: Path) -> Path:
    """A directory of `work` holding the OS layer's C header for `target` as llrm_os.h (`llrm-c --os-layer header`)."""
    directory = work / f"{target}-include"
    directory.mkdir(exist_ok=True)
    header = os_layer(target, "header", "c") + "\n"
    if not (directory / "llrm_os.h").exists() or (directory / "llrm_os.h").read_text() != header:
        (directory / "llrm_os.h").write_text(header)
    return directory


def c_support(target: str, work: Path) -> list[Path]:
    """The objects of C's externals (`report`, the link recipe's `last`) and the OS layer's operations (`final`) they call, made in
    `work`: for a build with another compiler's start-up, which brings neither."""
    link = target_link(target)
    defines = os_defines(target, "c")
    return [runtime_object(name, work / (Path(name).stem.upper() + ".OBJ"), defines) for name in (*link["last"], *link["final"])]


def os_objects(target: str, language: str, work: Path) -> dict[str, Path]:
    """The OS layer's objects a `language` program on `target` is linked with, made in `work`: `start`, `implementation`
    and, where the language has one, its `hook`. The one place that says which they are: the runtime's cut must see
    every one (a hook that calls N$EDIV keeps it), and the link names them."""
    defines = os_defines(target, language)
    directory = Path(os_layer(target, "directory", language))
    sources = {field: directory / os_layer(target, field, language) for field in ("start", "implementation")}
    if hook := os_layer(target, "language_file", language):
        sources["hook"] = Path(hook)
    return {field: runtime_object(str(source), work / f"{field}.obj", defines) for field, source in sources.items()}


def link_nib(target: str, source: Path, obj: Path, exe: Path, work: Path, level: str, foreign: tuple[Path, ...], abi: tuple[str, ...] = ()) -> tuple[Path, ...]:
    """A Nib program for `target`: its object, `runtime.nib` cut to what the program and the OS layer (`os_objects`) name, linked as the target says."""
    runtime = work / (obj.stem + "R.obj")
    used = [word for one in (obj, *foreign, *os_objects(target, "nib", work).values()) for word in ("--used-by", str(one))]
    _host([str(BIN / "llrm-nib"), str(ROOT / "crates/frontends/llrm-nib/src/runtime/runtime.nib"), m_flag(target), *abi, "-o", str(runtime), level, "--procedure-segments", "-Wno-target-width", *used])
    return link_target(target, obj, exe, work, runtime=(os_start(target, "nib"), []), objects_after=(runtime, *foreign), defines=os_defines(target, "nib"))


def link_c(obj: Path, exe: Path, work: Path, listing: Path | None = None, after: tuple[str, ...] = (), before: tuple[str, ...] = ()) -> None:
    """A C object for the 16-bit target, linked as it says."""
    link_target(REAL_MODE, obj, exe, work, listing, after, before)


@dataclass
class Job:
    """One program. kind: exe (linked here), obj (an object to LINK there),
    bas (a BASIC source for BC, then LINK). stem: at most 8 characters."""

    stem: str
    kind: str
    path: Path
    budget_ms: int | None = None
    switches: str = "/O /FPi"  # BC's, for a bas job
    libs: tuple[str, ...] = ()  # more libraries to link, by DOS path
    library: str = ""  # the runtime library, where the toolchain's own is not it
    runtime: str = "bcom45"  # bcom45, llrmqb or empty
    runtime_file: Path | None = None  # the copied LLRMQB or empty archive
    args: str = ""  # the program's command line
    objects: tuple[Path, ...] = ()  # more objects to link with an obj job's
    files: tuple[Path, ...] = ()  # files the program reads, copied beside it under their upper-case names
    map: bool = False  # LINK /MAP: NAME.MAP lists the public symbols too
    runner: str = ""  # a program that runs this one (it must be among `files`), e.g. a timer


def runtime_library(job: Job, tools: Toolchain, work: Path) -> str:
    """The one runtime LINK receives; replacement archives are always mounted explicitly."""
    if job.runtime == "bcom45":
        return job.library or tools.library
    if job.runtime not in ("llrmqb", "empty"):
        raise BuildError(f"unknown QB runtime '{job.runtime}'")
    if job.runtime_file is None or not job.runtime_file.is_file():
        raise BuildError(f"runtime file for {job.runtime} is missing")
    name = "LLRMQB.LIB" if job.runtime == "llrmqb" else "EMPTY.LIB"
    place(job.runtime_file, work / name)
    return rf"C:\{name}"


@dataclass
class Result:
    """status: ok, not built or stopped. text is stdout (what was written, if stopped)."""

    status: str
    text: str = ""
    detail: str = ""
    exit_code: int | None = None  # the program's, where it ran to its end


def check_loads(exe: Path) -> None:
    """Raise TooBig for an EXE too big for DOS memory, so a batch that outgrew
    it is run case by case, as one that did not link is."""
    head = exe.read_bytes()[:14]
    if head[:2] != b"MZ":
        return
    last, pages, _, header, minimum = (int.from_bytes(head[at : at + 2], "little") for at in (2, 4, 6, 8, 10))
    image = pages * 512 - (512 - last if last else 0) - header * 16
    if image + minimum * 16 > LOAD_LIMIT:
        raise TooBig(f"{exe.name} needs {image + minimum * 16} bytes, more than DOS has: {LOAD_LIMIT}")


def crlf(source: bytes) -> bytes:
    """BC ends a line at CR LF only: a LF-only source is one long line, a comment if it opens with one."""
    return source.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")


def read_dos(workdir: Path, name: str) -> str:
    """DOS writes 8.3 names in whatever case it likes; find one either way."""
    for candidate in (name, name.upper(), name.lower()):
        if (workdir / candidate).is_file():
            return (workdir / candidate).read_text(encoding="latin1")
    return ""


_PRIVATE = iter(range(1 << 30))


def private_work(name: str) -> Path:
    """A work directory under the target directory that no other run shares: `run` clears its work directory, so two
    runs given one would delete each other's files, and hang or read each other's results (#736)."""
    return llrmbin.target_dir() / f"{name}-{os.getpid()}-{next(_PRIVATE)}"


def discard(work: Path) -> None:
    """A private work directory of a run that passed, and the objects beside it."""
    for path in (work, work.with_name(work.name + "-obj")):
        shutil.rmtree(path, ignore_errors=True)


OUTPUT_CAP = 32 << 20


def _oversized(work: Path, cap: int) -> str | None:
    """A file in `work` past `cap` bytes: a program that prints without end fills the disk and the event log."""
    for entry in os.scandir(work):
        if entry.is_file() and entry.stat().st_size > cap:
            return entry.name
    return None


def _launch(command: list[str], work: Path, timeout: int, cap: int, **options) -> str | None:
    """Run `command` until it exits, or until a file in `work` passes `cap` bytes, which stops it: the file's name. A
    launch that outlives `timeout` is an error."""
    process = subprocess.Popen(command, **options)
    deadline = time.monotonic() + timeout
    while True:
        try:
            process.wait(timeout=0.2)
            return None
        except subprocess.TimeoutExpired:
            pass
        if (over := _oversized(work, cap)) is not None:
            process.kill()
            process.wait()
            return over
        if time.monotonic() > deadline:
            process.kill()
            process.wait()
            raise subprocess.TimeoutExpired(command, timeout)


def run(jobs: list[Job], work: Path, timeout: int = 1800, budget_ms: int = 120_000, build_ms: int = 1_200_000, tools: Toolchain = QB45_TOOLS, conf: str = CONF, cap: int = OUTPUT_CAP) -> dict[str, Result]:
    """Every job's result. One dosrun launch: a first job builds (BC, LINK),
    then each program runs as its own job with `budget_ms` of emulated time, so
    a hang ends that program alone."""
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    head = [f"mount c {work}", f"mount v {tools.mount}", r"set LIB=V:\LIB", "c:"]
    building = []
    for job in jobs:
        u = job.stem.upper()
        libraries = "+".join([runtime_library(job, tools, work), *job.libs])
        more = "".join(f"+{u}X{at}.OBJ" for at in range(len(job.objects)))
        link = f"{tools.link} /NOE{' /MAP' if job.map else ''} {u}.OBJ{more},{u}.EXE,,{libraries}; > {u}.LNK"
        for data in job.files:
            place(data, work / data.name.upper())
        if job.kind == "exe":
            shutil.copy(job.path, work / f"{u}.EXE")
        elif job.kind == "obj":
            shutil.copy(job.path, work / f"{u}.OBJ")
            for at, extra in enumerate(job.objects):
                shutil.copy(extra, work / f"{u}X{at}.OBJ")
            building.append(link)
        else:
            (work / f"{u}.BAS").write_bytes(crlf(job.path.read_bytes()))
            building.append(f"{tools.bc} {job.switches} {u}.BAS,{u}.OBJ; > {u}.BCO")
            building.append(link.replace(" /NOE", "", 1))
    script = [f":ms {build_ms}", *head, *building, "."]
    for job in jobs:
        u = job.stem.upper()
        script += [f":ms {job.budget_ms or budget_ms}", *head, f"if exist {u}.EXE {job.runner} {u}.EXE {job.args} > {u}.TXT", "."]
    (work / "job.conf").write_text(conf)
    (work / "jobs.txt").write_text("\n".join(script) + "\n")
    events = work / "events.txt"
    with open(work / "jobs.txt") as stdin, open(events, "w") as sink:
        try:
            over = _launch([str(DOSBOX), "-nolog", "-conf", str(work / "job.conf")], work, timeout, cap, stdin=stdin,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, pass_fds=(sink.fileno(),),
                           env={**os.environ, "SDL_VIDEODRIVER": "dummy", "DOSRUN_FD": str(sink.fileno())})
        except subprocess.TimeoutExpired:
            raise RuntimeError(f"DOSBox did not finish in {timeout}s ({work})")
    if over is not None:
        return stopped_for_output(jobs, work, over, cap)
    return collect(jobs, work, events)


def stopped_for_output(jobs: list[Job], work: Path, over: str, cap: int) -> dict[str, Result]:
    """Every job's result when the launch was stopped for the size of `over`: the program whose output it is failed, the
    rest did not run to the end. What the files hold is not read: it is as big as the disk allows."""
    return {job.stem: Result("over the output cap", detail=f"{over} passed {cap} bytes") if over.upper() == f"{job.stem.upper()}.TXT" else Result("stopped", detail=f"the launch was stopped when {over} passed {cap} bytes") for job in jobs}


def collect(jobs: list[Job], work: Path, events: Path) -> dict[str, Result]:
    """Each job's result, from a launch's events and files."""
    lines = [json.loads(line) for line in events.read_text().splitlines() if line.startswith('{"ev":')]
    ends = [one for one in lines if one["ev"] == "end"]
    written: dict[str, str] = {}
    for one in lines:
        if one["ev"] == "out" and one.get("file"):
            written[one["file"].upper()] = written.get(one["file"].upper(), "") + one["text"]
    if len(ends) != len(jobs) + 1:
        raise RuntimeError(f"DOSBox ran {len(ends)} of {len(jobs) + 1} jobs ({events})")
    out = {}
    for job, end in zip(jobs, ends[1:]):
        u = job.stem.upper()
        severe = re.search(r"(\d+) Severe\s+Error", read_dos(work, f"{u}.BCO"))
        if severe and int(severe.group(1)):
            # LINK makes an EXE of what BC refused: never run it
            out[job.stem] = Result("not built", detail="BC: " + read_dos(work, f"{u}.BCO").strip()[-600:])
        elif not (work / f"{u}.EXE").exists():
            out[job.stem] = Result("not built", detail=(read_dos(work, f"{u}.BCO") + read_dos(work, f"{u}.LNK")).strip()[-600:])
        elif end.get("reason") != "exit":
            out[job.stem] = Result("stopped", written.get(f"{u}.TXT", ""), f"{end.get('reason')} after {end.get('ms')} ms ({events})")
        else:
            out[job.stem] = Result("ok", read_dos(work, f"{u}.TXT"), exit_code=end.get("exit_code"))
    return out
