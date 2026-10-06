r"""
tests/run: compile each program with llrm, run them all in one DOSBox launch,
diff stdout with NAME.out.

    python3 tools/dosbatch/run_tests.py [LANG|NAME]... [--work DIR]

A header comment holds a program's settings:

    ' flags: -Os --cpu P5      extra compiler flags (default: -O2 --cpu 486); `a | b` builds and runs the program once for each
    ' dialect: pds71           qb45 (default), pds71 or vbdos: its compiler dialect and runtime; several, blank apart, run once each
    ' link: sortlib.nib        more sources built with it, beside the program (a .nib for BASIC; a .c or .asm for Nib)
    ' data: values.dat         a file the program reads, copied beside it; @dickens: a cached corpus, verified (skipped if unavailable)
    ' mask: \d+(?= spins)       text of the output that varies: each match reads as N
    ' known: #123              fails today, tracked by issue 123

A known program that passes fails the run: remove its mark.
"""

from __future__ import annotations

import os
import re
import sys
import argparse
import subprocess
from pathlib import Path
from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).parent))

import corpus  # noqa: E402
import dosbatch  # noqa: E402
from dosbatch import BIN, ROOT, Job  # noqa: E402

RUN = ROOT / "tests" / "run"
EXAMPLES = ROOT / "examples"
BENCH = ROOT / "bench"
DEFAULT_FLAGS = ["-O2", "--cpu", "486"]
KEYS = ("flags", "known", "bc", "diverges", "dialect", "link", "data", "mask")
HEADER = re.compile(rf"^\s*(?:'|//|#)\s*({'|'.join(KEYS)}):\s*(.*?)\s*$")
COMPILERS = {".bas": ["llrm-qb"], ".nib": [], ".c": ["llrm-c"]}
TOOLS = {"qb45": dosbatch.QB45_TOOLS, "pds71": dosbatch.PDS71_TOOLS, "vbdos": dosbatch.VBDOS_TOOLS}


@dataclass
class Program:
    source: Path
    flags: list[str]
    known: str | None
    dialect: str = "qb45"
    link: tuple[str, ...] = ()
    data: tuple[str, ...] = ()
    mask: str = ""
    label: str = ""

    @property
    def name(self) -> str:
        return f"{self.source.parent.name}/{self.source.name}{self.label}"

    @property
    def stem(self) -> str:
        return self.source.stem

    @property
    def out(self) -> Path:
        """NAME.out beside the source, or for bench/NAME/ and a directory of variants, the one NAME.out they all print."""
        own = self.source.with_suffix(".out")
        return own if own.exists() else self.source.parent / f"{self.source.parent.name}.out"


def header(source: Path) -> dict[str, str]:
    """The settings in the comment lines leading a source."""
    found: dict[str, str] = {}
    for line in source.read_text(encoding="latin1").splitlines():
        match = HEADER.match(line)
        if match:
            found[match[1]] = match[2]
        elif line.strip() and not re.match(r"^\s*(?:'|//|#)", line):
            break
    return found


def configurations(settings: dict[str, str]) -> list[tuple[str, list[str], str]]:
    """Each way a header asks for a program to be built: its label, its `flags:` and its `dialect:`.

    `flags: -O2 | -Os --cpu P5` is two, `dialect: qb45 pds71 vbdos` three, and both make their product; the
    label names what differs. One of each is one configuration with no label."""
    alternatives = [one.split() for one in settings["flags"].split("|")] if "flags" in settings else [DEFAULT_FLAGS]
    dialects = settings.get("dialect", "qb45").split()
    out = []
    for flags in alternatives:
        for dialect in dialects:
            shown = [" ".join(flags)] * (len(alternatives) > 1) + [dialect] * (len(dialects) > 1)
            out.append((f" [{', '.join(shown)}]" if shown else "", flags, dialect))
    return out


def kept_flags(flags: list[str]) -> list[str]:
    """`flags` less the optimization level and cpu, which the caller sets."""
    return [one for at, one in enumerate(flags) if not one.startswith("-O") and one != "--cpu" and flags[at - 1 : at] != ["--cpu"]]


