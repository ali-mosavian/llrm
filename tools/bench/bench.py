"""
Per-benchmark executed instructions and memory operands, per language and optimization level, against
the baseline in bench/NAME/expected.toml.

    uv run --project tools python tools/bench/bench.py [NAME ...] [--opt O2 Os] [--bless --reason TEXT] [--json FILE]

Without --bless this is the gate: a count above the baseline fails, and so does one below it, which has to be
blessed too so the baseline only ratchets. Every change to expected.toml carries its reason: --bless needs
--reason, and the text lands in the file. A variant marked `known: #N` is skipped; one that works fails
the gate until its mark goes.
"""

from __future__ import annotations

import os
import sys
import json
import math
import shutil
import argparse
import subprocess
import tomllib
from datetime import date
from pathlib import Path
from dataclasses import dataclass
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "dosbatch"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import maps  # noqa: E402
import corpus  # noqa: E402
import sizes  # noqa: E402
import icount  # noqa: E402
import dosbatch  # noqa: E402
import run_tests  # noqa: E402
from dosbatch import BIN, ROOT, Job  # noqa: E402

OW = Path(os.environ.get("OW_BIN", Path.home() / "work/personal/open-watcom-v2/build/binbuild"))
# Open Watcom's own medium-model C library, 8087 maths library and start-up (an OW v2 release; the tree's build of them
# needs its generated headers and 16-bit bootstrap, which this checkout lacks)
OWLIB = Path(os.environ.get("OW_LIB", Path.home() / "dos/devtools/dev/c/watcom/lib286"))
BENCH = ROOT / "bench"
LANGUAGES = {"bas": ".bas", "c": ".c", "nib": ".nib"}
BASIC_LANGUAGES = ("bas",)
COUNTERS = ("instructions", "memory_operands")
# Bytes of the program's own object (tools/sizes.py): code, initialised data, uninitialised data. Gated like the counters.
SIZES = ("code_bytes", "data_bytes", "bss_bytes")
# DOSBox at a fixed 75000 cycles per millisecond: RDTSC cycles / 75000 is milliseconds, the same on any host.
TIMING_CONF = dosbatch.CONF.replace("cycles=max", "cycles=fixed 75000")
CYCLES_PER_MS = 75_000
BCC = Path(os.environ.get("TOOLCHAINS", Path.home() / "work/other/d32x/toolchains")) / "bcpp31"
TURBO = Path(os.environ.get("TURBO", Path.home() / "scratch/toolchains"))
# Borland-family references: language -> (toolchain folder, compiler, switches per level). All medium model, 8087,
# linked with the toolchain's own C0M, CM, FP87 and MATHM. BCC's switches (-3 -Ox, -O1) are added by bcc.sh.
BORLAND = {
    "bcc": (BCC, "bcc", {"O2": "", "Os": "-O1"}),
    # TC 2.01: -G speed, -O jumps, -Z registers; `huge` is its spelling of __huge
    "tc": (TURBO / "tc201", "tcc", {"O2": "-G -O -Z -r -1 -D__huge=huge", "Os": "-O -Z -r -1 -D__huge=huge"}),
    "tcpp": (TURBO / "tcpp30", "tcc", {"O2": "-2 -G -O -Z -r", "Os": "-2 -O -Z -r"}),  # no -3 in TC++ 3.0; TC 2.01's optimiser switches
}
STARTUP = Path(__file__).with_name("startup")  # an empty kernel per language: what a toolchain's start-up costs
REFERENCES = {"c": ("ow", "bcc", "tc", "tcpp"), "bas": ("bc", "pds71", "vbdos")}  # the reference compilers per language; the BCs have one level, /O
# BASIC references: the name in expected.toml -> the toolchain (run_tests.TOOLS) whose BC compiles and whose runtime links.
BASIC_REFERENCES = {"bc": "qb45", "pds71": "pds71", "vbdos": "vbdos"}
# Kernel time is a difference of two whole-program times, each a few cycles off from run to run: parity/loop's 139
# instructions (0.0029 ms) read 3.5% apart on two runs. A change must clear both the relative and the absolute slack.
TIME_TOLERANCE = 0.01
TIME_SLACK_MS = 0.001


def reference_level(reference: str, opt: str) -> str:
    return "O2" if reference in BASIC_REFERENCES else opt


