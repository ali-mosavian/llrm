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
import re
import sys
import functools
import hashlib
import json
import math
import shutil
import tempfile
import argparse
import subprocess
import time
import tomllib
from datetime import date
from pathlib import Path
from dataclasses import dataclass
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "dosbatch"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))
# The real-mode DOS emulator is x86-m16 code: it lives with the target.
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "crates/target/llrm-x86-m16/bench"))
sys.path.insert(0, str(next((Path(__file__).resolve().parents[2] / "crates" / "target").glob("*/vsgcc"))))  # wrap.py: the comparison with gcc and clang's

import cpulimit  # noqa: E402
import benchlist  # noqa: E402
import basecounts  # noqa: E402
import basic_arrays  # noqa: E402
import shards  # noqa: E402
import maps  # noqa: E402
import native  # noqa: E402
import corpus  # noqa: E402
import sizes  # noqa: E402
import helpers  # noqa: E402
import icount  # noqa: E402
import flatcount  # noqa: E402
import dosbatch  # noqa: E402
llrmbin = dosbatch.llrmbin
import qb32  # noqa: E402
import qbbuild  # noqa: E402
import qbruntime  # noqa: E402
import run_tests  # noqa: E402
import wrap  # noqa: E402
from dosbatch import BIN, ROOT, Job  # noqa: E402
os.environ.setdefault("LLRM_VERIFY", "1")  # the gate checks each pass and phase (llrm_support::debug::verifying)

def physical_defines() -> list[str]:
    """`-DPHYSICAL_<NAME>=0x..UL` for each physical address the platform description names, as llrm-cc passes them: the
    reference compilers build the same program, which reads them (bench/textfill)."""
    text = (ROOT / "crates/target/llrm-x86/platform.toml").read_text()
    return [f"-DPHYSICAL_{name.upper()}=0x{address:X}UL" for name, address in tomllib.loads(text)["physical"].items()]


# The source of the module that defines `__doserror_`, which the shipped clibm.lib's creatdos/opendos ask for and its dictionary does not give (stft_single, which creates and opens files).
DOSERROR_ASM = llrmbin.ow_tree() / "bld/clib/_dos/a/error086.asm"
OW = Path(os.environ.get("OW_BIN", Path.home() / "work/personal/open-watcom-v2/build/binbuild"))
# Open Watcom's own medium-model C library, 8087 maths library and start-up (an OW v2 release; the tree's build of them
# needs its generated headers and 16-bit bootstrap, which this checkout lacks)
OWLIB = Path(os.environ.get("OW_LIB", Path.home() / "dos/devtools/dev/c/watcom/lib286"))
BENCH = ROOT / "bench"
# `bas` is BASIC on the Microsoft runtime (MS LINK, in DOSBox: only where QB45 and the others are installed); `bq16` the same source on
# our runtime, real mode, linked by jwlink: always available.
LANGUAGES = {"bas": ".bas", "bq16": ".bas", "bq32": ".bas", "c": ".c", "nib": ".nib"}
# Reference twins: a C source for a Nib semantic the C twin lacks (`init`: locals zero-initialised, `bounds`: indices checked), in
# `bench/NAME/twin/SEMANTIC.c`. It is a variant of the program named `NAME~SEMANTIC`, measured and given to the reference compilers as any
# C program, and not judged or ratcheted: the C compilers' bytes on it are the bar a Nib program with that semantic is held to
# (`bar_program`). It must print what the program prints (`NAME.out`), as every variant does.
TWINS = ("init", "bounds")
BASIC_LANGUAGES = ("bas",)
OURS_BASIC = ("bq16", "bq32")
# The flat target is counted by the emulator the program runs in (guest instructions, on virtual time), net of its empty start-up
# program: the unicorn emulator is real mode.
FLAT = "bq32"
LOADERS: dict[Path, tuple[Path, ...]] = {}  # an exe -> the files that must sit beside it (DOS/32A)
COUNTERS = ("instructions", "memory_operands")
# A kernel that runs fewer instructions than this was folded to a constant (`mov eax, imm; ret` is 2): its row measures the compiler's
# folding, not the kernel, and a column of such rows reads as an 8x-70x gap against languages that ran the work.
FOLDED_BELOW = 8
# Bytes of the program's own object (tools/sizes.py): code, initialised data, uninitialised data. Gated like the counters.
SIZES = ("code_bytes", "data_bytes", "bss_bytes")
JUDGED_SIZES = ("code_bytes", "data_bytes", "static_bytes")  # bss is judged through the footprint (data + bss), not alone


def with_footprint(row: dict, run_time: int = 0) -> dict:
    """The row with `static_bytes` = data + bss (+ `run_time`): a zero table moved from data to bss shrinks the image and leaves the footprint.
    `run_time` is what the program allocates at run time beyond its image (run_time_arrays): the cross-compiler verdicts add it, the gate against a base does not."""
    return {**row, "static_bytes": row["data_bytes"] + row["bss_bytes"] + run_time} if "data_bytes" in row and "bss_bytes" in row else row


def run_time_arrays(program: str, name: str) -> int:
    """The module-level array bytes `name` ('bas'/'bq16': llrm; a BASIC reference) allocates at run time for the BASIC program, else 0 (tools/bench/basic_arrays.py)."""
    side = "llrm" if name in ("bas", "bq16") else "reference" if name in BASIC_REFERENCES else None
    source = BENCH / program / f"{program.rsplit('/', 1)[-1]}.bas"
    return basic_arrays.run_time_bytes(source, side) if side and source.exists() else 0
# DOSBox at a fixed 75000 cycles per millisecond: RDTSC cycles / 75000 is milliseconds, the same on any host.
TIMING_CONF = dosbatch.CONF.replace(dosbatch.CYCLES, "cycles=fixed 75000")
CYCLES_PER_MS = 75_000
BCC = Path(os.environ.get("TOOLCHAINS", Path.home() / "work/other/d32x/toolchains")) / "bcpp31"
TURBO = Path(os.environ.get("TURBO", Path.home() / "scratch/toolchains"))
# Borland-family references: language -> (toolchain folder, compiler, switches per level). All medium model, 8087,
# linked with the toolchain's own C0M, CM, FP87 and MATHM. BCC's switches (-3 -Ox, -O1) are added by bcc.sh.
REFERENCE_OPTS = ("O1", "O2", "Os")  # the levels the reference compilers are built at (BORLAND's level maps, build_watcom's flags); O1 is their size level, the nearest to a light -O1: none has a separate one
BORLAND = {
    "bcc": (BCC, "bcc", {"O1": "-O1", "O2": "", "Os": "-O1"}),
    # TC 2.01: -G speed, -O jumps, -Z registers; `huge` is its spelling of __huge
    "tc": (TURBO / "tc201", "tcc", {"O1": "-O -Z -r -1 -D__huge=huge", "O2": "-G -O -Z -r -1 -D__huge=huge", "Os": "-O -Z -r -1 -D__huge=huge"}),
    "tcpp": (TURBO / "tcpp30", "tcc", {"O1": "-2 -O -Z -r", "O2": "-2 -G -O -Z -r", "Os": "-2 -O -Z -r"}),  # no -3 in TC++ 3.0; TC 2.01's optimiser switches
}
STARTUP = Path(__file__).with_name("startup")  # an empty kernel per language: what a toolchain's start-up costs
IO_SKELETON = Path(__file__).with_name("io_skeleton")  # a Nib program of std.io's file calls and nothing else: what the I/O of a bench's Nib cell costs (io_cost)
REFERENCES = {"c": ("ow", "bcc", "tc", "tcpp"), "bas": ("bc", "pds71", "vbdos"), "bq16": ("bc", "pds71", "vbdos")}  # the reference compilers per language; the BCs have one level, /O
# BASIC references: the name in expected.toml -> the toolchain (run_tests.TOOLS) whose BC compiles and whose runtime links.
BASIC_REFERENCES = {"bc": "qb45", "pds71": "pds71", "vbdos": "vbdos"}
# Kernel time is a difference of two whole-program times, each a few cycles off from run to run: parity/loop's 139
# instructions (0.0029 ms) read 3.5% apart on two runs. A change must clear both the relative and the absolute slack.
TIME_TOLERANCE = 0.01
TIME_SLACK_MS = 0.001


def tools_missing(variant: "Variant") -> str | None:
    """Why a Microsoft-runtime BASIC variant cannot be built here, or None: BC and LINK are the dialect's own install (the
    environment variable names where it is), which a runner may not have."""
    if variant.language not in BASIC_LANGUAGES:
        return None
    tools = run_tests.TOOLS[variant.dialect]
    if tools.mount.is_dir():
        return None
    return f"{variant.dialect}: no Microsoft BASIC install at {tools.mount} (set {variant.dialect.upper()}_DIR); `{variant.language}` needs BC and LINK, `bq16` and `bq32` do not"


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
    ci_bytes: int = 0  # the default bench (no --time) feeds a timed-only program this many bytes of its corpus, and checks NAME.ci.out
    emulated: bool = False  # timed_only, and yet counted by the m16 emulator, which serves its declared `data` files from a private folder


def directory_name(directory: Path) -> str:
    return str(directory.relative_to(BENCH)) if directory.is_relative_to(BENCH) else directory.name


def benchmarks(selected: list[str]) -> list[Path]:
    found = benchlist.directories(BENCH)
    return [one for one in found if not selected or one.name in selected or str(one.relative_to(BENCH)) in selected]


ONLY: list[str] | None = None  # `--lang`: the languages measured; None: every one the directory has


def variants(directory: Path, every_language: bool = False) -> list[Variant]:
    settings = tomllib.loads((directory / "bench.toml").read_text())
    name = directory_name(directory)
    out = []
    for language, suffix in LANGUAGES.items():
        source = directory / f"{directory.name}{suffix}"
        if source.exists() and (every_language or ONLY is None or language in ONLY):
            head = run_tests.header(source)
            region = settings["region"].get(language) or settings["region"]["bas"]  # our BASIC is the same kernel
            out.append(Variant(name, language, source, region, None if language in OURS_BASIC else head.get("known"), head.get("dialect", "qb45").split()[0],
                              bool(settings.get("timed_only")), tuple(head.get("data", "").split()), int(settings.get("ci_bytes", 0)),
                              bool(settings.get("emulated"))))
    for semantic in TWINS:
        source = directory / "twin" / f"{semantic}.c"
        if source.exists() and (every_language or ONLY is None or "c" in ONLY):
            head = run_tests.header(source)
            out.append(Variant(f"{name}~{semantic}", "c", source, settings["region"]["c"], head.get("known"), "qb45", bool(settings.get("timed_only")),
                              tuple(head.get("data", "").split()), int(settings.get("ci_bytes", 0)), bool(settings.get("emulated"))))
    return out


def is_twin_name(name: str) -> bool:
    """A reference twin's program name (`NAME~SEMANTIC`)."""
    return "~" in name


def is_twin(variant: Variant) -> bool:
    """A reference twin (TWINS): measured, not judged."""
    return is_twin_name(variant.name)


def bar_program(name: str, language: str, present: set[str]) -> str:
    """The program whose C reference compilers are the bar for `name` in `language`, among the programs `present` in a table: a Nib program
    is held to its C twin with the Nib semantics (`NAME~init`, then `NAME~bounds`) where the program has one, and to `NAME` otherwise;
    the plain twin stays a column of its own."""
    if language != "nib":
        return name
    return next((f"{name}~{semantic}" for semantic in TWINS if f"{name}~{semantic}" in present), name)


