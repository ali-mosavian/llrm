"""The llrm side of the QB tests: a BASIC program on our runtime for both targets, linked by jwlink.

The real-mode target is built with llrm-qb and llrm-c and the target's own start-up, the flat one runs under DOS/32A.  Neither
goes through Microsoft's LINK or LIB; BCOM45 (BC, LINK and its runtime) stays what a test compares against, built by
dosbatch.  The targets are the ones the link recipes name."""

from __future__ import annotations

import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbruntime  # noqa: E402

REAL, FLAT = dosbatch.REAL_MODE, qb32.TARGET
TARGETS = (REAL, FLAT)


@dataclass(frozen=True)
class Built:
    """A linked program and the files that must sit beside it (DOS/32A for the flat target)."""

    exe: Path
    files: tuple[Path, ...] = ()


def runtime16(work: Path) -> dict[str, Path]:
    """The real-mode runtime's objects by source name, from the cache where nothing it is made of has changed."""
    work.mkdir(parents=True, exist_ok=True)
    include = dosbatch.c_include(dosbatch.REAL_MODE, work)
    tree, tools = qb32._tree(), qb32._stamp("llrm-c", "jwasm")
    made: dict[str, Path] = {}

    def make(name: str, source: Path, produce) -> None:
        obj = work / f"{name}.obj"
        qb32.cached(qb32.runtime_key(source, tree, tools) + f"|{dosbatch.REAL_MODE}|{name}", obj, lambda: produce(obj))
        made[name] = obj

    shared = qbruntime.shared_sources()
    for source in qbruntime.portable_sources():
        make(source.stem, source, lambda obj, source=source: qbruntime._compile_c(source, obj, include))
    for source in [*qbruntime.platform_sources(), *shared]:
        if source.suffix == ".c":
            make(source.stem, source, lambda obj, source=source: qbruntime._compile_c(source, obj, include))
        elif source not in shared:
            make(source.stem, source, lambda obj, source=source: dosbatch.assemble(source, obj))
    for source in shared:
        for group, bit in qbruntime.os_group_bits(source).items():
            make(
                f"{source.stem}_{group}",
                source,
                lambda obj, source=source, bit=bit: dosbatch.assemble(source, obj, *dosbatch.os_defines(dosbatch.REAL_MODE, "c"), f"OS_GROUPS={bit}"),
            )
    return made


def runtimes(work: Path) -> dict[str, dict[str, Path]]:
    """Both targets' runtime objects."""
    return {REAL: runtime16(work / "real"), FLAT: qb32.build(work / "flat")}


def build(source: Path, work: Path, objects: dict[str, dict[str, Path]], flags: tuple[str, ...] = ("-O2",)) -> dict[str, Built]:
    """`source` compiled and jwlink-linked for each target against `objects` (from `runtimes`)."""
    work.mkdir(parents=True, exist_ok=True)
    built = {}
    for target in TARGETS:
        obj, exe = work / f"{source.stem}.{target}.obj", work / f"{source.stem}.{target}.EXE"
        extra = ["-march=i486", dosbatch.m_flag(FLAT)] if target == FLAT else []
        done = dosbatch.subprocess.run(
            [str(dosbatch.BIN / "llrm-qb"), str(source), "--dialect", "qb45", "-fqb-runtime=llrm", *qbruntime.frontend_flags(source), *flags, *extra, "-o", str(obj)],
            capture_output=True,
            text=True,
        )
        if done.returncode != 0 or not obj.exists():
            raise dosbatch.BuildError(f"{source.name} {target}: {(done.stderr or done.stdout).strip()[-400:]}")
        if target == FLAT:
            built[target] = Built(exe, tuple(qb32.link(obj, objects[target], exe, work)))
        else:
            link16(obj, objects[target], exe)
            built[target] = Built(exe)
    return built


def link16(program: Path, objects: dict[str, Path], exe: Path) -> None:
    command = [str(dosbatch.BIN / "jwlink"), "option", "quiet", "format", "dos", "name", str(exe)]
    for one in (program, *qb32.closure(program, objects)):
        command += ["file", str(one)]
    dosbatch._host(command)