@dataclass(frozen=True)
class Variant:
    name: str  # bench/NAME, parity/NAME
    language: str
    source: Path
    region: str  # the kernel function's name
    known: str | None
    dialect: str = "qb45"
    timed_only: bool = False  # no instruction count (the emulator would step ~10^8 of them): timed in DOSBox, sized, output-checked there
    data: tuple[str, ...] = ()  # files it reads, as run_tests' `data:` header: `@name` is a cached corpus


def directory_name(directory: Path) -> str:
    return str(directory.relative_to(BENCH)) if directory.is_relative_to(BENCH) else directory.name


def benchmarks(selected: list[str]) -> list[Path]:
    dirs = [one for one in sorted(BENCH.glob("*/bench.toml"))] + [one for one in sorted(BENCH.glob("parity/*/bench.toml"))]
    found = [one.parent for one in dirs]
    return [one for one in found if not selected or one.name in selected or str(one.relative_to(BENCH)) in selected]


def variants(directory: Path) -> list[Variant]:
    settings = tomllib.loads((directory / "bench.toml").read_text())
    name = directory_name(directory)
    out = []
    for language, suffix in LANGUAGES.items():
        source = directory / f"{directory.name}{suffix}"
        if source.exists():
            head = run_tests.header(source)
            out.append(Variant(name, language, source, settings["region"][language], head.get("known"), head.get("dialect", "qb45").split()[0],
                              bool(settings.get("timed_only")), tuple(head.get("data", "").split())))
    return out


def expected_output(directory: Path) -> list[str]:
    return run_tests.lines((directory / f"{directory.name}.out").read_text())


def build(variant: Variant, opt: str, work: Path, stem: str) -> tuple[Path, Path] | str:
    """(exe, linker map) of the variant at -`opt`, or why it did not build. BASIC is only compiled here: LINK runs in DOSBox."""
    obj, listing = work / f"{stem}.obj", work / f"{stem}.map"
    # The kernel stays a call: inlined into main it has no entry to count from.
    # LLRM_BENCH_FLAGS: more compiler flags for a measurement, `-fsanitize=stack`'s overhead; the gate fails on it.
    more = os.environ.get("LLRM_BENCH_FLAGS", "").split()
    flags = [f"-{opt}", "--cpu", "486", "-fno-inline-functions", *more]
    try:
        if variant.language == "nib":
            exe = work / f"{stem}.exe"
            extras = [str(variant.source.parent / one) for one in run_tests.header(variant.source).get("link", "").split()]
            done = subprocess.run([str(ROOT / "tools" / "nib-build.sh"), str(variant.source), str(exe), f"-{opt}", *extras], capture_output=True, text=True, timeout=300,
                                  env={**os.environ, "LLRM_BIN": str(BIN), "TOOLCHAIN": str(BIN), "NIB_MAP": str(listing), "NIB_OBJ": str(obj), "NIB_FLAGS": " ".join(["-fno-inline-functions", *more])})
            return (exe, listing) if done.returncode == 0 and exe.exists() else "build: " + (done.stderr or done.stdout).strip()[-300:]
        tool = "llrm-qb" if variant.language in BASIC_LANGUAGES else "llrm-c"
        arguments = ["--dialect", variant.dialect, "--runtime", variant.dialect] if variant.language in BASIC_LANGUAGES else []
        extra = [one for one in run_tests.compiler_arguments(variant.source) if one not in ("--dialect", "--runtime", variant.dialect)]
        done = subprocess.run([str(BIN / tool), str(variant.source), *arguments, *extra, *flags, "-o", str(obj)], capture_output=True, text=True, timeout=300)
        if done.returncode != 0 or not obj.exists():
            return "compile: " + (done.stderr or done.stdout).strip()[-300:]
        if variant.language == "c":
            exe = work / f"{stem}.exe"
            dosbatch.link_c(obj, exe, work, listing)
            return exe, listing
    except (dosbatch.BuildError, subprocess.TimeoutExpired) as error:
        return f"build: {error}"
    return obj, listing  # BASIC: linked below


def link_basic(objs: dict[str, tuple[Path, str]], work: Path) -> dict[str, str]:
    """Link every BASIC object in one DOSBox launch per dialect; the EXEs and maps land in `work`. Returns why one did not."""
    failed = {}
    for dialect, tools in run_tests.TOOLS.items():
        mine = {stem: path for stem, (path, one) in objs.items() if one == dialect}
        if not mine:
            continue
        ran = dosbatch.run([Job(stem, "obj", path, map=True) for stem, path in mine.items()], work / f"link-{dialect}", tools=tools)
        for stem in mine:
            if ran[stem].status == "not built":
                failed[stem] = "link: " + ran[stem].detail
            else:
                for suffix in ("EXE", "MAP"):
                    shutil.copy(work / f"link-{dialect}" / f"{stem.upper()}.{suffix}", work / f"{stem}.{suffix.lower()}")
    return failed