def expected_output(directory: Path) -> list[str]:
    return run_tests.lines((directory / f"{directory.name}.out").read_text())


def nib_extras(source: Path) -> list[str]:
    """The files a Nib benchmark links, as nib-build.sh takes them (m16: the bench measures real mode)."""
    return [str(one) for one in dosbatch.link_files(source, run_tests.header(source).get("link", "").split(), dosbatch.REAL_MODE)]


def c_environment(variant: Variant) -> dict[str, str]:
    """What llrm-cc is told of a C program's target: the Watcom row of the 16-bit target, whose programs enter `main_` as dosbatch links them."""
    return {**os.environ, "LLRM_CC_TARGET": dosbatch.WATCOM_ROWS[dosbatch.target_bits(dosbatch.REAL_MODE)]} if variant.language == "c" else dict(os.environ)


def kept_a_call(variant: Variant, work: Path, stem: str, shim: bool = True) -> tuple[Path, Path] | None:
    """A C kernel's sources that keep it a call without `-fno-inline-functions`, which also kept a recursive function from inlining
    itself, as gcc and clang do (`rectwo` ran 4.2x gcc's clocks, 2.6x with it on): `main` calling it through a volatile pointer
    (vsgcc's `wrap.py`, which a kernel of constants cannot be folded out of), to run and count, and the kernel `noinline` with `main` as
    it was, whose object is the one sized: the pointer is 18 bytes of code and 4 of data no reference compiler's object has. The
    attribute is added to what llrm-cc reads; the reference compilers read the source as it is. None where the source has no such kernel
    (the build then keeps the flag). `shim`: file programs call the `vs_*` stubs of vsgcc's harness (gcc and clang); m16 passes False and links DOS's file API."""
    text = variant.source.read_text()
    found = re.search(rf"^([a-z][a-z ]*[ *]){re.escape(variant.region)}\(", text, re.M)
    if found is None:
        return None
    try:
        wrapped = wrap.wrapped(variant.region.removeprefix("bench_"), text, shim)
    except SystemExit:
        return None
    (counted, sized) = (work / f"{stem}.wrapped.c", work / f"{stem}.noinline.c")
    counted.write_text(wrapped)
    sized.write_text(text[: found.start()] + "__attribute__((noinline)) " + text[found.start():])
    return counted, sized


def compile_flags(variant: Variant, opt: str, kept: bool = False) -> list[str]:
    """The flags the harness gives the compiler for `variant` at -`opt`: the kernel stays a call (inlined into main it has no entry to
    count from), a C one by the volatile pointer in `kept_a_call` or else the flag, a BASIC one (a private SUB called once) by turning
    called-once inlining off too. LLRM_BENCH_FLAGS: more flags for a measurement, `-fsanitize=stack`'s overhead; the gate fails on it."""
    flags = [f"-{opt}", *([] if kept else ["-fno-inline-functions"]), *os.environ.get("LLRM_BENCH_FLAGS", "").split()]
    if variant.language in (*BASIC_LANGUAGES, *OURS_BASIC):
        flags.append("-fno-inline-functions-called-once")
        # The program is this module: only the kernel is linked to from outside (as a C helper is `static`), so the rest is internal, near, by value.
        # Not on the flat target: there every call is near already, and queens (+6% instructions) and four more cells came out worse.
        if variant.language != FLAT:
            flags += ["--whole-program", "--export", variant.region]
    return flags


def nib_flags(variant: Variant) -> list[str]:
    """What nib-build.sh is given for a Nib benchmark's compile: only the kernel stays a call, helpers inline as they do in the C twin."""
    return [f"-fno-inline={variant.region}", *os.environ.get("LLRM_BENCH_FLAGS", "").split()]


def compiles(variant: Variant, opt: str, work: Path, stem: str) -> list[tuple[str, list[str], Path]]:
    """Every compile of llrm's own the harness makes of `variant` at -`opt`, as (tool, arguments, object): what `build` runs, from the same
    flags, for whatever must compile the bench's programs as the bench does (tools/identical.py). The Nib program's compile is what
    nib-build.sh gives llrm-nib; its runtime and foreign parts do not depend on the program."""
    obj = work / f"{stem}.obj"
    if variant.language == "nib":
        return [("llrm-nib", [str(variant.source), "-o", str(obj), f"-{opt}", "--procedure-segments", *nib_flags(variant)], obj)]
    if variant.language == FLAT:
        source = qbbuild.flat_source(variant.source)
        return [("llrm-qb", qbbuild.compile_arguments(source, (*compile_flags(variant, opt), "-g"), qbbuild.FLAT, obj), obj)]
    tool = "llrm-qb" if variant.language in (*BASIC_LANGUAGES, *OURS_BASIC) else "llrm-cc"
    arguments = ["--dialect", variant.dialect, "--runtime", variant.dialect] if variant.language in BASIC_LANGUAGES else []
    if variant.language in OURS_BASIC:
        arguments = ["--dialect", "qb45", "-fqb-runtime=llrm"]
    extra = [one for one in run_tests.compiler_arguments(variant.source) if one not in ("--dialect", "--runtime", variant.dialect)]
    kept = kept_a_call(variant, work, stem, shim=False) if variant.language == "c" else None
    if variant.language == "c":
        extra += ["-I", str(variant.source.parent), "-c"]
    flags = compile_flags(variant, opt, kept is not None)
    out = [(tool, [str(kept[1] if kept else variant.source), *arguments, *extra, *flags, "-o", str(obj)], obj)]
    if kept:
        counted = work / f"{stem}.counted.obj"
        out.append((tool, [str(kept[0]), *arguments, *extra, *flags, "-o", str(counted)], counted))
    return out


def build(variant: Variant, opt: str, work: Path, stem: str, objects: dict | None = None) -> tuple[Path, Path] | str:
    """(exe, linker map) of the variant at -`opt`, or why it did not build. BASIC is only compiled here: LINK runs in DOSBox."""
    obj, listing = work / f"{stem}.obj", work / f"{stem}.map"
    # The kernel stays a call: inlined into main it has no entry to count from.
    source, counted = variant.source, None
    kept = kept_a_call(variant, work, stem, shim=False) if variant.language == "c" else None
    flags = compile_flags(variant, opt, kept is not None)
    if kept is not None:
        counted, source = kept
    try:
        if variant.language == "nib":
            exe = work / f"{stem}.exe"
            extras = nib_extras(variant.source)
            done = cpulimit.run([str(ROOT / "tools" / "nib-build.sh"), str(variant.source), str(exe), f"-{opt}", *extras], 300, capture_output=True, text=True,
                                env={**os.environ, "LLRM_BIN": str(BIN), "TOOLCHAIN": str(BIN), "NIB_MAP": str(listing), "NIB_OBJ": str(obj), "NIB_FLAGS": " ".join(nib_flags(variant))})
            return (exe, listing) if done.returncode == 0 and exe.exists() else "build: " + (done.stderr or done.stdout).strip()[-300:]
        if variant.language == FLAT:
            # -g: the function's length, from the image's CodeView, is where the kernel's count ends (the map has none); the code is the same
            built = qbbuild.one_target(variant.source, work, objects, (*flags, "-g"), qbbuild.FLAT, stem, listing, before=("debug", "codeview"), image=work / f"{stem}.elf")
            LOADERS[built.exe] = built.files
            return built.exe, listing
        tool = "llrm-qb" if variant.language in (*BASIC_LANGUAGES, *OURS_BASIC) else "llrm-cc"
        arguments = ["--dialect", variant.dialect, "--runtime", variant.dialect] if variant.language in BASIC_LANGUAGES else []
        if variant.language in OURS_BASIC:
            arguments = ["--dialect", "qb45", "-fqb-runtime=llrm"]
        extra = [one for one in run_tests.compiler_arguments(variant.source) if one not in ("--dialect", "--runtime", variant.dialect)]
        if variant.language == "c":
            extra += ["-I", str(variant.source.parent), "-c"]
        done = cpulimit.run(dosbatch.with_isel([str(BIN / tool), str(source), *arguments, *extra, *flags, "-o", str(obj)]), 300, env=c_environment(variant), capture_output=True, text=True)
        if done.returncode != 0 or not obj.exists():
            return "compile: " + (done.stderr or done.stdout).strip()[-300:]
        if variant.language == "c":
            exe, linked = work / f"{stem}.exe", obj
            if counted is not None:
                linked = work / f"{stem}.counted.obj"
                done = cpulimit.run(dosbatch.with_isel([str(BIN / tool), str(counted), *arguments, *extra, *flags, "-o", str(linked)]), 300, env=c_environment(variant), capture_output=True, text=True)
                if done.returncode != 0 or not linked.exists():
                    return "compile: " + (done.stderr or done.stdout).strip()[-300:]
            dosbatch.link_c(linked, exe, work, listing)
            return exe, listing
        if variant.language in OURS_BASIC:
            exe = work / f"{stem}.exe"
            qbbuild.link16(obj, objects[qbbuild.REAL], exe, qbruntime.linked_objects(variant.source, work, "llrm", (), stem), listing)
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


STARTUP_SYMBOLS = helpers.STARTUP_SYMBOLS


def code_fields(obj: Path, listing: Path | None = None, source: Path | None = None) -> dict:
    """The object's code bytes without alignment padding, the padding apart (`align_bytes`, reported not gated), and why the padding could
    not be told where the code is counted whole (`align_unread` bytes, printed by `align_lines`). `lib_bytes`, reported not gated, is the
    standard library's functions in the object, which `code_bytes` leaves out; absent where the object has none (C's I/O is extern).

    With the linker map `listing` and the `source` (a reference compiler's program), the compiler-support routines the object links are in the
    program and counted: `code_bytes` is the object's plus theirs, `helper_bytes` (0 when none) the part that is theirs. A reference that calls
    a long divide where ours inlines it carries those bytes once: tc's recpow -O1 read 211 B by its object, 399 as linked. A helper the map
    does not define refuses the cell."""
    size = sizes.code_size(obj)
    called = [name for name in sizes.externs(obj) if name not in STARTUP_SYMBOLS and "report" not in name] if not sizes.is_elf(obj) else []
    fields = {"code_bytes": size.code, "align_bytes": size.align, **({"align_unread": size.unread} if size.unread else {}), **({"lib_bytes": size.library} if size.library else {}), **({"externs": called} if called else {})}
    if listing is not None:
        try:
            found = helpers.linked_helpers(obj, listing, helpers.written_identifiers(source))
        except helpers.HelperUnresolved as missing:
            return {"error": f"helper {missing} is called and not in the map: its bytes would be left out"}
        extra = sum(one for _, one in found)
        fields |= {"code_bytes": size.code + extra, "helper_bytes": extra, "helper_modules": [name for name, _ in found]}
    return fields


def skip_lines(measured: dict) -> list[str]:
    """One `SKIP reason (N programs)` line per reason a row was skipped, so a program with no column is counted in the header, not missing from it."""
    return [f"SKIP {why} ({len({key[0] for key, value in measured.items() if value.get('skip') == why})} programs)" for why in sorted({value["skip"] for value in measured.values() if "skip" in value})]


