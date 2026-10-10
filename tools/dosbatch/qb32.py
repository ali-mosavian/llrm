"""The QB runtime on the flat 32-bit target (DOS/32A): build it, build programs for it, run them, compare their output with
the one each bench program states (bench/NAME/NAME.out).

    python tools/dosbatch/qb32.py [--work DIR] [NAME ...]     (the 25 bench programs by default)
"""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbruntime  # noqa: E402

TARGET = dosbatch.linkrecipe.named(32)
RUNTIME = qbruntime.RUNTIME
PLATFORM = RUNTIME / "dos" / "m32"
SHARED = dosbatch.ROOT / "runtime" / "shared" / "dos" / "m32"
START_FILES = ["runtime/qb/dos/m32/qbstart.asm", "runtime/shared/dos/m32/start.asm"]


def records(path: Path):
    """The records of an OMF object: (type, body)."""
    data = path.read_bytes()
    at = 0
    while at < len(data):
        length = int.from_bytes(data[at + 1 : at + 3], "little")
        yield data[at], data[at + 3 : at + 3 + length - 1]
        at += 3 + length


def name_at(body: bytes, at: int) -> tuple[str, int]:
    size = body[at]
    return body[at + 1 : at + 1 + size].decode("latin-1"), at + 1 + size


def symbols(path: Path) -> tuple[set[str], set[str]]:
    """The names an object defines (PUBDEF) and the ones it needs (EXTDEF)."""
    defined, needed = set(), set()
    for kind, body in records(path):
        if kind in (0x90, 0x91):
            at = 0
            for _ in range(2):  # group and segment indexes
                at += 2 if body[at] & 0x80 else 1
            if body[at - 1] == 0 and (body[at - 2] == 0 if at >= 2 else False):
                at += 2
            step = 4 if kind == 0x91 else 2
            while at < len(body):
                name, at = name_at(body, at)
                at += step
                at += 2 if body[at] & 0x80 else 1
                defined.add(name)
        elif kind == 0x8C:
            at = 0
            while at < len(body):
                name, at = name_at(body, at)
                at += 2 if body[at] & 0x80 else 1
                needed.add(name)
    return defined, needed


def build(work: Path) -> dict[str, Path]:
    """The runtime's objects, by source name: the portable C, the target's own, and the OS layer's."""
    work.mkdir(parents=True, exist_ok=True)
    include = dosbatch.c_include(TARGET, work)
    made: dict[str, Path] = {}
    failures = []
    for source in sorted([*RUNTIME.glob("*.c"), *PLATFORM.glob("*.c"), *PLATFORM.glob("*.asm")]):
        if source.name == "qbstart.asm":
            continue
        obj = work / f"{source.stem}.obj"
        try:
            if source.suffix == ".c":
                dosbatch._host([str(dosbatch.BIN / "llrm-c"), str(source), dosbatch.m_flag(TARGET), "-Os", "-I", str(include), "-I", str(RUNTIME), "-I", str(PLATFORM), "-o", str(obj)])
            else:
                dosbatch.assemble(source, obj)
        except dosbatch.BuildError as error:
            failures.append(f"{source.name}: {str(error).splitlines()[-1][:160]}")
            continue
        made[source.stem] = obj
    if failures:
        print("not built:", *failures, sep="\n  ", file=sys.stderr)
    return made


def closure(program: Path, objects: dict[str, Path]) -> list[Path]:
    """The runtime objects a program needs: those defining what it, and what they, leave undefined."""
    owners: dict[str, str] = {}
    needs: dict[str, set[str]] = {}
    for name, obj in objects.items():
        defined, needed = symbols(obj)
        needs[name] = needed
        owners.update({symbol: name for symbol in defined})
    chosen: list[str] = []
    pending = list(symbols(program)[1])
    while pending:
        symbol = pending.pop()
        owner = owners.get(symbol)
        if owner and owner not in chosen:
            chosen.append(owner)
            pending.extend(needs[owner])
    return [objects[name] for name in chosen]


def compile_basic(source: Path, obj: Path, flags: tuple[str, ...] = ("-O2",)) -> str | None:
    done = dosbatch.subprocess.run([str(dosbatch.BIN / "llrm-qb"), str(source), "--dialect", "qb45", "-fqb-runtime=llrm", dosbatch.m_flag(TARGET), *flags, "-o", str(obj)], capture_output=True, text=True)
    return None if done.returncode == 0 else (done.stderr or done.stdout).strip()[-400:]


def link(program: Path, objects: dict[str, Path], exe: Path, work: Path) -> tuple[Path, ...]:
    return dosbatch.link_target(TARGET, program, exe, work, runtime=(START_FILES, []), objects_after=tuple(closure(program, objects)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, default=None)
    parser.add_argument("names", nargs="*")
    args = parser.parse_args()
    work = args.work or Path(tempfile.mkdtemp(prefix="qb32-"))
    work.mkdir(parents=True, exist_ok=True)
    names = args.names or list(qbruntime.MILESTONE_ONE)
    runtime = build(work / "runtime")
    jobs, wanted, loaders = [], {}, ()
    for at, name in enumerate(names):
        source = dosbatch.ROOT / "bench" / name / f"{name}.bas"
        obj = work / f"{name}.obj"
        if reason := compile_basic(source, obj):
            print(f"{name}: compile: {reason}")
            continue
        try:
            loaders = link(obj, runtime, work / f"{name}.exe", work)
        except dosbatch.BuildError as error:
            print(f"{name}: link: {str(error)[-300:]}")
            continue
        stem = f"P{at:03d}"
        jobs.append(dosbatch.Job(stem, "exe", work / f"{name}.exe", files=loaders))
        wanted[stem] = (name, (source.parent / f"{name}.out").read_bytes())
    if not jobs:
        return 1
    results = dosbatch.run(jobs, work / "run")
    bad = 0
    for job in jobs:
        name, want = wanted[job.stem]
        got = qbruntime.raw_output(work / "run", job.stem)
        same = got.replace(b"\r\n", b"\n") == want.replace(b"\r\n", b"\n")
        bad += not same
        print(f"{name:<14}{results[job.stem].status:<10}{'same' if same else 'DIFFERENT'}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