def measure(job: tuple[str, Path, Path, str, list[str], Path]) -> dict:
    """Counts of one built variant: the entry address from its map, the emulated run, the output check, and the bytes
    of its own object `obj` (tools/sizes.py): not the image, which holds the start-up and the libraries as well."""
    stem, exe, listing, region, want, obj = job
    if region is None:  # timed only: its output is checked where it is timed
        data, bss = sizes.data_bytes(obj)
        return {"code_bytes": sizes.code_bytes(obj), "data_bytes": data, "bss_bytes": bss}
    entry = maps.locate(exe, listing, region)
    if entry is None:
        return {"error": f"{region} is not in the map, and main does not call one function of its own"}
    result = icount.run(exe, entry, exe.parent)
    if result.error:
        return {"error": result.error}
    got = run_tests.lines(result.output.decode("latin1"))
    if got != want:
        return {"error": f"prints {got}, want {want}"}
    data, bss = sizes.data_bytes(obj)
    return {"instructions": result.region.instructions, "memory_operands": result.region.memory_operands, "code_bytes": sizes.code_bytes(obj), "data_bytes": data, "bss_bytes": bss}


def references_available() -> bool:
    return (OW / "bwcc").exists() and (OWLIB / "dos" / "clibm.lib").exists() and (dosbatch.QB45 / "BC.EXE").exists() and (BCC / "lib" / "C0M.OBJ").exists()


def build_watcom(variant: Variant, opt: str, work: Path, stem: str) -> tuple[Path, Path] | str:
    """Open Watcom, medium model, cdecl as llrm-c calls, no stack checks, inline 8087: tools/loops' reference build.
    Linked with Watcom's own start-up, CLIBM and MATH87M (NOEMU87: no emulator, as BCC's FP87). Only `report` comes
    from the corpus's ext.asm."""
    obj, exe, listing = work / f"{stem}.obj", work / f"{stem}.exe", work / f"{stem}.map"
    flags = ["-ox", "-oe=0"] if opt == "O2" else ["-os", "-ol"]  # -oe=0: the kernel stays a call, as llrm-c compiles it
    done = subprocess.run([str(OW / "bwcc"), "-zq", "-mm", "-ecc", "-s", "-DOWREF", "-4", "-fpi87", *flags, str(variant.source), f"-fo={obj}"], capture_output=True, text=True, timeout=300)
    if done.returncode != 0 or not obj.exists():
        return "compile: " + (done.stderr or done.stdout).strip()[-300:]
    ext = work / "EXT.OBJ"
    try:
        if not ext.exists():
            dosbatch.assemble(dosbatch.C_RUNTIME / "ext.asm", ext)
        # Watcom names main `main_`; its start-up asks for `_cstart_` and the library calls it
        dosbatch._host([str(BIN / "jwlink"), "option", "quiet", "option", f"map={listing}", "option", "start=_cstart_", "option", "stack=16k", "format", "dos", "name", str(exe),
                        "libpath", str(OWLIB / "dos"), "libpath", str(OWLIB), "file", str(obj), "file", str(ext),
                        "library", "clibm.lib", "library", "math87m.lib", "library", "noemu87.lib"])
    except dosbatch.BuildError as error:
        return f"build: {error}"
    return exe, listing


def c_source(path: Path) -> bytes:
    """A C program as the references compile it: without the `// key: value` header lines (run_tests.header), which
    Turbo C 2.01 does not read as comments ("Declaration syntax error" on line 1 of grep.c)."""
    lines = path.read_bytes().splitlines(keepends=True)
    head = 0
    while head < len(lines) and run_tests.HEADER.match(lines[head].decode("latin1")):
        head += 1
    return b"".join(lines[head:])