def compiler_arguments(source: Path, flags: list[str] | None = None, dialect: str | None = None) -> list[str]:
    """What a program's header asks of its compiler apart from the optimization level and cpu, which the caller sets:
    `dialect:` and the rest of `flags:`; for the configuration given, else the first."""
    settings = header(Path(source))
    _, first, named = configurations(settings)[0]
    chosen = dialect or named
    return [*(["--dialect", chosen, "--runtime", chosen] if "dialect" in settings else []), *kept_flags(flags or first)]


def discover(selected: list[str]) -> list[Program]:
    programs = []
    for source in [*sorted(RUN.glob("*/*")), *sorted(EXAMPLES.glob("*.nib")), *sorted(EXAMPLES.glob("*/*")), *sorted(BENCH.glob("*/*")), *sorted(BENCH.glob("parity/*/*"))]:
        if source.suffix in COMPILERS:
            settings = header(source)
            for label, flags, dialect in configurations(settings):
                program = Program(source, flags, settings.get("known"), dialect, tuple(settings.get("link", "").split()), tuple(settings.get("data", "").split()), settings.get("mask", ""), label)
                if program.out.exists():
                    programs.append(program)
    if selected:
        programs = [p for p in programs if p.source.parent.name in selected or p.source.stem in selected or p.name in selected]
    return programs


def lines(text: str) -> list[str]:
    return [one.rstrip() for one in text.replace("\r\n", "\n").split("\n") if one.strip()]


def masked(output: list[str], mask: str) -> list[str]:
    """`output` with each match of `mask` (the text that varies run to run) read as N."""
    return [re.sub(mask, "N", one) for one in output] if mask else output


def first_difference(want: list[str], got: list[str]) -> str:
    for at in range(max(len(want), len(got))):
        w = want[at] if at < len(want) else "<end>"
        g = got[at] if at < len(got) else "<end>"
        if w != g:
            return f"line {at + 1}: want {w!r}, got {g!r}"
    return ""


def compile_one(program: Program, obj: Path) -> str | None:
    tool, *rest = COMPILERS[program.source.suffix]
    dialect = ["--dialect", program.dialect, "--runtime", program.dialect] if program.source.suffix == ".bas" else []
    done = subprocess.run([str(BIN / tool), str(program.source), *rest, *dialect, *program.flags, "-o", str(obj)],
                          capture_output=True, text=True, timeout=300)
    if done.returncode != 0 or not obj.exists():
        return "compile: " + (done.stderr or done.stdout).strip()[-600:]
    return None


def data_files(program: Program) -> tuple[Path, ...]:
    """The files a program reads: beside its source, or `@name` for a cached corpus (tools/dosbatch/corpus.py)."""
    return tuple(corpus.path(one[1:]) if one.startswith("@") else program.source.parent / one for one in program.data)


def unavailable(program: Program) -> str | None:
    """Why a program's corpus cannot be had, or None."""
    for one in program.data:
        if one.startswith("@"):
            try:
                corpus.path(one[1:])
            except corpus.Unavailable as error:
                return str(error)
    return None


