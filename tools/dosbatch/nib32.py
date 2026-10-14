"""Nib programs on the flat 32-bit target (DOS/32A): build them, run them, compare what they print with the output each bench
program states (bench/NAME/NAME.out).

    python tools/dosbatch/nib32.py [--work DIR] [NAME ...]     (every bench program with a Nib source by default)

A bench program's own header says how: `# flags: A | B` (B is the flat target's), `# data: FILE...` (beside it, or `@name` for a
cached corpus; the program opens it by its upper-case name) and `# targets: x86-m16 ...`, which leaves the flat target out.
"""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import corpus  # noqa: E402
import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbbuild  # noqa: E402

TARGET = dosbatch.linkrecipe.named(32)
RUNTIME = dosbatch.ROOT / "crates/frontends/llrm-nib/src/runtime/runtime.nib"
LAYER = Path(dosbatch.os_layer(TARGET, "directory", "nib"))


def header(source: Path, key: str) -> str | None:
    """The `# KEY: value` line of a Nib source's first lines."""
    for line in source.read_text(encoding="latin-1").splitlines()[:6]:
        if match := re.match(rf"#\s*{key}:\s*(.*?)\s*$", line):
            return match.group(1)
    return None


def flat(source: Path) -> str | None:
    """Why the program has no flat build, or None where it has one."""
    targets = header(source, "targets")
    if targets is not None and "x86-m32" not in targets:
        return f"its header says `targets: {targets}`"
    return None


def flags(source: Path) -> list[str]:
    """The compiler flags of the flat target: the second set of `# flags: A | B`, the only one where there is no `|`."""
    line = header(source, "flags") or "-O2"
    return line.split("|")[-1].split()


def data_files(source: Path) -> tuple[Path, ...]:
    """The files the program reads: beside it, or `@name` for a cached corpus."""
    names = (header(source, "data") or "").split()
    return tuple(corpus.path(one[1:]) if one.startswith("@") else (source.parent / one).resolve() for one in names)


def build(source: Path, work: Path, level: str | None = None, extra: tuple[str, ...] = ()) -> qbbuild.Built:
    """`source` for the flat target: a COMDAT per procedure, the runtime cut to what it, the start-up and the OS layer name,
    linked; at `level` where one is given (its header's flags otherwise)."""
    work.mkdir(parents=True, exist_ok=True)
    nib = str(dosbatch.BIN / "llrm-nib")
    chosen = [level] if level else flags(source)
    program, runtime, start = work / "program.obj", work / "runtime.obj", work / "start.obj"
    dosbatch.assemble(LAYER / dosbatch.os_layer(TARGET, "start", "nib"), start, *dosbatch.os_defines(TARGET, "nib"))
    common = ["-m32", *[one for one in chosen if one != "-m32"], "--procedure-segments", *extra]
    dosbatch._host([nib, *common, str(source), "-o", str(program)])
    dosbatch._host([nib, *common, "--used-by", str(program), "--used-by", str(start), str(RUNTIME), "-o", str(runtime)])
    exe = work / "PROGRAM.EXE"
    start_file = str((LAYER / dosbatch.os_layer(TARGET, "start", "nib")).relative_to(dosbatch.ROOT))
    loaders = dosbatch.link_target(TARGET, program, exe, work, runtime=([start_file], []), objects_after=(runtime,))
    return qbbuild.Built(exe, tuple(loaders))


def programs() -> dict[str, Path]:
    """The bench programs that have a Nib source, by name."""
    return {path.name: path / f"{path.name}.nib" for path in sorted((dosbatch.ROOT / "bench").iterdir()) if (path / f"{path.name}.nib").exists()}


def run(names: list[str], work: Path, level: str | None = None) -> dict[str, tuple[str, bool]]:
    """Each program built and run in one session: how it ended and whether it printed its `.out`; a program with no flat build or
    that did not build says why in place of the status."""
    found: dict[str, tuple[str, bool]] = {}
    jobs, wanted = [], {}
    sources = programs()
    for at, name in enumerate(names):
        source = sources[name]
        if reason := flat(source):
            found[name] = (f"excused: {reason}", True)
            continue
        try:
            built = build(source, work / name, level)
            files = data_files(source)
        except (dosbatch.BuildError, corpus.Unavailable) as error:
            found[name] = (f"build: {str(error).strip().splitlines()[-1][:200]}", False)
            continue
        stem = f"P{at:03d}"
        jobs.append(dosbatch.Job(stem, "exe", built.exe, files=(*built.files, *files)))
        wanted[stem] = (name, (source.parent / f"{name}.out").read_bytes())
    if jobs:
        results = qb32.run_jobs(jobs, work / "run")
        for job in jobs:
            name, want = wanted[job.stem]
            got = qb32.qbruntime.raw_output(work / "run", job.stem)
            found[name] = (results[job.stem].status, got.replace(b"\r\n", b"\n").split() == want.replace(b"\r\n", b"\n").split())
    return found


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", type=Path)
    parser.add_argument("--level")
    parser.add_argument("names", nargs="*")
    args = parser.parse_args()
    names = args.names or list(programs())
    found = run(names, args.work or Path(tempfile.mkdtemp(prefix="nib32.")), args.level)
    for name, (status, same) in found.items():
        print(f"{'ok  ' if same and status in ('ok', 'exit') or status.startswith('excused') else 'FAIL'} {name}: {status}")
    return 0 if all(same for _, same in found.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