def build_borland(language: str, variants_: list[tuple[Variant, str]], opt: str, work: Path) -> dict[str, tuple[Path, Path] | str]:
    """A Borland-family reference (BORLAND), every program in one DOSBox boot. Keyed by stem.
    Linked with the toolchain's own start-up (C0M) and libraries: the floating-point programs need its FP87 and MATHM,
    which ask C0M for `__version` and `_errno`. Only `report` comes from the corpus's ext.asm."""
    home, compiler, levels = BORLAND[language]
    folder = work / f"{language}{opt}"
    folder.mkdir(parents=True)
    for variant, stem in variants_:
        text = c_source(variant.source)
        if home.name == "tc201":  # TC 2.01 reads a `#` after a bare LF as an illegal character: every program with a #define failed
            text = text.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")
        (folder / f"{stem}.c").write_bytes(text)
    subprocess.run([str(ROOT / "tools" / "callconv" / "bcc.sh"), str(folder), *[f"{stem}.c {levels[opt]}".strip() for _, stem in variants_]],
                   env={**os.environ, "CCROOT": str(home.parent), "CCDIR": home.name, "CCEXE": compiler}, capture_output=True, timeout=1800)
    ext = folder / "EXT.OBJ"
    dosbatch.assemble(dosbatch.C_RUNTIME / "ext.asm", ext)
    out: dict[str, tuple[Path, Path] | str] = {}
    for _, stem in variants_:
        obj, exe, listing = folder / f"{stem}.OBJ", folder / f"{stem}.exe", folder / f"{stem}.map"
        if not obj.exists():
            message = folder / f"{stem}.MSG"
            out[stem] = f"{compiler}: " + (message.read_text(errors="replace").strip()[-200:] if message.exists() else "no object")
            continue
        lib = home / "lib"
        try:
            dosbatch._host([str(BIN / "jwlink"), "option", "quiet", "option", f"map={listing}", "format", "dos", "name", str(exe), "file", str(lib / "C0M.OBJ"), "file", str(obj), "file", str(ext),
                            "library", str(lib / "CM.LIB"), "library", str((BCC / "lib" if language == "tcpp" else lib) / "FP87.LIB"), "library", str(lib / "MATHM.LIB")])
        except dosbatch.BuildError as error:
            out[stem] = f"link: {error}"
            continue
        out[stem] = (exe, listing)
    return out


def with_startup(selected: list[str], timing: bool) -> list[tuple[Path, Variant]]:
    """Every (directory, variant) to build, plus under --time the empty-kernel programs that price each start-up."""
    todo = [(directory, variant) for directory in benchmarks(selected) for variant in variants(directory)]
    return todo + [(STARTUP, variant) for variant in variants(STARTUP)] if timing else todo


def measure_references(selected: list[str], opts: list[str], work: Path, timing: bool = False) -> dict[tuple[str, str, str], dict]:
    """The reference compilers on the same programs: Open Watcom and BCC 3.1 for C (languages `ow`, `bcc`, at each level)
    and QuickBASIC 4.5's BC for BASIC (language `bc`, one level: /O). With --time each also gets its kernel time."""
    out: dict[tuple[str, str, str], dict] = {}
    folder = work / "reference"
    shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True)
    todo = with_startup(selected, timing)
    stems = [f"R{at:03d}" for at in range(len(todo))]
    jobs, owned, plain, needs = [], [], {}, {}
    def own(key, stem, directory, variant, built, obj=None):
        if isinstance(built, str):
            out[key] = {"error": built}
        elif directory == STARTUP:
            plain[key] = built[0]
        else:
            owned.append((key, (stem, built[0], built[1], None if variant.timed_only else variant.region, expected_output(directory), obj)))
            needs[key] = timing_input(variant, directory)
    for opt in opts:
        c_todo = [(directory, variant, stem) for (directory, variant), stem in zip(todo, stems) if variant.language == "c"]
        for directory, variant, stem in c_todo:
            if (OW / "bwcc").exists():
                own((variant.name, "ow", opt), stem, directory, variant, build_watcom(variant, opt, folder, f"{stem}{opt}"), folder / f"{stem}{opt}.obj")
        for language, (home, _, _) in BORLAND.items():
            if (home / "lib" / "C0M.OBJ").exists():
                built = build_borland(language, [(variant, stem) for _, variant, stem in c_todo], opt, folder)
                for directory, variant, stem in c_todo:
                    own((variant.name, language, opt), stem, directory, variant, built[stem], folder / f"{language}{opt}" / f"{stem}.OBJ")
    bas = [(directory, variant, stem) for (directory, variant), stem in zip(todo, stems) if variant.language == "bas"]
    for reference, dialect in BASIC_REFERENCES.items():
        tools = run_tests.TOOLS[dialect]
        if not bas or not tools.mount.is_dir():
            continue
        # /AH where the program asks for huge arrays; the dialect of a program is llrm's, every BC is given every program
        jobs = [Job(stem, "bas", variant.source, switches="/O /FPi" + (" /AH" if "--huge-arrays" in run_tests.compiler_arguments(variant.source) else ""), map=True) for _, variant, stem in bas]
        ran = dosbatch.run(jobs, folder / reference, tools=tools)
        for (directory, variant, stem) in bas:
            key = (variant.name, reference, "O2")
            if ran[stem].status == "not built":
                out[key] = {"error": f"{reference}: " + ran[stem].detail}
                continue
            exe, listing, obj = folder / f"{reference}-{stem}.exe", folder / f"{reference}-{stem}.map", folder / f"{reference}-{stem}.obj"
            for source, target in ((f"{stem}.EXE", exe), (f"{stem}.MAP", listing), (f"{stem}.OBJ", obj)):
                shutil.copy(folder / reference / source, target)
            own(key, stem, directory, variant, (exe, listing), obj)
    with ProcessPoolExecutor() as pool:
        for (key, _), counts in zip(owned, pool.map(measure, [job for _, job in owned])):
            out[key] = counts
    if timing:
        exes = {key: job[1] for key, job in owned if "error" not in out[key]} | plain
        for key, timed in time_programs(exes, folder / "time", {key: needs[key] for key in exes if needs.get(key)}).items():
            out[key] = out.get(key, {}) | ({"time_error": timed["error"]} if "error" in timed else timed)
    return net_times(out)


