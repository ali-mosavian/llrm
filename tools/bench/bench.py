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
import shutil
import argparse
import subprocess
import tomllib
from datetime import date
from pathlib import Path
from dataclasses import dataclass
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "dosbatch"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import maps  # noqa: E402
import icount  # noqa: E402
import dosbatch  # noqa: E402
import run_tests  # noqa: E402
from dosbatch import BIN, ROOT, Job  # noqa: E402

OW = Path(os.environ.get("OW_BIN", Path.home() / "work/personal/open-watcom-v2/build/binbuild"))
BENCH = ROOT / "bench"
LANGUAGES = {"bas": ".bas", "c": ".c", "nib": ".nib"}
COUNTERS = ("instructions", "memory_operands")
# DOSBox at a fixed 75000 cycles per millisecond: RDTSC cycles / 75000 is milliseconds, the same on any host.
TIMING_CONF = dosbatch.CONF.replace("cycles=max", "cycles=fixed 75000")
CYCLES_PER_MS = 75_000


@dataclass(frozen=True)
class Variant:
    name: str  # bench/NAME, parity/NAME
    language: str
    source: Path
    region: str  # the kernel function's name
    known: str | None
    dialect: str = "qb45"


def benchmarks(selected: list[str]) -> list[Path]:
    dirs = [one for one in sorted(BENCH.glob("*/bench.toml"))] + [one for one in sorted(BENCH.glob("parity/*/bench.toml"))]
    found = [one.parent for one in dirs]
    return [one for one in found if not selected or one.name in selected or str(one.relative_to(BENCH)) in selected]


def variants(directory: Path) -> list[Variant]:
    settings = tomllib.loads((directory / "bench.toml").read_text())
    name = str(directory.relative_to(BENCH))
    out = []
    for language, suffix in LANGUAGES.items():
        source = directory / f"{directory.name}{suffix}"
        if source.exists():
            head = run_tests.header(source)
            out.append(Variant(name, language, source, settings["region"][language], head.get("known"), head.get("dialect", "qb45")))
    return out


def expected_output(directory: Path) -> list[str]:
    return run_tests.lines((directory / f"{directory.name}.out").read_text())


def build(variant: Variant, opt: str, work: Path, stem: str) -> tuple[Path, Path] | str:
    """(exe, linker map) of the variant at -`opt`, or why it did not build. BASIC is only compiled here: LINK runs in DOSBox."""
    obj, listing = work / f"{stem}.obj", work / f"{stem}.map"
    # The kernel stays a call: inlined into main it has no entry to count from.
    flags = [f"-{opt}", "--cpu", "486", "-fno-inline-functions"]
    try:
        if variant.language == "nib":
            exe = work / f"{stem}.exe"
            extras = [str(variant.source.parent / one) for one in run_tests.header(variant.source).get("link", "").split()]
            done = subprocess.run([str(ROOT / "tools" / "nib-build.sh"), str(variant.source), str(exe), f"-{opt}", *extras], capture_output=True, text=True, timeout=300,
                                  env={**os.environ, "LLRM_BIN": str(BIN), "TOOLCHAIN": str(BIN), "NIB_MAP": str(listing), "NIB_FLAGS": "-fno-inline-functions"})
            return (exe, listing) if done.returncode == 0 and exe.exists() else "build: " + (done.stderr or done.stdout).strip()[-300:]
        tool = "llrm-qb" if variant.language == "bas" else "llrm-c"
        arguments = ["--dialect", variant.dialect, "--runtime", variant.dialect] if variant.language == "bas" else []
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


def measure(job: tuple[str, Path, Path, str, list[str]]) -> dict:
    """Counts of one built variant: the entry address from its map, the emulated run, the output check."""
    stem, exe, listing, region, want = job
    entry = maps.locate(exe, listing, region)
    if entry is None:
        return {"error": f"{region} is not in the map, and main does not call one function of its own"}
    result = icount.run(exe, entry, exe.parent)
    if result.error:
        return {"error": result.error}
    got = run_tests.lines(result.output.decode("latin1"))
    if got != want:
        return {"error": f"prints {got}, want {want}"}
    return {"instructions": result.region.instructions, "memory_operands": result.region.memory_operands}


def references_available() -> bool:
    return (OW / "bwcc").exists() and (dosbatch.QB45 / "BC.EXE").exists()