def io_variants() -> tuple[Variant, Variant]:
    """The std.io skeleton and the empty Nib program (startup/) it is measured against."""
    empty = next(one for one in variants(STARTUP, True) if one.language == "nib")
    return Variant("io_skeleton", "nib", IO_SKELETON / "io_skeleton.nib", "bench_io_skeleton", None), empty


def io_cost(opt: str, folder: Path) -> int | str:
    """Code bytes of std.io's file calls at -`opt`, or why they could not be measured: the skeleton's object less the empty program's, both
    built as the benches' Nib cells are. Informational (note_io_skeleton): a Nib bench that reads or writes a DOS file holds that code in its object."""
    sizes_of = []
    for at, variant in enumerate(io_variants()):
        built = build(variant, opt, folder, f"IOS{at}")
        if isinstance(built, str):
            return f"{variant.name}: {built}"
        sizes_of.append(sizes.code_size(folder / f"IOS{at}.obj").code)
    return sizes_of[0] - sizes_of[1]


def io_programs(selected: list[str]) -> set[str]:
    """The benchmarks whose bench.toml says `io_skeleton = true`: their Nib cells do file I/O through std.io."""
    return {directory_name(one) for one in benchmarks(selected) if tomllib.loads((one / "bench.toml").read_text()).get("io_skeleton")}


def note_io_skeleton(rows: dict, programs: set[str], cost: int) -> None:
    """On each Nib cell of `programs`, `io_skeleton_bytes`: what std.io's file calls cost in our object, in place. Informational: `code_bytes` stays the
    object's gross, and the twin's object holds its own I/O functions (bench_io.h, BENCH_IO_FUNCTIONS), so both sides pay for the same calls and a
    skeleton that grows shows on its own line instead of making every cell look better."""
    for key, row in rows.items():
        if key[1] == "nib" and key[0] in programs and "code_bytes" in row:
            row["io_skeleton_bytes"] = cost


def io_lines(measured: dict) -> list[str]:
    """One `IO-SKELETON` line per level: the size of std.io's file calls in our Nib objects, and how many cells hold them."""
    found: dict[str, tuple[int, int]] = {}
    for (_, language, opt), row in measured.items():
        if language == "nib" and "io_skeleton_bytes" in row:
            found[opt] = (row["io_skeleton_bytes"], found.get(opt, (0, 0))[1] + 1)
    return [f"IO-SKELETON {opt}: {size} B of std.io file calls in each of {cells} Nib cells (code_bytes is gross; the twin's object holds its own I/O)" for opt, (size, cells) in sorted(found.items())]


def note_lines(measured: dict) -> list[str]:
    """One `NOTE reason (N cells)` line per distinct note on a kept row, so an annotated row is counted in the header, not only carried in the json."""
    notes = [value["note"] for value in measured.values() if "note" in value]
    return [f"NOTE {why} ({notes.count(why)} cells)" for why in sorted(set(notes))]


def reference_job(key: tuple[str, str, str], stem: str, built: tuple[Path, Path], region: str | None, want: list[str], obj: Path, files: tuple, source: Path) -> tuple:
    """The job `measure` takes for one reference cell: the helpers are counted only where the reference's map names them."""
    return (stem, built[0], built[1], region, want, obj, files, links_helpers(key[1]), source)


def links_helpers(reference: str) -> bool:
    """Whether a reference's linker map names the library modules its program pulls in: a C reference's does (`Module: LIB(NAME)`). BC's is Microsoft's, its
    runtime is linked whole, and counting helpers refused every BASIC reference cell (`helper $AC is called and not in the map`, 31 programs x 3 compilers)."""
    return reference in REFERENCES["c"]


def measure(job: tuple[str, Path, Path, str, list[str], Path]) -> dict:
    """Counts of one built variant: the entry address from its map, the emulated run, the output check, and the bytes
    of its own object `obj` (tools/sizes.py): not the image, which holds the start-up and the libraries as well."""
    stem, exe, listing, region, want, obj, *more = job
    counted = (listing, more[2]) if len(more) > 2 and more[1] else None  # a reference's map and source: the helpers it links are counted
    if region is None:  # timed only: its output is checked where it is timed
        data, bss = sizes.data_bytes(obj, exe, listing)
        return {**code_fields(obj, *(counted or ())), "data_bytes": data, "bss_bytes": bss}
    entry = maps.locate(exe, listing, region)
    if entry is None:
        return {"error": f"unmeasured: kernel folded away: {region} is not in the map, and main does not call one function of its own"}
    cwd = exe.parent
    if more and more[0]:  # the files it reads, in a folder of its own: the programs of one folder write the same output names
        cwd = exe.with_suffix(".run")
        shutil.rmtree(cwd, ignore_errors=True)
        cwd.mkdir()
        for file in more[0]:
            shutil.copy(file, cwd / file.name)
    result = icount.run(exe, entry, cwd)
    got = run_tests.lines(result.output.decode("latin1"))
    if result.error and not (result.error == icount.NEVER_ENTERED and got == want):
        return {"error": result.error}
    if got != want:
        return {"error": f"prints {got}, want {want}"}
    data, bss = sizes.data_bytes(obj, exe, listing)
    if result.region is None:  # main prints the kernel's value without calling it (the call folded to a constant): a folded row, not a failed one
        return {"instructions": 0, **code_fields(obj, *(counted or ())), "data_bytes": data, "bss_bytes": bss}
    if result.region.instructions < FOLDED_BELOW:
        return {"error": f"unmeasured: kernel folded away: {region} ran {result.region.instructions} instructions (a kernel that did its work runs {FOLDED_BELOW} or more)"}
    return {"instructions": result.region.instructions, "memory_operands": result.region.memory_operands, "clocks": result.region.clocks, "layout_clocks": result.region.layout_clocks, "pad_nops": result.region.pad_nops, **code_fields(obj, *(counted or ())), "data_bytes": data, "bss_bytes": bss}


def references_available() -> bool:
    return (OW / "bwcc").exists() and (OWLIB / "dos" / "clibm.lib").exists() and (dosbatch.QB45 / "BC.EXE").exists() and (BCC / "lib" / "C0M.OBJ").exists()