def net_times(measured: dict) -> dict:
    """Each program's time less its toolchain's start-up (the same toolchain and level, built from startup/): kernel_ms.
    The whole program's time prices C0M's or the BASIC runtime's initialisation against llrm's small crt, which the
    kernel does not do. The start-up programs themselves leave the results."""
    for key, value in measured.items():
        if "cycles" in value and key[0] != "startup":
            base = measured.get(("startup", key[1], key[2]), {})
            if "cycles" in base:
                value["kernel_ms"] = round((value["cycles"] - base["cycles"]) / CYCLES_PER_MS, 4)
    return {key: value for key, value in measured.items() if key[0] != "startup"}


def measure_all(selected: list[str], opts: list[str], work: Path, timing: bool = False) -> dict[tuple[str, str, str], dict]:
    """{(benchmark, language, opt): counts or {"error": why} or {"known": issue}}"""
    out: dict[tuple[str, str, str], dict] = {}
    for opt in opts:
        folder = work / opt
        shutil.rmtree(folder, ignore_errors=True)
        folder.mkdir(parents=True)
        todo = with_startup(selected, timing)
        stems = {(v.name, v.language): f"V{at:03d}" for at, (_, v) in enumerate(todo)}
        with ThreadPoolExecutor() as pool:
            built = list(pool.map(lambda one: build(one[1], opt, folder, stems[(one[1].name, one[1].language)]), todo))
        basic = {stems[(v.name, v.language)]: (b[0], v.dialect) for (_, v), b in zip(todo, built) if v.language in BASIC_LANGUAGES and not isinstance(b, str)}
        failed = link_basic(basic, folder)
        jobs, plain, needs = [], {}, {}
        for (directory, variant), result in zip(todo, built):
            key = (variant.name, variant.language, opt)
            stem = stems[(variant.name, variant.language)]
            why = result if isinstance(result, str) else failed.get(stem)
            if why:
                out[key] = {"error": why}
                continue
            exe, listing = (folder / f"{stem}.exe", folder / f"{stem}.map") if variant.language in BASIC_LANGUAGES else result
            if directory == STARTUP:
                plain[key] = exe  # timed, not counted
            else:
                jobs.append((key, (stem, exe, listing, None if variant.timed_only else variant.region, expected_output(directory), folder / f"{stem}.obj")))
                needs[key] = timing_input(variant, directory)
        with ProcessPoolExecutor() as pool:
            for (key, _), counts in zip(jobs, pool.map(measure, [job for _, job in jobs])):
                out[key] = counts
        if timing:
            exes = {key: job[1] for key, job in jobs if "error" not in out[key]} | plain
            for key, timed in time_programs(exes, folder / "time", {key: needs[key] for key in exes if needs.get(key)}).items():
                out[key] = out.get(key, {}) | ({"time_error": timed["error"]} if "error" in timed else timed)
    return mark_known(net_times(out) if timing else out, [v for directory in benchmarks(selected) for v in variants(directory)], opts)