def build_watcom(variant: Variant, opt: str, work: Path, stem: str) -> tuple[Path, Path] | str:
    """Open Watcom, medium model, cdecl as llrm-c calls, no stack checks: tools/loops' reference build."""
    obj, exe, listing = work / f"{stem}.obj", work / f"{stem}.exe", work / f"{stem}.map"
    flags = ["-ox"] if opt == "O2" else ["-os", "-ol"]
    done = subprocess.run([str(OW / "bwcc"), "-zq", "-mm", "-ecc", "-s", "-zl", "-DOWREF", "-4", *flags, str(variant.source), f"-fo={obj}"], capture_output=True, text=True, timeout=300)
    if done.returncode != 0 or not obj.exists():
        return "compile: " + (done.stderr or done.stdout).strip()[-300:]
    stub = work / "OWSTUB.OBJ"
    try:
        if not stub.exists():
            dosbatch.assemble(Path(__file__).with_name("owstub.asm"), stub)
        # Watcom names main `main_`, and its objects ask for its own start-up code: crt.asm and the stub stand in
        dosbatch.link_c(obj, exe, work, listing, ("file", str(stub)), ("alias", "_main=main_"))
    except dosbatch.BuildError as error:
        return f"build: {error}"
    return exe, listing


def measure_references(selected: list[str], opts: list[str], work: Path) -> dict[tuple[str, str, str], dict]:
    """The reference compilers on the same programs: Open Watcom for C (language `ow`, at each level) and QuickBASIC 4.5's
    BC for BASIC (language `bc`, one level: /O). Never gated; the history records them beside llrm's."""
    out: dict[tuple[str, str, str], dict] = {}
    folder = work / "reference"
    shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True)
    todo = [(directory, variant) for directory in benchmarks(selected) for variant in variants(directory)]
    jobs, owned = [], []
    for at, (directory, variant) in enumerate(todo):
        stem = f"R{at:03d}"
        if variant.language == "c":
            for opt in opts:
                built = build_watcom(variant, opt, folder, f"{stem}{opt}")
                key = (variant.name, "ow", opt)
                if isinstance(built, str):
                    out[key] = {"error": built}
                else:
                    owned.append((key, (stem, built[0], built[1], variant.region, expected_output(directory))))
        elif variant.language == "bas" and variant.dialect == "qb45":
            jobs.append((variant, Job(stem, "bas", variant.source, switches="/O /FPi", map=True)))
    if jobs:
        ran = dosbatch.run([job for _, job in jobs], folder / "bc")
        for (variant, job), directory in zip(jobs, [d for d, v in todo if v.language == "bas" and v.dialect == "qb45"]):
            key = (variant.name, "bc", "O2")
            if ran[job.stem].status == "not built":
                out[key] = {"error": "bc: " + ran[job.stem].detail}
                continue
            exe, listing = folder / f"{job.stem}.exe", folder / f"{job.stem}.map"
            shutil.copy(folder / "bc" / f"{job.stem}.EXE", exe)
            shutil.copy(folder / "bc" / f"{job.stem}.MAP", listing)
            owned.append((key, (job.stem, exe, listing, variant.region, expected_output(directory))))
    with ProcessPoolExecutor() as pool:
        for (key, _), counts in zip(owned, pool.map(measure, [job for _, job in owned])):
            out[key] = counts
    return out


def measure_all(selected: list[str], opts: list[str], work: Path, timing: bool = False) -> dict[tuple[str, str, str], dict]:
    """{(benchmark, language, opt): counts or {"error": why} or {"known": issue}}"""
    out: dict[tuple[str, str, str], dict] = {}
    for opt in opts:
        folder = work / opt
        shutil.rmtree(folder, ignore_errors=True)
        folder.mkdir(parents=True)
        todo = [(directory, variant) for directory in benchmarks(selected) for variant in variants(directory)]
        stems = {(v.name, v.language): f"V{at:03d}" for at, (_, v) in enumerate(todo)}
        with ThreadPoolExecutor() as pool:
            built = list(pool.map(lambda one: build(one[1], opt, folder, stems[(one[1].name, one[1].language)]), todo))
        basic = {stems[(v.name, v.language)]: (b[0], v.dialect) for (_, v), b in zip(todo, built) if v.language == "bas" and not isinstance(b, str)}
        failed = link_basic(basic, folder)
        jobs = []
        for (directory, variant), result in zip(todo, built):
            key = (variant.name, variant.language, opt)
            stem = stems[(variant.name, variant.language)]
            why = result if isinstance(result, str) else failed.get(stem)
            if why:
                out[key] = {"error": why}
                continue
            exe, listing = (folder / f"{stem}.exe", folder / f"{stem}.map") if variant.language == "bas" else result
            jobs.append((key, (stem, exe, listing, variant.region, expected_output(directory))))
        with ProcessPoolExecutor() as pool:
            for (key, _), counts in zip(jobs, pool.map(measure, [job for _, job in jobs])):
                out[key] = counts
        if timing:
            exes = {key: job[1] for key, job in jobs if "error" not in out[key]}
            for key, timed in time_programs(exes, folder / "time").items():
                out[key] |= {"time_error": timed["error"]} if "error" in timed else timed
    return mark_known(out, [v for directory in benchmarks(selected) for v in variants(directory)], opts)


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