def build_watcom(variant: Variant, opt: str, work: Path, stem: str) -> tuple[Path, Path] | str:
    """Open Watcom, medium model, cdecl as llrm-cc calls, no stack checks, inline 8087: tools/loops' reference build.
    Linked with Watcom's own start-up, CLIBM and MATH87M (NOEMU87: no emulator, as BCC's FP87). Only `report` comes
    from the corpus's ext.asm."""
    obj, exe, listing = work / f"{stem}.obj", work / f"{stem}.exe", work / f"{stem}.map"
    flags = ["-ox", "-oe=0"] if opt == "O2" else ["-os", "-ol"]  # -oe=0: the kernel stays a call, as llrm-cc compiles it
    done = cpulimit.run([str(OW / "bwcc"), "-zq", "-mm", "-ecc", "-s", "-DOWREF", "-4", "-fpi87", f"-i={OWLIB.parent / 'h'}", *physical_defines(), *flags, str(variant.source), f"-fo={obj}"], 300, capture_output=True, text=True, timeout=native.COMPILE_SECONDS)
    if done.returncode != 0 or not obj.exists():
        return "compile: " + (done.stderr or done.stdout).strip()[-300:]
    support = dosbatch.c_support(dosbatch.REAL_MODE, work)
    extra: list[str] = []  # error086.obj, once the library turns out not to define `__doserror_`
    stack = 60 * 1024  # the most DGROUP holds: a deep recursion (recsum, 2000 frames) overran the 16K one into the code; a program with more data gets what is left
    while True:
        try:
            # Watcom names main `main_`; its start-up asks for `_cstart_` and the library calls it
            dosbatch._host([str(BIN / "jwlink"), "option", "quiet", "option", f"map={listing}", "option", "start=_cstart_", "option", f"stack={stack}", *dosbatch.linkrecipe.link(dosbatch.REAL_MODE, "format"), "name", str(exe),
                            "libpath", str(OWLIB / "dos"), "libpath", str(OWLIB), "file", str(obj), *extra, *[word for one in support for word in ("file", str(one))],
                            "library", "clibm.lib", "library", "math87m.lib", "library", "noemu87.lib"])
            break
        except dosbatch.BuildError as error:
            if "__doserror_ is an undefined reference" in str(error) and not extra and DOSERROR_ASM.exists():
                far = far_doserror(work)
                if isinstance(far, str):
                    return far
                extra = ["file", str(far)]
                continue
            over = re.search(r"DGROUP exceeds 64k by (\d+) bytes", str(error))
            if not over or stack <= 4096:
                return f"build: {error}"
            stack -= -(-int(over.group(1)) // 16) * 16
    return exe, listing


def far_doserror(work: Path) -> Path | str:
    """error086.obj for the medium model: the object in the tree returns near (`defn`, `xrefn`: its `ret` and its call of
    `__set_errno_dos`), and clibm.lib's callers `call far` it. The first DOS error (a missing file, an open that fails)
    returned into the stack: stft_single's and jpeg_*'s Watcom rows died with UC_ERR_INSN_INVALID. Assembled again with the
    far forms (`defp`, `xrefp`), as the library's own big-code routines are."""
    tree = DOSERROR_ASM.parents[3]  # bld/
    source = work / "error086.asm"
    source.write_text(DOSERROR_ASM.read_text().replace("defn ", "defp ").replace("xrefn", "xrefp"))
    obj = work / "error086.obj"
    includes = [f"-I{tree / name}" for name in ("clib/_dos/h", "clib/h", "watcom/h", "lib_misc/h", "comp_cfg/h")]
    done = cpulimit.run([str(OW / "bwasm"), "-q", "-DNDEBUG", str(source), *includes, "-we", f"-fo={obj}", "-mm", "-nt=_TEXT", "-w8", "-cx", "-ee", "-fpc", "-0", "-bt=dos"], 300, capture_output=True, text=True, timeout=native.COMPILE_SECONDS)
    if done.returncode != 0 or not obj.exists():
        return "error086: " + (done.stderr or done.stdout).strip()[-300:]
    return obj


def c_source(path: Path) -> bytes:
    """A C program as the references compile it: without the `// key: value` header lines (run_tests.header), which
    Turbo C 2.01 does not read as comments ("Declaration syntax error" on line 1 of grep.c)."""
    lines = path.read_bytes().splitlines(keepends=True)
    head = 0
    while head < len(lines) and run_tests.HEADER.match(lines[head].decode("latin1")):
        head += 1
    return b"".join(lines[head:])


def stage_shared_headers(folder: Path, text: bytes) -> bytes:
    """`text`, a program's source, as it builds in `folder` (a DOS volume's root, which has no `..`): the benches' shared header
    beside it, and `#include "../bench_io.h"` naming it there."""
    shutil.copy(BENCH / "bench_io.h", folder / "bench_io.h")
    return re.sub(rb'"(?:\.\./)+bench_io\.h"', b'"bench_io.h"', text)  # a twin sits one folder deeper: "../../bench_io.h"


BORLAND_DEFINES = {"tcpp": ["-D__huge=huge"]}  # Turbo C++ 3.0 knows `huge`, not `__huge` (BCC 3.1 and Watcom know both); TC 2.01 has no huge arrays at all: its row stays an error


def build_borland(language: str, variants_: list[tuple[Variant, str]], opt: str, work: Path) -> dict[str, tuple[Path, Path] | str]:
    """A Borland-family reference (BORLAND), every program in one DOSBox boot. Keyed by stem.
    Linked with the toolchain's own start-up (C0M) and libraries: the floating-point programs need its FP87 and MATHM,
    which ask C0M for `__version` and `_errno`. Only `report` comes from the corpus's ext.asm.
    Turbo C++ 3.0's CM.LIB (SETUPIO) calls `_isatty`, which jwlink does not find in CM.LIB's dictionary: tcpp links that module as a file.
    (CS.LIB defines it too, but small-model: it linked, and 94 of 116 programs then never entered their kernel.)"""
    home, compiler, levels = BORLAND[language]
    folder = work / f"{language}{opt}"
    folder.mkdir(parents=True)
    for variant, stem in variants_:
        text = stage_shared_headers(folder, c_source(variant.source))
        if home.name == "tc201":  # TC 2.01 reads a `#` after a bare LF as an illegal character: every program with a #define failed
            text = text.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")
        (folder / f"{stem}.c").write_bytes(text)
        if home.name == "tc201":  # the shared header too: its first #ifndef read as unterminated, "Unexpected end of file in conditional"
            header = folder / "bench_io.h"
            if header.exists():
                header.write_bytes(header.read_bytes().replace(b"\r\n", b"\n").replace(b"\n", b"\r\n"))
    ran = native.run_limited([str(ROOT / "tools" / "callconv" / "bcc.sh"), str(folder), *[f"{stem}.c {levels[opt]} {" ".join([*physical_defines(), *BORLAND_DEFINES.get(language, [])])}".strip() for _, stem in variants_]],
                   env={**os.environ, "CCROOT": str(home.parent), "CCDIR": home.name, "CCEXE": compiler}, seconds=int(os.environ.get("LLRM_BORLAND_SECONDS", "1800")))
    support = [word for one in dosbatch.c_support(dosbatch.REAL_MODE, folder) for word in ("file", str(one))]
    out: dict[str, tuple[Path, Path] | str] = {}
    for _, stem in variants_:
        obj, exe, listing = folder / f"{stem}.OBJ", folder / f"{stem}.exe", folder / f"{stem}.map"
        if not obj.exists():
            message = folder / f"{stem}.MSG"
            out[stem] = f"{compiler}: " + (message.read_text(errors="replace").strip()[-200:] if message.exists() else ran if isinstance(ran, str) else "no object")  # a timed-out job said "no object" for all 31 programs of bcc -Os
            continue
        lib = home / "lib"
        try:
            extra = ["file", str(helpers.omf_library_module(lib / "CM.LIB", "ISATTY", folder / "ISATTY.OBJ"))] if language == "tcpp" else []
            dosbatch._host([str(BIN / "jwlink"), "option", "quiet", "option", f"map={listing}", *dosbatch.linkrecipe.link(dosbatch.REAL_MODE, "format"), "name", str(exe), "file", str(lib / "C0M.OBJ"), "file", str(obj), *extra, *support,
                            "library", str(lib / "CM.LIB"), "library", str((BCC / "lib" if language == "tcpp" else lib) / "FP87.LIB"), "library", str(lib / "MATHM.LIB")])
        except dosbatch.BuildError as error:
            out[stem] = f"link: {error}"
            continue
        out[stem] = (exe, listing)
    return out


def with_startup(selected: list[str], timing: bool) -> list[tuple[Path, Variant]]:
    """Every (directory, variant) to build, plus under --time the empty-kernel programs that price each start-up."""
    todo = [(directory, variant) for directory in benchmarks(selected) for variant in variants(directory)]
    return todo + [(STARTUP, variant) for variant in variants(STARTUP) if timing]


def reference_programs(todo: list[tuple[Path, Variant]]) -> list[tuple[Path, Variant]]:
    """`todo` less the programs whose bench.toml says `references = false` (C99 or llrm-only sources the reference compilers cannot build),
    each named on a line: a program left out is announced, not silently absent."""
    left_out = sorted({directory for directory, _ in todo if tomllib.loads((directory / "bench.toml").read_text()).get("references") is False})
    for directory in left_out:
        print(f"not a reference program: {directory_name(directory)} (bench.toml: references = false)", flush=True)
    return [(directory, variant) for directory, variant in todo if directory not in left_out]


def measure_references(selected: list[str], opts: list[str], work: Path, timing: bool = False) -> dict[tuple[str, str, str], dict]:
    """The reference compilers on the same programs: Open Watcom and BCC 3.1 for C (languages `ow`, `bcc`, at each level)
    and QuickBASIC 4.5's BC for BASIC (language `bc`, one level: /O). With --time each also gets its kernel time."""
    out: dict[tuple[str, str, str], dict] = {}
    opts = [opt for opt in opts if opt in REFERENCE_OPTS]  # a reference has no O3: `--opt O1 O3` raised KeyError after 35 minutes and dropped the run's JSON
    folder = work / "reference"
    shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True)
    todo = reference_programs(with_startup(selected, timing))
    stems = [f"R{at:03d}" for at in range(len(todo))]
    jobs, owned, plain, needs = [], [], {}, {}
    def own(key, stem, directory, variant, built, obj=None):
        if isinstance(built, str):
            out[key] = {"error": built}
        elif directory == STARTUP:
            plain[key] = built[0]
        else:
            files, want, counted = counting_input(variant, directory, folder / "ci-input" / stem)
            owned.append((key, reference_job(key, stem, built, variant.region if counted else None, want, obj, files, variant.source)))
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
    with ProcessPoolExecutor(max_workers=workers()) as pool:
        for (key, _), counts in zip(owned, pool.map(measure, [job for _, job in owned])):
            out[key] = counts
    if timing:
        exes = {key: job[1] for key, job in owned if "error" not in out[key]} | plain
        for key, timed in time_programs(exes, folder / "time", {key: needs[key] for key in exes if needs.get(key)}).items():
            out[key] = out.get(key, {}) | ({"time_error": timed["error"]} if "error" in timed else timed)
    return net_times(out)


REFERENCE_STAMP = BENCH / "references.stamp.json"


def reference_stamp() -> dict:
    """What the stored reference numbers were measured with: the compilers' md5, their switches per level, the host's load. Written by
    --bless --references beside expected.toml, so a clock or count that moved can be told from a compiler or host that changed."""
    import hashlib

    def md5(path: Path) -> str | None:
        return hashlib.md5(path.read_bytes()).hexdigest() if path.is_file() else None

    compilers = {"ow": md5(OW / "bwcc")} | {name: md5(home / f"{tool}.exe") for name, (home, tool, _) in BORLAND.items()}
    return {
        "date": str(date.today()),
        "load_average": list(os.getloadavg()),
        "compilers_md5": compilers,
        "switches": {name: levels for name, (_, _, levels) in BORLAND.items()} | {"ow": {"O2": "-ox -oe=0", "O1": "-os -ol", "Os": "-os -ol"}, "basic": "/O /FPi"},
    }


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


def flat_clocks(key: tuple, image: Path, region: str, instructions: int, cwd: Path) -> dict:
    """The kernel's 486 clocks, from the emulator that prices the flat image (tools/bench/flatcount.py). The emulator counts every
    instruction itself; DOSBox's count is the check that it ran the same kernel (within AGREE, DOSBox's repeated string instructions counted as it does: flatcount.DOSBOX_REP_SLICE), and a price of another run is no price. Reported, not gated."""
    limit = int(os.environ.get("LLRM_FLAT_LIMIT") or flatcount.LIMIT)  # raised for the cells the default cap cannot reach (grep: 642M instructions), the same for main and tip
    if instructions > limit // 2:
        print(f"WARNING {key}: no clocks: {instructions} instructions are too many to emulate", file=sys.stderr)
        return {}
    ran = flatcount.run(image, region.upper(), cwd, limit)
    if ran.error or ran.region is None:
        print(f"WARNING {key}: no clocks: {ran.error}", file=sys.stderr)
        return {}
    if not flatcount.agrees(ran.region.instructions, instructions, ran.rep_iterations):
        print(f"WARNING {key}: no clocks: the emulator counted {ran.region.instructions} instructions, DOSBox {instructions}", file=sys.stderr)
        return {}
    return {"clocks": ran.region.clocks, **({"emu_limit": limit} if limit != flatcount.LIMIT else {})}


def measure_flat(flat: dict, work: Path) -> dict[tuple[str, str, str], dict]:
    """The flat target's programs, each run on its own in DOSBox on virtual time with the emulator counting by function from the
    program's link map: `instructions` are the kernel's own, its entry to its return and what it called, as the real-mode
    emulator counts them. Not the whole run's: that holds the extender's load (about 40 instructions per relocation, so the
    number of address references in the program) and the runtime's start-up, which are not what the compiler made. With the
    program's object's sizes, and its output checked."""
    work.mkdir(parents=True, exist_ok=True)

    def one(item):
        key, (stem, exe, obj, want, needed, listing, region, *more) = item
        image = more[0] if more else None  # the ELF the emulator prices; a row without one has no clocks
        if needed and "skip" in needed:
            return key, {"error": "skipped: " + needed["skip"]}
        ran = qbbuild.run(
            qbbuild.Built(exe, LOADERS.get(exe, ())), work / f"{stem}-{key[2]}", needed["files"] if needed else (), qb32.BENCH_CONF, qb32.BUDGET_MS, profile=listing
        )
        if ran.status != "ok":
            return key, {"error": f"run: {ran.status}"}
        got = run_tests.lines(ran.output.decode("latin1"))
        # BASIC's PRINT of a number leads with a blank
        if [line.strip() for line in got] != [line.strip() for line in want]:
            return key, {"error": f"prints {got}, want {want}"}
        if region.upper() not in ran.inclusive:
            return key, {"error": f"the kernel {region} is not in the link map's functions"}
        if not ran.sized.get(region.upper()):
            print(f"WARNING {key}: {region} has no length in the image's debug info, so code after it that no symbol names is counted in it", file=sys.stderr)
        data, bss = sizes.data_bytes(obj)
        return key, {"instructions": ran.inclusive[region.upper()], **code_fields(obj), "data_bytes": data, "bss_bytes": bss} | (flat_clocks(key, image, region, ran.inclusive[region.upper()], work / f"{stem}-{key[2]}") if image else {})

    with ThreadPoolExecutor(max_workers=workers()) as pool:
        found = dict(pool.map(one, flat.items()))
    return {key: counts for key, counts in found.items() if key[0] != "startup"}


START = time.monotonic()


def workers() -> int:
    """Pool size: the guard's base slot count, so a bench run takes no more cores than one admitted job (a storm of 32 workers starved the others)."""
    return cpulimit.pool_workers()


def guarded_argv() -> list[str] | None:
    """The command that runs this one under tools/load_guard.sh, or None when it already is."""
    if os.environ.get("LG_GUARDED") == "1":
        return None
    return [str(Path(__file__).resolve().parents[1] / "load_guard.sh"), sys.executable, *sys.argv]


def cores() -> str:
    """What the pools can use, for the first stage line: a hosted runner's four cores and this host's thirty-two read alike in a duration."""
    return f"{len(os.sched_getaffinity(0))} usable of {os.cpu_count()} cores"


def stage(text: str) -> None:
    """A progress line on stderr, which the gate's bench step lets through (its stdout is judged): a shard that hit the job timeout printed nothing for 45 minutes."""
    print(f"bench: [{time.monotonic() - START:6.0f}s] {text}", file=sys.stderr, flush=True)