def mark_known(measured: dict, all_variants: list[Variant], opts: list[str]) -> dict:
    """A variant with a `known:` mark fails at some level; there its result is {"known": issue, ...}. If it works at
    every level, the mark is stale: {"xpass": issue, ...} fails the gate. At a level where it works while another
    still fails, it is measured like any other."""
    for variant in all_variants:
        if not variant.known:
            continue
        keys = [(variant.name, variant.language, opt) for opt in opts]
        broken = [key for key in keys if "error" in measured[key]]
        for key in keys:
            if key in broken:
                measured[key] = {"known": variant.known, **measured[key]}
            elif not broken:
                measured[key] = {"xpass": variant.known, **measured[key]}
    return measured


def timing_input(variant: Variant, directory: Path) -> dict | None:
    """{files a timed-only program reads, want: the output it must print}, or {skip: why} when its corpus is unavailable, or None
    for the programs the emulator counts."""
    if not variant.timed_only:
        return None
    try:
        files = tuple(corpus.path(one[1:]) if one.startswith("@") else variant.source.parent / one for one in variant.data)
    except corpus.Unavailable as error:
        return {"skip": str(error)}
    return {"files": files, "want": expected_output(directory)}


def time_programs(exes: dict[tuple[str, str, str], Path], work: Path, inputs: dict | None = None) -> dict[tuple[str, str, str], dict]:
    """Wall time of each whole program in DOSBox, from RDTSC around the run (tools/bench/timeit.asm): {"cycles", "ms"}.
    Whole-program: it includes the runtime's start-up and the print, which differ per toolchain. net_times takes the
    start-up out."""
    if not exes:
        return {}
    work.mkdir(parents=True, exist_ok=True)
    stub = work / "TIMEIT.COM"
    done = subprocess.run([str(BIN / "jwasm"), "-q", "-bin", f"-Fo{stub}", str(Path(__file__).with_name("timeit.asm"))], capture_output=True, text=True)
    if done.returncode != 0:
        return {key: {"error": "timeit.asm: " + done.stderr.strip()[-200:]} for key in exes}
    inputs = inputs or {}
    out = {}
    for why in {one["skip"] for one in inputs.values() if "skip" in one}:
        print(f"SKIP timing: {why}")
    out |= {key: {"error": "skipped: " + one["skip"]} for key, one in inputs.items() if "skip" in one}
    keys = [key for key in exes if key not in out]
    jobs = [Job(f"W{at:03d}", "exe", exes[key], runner="TIMEIT.COM", files=(stub, *inputs.get(key, {}).get("files", ())), budget_ms=600_000 if key in inputs else None) for at, key in enumerate(keys)]
    ran = dosbatch.run(jobs, work / "run", conf=TIMING_CONF)
    for key, job in zip(keys, jobs):
        text = ran[job.stem].text
        found = [line for line in text.splitlines() if line.startswith("TSC ")]
        printed = run_tests.lines("\n".join(line for line in text.splitlines() if not line.startswith("TSC ")))
        if found and key in inputs and printed != inputs[key]["want"]:  # nothing else checks a timed-only program's output
            out[key] = {"error": f"prints {printed}, want {inputs[key]['want']}"}
        else:
            out[key] = {"cycles": int(found[-1][4:]), "ms": round(int(found[-1][4:]) / CYCLES_PER_MS, 3)} if found else {"error": f"no timing line: {text[-80:]!r}"}
    return out


def read_expected(directory: Path) -> dict:
    path = directory / "expected.toml"
    return tomllib.loads(path.read_text()) if path.exists() else {}


def write_expected(directory: Path, measured: dict, name: str, reason: str) -> None:
    """Merge what was measured into expected.toml: a table not measured this run (another level, a reference left alone)
    stays. The kernel time of a table whose instructions moved is dropped: it belongs to the old counts."""
    tables = {one: dict(table) for one, table in read_expected(directory).items() if isinstance(table, dict)}
    for (benchmark, language, opt), counts in measured.items():
        if benchmark != name or "error" in counts:
            continue
        new = {one: counts[one] for one in (*COUNTERS, *SIZES, "kernel_ms") if one in counts}
        old = tables.get(language, {}).get(opt, {})
        if "kernel_ms" not in new and "kernel_ms" in old and old.get("instructions") == new.get("instructions"):
            new["kernel_ms"] = old["kernel_ms"]
        tables.setdefault(language, {})[opt] = new
    lines = [f"reason = {json.dumps(reason)}", f'blessed = "{date.today()}"', ""]  # JSON escapes are TOML's
    for language in dict.fromkeys([*LANGUAGES, *BASIC_LANGUAGES, *(one for many in REFERENCES.values() for one in many)]):
        for opt in sorted(tables.get(language, {})):
            lines += [f"[{language}.{opt}]", *(f"{one} = {value}" for one, value in tables[language][opt].items()), ""]
    (directory / "expected.toml").write_text("\n".join(lines))