def time_programs(exes: dict[tuple[str, str, str], Path], work: Path) -> dict[tuple[str, str, str], dict]:
    """Wall time of each whole program in DOSBox, from RDTSC around the run (tools/bench/timeit.asm): {"cycles", "ms"}.
    It includes the runtime's start-up and the print, so it compares one program across commits, not across languages."""
    if not exes:
        return {}
    work.mkdir(parents=True, exist_ok=True)
    stub = work / "TIMEIT.COM"
    done = subprocess.run([str(BIN / "jwasm"), "-q", "-bin", f"-Fo{stub}", str(Path(__file__).with_name("timeit.asm"))], capture_output=True, text=True)
    if done.returncode != 0:
        return {key: {"error": "timeit.asm: " + done.stderr.strip()[-200:]} for key in exes}
    keys = list(exes)
    jobs = [Job(f"W{at:03d}", "exe", exes[key], runner="TIMEIT.COM", files=(stub,)) for at, key in enumerate(keys)]
    ran = dosbatch.run(jobs, work / "run", conf=TIMING_CONF)
    out = {}
    for key, job in zip(keys, jobs):
        text = ran[job.stem].text
        found = [line for line in text.splitlines() if line.startswith("TSC ")]
        out[key] = {"cycles": int(found[-1][4:]), "ms": round(int(found[-1][4:]) / CYCLES_PER_MS, 3)} if found else {"error": f"no timing line: {text[-80:]!r}"}
    return out


def read_expected(directory: Path) -> dict:
    path = directory / "expected.toml"
    return tomllib.loads(path.read_text()) if path.exists() else {}


def write_expected(directory: Path, measured: dict, name: str, reason: str) -> None:
    lines = [f"reason = {json.dumps(reason)}", f'blessed = "{date.today()}"', ""]  # JSON escapes are TOML's
    for language in LANGUAGES:
        for opt in sorted({key[2] for key in measured if key[0] == name}):
            counts = measured.get((name, language, opt))
            if counts and "error" not in counts:
                lines += [f"[{language}.{opt}]", *(f"{one} = {counts[one]}" for one in COUNTERS), ""]
    (directory / "expected.toml").write_text("\n".join(lines))


def against(label: str, got: dict, want: dict | None) -> list[str]:
    """What fails the gate for one measurement: no baseline, a count above it (worse), one below it (better, to bless)."""
    if want is None:
        return [f"{label}: no baseline; run --bless --reason"]
    problems = []
    for counter in COUNTERS:
        if got[counter] > want[counter]:
            problems.append(f"{label}: {counter} {got[counter]} > {want[counter]} (worse)")
        elif got[counter] < want[counter]:
            problems.append(f"{label}: {counter} {got[counter]} < {want[counter]} (better: --bless --reason)")
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
                    problems += against(label, got, baseline.get(variant.language, {}).get(opt))
    return problems


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("select", nargs="*")
    parser.add_argument("--opt", nargs="+", default=["O2", "Os"])
    parser.add_argument("--time", action="store_true", help="also the whole program's time in DOSBox from RDTSC (advisory, never gated)")
    parser.add_argument("--references", action="store_true", help="also measure Open Watcom (C) and BC 4.5 (BASIC); never gated")
    parser.add_argument("--bless", action="store_true")
    parser.add_argument("--reason", default="")
    parser.add_argument("--json", type=Path)
    parser.add_argument("--work", type=Path, default=ROOT / "target" / "bench")
    args = parser.parse_args()
    if args.bless and not args.reason:
        parser.error("--bless needs --reason: every change to expected.toml says why")
    measured = measure_all(args.select, args.opt, args.work, args.time)
    if args.references:
        measured |= measure_references(args.select, args.opt, args.work)
    if args.json:
        args.json.write_text(json.dumps({"/".join(key): value for key, value in measured.items()}, indent=1))
    if args.bless:
        names = {key[0] for key in measured}
        for directory in benchmarks(args.select):
            name = str(directory.relative_to(BENCH))
            broken = [f"{key}: {value['error']}" for key, value in measured.items() if key[0] == name and "error" in value and "known" not in value]
            if broken:
                print(f"not blessing {name}: {broken}")
                return 1
            write_expected(directory, measured, name, args.reason)
        print(f"blessed {len(names)} benchmarks")
        return 0
    problems = judge(measured, args.select, args.opt)
    for one in problems:
        print("FAIL", one)
    print(f"{len(measured)} measurements: {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