def native_sources(name: str, directory: Path, work: Path) -> tuple[Path, Path] | None:
    """The sources the x86_64 row runs and sizes: the harness's own (kept_a_call) for the program's C variant, in a folder of its own."""
    variant = next((one for one in variants(directory, every_language=True) if one.language == "c"), None)  # `--lang c64` leaves ONLY without "c"
    folder = work / "kept"
    folder.mkdir(parents=True, exist_ok=True)
    return kept_a_call(variant, folder, name.replace("/", "_")) if variant else None


def measure_all(selected: list[str], opts: list[str], work: Path, timing: bool = False) -> dict[tuple[str, str, str], dict]:
    """{(benchmark, language, opt): counts or {"error": why} or {"known": issue}}. The levels are independent (a folder each) and are
    measured at once, in a process each; a timed run keeps them one after the other."""
    out: dict[tuple[str, str, str], dict] = {}
    if timing or len(opts) < 2:
        for opt in opts:
            out |= measure_opt(selected, opt, work, timing)
    else:
        with ProcessPoolExecutor(max_workers=min(len(opts), workers())) as pool:
            for part in pool.map(measure_opt, [selected] * len(opts), opts, [work] * len(opts), [timing] * len(opts)):
                out |= part
    if ONLY is None or native.OURS in ONLY:
        out |= native.measure([(directory_name(one), one) for one in benchmarks(selected)], BIN, opts, work / "native", BENCH, native_sources)
    return mark_known(net_times(out) if timing else out, [v for directory in benchmarks(selected) for v in variants(directory)], opts)


def measure_opt(selected: list[str], opt: str, work: Path, timing: bool = False) -> dict[tuple[str, str, str], dict]:
    """The measurements at one optimisation level."""
    out: dict[tuple[str, str, str], dict] = {}
    folder = work / opt
    shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True)
    everything = with_startup(selected, timing)
    for _, variant in everything:
        if reason := tools_missing(variant):
            out[(variant.name, variant.language, opt)] = {"skip": reason}
    todo = [(directory, variant) for directory, variant in everything if not tools_missing(variant)]
    stems = {(v.name, v.language): f"V{at:03d}" for at, (_, v) in enumerate(todo)}
    objects = qbbuild.runtimes(work / f"runtime-{opt}") if any(v.language in OURS_BASIC for _, v in todo) else None
    with ThreadPoolExecutor(max_workers=workers()) as pool:
        built = list(pool.map(lambda one: build(one[1], opt, folder, stems[(one[1].name, one[1].language)], objects), todo))
    stage(f"{opt}: built {len(todo)} programs")
    basic = {stems[(v.name, v.language)]: (b[0], v.dialect) for (_, v), b in zip(todo, built) if v.language in BASIC_LANGUAGES and not isinstance(b, str)}
    failed = link_basic(basic, folder)
    stage(f"{opt}: linked {len(basic)} BASIC programs")
    jobs, plain, needs, flat = [], {}, {}, {}
    for (directory, variant), result in zip(todo, built):
        key = (variant.name, variant.language, opt)
        stem = stems[(variant.name, variant.language)]
        why = result if isinstance(result, str) else failed.get(stem)
        if why:
            out[key] = {"error": why}
            continue
        exe, listing = (folder / f"{stem}.exe", folder / f"{stem}.map") if variant.language in BASIC_LANGUAGES else result
        if variant.language == FLAT:
            flat[key] = (stem, exe, folder / f"{stem}.{qbbuild.FLAT}.obj", expected_output(directory), timing_input(variant, directory), listing, variant.region, folder / f"{stem}.elf")
            if directory == STARTUP:
                plain[key] = exe
            else:
                needs[key] = flat[key][4]
                jobs.append((key, None))
        elif directory == STARTUP:
            plain[key] = exe  # timed, not counted
        else:
            # an emulated program is counted on the cut of its corpus the default bench times (ci_bytes), even under --time
            files, want, counted = counting_input(variant, directory, folder / "ci-input" / stem)
            jobs.append((key, (stem, exe, listing, variant.region if counted else None, want, folder / f"{stem}.obj", files)))
            needs[key] = timing_input(variant, directory, None if timing else folder / "ci-input" / stem)
    with ProcessPoolExecutor(max_workers=workers()) as pool:
        counted = [(key, job) for key, job in jobs if job is not None]
        for (key, _), counts in zip(counted, pool.map(measure, [job for _, job in counted])):
            out[key] = counts
    stage(f"{opt}: counted {len(jobs)} programs")
    out |= measure_flat(flat, folder / "flat")
    stage(f"{opt}: flat target done")
    jobs = [(key, job if job is not None else (flat[key][0], flat[key][1], None, None, None, None)) for key, job in jobs]
    exes = {key: job[1] for key, job in jobs if "error" not in out[key]} | plain
    run_timed(out, exes, {key: needs[key] for key in exes if needs.get(key)}, folder / "time", timing, already=set(flat))
    stage(f"{opt}: DOSBox runs done")
    if programs := io_programs(selected):
        if any(key[1] == "nib" and key[0] in programs for key in out):
            cost = io_cost(opt, folder)
            if isinstance(cost, str):
                print(f"WARNING {opt}: no std.io skeleton: {cost}", file=sys.stderr)
            else:
                note_io_skeleton(out, programs, cost)
                stage(f"{opt}: std.io skeleton {cost} B")
    return out


def run_timed(out: dict, exes: dict, needs: dict, work: Path, timing: bool, already: frozenset | set = frozenset()) -> None:
    """The timed-only programs run in DOSBox and their output is compared with the expected one, always: a program with no instruction count
    is otherwise sized and never run, and a miscompile there reaches no gate. `timing` also keeps the times (--time); without it a row gets
    only `checked: true`, or an error row when it prints something else, or `unchecked: why` when its corpus is not here. `already`: the flat
    target's rows, which measure_flat has run on the whole corpus and compared: run again here beside the others, whose files are the reduced
    corpus under the same DOS name, they read whichever copy the shared volume held (grep bq32 printed the 1 MiB answer, 115, against 4745)."""
    if not timing:
        exes = {key: path for key, path in exes.items() if key in needs and key not in already}
    for key, timed in time_programs(exes, work, needs).items():
        if timing:
            out[key] = out.get(key, {}) | ({"time_error": timed["error"]} if "error" in timed else timed)
        elif "error" in timed and timed["error"].startswith("skipped: "):
            out[key] = out.get(key, {}) | {"unchecked": timed["error"][len("skipped: "):]}
        elif "error" in timed:
            out[key] = {"error": timed["error"]}
        else:
            out[key] = out.get(key, {}) | {"checked": True}


def mark_known(measured: dict, all_variants: list[Variant], opts: list[str]) -> dict:
    """A variant with a `known:` mark fails at some level; there its result is {"known": issue, ...}. If it works at
    every level, the mark is stale: {"xpass": issue, ...} fails the gate. At a level where it works while another
    still fails, it is measured like any other."""
    for variant in all_variants:
        if not variant.known:
            continue
        keys = [(variant.name, variant.language, opt) for opt in opts if "skip" not in measured.get((variant.name, variant.language, opt), {})]
        broken = [key for key in keys if "error" in measured[key]]
        for key in keys:
            if key in broken:
                measured[key] = {"known": variant.known, **measured[key]}
            elif not broken:
                measured[key] = {"xpass": variant.known, **measured[key]}
    return measured


def input_files(variant: Variant) -> tuple[Path, ...]:
    """The files `variant` declares it reads (`// data:`; `@name` a cached corpus), or () where the corpus is not here."""
    try:
        return tuple(corpus.path(one[1:]) if one.startswith("@") else variant.source.parent / one for one in variant.data)
    except corpus.Unavailable:
        return ()


def timing_input(variant: Variant, directory: Path, reduced: Path | None = None) -> dict | None:
    """{files a timed-only program reads, want: the output it must print}, or {skip: why} when its corpus is unavailable, or None
    for the programs the emulator counts. With `reduced` (a work folder) and a program that names `ci_bytes`, the first ci_bytes of each
    corpus are copied there under the corpus's own name and the output wanted is NAME.ci.out's (tools/bench/mkci.py): the default bench
    runs the program on every gate, --time runs it on all of its corpus."""
    if not variant.timed_only:
        return None
    try:
        files = tuple(corpus.path(one[1:]) if one.startswith("@") else variant.source.parent / one for one in variant.data)
    except corpus.Unavailable as error:
        return {"skip": str(error)}
    if reduced is not None and variant.ci_bytes:
        reduced.mkdir(parents=True, exist_ok=True)
        small = []
        for file in files:
            (reduced / file.name).write_bytes(file.read_bytes()[: variant.ci_bytes])
            small.append(reduced / file.name)
        return {"files": tuple(small), "want": run_tests.lines((directory / f"{directory.name}.ci.out").read_text())}
    return {"files": files, "want": expected_output(directory)}


def counting_input(variant: Variant, directory: Path, reduced: Path) -> tuple[tuple[Path, ...], list[str], bool]:
    """(files the emulated run reads, the output it must print, whether it is counted): an emulated program is counted on the cut of its corpus
    the default bench times (`ci_bytes`), even under --time; any other, and one whose corpus is unavailable, is timed only if it is timed_only."""
    fed = timing_input(variant, directory, reduced) if variant.emulated else None
    files, want = (fed["files"], fed["want"]) if fed and "files" in fed else ((), expected_output(directory))
    return files, want, not variant.timed_only or bool(files)


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
    jobs = [Job(f"W{at:03d}", "exe", exes[key], runner="TIMEIT.COM", files=(stub, *LOADERS.get(exes[key], ()), *inputs.get(key, {}).get("files", ())), budget_ms=600_000 if key in inputs else None) for at, key in enumerate(keys)]
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


def corpus_digest(directory: Path) -> str:
    """md5 of a program's sources (not its expected.toml or .out): what its stored reference numbers were measured from."""
    parts = sorted(f for f in directory.iterdir() if f.is_file() and f.name != "expected.toml" and f.suffix != ".out")
    return hashlib.md5(b"".join(f.name.encode() + f.read_bytes() for f in parts)).hexdigest()


def stale_corpus(directory: Path) -> str | None:
    """Why the stored reference numbers of `directory` no longer belong to its sources, or None (also for a file blessed before `corpus` was recorded)."""
    recorded = read_expected(directory).get("corpus")
    if recorded is None or recorded == corpus_digest(directory):
        return None
    return f"{directory_name(directory)}: sources changed since the references were blessed (corpus {recorded[:8]}, now {corpus_digest(directory)[:8]}): --bless --references --reason"


def read_expected(directory: Path) -> dict:
    path = directory / "expected.toml"
    return tomllib.loads(path.read_text()) if path.exists() else {}