def against(label: str, got: dict, want: dict | None) -> list[str]:
    """What fails the gate for one measurement: no baseline, a count above it (worse), one below it (better, to bless)."""
    if want is None:
        return [f"{label}: no baseline; run --bless --reason"]
    problems = []
    for counter in (*COUNTERS, *SIZES):
        if counter not in got:
            continue
        if counter not in want:
            problems.append(f"{label}: no baseline for {counter}; run --bless --reason")
        elif got[counter] > want[counter]:
            problems.append(f"{label}: {counter} {got[counter]} > {want[counter]} (worse)")
        elif got[counter] < want[counter]:
            problems.append(f"{label}: {counter} {got[counter]} < {want[counter]} (better: --bless --reason)")
    return problems


def time_against(label: str, got: dict, want: dict | None) -> list[str]:
    """A timed-only program has no count to ratchet: its own kernel time gates, with the time tolerance, up (worse) and down (bless)."""
    if want is None or "kernel_ms" not in got:
        return []
    if "kernel_ms" not in want:
        return [f"{label}: no baseline for kernel_ms; run --bless --time --reason"]
    slack = max(TIME_TOLERANCE * want["kernel_ms"], TIME_SLACK_MS)
    if got["kernel_ms"] > want["kernel_ms"] + slack:
        return [f"{label}: kernel_ms {got['kernel_ms']} > {want['kernel_ms']} (worse)"]
    if got["kernel_ms"] < want["kernel_ms"] - slack:
        return [f"{label}: kernel_ms {got['kernel_ms']} < {want['kernel_ms']} (better: --bless --reason)"]
    return []


def ratio_problems(label: str, reference: str, got: dict, want: dict, ref: dict) -> list[str]:
    """llrm/reference worse than it was blessed at. Against a stored reference this is the count gate seen from the
    reference's side; it stays a gate of its own for the day the reference is re-blessed."""
    problems = []
    for counter in (*COUNTERS, *SIZES, "kernel_ms"):
        if all(counter in one for one in (got, want, ref)) and ref[counter] > 0:
            now, then = got[counter] / ref[counter], want[counter] / ref[counter]
            time = counter == "kernel_ms"
            if now > then * (1 + (TIME_TOLERANCE if time else 0)) and got[counter] - want[counter] > (TIME_SLACK_MS if time else 0):
                problems.append(f"{label}: {counter} vs {reference} {now:.3f}x > blessed {then:.3f}x (worse)")
    return problems


def drift_problems(label: str, got: dict, stored: dict | None) -> list[str]:
    """A reference measured again against the one stored: the compiler or the harness changed."""
    if "error" in got:
        return [f"{label}: no longer builds: {got['error']}"] if stored else []
    if stored is None:
        return [f"{label}: not recorded; run --bless --references --reason"]
    problems = []
    for counter in (*COUNTERS, *SIZES, "kernel_ms"):
        if counter in got and counter in stored:
            slack = max(TIME_TOLERANCE * abs(stored[counter]), TIME_SLACK_MS) if counter == "kernel_ms" else 0
            if abs(got[counter] - stored[counter]) > slack:
                problems.append(f"{label}: {counter} {got[counter]} != stored {stored[counter]} (reference changed: --bless --references --reason)")
    return problems