def build(program: Program, work: Path, stem: str) -> Job | str:
    """The job that runs `program`, or why it did not build."""
    target = program.flags[program.flags.index("--target") + 1] if "--target" in program.flags else "x86-code16"
    if program.source.suffix == ".nib" and target != "x86-code16":
        exe, obj = work / f"{stem}.exe", work / f"{stem}.obj"
        done = subprocess.run([str(BIN / "llrm-nib"), str(program.source), *program.flags, "-o", str(obj), "--procedure-segments"], capture_output=True, text=True, timeout=300)
        if done.returncode != 0 or not obj.exists():
            return "compile: " + (done.stderr or done.stdout).strip()[-600:]
        try:
            loaders = dosbatch.link_nib(target, program.source, obj, exe, work, program.flags[0] if program.flags else "-O2", ())
            dosbatch.check_loads(exe)
        except (dosbatch.BuildError, dosbatch.TooBig) as error:
            return f"link: {error}"
        return Job(stem, "exe", exe, files=(*data_files(program), *loaders))
    if program.source.suffix == ".nib":
        exe = work / f"{stem}.exe"
        extras = [str(program.source.parent / one) for one in program.link]
        nib_flags = " ".join(compiler_arguments(program.source, program.flags))
        done = subprocess.run([str(ROOT / "tools" / "nib-build.sh"), str(program.source), str(exe), *program.flags[:1], *extras],
                              capture_output=True, text=True, timeout=300, env={**os.environ, "LLRM_BIN": str(BIN), "TOOLCHAIN": str(BIN), "NIB_FLAGS": nib_flags})
        if done.returncode != 0 or not exe.exists():
            return "build: " + (done.stderr or done.stdout).strip()[-600:]
        try:
            dosbatch.check_loads(exe)
        except dosbatch.TooBig as error:
            return f"build: {error}"
        return Job(stem, "exe", exe, files=data_files(program))
    obj = work / f"{stem}.obj"
    if problem := compile_one(program, obj):
        return problem
    if program.source.suffix == ".c":
        exe = work / f"{stem}.exe"
        target = program.flags[program.flags.index("--target") + 1] if "--target" in program.flags else "x86-code16"
        try:
            loaders = dosbatch.link_target(target, obj, exe, work)
            dosbatch.check_loads(exe)
        except (dosbatch.BuildError, dosbatch.TooBig) as error:
            return f"link: {error}"
        return Job(stem, "exe", exe, files=(*data_files(program), *loaders))
    extras = []
    for at, one in enumerate(program.link):
        extra = work / f"{stem}L{at}.obj"
        done = subprocess.run([str(BIN / "llrm-nib"), str(program.source.parent / one), "-o", str(extra), "-O2"], capture_output=True, text=True, timeout=300)
        if done.returncode != 0 or not extra.exists():
            return f"build {one}: " + (done.stderr or done.stdout).strip()[-600:]
        extras.append(extra)
    return Job(stem, "obj", obj, objects=tuple(extras), files=data_files(program))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("select", nargs="*")
    parser.add_argument("--work", type=Path, default=ROOT / "target" / "tests-run")
    args = parser.parse_args()
    programs = discover(args.select)
    for program in [one for one in programs if unavailable(one)]:
        print(f"SKIP  {program.name}: {unavailable(program)}")
        programs.remove(program)
    if not programs:
        print("no programs selected")
        return 1
    work = args.work
    objs = work.with_name(work.name + "-obj")
    objs.mkdir(parents=True, exist_ok=True)
    stems = {p.name: f"T{at:03d}" for at, p in enumerate(programs)}
    with ThreadPoolExecutor() as pool:
        built = dict(zip((p.name for p in programs), pool.map(lambda p: build(p, objs, stems[p.name]), programs)))
    ran = {}
    for dialect, tools in TOOLS.items():
        jobs = [built[p.name] for p in programs if p.dialect == dialect and isinstance(built[p.name], Job)]
        if jobs:
            ran |= dosbatch.run(jobs, work / dialect, tools=tools)
    bad = passed = known = 0
    for program in programs:
        problem = built[program.name] if isinstance(built[program.name], str) else None
        if not problem:
            result = ran[stems[program.name]]
            if result.status != "ok":
                problem = f"{result.status}: {result.detail}"
            else:
                got = masked(lines(result.text), program.mask)
                problem = first_difference(lines(program.out.read_text()), got)
        if problem and program.known:
            known += 1
            print(f"KNOWN {program.name} ({program.known}): {problem}")
        elif problem:
            bad += 1
            print(f"FAIL  {program.name}: {problem}")
        elif program.known:
            bad += 1
            print(f"XPASS {program.name}: passes; remove its known mark ({program.known})")
        else:
            passed += 1
    print(f"{len(programs)} programs: {passed} pass, {known} known, {bad} fail")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