def write_expected(directory: Path, measured: dict, name: str, reason: str) -> bool:
    """Merge what was measured into expected.toml: a table not measured this run (another level, a reference left alone)
    stays. The kernel time of a table whose instructions moved is dropped: it belongs to the old counts.

    The file is written, and its reason and date replaced, only where a number changed: whether any did is returned. A bless that
    rewrote the reason of every benchmark made a pull request of 43 files where 6 had moved."""
    before = {one: dict(table) for one, table in read_expected(directory).items() if isinstance(table, dict)}
    tables = {one: dict(table) for one, table in before.items()}
    for (benchmark, language, opt), counts in measured.items():
        if benchmark != name or "error" in counts or "skip" in counts:
            continue
        new = {one: counts[one] for one in (*COUNTERS, "clocks", "layout_clocks", "pad_nops", *SIZES, "helper_bytes", "kernel_ms") if one in counts}
        old = tables.get(language, {}).get(opt, {})
        if "kernel_ms" not in new and "kernel_ms" in old and old.get("instructions") == new.get("instructions"):
            new["kernel_ms"] = old["kernel_ms"]
        tables.setdefault(language, {})[opt] = new
    if {language: {opt: dict(table) for opt, table in by_opt.items()} for language, by_opt in tables.items()} == {language: {opt: dict(table) for opt, table in by_opt.items()} for language, by_opt in before.items()} and (directory / "expected.toml").exists() and not stale_corpus(directory):
        return False
    lines = [f"reason = {json.dumps(reason)}", f'blessed = "{date.today()}"', f'corpus = "{corpus_digest(directory)}"', ""]  # JSON escapes are TOML's
    for language in dict.fromkeys([*LANGUAGES, *BASIC_LANGUAGES, *(one for many in REFERENCES.values() for one in many)]):
        for opt in sorted(tables.get(language, {})):
            lines += [f"[{language}.{opt}]", *(f"{one} = {value}" for one, value in tables[language][opt].items()), ""]
    (directory / "expected.toml").write_text("\n".join(lines))
    return True


TUNE_486 = ROOT / "machine" / "tune" / "x86" / "i486.toml"  # the CPU the flat clocks are priced for (flatcount.py)
M16_TIMINGS = ROOT / "crates" / "target" / "llrm-x86-m16" / "src" / "timings.times"


def m16_rate() -> int:
    """The `speed_milliclocks_per_byte` row of the m16 sheet: one price for every CPU column (policy); 0 where the row is absent or the columns disagree."""
    for line in M16_TIMINGS.read_text().splitlines():
        cells = line.split()
        if cells[:1] == ["speed_milliclocks_per_byte"]:
            values = set(cells[1:])
            return int(values.pop()) if len(values) == 1 and next(iter(values), "").isdigit() else 0
    return 0


def rate_for(language: str, opt: str) -> int:
    """What a byte of code is worth in thousandths of a clock at speed on the target of `language`, from the target's own rows: m16 at -O2/-O3 its
    `speed_milliclocks_per_byte`; the flat x86-32 target at -O2/-O3 the 486's line fill (clocks to fill a line, per byte of line). 0 where the target has
    none and at -Os."""
    if language == FLAT and opt in ("O2", "O3"):
        tune = tomllib.loads(TUNE_486.read_text())
        return 1000 * tune["line_fill_clocks"] // tune["cache_line"] if "line_fill_clocks" in tune else 0
    return m16_rate() if language in ("bas", "bq16", "c", "nib") and opt in ("O2", "O3") else 0


def against(label: str, got: dict, want: dict | None, waived: list[dict] = (), rate: int = 0) -> list[str]:
    """What fails the gate for one measurement: a count above its baseline (worse) unless a waiver (tools/bench/waivers) names its row,
    counter and this count. A count below it passes: it is the next base. The baseline of a row of ours is the base's measurement
    (basecounts.py); of a reference, its stored one. A row with none (a new program) is not judged. Clocks (the 486 kernel clocks) are judged like the counts.
    Where the target prices a byte (`rate`, milliclocks per byte; -O2/-O3), the cell passes when 1000 * delta clocks + rate * delta code bytes <= 0 and
    clocks, code bytes, instructions and memory operands are not gated alone: they are in the row.
    Elsewhere (-Os, -O1 aside, and targets without a rate) code bytes and clocks are strict; instructions and memory operands are reported and gate
    only when bytes and clocks both equal the base's. A cell without clocks on either side is strict on every metric."""
    if want is None:
        return []
    if want.get("instructions") == 0 and "error" not in got:  # a base that executed nothing is no base: parity/control Os read 0 on main and 158 B was compared
        return [f"{label}: INVALID BASE: the base kernel executed 0 instructions (folded or never run), nothing to compare with"]
    if got.get("instructions") == 0 and want.get("instructions") and "error" not in got:  # the kernel never ran: not a win
        return [f"{label}: INVALID: the kernel executed 0 instructions (inlined or folded into the harness); base {want['instructions']}"]
    got, want = pad_corrected(with_footprint(got), with_footprint(want))
    problems = []
    priced = rate > 0 and all(one in got and one in want for one in ("clocks", "code_bytes"))
    if priced:
        clocks, size = got["clocks"] - want["clocks"], got["code_bytes"] - want["code_bytes"]
        if 1000 * clocks + rate * size > 0:
            problems.append(f"{label}: {clocks:+d} clocks {size:+d} code bytes at {rate} milliclocks/byte is worse")
    timed = not priced and all(one in side for one in ("clocks", "code_bytes") for side in (got, want))
    tied = timed and got["clocks"] == want["clocks"] and got["code_bytes"] == want["code_bytes"]
    for counter in (*COUNTERS, "clocks", *JUDGED_SIZES):
        if counter not in got:
            continue
        if counter not in want:
            if counter in COUNTERS:  # a base without it ran no strict check on it: the gate would pass on a number never compared
                problems.append(f"{label}: base has no {counter} (not judged; regenerate the base)")
            continue
        if priced and counter in ("instructions", "memory_operands", "clocks", "code_bytes"):
            continue
        if timed and not tied and counter in COUNTERS:
            continue
        if got[counter] > want[counter] and any(w["label"] == label and w["counter"] == counter and w["to"] == got[counter] for w in waived):
            continue
        if got[counter] > want[counter]:
            problems.append(f"{label}: {counter} {got[counter]} > {want[counter]} (worse)")
    return problems


def pad_corrected(got: dict, want: dict) -> tuple[dict, dict]:
    """m16 clocks as the judge sees them (M16-CLOCK-FARCALL-PAD): jwlink's far-call pad is a `nop` that runs at even offsets and a free prefix at odd
    ones, so a one-byte shift earlier in a segment moves one clock per executed call between sites (#8545: jpeg_fixed +172, jpeg_single -170 on
    the same code). Both sides with `layout_clocks`: those are compared (raw stays in the row). A base without the column: a rise of at most the
    new side's executed pad nops can be the pad alone (delta = layout delta + new pads - old pads <= new pads), but so can a real rise of that size:
    that excuse is off unless LLRM_BENCH_EXCUSE_PAD is set, and each excused cell prints a warning. Bless the bases to end it."""
    if "clocks" not in got or "clocks" not in want:
        return got, want
    if "layout_clocks" in got and "layout_clocks" in want:
        return {**got, "clocks": got["layout_clocks"]}, {**want, "clocks": want["layout_clocks"]}
    if os.environ.get("LLRM_BENCH_EXCUSE_PAD") and 0 < got["clocks"] - want["clocks"] <= got.get("pad_nops", 0):
        print(f"WARNING: clock rise {got['clocks'] - want['clocks']} excused: the base has no layout_clocks and the new side runs {got['pad_nops']} pad nops", file=sys.stderr)
        return {**got, "clocks": want["clocks"]}, want
    return got, want


def time_against(label: str, got: dict, want: dict | None) -> list[str]:
    """A timed-only program has no count to gate: its own kernel time does, with the time tolerance, against the base's (worse only)."""
    if want is None or "kernel_ms" not in got or "kernel_ms" not in want:
        return []
    if got["kernel_ms"] > want["kernel_ms"] + max(TIME_TOLERANCE * want["kernel_ms"], TIME_SLACK_MS):
        return [f"{label}: kernel_ms {got['kernel_ms']} > {want['kernel_ms']} (worse)"]
    return []


def ratio_problems(label: str, reference: str, got: dict, want: dict, ref: dict, waived: list[dict] = ()) -> list[str]:
    """llrm/reference worse than the base's. Against a stored reference this is the count gate seen from the reference's side; a counter
    a waiver excuses (its row, counter and this count) is excused here too: the ratio is the count over a constant, so a waived count
    always moved it and no waiver on a row with references could pass."""
    problems = []
    got, want, ref = with_footprint(got), with_footprint(want), with_footprint(ref)
    if "clocks" in got and "clocks" in want and "clocks" not in ref:
        problems.append(f"{label}: no stored clocks for {reference}: --bless --references --reason")
    for counter in (*COUNTERS, "clocks", *JUDGED_SIZES, "kernel_ms"):
        if any(w["label"] == label and w["counter"] == counter and w["to"] == got.get(counter) for w in waived):
            continue
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
    for counter in (*COUNTERS, "clocks", *SIZES, "kernel_ms"):
        if counter in got and counter in stored:
            slack = max(TIME_TOLERANCE * abs(stored[counter]), TIME_SLACK_MS) if counter == "kernel_ms" else 0
            if abs(got[counter] - stored[counter]) > slack:
                problems.append(f"{label}: {counter} {got[counter]} != stored {stored[counter]} (reference changed: --bless --references --reason)")
    return problems


def native_problems(label: str, got: dict, want: dict | None, waived: list[dict]) -> list[str]:
    """The x86_64 row: an error fails; a count above the base's fails (code bytes: executable sections of the object; instructions: user-mode, per run)."""
    if "skip" in got:
        return []
    if "error" in got:
        return [f"{label}: {got['error']}"]
    if want is not None and "instructions" in want:
        want = {**want, "instructions": want["instructions"] + 1}  # the count is a rounded mean over 100 runs of a native process: one instruction is its resolution
    return against(label, got, want, waived)


def judge(measured: dict, selected: list[str], opts: list[str], base: dict | None = None, same: set[str] | None = None) -> list[str]:
    """What fails the gate. `base`: the base's rows of ours (basecounts.base_counts), what a row of ours may not exceed; `same`: the programs
    the base and this tree have alike (a row of another program compares two programs, not two compilers). The references' rows come from
    each benchmark's expected.toml."""
    problems = []
    waived = basecounts.waivers(ROOT)
    for directory in benchmarks(selected):
        name = str(directory.relative_to(BENCH))
        baseline = read_expected(directory)
        for language in basecounts.OURS:
            baseline.pop(language, None)
        for (one, language, opt), row in (base or {}).items():
            if one == name and (same is None or name in same):
                baseline.setdefault(language, {})[opt] = row
        stale = stale_corpus(directory)
        if stale:
            problems.append(stale)
        for variant in variants(directory):
            if is_twin(variant):
                continue
            for opt in opts:
                got = measured[(name, variant.language, opt)]
                label = f"{name} {variant.language} -{opt}"
                if "known" in got or "skip" in got:
                    continue
                if "xpass" in got:
                    problems.append(f"{label}: works; remove its known mark ({got['xpass']})")
                elif "error" in got:
                    problems.append(f"{label}: {got['error']}")
                else:
                    if variant.language == "bq32" and got.get("instructions", 0) >= qb32.BENCH_RATE * 55:
                        problems.append(f"{label}: {got['instructions']} instructions run long enough for a timer tick (55 ms at the flat rate): the extender's handler lands inside the count")
                    want = baseline.get(variant.language, {}).get(opt)
                    problems += against(label, got, want, waived, rate_for(variant.language, opt))
                    if variant.timed_only:
                        problems += time_against(label, got, want)
                    for reference in REFERENCES.get(variant.language, ()):
                        ref = baseline.get(reference, {}).get(reference_level(reference, opt))
                        if want and ref:
                            problems += ratio_problems(label, reference, got, want, ref, waived)
            for reference in REFERENCES.get(variant.language, ()):
                for level in sorted({reference_level(reference, opt) for opt in opts}):
                    if (name, reference, level) in measured and not stale:
                        problems += drift_problems(f"{name} {reference} -{level}", measured[(name, reference, level)], baseline.get(reference, {}).get(level))
        for opt in opts:  # the x86_64 row: no variant of its own, it runs the C source
            got = measured.get((name, native.OURS, opt))
            if got is not None:
                problems += native_problems(f"{name} {native.OURS} -{opt}", got, (base or {}).get((name, native.OURS, opt)) if same is None or name in same else None, waived)
    return problems