def judge(measured: dict, selected: list[str], opts: list[str]) -> list[str]:
    """What fails the gate."""
    problems = []
    for directory in benchmarks(selected):
        name = str(directory.relative_to(BENCH))
        baseline = read_expected(directory)
        for variant in variants(directory):
            for opt in opts:
                got = measured[(name, variant.language, opt)]
                label = f"{name} {variant.language} -{opt}"
                if "known" in got:
                    continue
                if "xpass" in got:
                    problems.append(f"{label}: works; remove its known mark ({got['xpass']})")
                elif "error" in got:
                    problems.append(f"{label}: {got['error']}")
                else:
                    want = baseline.get(variant.language, {}).get(opt)
                    problems += against(label, got, want)
                    if variant.timed_only:
                        problems += time_against(label, got, want)
                    for reference in REFERENCES.get(variant.language, ()):
                        ref = baseline.get(reference, {}).get(reference_level(reference, opt))
                        if want and ref:
                            problems += ratio_problems(label, reference, got, want, ref)
            for reference in REFERENCES.get(variant.language, ()):
                for level in sorted({reference_level(reference, opt) for opt in opts}):
                    if (name, reference, level) in measured:
                        problems += drift_problems(f"{name} {reference} -{level}", measured[(name, reference, level)], baseline.get(reference, {}).get(level))
    return problems


def geomean(values: list[float]) -> float:
    return math.exp(sum(map(math.log, values)) / len(values))


def standing(measured: dict, selected: list[str], opts: list[str]) -> list[str]:
    """llrm against each reference, as a geomean over the benchmarks both have, per language and level: llrm/reference
    less one, so -25% is llrm's 25% below. Instructions and memory operands against the stored reference; time (the
    kernel's, net of start-up) when --time measured it. BC has one level: both BASIC levels meet its /O."""
    lines = []
    for language, references in REFERENCES.items():
        for reference in references:
            for opt in opts:
                level = reference_level(reference, opt)
                rows = []
                for directory in benchmarks(selected):
                    name = str(directory.relative_to(BENCH))
                    got = measured.get((name, language, opt), {})
                    ref = read_expected(directory).get(reference, {}).get(level)
                    if ref and "error" not in got and any(one in got for one in ("instructions", "kernel_ms", "code_bytes")):
                        rows.append((got, ref))
                parts = []
                for counter, title in (("instructions", "instructions"), ("memory_operands", "memory operands"), ("kernel_ms", "kernel time"),
                                       ("code_bytes", "code bytes"), ("data_bytes", "data bytes"), ("bss_bytes", "bss bytes")):
                    ratios = [got[counter] / ref[counter] for got, ref in rows if counter in got and counter in ref and got[counter] > 0 and ref[counter] > 0]
                    if ratios:
                        parts.append(f"{title} {100 * (geomean(ratios) - 1):+.1f}% (n={len(ratios)})")
                if parts:
                    lines.append(f"llrm {language} -{opt} vs {reference} {'/O' if reference in BASIC_REFERENCES else '-' + level}: " + ", ".join(parts))
    return lines


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("select", nargs="*")
    parser.add_argument("--opt", nargs="+", default=["O2", "Os"])
    parser.add_argument("--time", action="store_true", help="also the kernel's time in DOSBox from RDTSC, net of start-up; gates the time ratio to each reference")
    parser.add_argument("--references", action="store_true", help="measure Open Watcom and BCC (C) and BC 4.5 (BASIC) again: they must equal the stored ones, or --bless records them")
    parser.add_argument("--bless", action="store_true")
    parser.add_argument("--reason", default="")
    parser.add_argument("--json", type=Path)
    parser.add_argument("--work", type=Path, default=ROOT / "target" / "bench")
    args = parser.parse_args()
    if args.bless and not args.reason:
        parser.error("--bless needs --reason: every change to expected.toml says why")
    measured = measure_all(args.select, args.opt, args.work, args.time)
    if args.references:
        measured |= measure_references(args.select, args.opt, args.work, args.time)
    if args.json:
        args.json.write_text(json.dumps({"/".join(key): value for key, value in measured.items()}, indent=1))
    if args.bless:
        names = {key[0] for key in measured}
        for directory in benchmarks(args.select):
            name = str(directory.relative_to(BENCH))
            broken = [f"{key}: {value['error']}" for key, value in measured.items() if key[0] == name and key[1] in (*LANGUAGES, *BASIC_LANGUAGES) and "error" in value and "known" not in value]
            if broken:
                print(f"not blessing {name}: {broken}")
                return 1
            write_expected(directory, measured, name, args.reason)
        print(f"blessed {len(names)} benchmarks")
        print(*standing(measured, args.select, args.opt), sep="\n")
        return 0
    problems = judge(measured, args.select, args.opt)
    print(*standing(measured, args.select, args.opt), sep="\n")
    for one in problems:
        print("FAIL", one)
    print(f"{len(measured)} measurements: {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