def folded(got: dict) -> bool:
    """The compiler folded the kernel away: the program ran and printed the right text, and the kernel was never entered (0 instructions).
    Such a row is marked, not counted: a ratio to 0 is no ratio, and an unmeasured kernel is not a faster one."""
    return got.get("instructions") == 0 and "error" not in got


def clock_lines(measured: dict, selected: list[str]) -> list[str]:
    """The 486 clocks of each selected benchmark's kernel beside its counts. Reported, not gated: the counts ratchet, and
    a change that trades instructions for fewer clocks is judged by these."""
    lines = []
    for (name, language, opt), got in sorted(measured.items()):
        if name in selected and language in (*LANGUAGES, *BASIC_LANGUAGES) and folded(got):
            lines.append(f"{name} {language} -{opt}: folded (the kernel is never entered)")
        elif name in selected and language in (*LANGUAGES, *BASIC_LANGUAGES) and "clocks" in got:
            lines.append(f"{name} {language} -{opt}: {got['clocks']} clocks, {got['instructions']} instructions" + (f", {got['memory_operands']} memory operands" if "memory_operands" in got else ""))
    return lines


def fold_lines(measured: dict) -> list[str]:
    """Warnings, not failures: a Nib cell that executed under half of its C twin's instructions. Same algorithm, so the kernel's work folded away
    (lru read 39 against C's 133 once its helpers inlined: an internal kernel, constant argument, local cache)."""
    return [
        f"SUSPECT-FOLD {name} nib -{opt}: {got['instructions']} instructions against C's {twin['instructions']}"
        for (name, language, opt), got in sorted(measured.items())
        if language == "nib" and (twin := measured.get((name, "c", opt))) and got.get("instructions") is not None and twin.get("instructions") and got["instructions"] * 2 < twin["instructions"]
    ]


def align_lines(measured: dict) -> list[str]:
    """Warnings, not failures: how many cells count some code bytes whole because those did not decode to their end, naming the cells where most of the
    code is so (the padding there is not told), and padding in a size-optimised cell (no loop alignment is expected at -Os or -Oz)."""
    lines, unread, mostly = [], [], []
    for (name, language, opt), got in sorted(measured.items()):
        if got.get("align_unread"):
            unread.append(got["align_unread"])
            if got["align_unread"] * 2 >= got["code_bytes"] + got.get("align_bytes", 0):
                mostly.append(f"ALIGN-UNREAD {name} {language} -{opt}: {got['align_unread']} of {got['code_bytes']} bytes are counted with their padding")
        if got.get("align_bytes", 0) and opt in ("Os", "Oz"):
            lines.append(f"ALIGN {name} {language} -{opt}: {got['align_bytes']} bytes of alignment padding at a size level")
    summary = [f"ALIGN-UNREAD {len(unread)} cells count {sum(unread)} bytes whole (data inside code: not decoded for padding)"] if unread else []
    return summary + mostly + lines


def helper_lines(measured: dict) -> list[str]:
    """Notes, not failures: a cell whose object calls library functions (`externs`). Their bytes are in no `code_bytes`, so that column
    reads smaller than the program is where a reference calls a helper (a long divide) that another build inlines."""
    return [f"HELPERS {name} {language} -{opt}: code_bytes excludes {', '.join(got['externs'])}" for (name, language, opt), got in sorted(measured.items()) if got.get("externs")]


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
                skipped = []
                for directory in benchmarks(selected):
                    name = str(directory.relative_to(BENCH))
                    got = measured.get((name, language, opt), {})
                    ref = read_expected(directory).get(reference, {}).get(level)
                    if ref and folded(got):
                        skipped.append(name)
                    elif ref and "error" not in got and any(one in got for one in ("instructions", "kernel_ms", "code_bytes")):
                        rows.append((got, ref))
                parts = []
                for counter, title in (("instructions", "instructions"), ("memory_operands", "memory operands"), ("kernel_ms", "kernel time"),
                                       ("code_bytes", "code bytes"), ("data_bytes", "data bytes"), ("bss_bytes", "bss bytes")):
                    ratios = [got[counter] / ref[counter] for got, ref in rows if counter in got and counter in ref and got[counter] > 0 and ref[counter] > 0]
                    if ratios:
                        parts.append(f"{title} {100 * (geomean(ratios) - 1):+.1f}% (n={len(ratios)})")
                if skipped:
                    parts.append(f"folded, out of the geomeans: {', '.join(skipped)}")
                broken = [f"{name}: {measured[(name, reference, opt)]['error']}" for name in map(directory_name, benchmarks(selected)) if "error" in measured.get((name, reference, opt), {})]
                if broken:  # a reference that did not build is no column, not an empty one
                    lines.append(f"REFERENCE BROKEN {reference} -{level}: {len(broken)} programs unmeasured, e.g. {broken[0][:200]}")
                if parts:
                    lines.append(f"llrm {language} -{opt} vs {reference} {'/O' if reference in BASIC_REFERENCES else '-' + level}: " + ", ".join(parts))
    return lines


def base_rows(args, names: list[str] | None = None) -> tuple[dict, set[str], str]:
    """(the base's rows of ours, the programs the base and this tree have alike, the base's commit): the base is the merge-base with
    origin/main, built and measured by its own tree (basecounts.py), once per tree. `names` (a slice of the base leg): only those
    programs, and of them only the base's own: its bench.py rejects a program the head added."""
    import measure

    head = measure.git("rev-parse", "HEAD")
    base_sha = measure.base_of(head, os.environ.get("LLRM_BENCH_BASE_REF", "origin/main"))
    built = functools.cache(lambda sha: measure.built(sha, measure.BENCH_BASE_PROFILE))  # a cache miss and the edited programs both need it: one build
    had = None if names is None else basecounts.programs_at(base_sha)
    only = None if names is None else [name for name in names if name in had]
    with measure.locked(f"bench-base-{measure.tree_of(base_sha) or base_sha}"):
        base, was = basecounts.base_counts(
            base_sha, args.opt, basecounts.compiler_key, built, lambda directory, opts, names=None: basecounts.measure_in_subprocess(directory, opts, args.work / "base", args.time, names), args.time, only=only
        )
        now = {name: hashed for name, hashed in basecounts.programs(ROOT).items() if names is None or name in names}
        base, same, unbuilt = basecounts.with_edited(
            base, was, now, args.opt, lambda names: basecounts.measure_head_sources(built(base_sha), args.opt, args.work / "base-edited", names), tree=measure.tree_of(base_sha) or base_sha
        )
    if unbuilt:
        print("edited programs the base compiler could not build, not judged: " + " ".join(unbuilt), flush=True)
    return base, same, base_sha


def source_digests(names: list[str]) -> dict[str, str]:
    """md5 of each program's sources (not its expected.toml or .out), so two runs say whether they measured the same programs."""
    out = {}
    for one in benchmarks(set(names)):
        out[directory_name(one)] = corpus_digest(one)
    return out


def compare_runs(first: Path, second: Path, allow_source_diff: bool = False) -> int:
    """Exit 1, naming the programs, when two --json runs measured different sources: an A/B across them compares the sources, not the compilers."""
    sides = [json.loads(path.with_name(path.name + ".sources").read_text()) if path.with_name(path.name + ".sources").exists() else None for path in (first, second)]
    if None in sides:
        print("REFUSED: a run carries no .sources digest; measure it again")
        return 1
    differ = sorted(name for name in sides[0].keys() & sides[1].keys() if sides[0][name] != sides[1][name])
    if differ and not allow_source_diff:
        print(f"REFUSED: {len(differ)} programs measured from different sources: {' '.join(differ)}")
        return 1
    print(f"SAME SOURCES {len(sides[0].keys() & sides[1].keys()) - len(differ)} programs")
    return 0


def compare_baselines(first: Path, second: Path) -> int:
    """Exit 1, naming the cells, when two runs of one compiler disagree: a baseline that two runs do not reproduce judges nothing."""
    one, two = (basecounts.rows_of(json.loads(path.read_text())) for path in (first, second))
    problems = basecounts.unstable_cells(one, two)
    print("\n".join(problems) or f"STABLE {len(one.keys() & two.keys())} cells")
    return 1 if problems else 0


def base_leg(args, slice_of: tuple[int, int] | None = None) -> int:
    """`--shard base`: the base's counts, measured (or restored) once for the whole run and written to `base.json` for --merge. The numbered slices
    measure only the head: each of them built the base and measured its part of it (bench-3 of gate run 2 was cancelled at 60 minutes in it).
    `--shard base:I/N`: slice I of the base's programs, the split the head's slices use, written to `base-I.json`: the one leg took over
    73 minutes on a hosted runner (gate run 7, cancelled at 90)."""
    names = None if slice_of is None else shards.of(*slice_of)
    stage("base counts" if slice_of is None else f"base counts, slice {slice_of[0]} of {slice_of[1]}: {len(names)} programs, {cores()}")
    try:
        base, same, base_sha = base_rows(args) if slice_of is None else base_rows(args, names)
    except BaseException as why:  # noqa: BLE001: the reason is the line the gate prints
        print("FAIL BASE: the base could not be measured: " + type(why).__name__ + ": " + " ".join(str(why).split())[-400:])
        return 1
    stage(f"base {base_sha[:9]}: {len(base)} rows")
    if args.json:
        args.json.mkdir(parents=True, exist_ok=True)
        (args.json / ("base.json" if slice_of is None else f"base-{slice_of[0]}.json")).write_text(
            json.dumps({"base": {"/".join(key): value for key, value in base.items()}, "same": sorted(same), "base_sha": base_sha, **({} if names is None else {"programs": names})})
        )
    print(f"base {base_sha[:9]}: {len(base)} rows: 0 problems")
    return 0


def partition_problems(files: list[Path], legs: list[Path], count: int) -> list[str]:
    """Each head shard and each base slice must have measured exactly the programs the split gives it: all of them are then judged, none twice. A
    slice that came back with fewer would read as zero regressions on the rest."""
    out = []
    for kind, paths in (("shard", files), ("base slice", [one for one in legs if one.name != "base.json"])):
        seen: dict[str, int] = {}
        for one in paths:
            index = int(one.stem.split("-")[1])
            listed = json.loads(one.read_text()).get("programs")
            want = shards.of(index, count)
            if listed is None or sorted(listed) != sorted(want):
                lack, extra = sorted(set(want) - set(listed or [])), sorted(set(listed or []) - set(want))
                out.append(f"FAIL BASE: bench {kind} {index} of {count} measured {len(listed or [])} programs, the split gives it {len(want)}" + (f"; lacks {' '.join(lack)}" if lack else "") + (f"; extra {' '.join(extra)}" if extra else ""))
            for name in listed or []:
                seen[name] = seen.get(name, 0) + 1
        out += [f"FAIL BASE: bench {kind}s measured {name} {n} times" for name, n in sorted(seen.items()) if n > 1]
    return out


def merge(folder: Path, count: int, opts: list[str], record: bool) -> int:
    """The verdict of the `count` shard files in `folder` (bench.py --shard writes them) judged against `base.json` (--shard base): the lines the
    whole run prints that a shard cannot (the standing against each reference, the waivers nothing needed, the problems last), and with `record`
    the head's counts as the base of the next pull requests. A missing shard, or no base, is a failure, named: its rows were not judged."""
    files = sorted(folder.glob("shard-*.json"))
    have = {int(one.stem.split("-")[1]) for one in files}
    if missing := sorted(set(range(1, count + 1)) - have):
        print(f"FAIL BASE: no result from bench shard {' '.join(map(str, missing))} of {count}: its rows were not judged")
        return 1
    legs = [folder / "base.json"] if (folder / "base.json").is_file() else sorted(folder.glob("base-*.json"))
    if not legs:
        print("FAIL BASE: no result from bench-base: no row was judged against the base")
        return 1
    if legs[0].name != "base.json" and (missing := sorted(set(range(1, count + 1)) - {int(one.stem.split("-")[1]) for one in legs})):
        print(f"FAIL BASE: no result from bench-base slice {' '.join(map(str, missing))} of {count}: its rows were not judged against the base")
        return 1
    if cut := partition_problems(files, legs, count):
        print(*cut, sep="\n")
        return 1
    measured = {}
    for one in files:
        measured |= basecounts.rows_of(json.loads(one.read_text())["measured"])
    base, same, shas = {}, set(), set()
    for one in legs:
        data = json.loads(one.read_text())
        base |= basecounts.rows_of(data["base"])
        same |= set(data["same"])
        shas.add(data["base_sha"])
    if len(shas) != 1:
        print(f"FAIL BASE: the base slices measured different bases: {' '.join(sorted(sha[:9] for sha in shas))}")
        return 1
    base_sha = shas.pop()
    problems = judge(measured, [], opts, base, same)
    judged, judged_programs, left_out, uncovered = basecounts.coverage(measured, base, same)
    problems += uncovered
    stale = basecounts.judge(measured, base, basecounts.waivers(), same)[2]
    applied, more = basecounts.waived_lines(basecounts.waived_rows(measured, base, basecounts.waivers(), same), basecounts.waivers(), stale)
    problems += more
    print(*standing(measured, [], opts), sep="\n")
    print(f"BASE {base_sha[:9]}: {len(base)} rows")
    print(*basecounts.coverage_lines(judged, judged_programs, left_out), sep="\n")
    print(*basecounts.stale_summary(stale), sep="\n")
    print(*applied, sep="\n")
    if record:
        import measure

        print("recorded", basecounts.record(measured, basecounts.CACHE, basecounts.compiler_key(measure.git("rev-parse", "HEAD"))))
    for one in problems:
        print("FAIL", one)
    print(f"{len(measured)} measurements: {len(problems)} problems")
    return 1 if problems else 0


def main() -> int:
    if (argv := guarded_argv()) and os.environ.get("LG_BENCH_UNGUARDED") != "1":
        os.execv(argv[0], argv)
    parser = argparse.ArgumentParser()
    parser.add_argument("select", nargs="*")
    parser.add_argument("--opt", nargs="+", default=["O2", "Os"])
    parser.add_argument("--lang", nargs="+", choices=[*LANGUAGES, native.OURS], help="measure only these languages (a BASIC build failing on the head stops every other language's run)")
    parser.add_argument("--time", action="store_true", help="also the kernel's time in DOSBox from RDTSC, net of start-up; gates the time ratio to each reference")
    parser.add_argument("--references", action="store_true", help="measure Open Watcom and BCC (C) and BC 4.5 (BASIC) again: they must equal the stored ones, or --bless records them")
    parser.add_argument("--bless", action="store_true", help="record the references' counts (with --references): the compiler's own are judged against the base, never recorded")
    parser.add_argument("--reason", default="")
    parser.add_argument("--no-judge", action="store_true", help="measure, write --json and stop: the base's counts are made this way")
    parser.add_argument("--record", action="store_true", help="measure the head and keep its counts as the base of the next pull requests (a main push); no base, no judging")
    parser.add_argument("--shard", metavar="I/N", help="only shard I of N of the benchmarks (shards.toml), judged alone; --json then writes shard-I.json for --merge. `base` or `base:I/N`: the base's counts, all or slice I, in base.json or base-I.json")
    parser.add_argument("--merge", nargs=2, metavar=("DIR", "N"), help="judge nothing: print the verdict of the N shard-*.json in DIR (add --record on a main push)")
    parser.add_argument("--compare-runs", nargs=2, type=Path, metavar=("A", "B"), help="refuse (exit 1, programs named) when two --json runs measured different program sources, unless --allow-source-diff")
    parser.add_argument("--allow-source-diff", action="store_true")
    parser.add_argument("--compare-baselines", nargs=2, type=Path, metavar=("A", "B"), help="judge nothing: refuse (exit 1, cells named) when two --json runs of one compiler differ in any count")
    parser.add_argument("--json", type=Path)
    parser.add_argument("--work", type=Path, default=llrmbin.target_dir() / "bench")
    args = parser.parse_args()
    global ONLY
    ONLY = args.lang
    if args.compare_runs:
        return compare_runs(*args.compare_runs, args.allow_source_diff)
    if args.compare_baselines:
        return compare_baselines(*args.compare_baselines)
    if args.merge:
        return merge(Path(args.merge[0]), int(args.merge[1]), args.opt, args.record)
    if args.shard == "base":
        return base_leg(args)
    if args.shard and args.shard.startswith("base:"):
        return base_leg(args, tuple(map(int, args.shard[5:].split("/"))))
    if args.shard:
        index, count = map(int, args.shard.split("/"))
        args.select = shards.of(index, count)
    if args.bless and not (args.reason and args.references):
        parser.error("--bless records the references, so it needs --references and --reason: every change to expected.toml says why")
    stage(f"measuring {len(args.select) or 'every'} benchmarks at {' '.join(args.opt)}, {cores()}")
    measured = measure_all(args.select, args.opt, args.work, args.time)
    stage(f"measured {len(measured)} rows")
    if args.shard:  # a slice only measures: --merge judges every row against the base that `--shard base` measured once
        print(f"shard {args.shard}: {len(measured)} measurements: 0 problems")
        if args.json:
            args.json.mkdir(parents=True, exist_ok=True)
            (args.json / f"shard-{index}.json").write_text(json.dumps({"measured": {"/".join(key): value for key, value in measured.items()}, "programs": args.select}))
        return 0
    if args.references:
        import flag_parity

        flag_parity.check()  # a reference whose codegen flags differ from ours is refused before it is built, as tools/wasm_flag_check.py does for wasm
        measured |= measure_references(args.select, args.opt, args.work, args.time)
    if args.json and not args.shard:
        args.json.write_text(json.dumps({"/".join(key): value for key, value in measured.items()}, indent=1))
        args.json.with_name(args.json.name + ".harness").write_text(basecounts.harness_digest() + "\n")
        args.json.with_name(args.json.name + ".sources").write_text(json.dumps(source_digests(sorted({key[0] for key in measured})), indent=1) + "\n")
        import stamp

        # Each compiler's build commit and md5 beside the counts; with LLRM_EXPECT_SHA named, one built from a commit that is not its ancestor is refused.
        expect = os.environ.get("LLRM_EXPECT_SHA")
        recorded = stamp.require_ancestor(BIN, expect, ROOT) if expect else stamp.dist_stamps(BIN)
        args.json.with_name(args.json.name + ".provenance").write_text(json.dumps({name: {"sha": sha, "md5": md5} for name, (sha, md5) in recorded.items()}, indent=1) + "\n")
    print("HARNESS", basecounts.harness_digest())
    print(*skip_lines(measured), sep="\n")
    print(*note_lines(measured), sep="\n")
    print(*io_lines(measured), sep="\n")
    unbuilt = [f"{'/'.join(key)}: {value['error']}" for key, value in measured.items() if "error" in value and not value["error"].startswith("skipped: ")]
    if unbuilt and (args.record or args.no_judge):  # a base or leg with cells that did not build is no base: say how many, exit nonzero
        print(f"NOHEAD {len(unbuilt)} cells did not build:", *unbuilt[:20], sep="\n  ")
        return 1
    if args.record:
        import measure

        print("recorded", basecounts.record(measured, basecounts.CACHE, basecounts.compiler_key(measure.git("rev-parse", "HEAD"))))
        return 0
    if args.no_judge:
        return 0
    if args.bless:
        references = {key: value for key, value in measured.items() if key[1] not in basecounts.OURS}
        changed = 0
        for directory in benchmarks(args.select):
            name = str(directory.relative_to(BENCH))
            broken = [f"{key}: {value['error']}" for key, value in references.items() if key[0] == name and key[1] in BASIC_LANGUAGES and "error" in value and "known" not in value]
            if broken:
                print(f"not blessing {name}: {broken}")
                return 1
            changed += write_expected(directory, references, name, args.reason)
        REFERENCE_STAMP.write_text(json.dumps(reference_stamp(), indent=1) + "\n")
        print(f"blessed {len({key[0] for key in references})} benchmarks, {changed} files changed")
        print(*standing(measured, args.select, args.opt), sep="\n")
        return 0
    stage("base counts")
    try:
        base, same, base_sha = base_rows(args)
    except BaseException as why:  # noqa: BLE001: the reason is the line the gate prints
        print("FAIL BASE: the base could not be measured: " + type(why).__name__ + ": " + " ".join(str(why).split())[-400:])
        return 1
    stage(f"base {base_sha[:9]}: {len(base)} rows")
    problems = judge(measured, args.select, args.opt, base, same)
    if not args.select:
        judged, judged_programs, left_out, uncovered = basecounts.coverage(measured, base, same)
        problems += uncovered
        print(*basecounts.coverage_lines(judged, judged_programs, left_out), sep="\n")
        print(*basecounts.coverage_detail(measured, base, same), sep="\n")
    print(*standing(measured, args.select, args.opt), sep="\n")
    print(*align_lines(measured), sep="\n")
    print(*helper_lines(measured), sep="\n")
    print(*fold_lines(measured), sep="\n")
    if args.select:
        print(*clock_lines(measured, args.select), sep="\n")
    stale = [] if args.select else basecounts.judge(measured, base, basecounts.waivers(), same)[2]  # a selection leaves the other rows' waivers unused
    print(f"BASE {base_sha[:9]}: {len(base)} rows")
    print(*basecounts.stale_summary(stale), sep="\n")
    if not args.select:
        applied, more = basecounts.waived_lines(basecounts.waived_rows(measured, base, basecounts.waivers(), same), basecounts.waivers(), stale)
        print(*applied, sep="\n")
        problems += more
    for one in problems:  # last: the tail of the log is what a reader sees
        print("FAIL", one)
    print(f"{len(measured)} measurements: {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
