"""The llrm side of the QB tests: a BASIC program on our runtime for both targets, linked by jwlink.

The real-mode target is built with llrm-qb and llrm-cc and the target's own start-up, the flat one runs under DOS/32A.  Neither
goes through Microsoft's LINK or LIB; BCOM45 (BC, LINK and its runtime) stays what a test compares against, built by
dosbatch.  The targets are the ones the link recipes name."""

from __future__ import annotations

import sys
from dataclasses import dataclass
import shutil
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


def flat_source(source: Path) -> Path:
    """`source`'s port to the flat target, `NAME.flat.bas` beside it, where there is one: absolute addresses (video memory) are
    held in variables there, DEF SEG being ignored."""
    ported = source.with_name(f"{source.stem}.flat{source.suffix}")
    return ported if ported.exists() else source


def runtime16(work: Path) -> dict[str, Path]:
    """The real-mode runtime's objects by source name, from the cache where nothing it is made of has changed."""
    work.mkdir(parents=True, exist_ok=True)
    include = dosbatch.c_include(dosbatch.REAL_MODE, work)
    tree, tools = qb32._tree(), qb32._stamp(dosbatch.c_frontend(), "jwasm")
    made: dict[str, Path] = {}

    def make(name: str, source: Path, produce) -> None:
        obj = work / f"{name}.obj"
        qb32.cached(qb32.runtime_key(source, tree, tools) + f"|{dosbatch.REAL_MODE}|{qbruntime.c_build_key()}|{name}", obj, lambda: produce(obj))
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
                lambda obj, source=source, bit=bit: dosbatch.assemble(source, obj, *dosbatch.os_defines(dosbatch.REAL_MODE, "qb"), f"OS_GROUPS={bit}"),
            )
    return made


def runtimes(work: Path) -> dict[str, dict[str, Path]]:
    """Both targets' runtime objects."""
    return {REAL: runtime16(work / "real"), FLAT: qb32.build(work / "flat")}


def build(source: Path, work: Path, objects: dict[str, dict[str, Path]], flags: tuple[str, ...] = ("-O2",), tolerant: bool = False) -> dict[str, Built | str]:
    """`source` compiled and jwlink-linked for each target against `objects` (from `runtimes`).  A target that does not build
    raises BuildError, or with `tolerant` is the reason, as text, in its place."""
    work.mkdir(parents=True, exist_ok=True)
    built = {}
    for target in TARGETS:
        try:
            built[target] = one_target(source, work, objects, flags, target)
        except dosbatch.BuildError as error:
            if not tolerant:
                raise
            built[target] = str(error)[-400:]
    return built


def compile_arguments(source: Path, flags: tuple[str, ...], target: str, obj: Path) -> list[str]:
    """llrm-qb's arguments for `source` (already the target's port) on `target`: the one place they are spelled, for the build and for
    whatever must compile what the build compiles."""
    extra = ["-march=i486", dosbatch.m_flag(FLAT)] if target == FLAT else []
    return [str(source), "--dialect", "qb45", "-fqb-runtime=llrm", *qbruntime.frontend_flags(source), *flags, *extra, "-o", str(obj)]


def one_target(source: Path, work: Path, objects: dict[str, dict[str, Path]], flags: tuple[str, ...], target: str, name: str | None = None, listing: Path | None = None, before: tuple[str, ...] = (), image: Path | None = None) -> Built:
    """`source` for one target; its files are `name`'s (the source's stem) in `work`."""
    stem = name or source.stem
    obj, exe = work / f"{stem}.{target}.obj", work / f"{stem}.{target}.EXE"
    # a program that needs hand porting to the flat target (DEF SEG is ignored there) has NAME.flat.bas beside it
    source = flat_source(source) if target == FLAT else source
    extra = ["-march=i486", dosbatch.m_flag(FLAT)] if target == FLAT else []
    done = dosbatch.subprocess.run(dosbatch.with_isel([str(dosbatch.BIN / "llrm-qb"), *compile_arguments(source, flags, target, obj)]), capture_output=True, text=True)
    if done.returncode != 0 or not obj.exists():
        raise dosbatch.BuildError(f"{source.name} {target}: {(done.stderr or done.stdout).strip()[-400:]}")
    libs = qbruntime.linked_objects(source, work, "llrm", tuple(extra[1:]) if target == FLAT else (), f".{target}")
    if target == FLAT:
        loaders = dosbatch.link_target(
            qb32.TARGET, obj, exe, work, listing, before=before, runtime=(qb32.START_FILES, []), objects_after=(*libs, *needed((obj, *libs), objects[target])), image=image, defines=dosbatch.os_defines(qb32.TARGET, "qb")
        )
        return Built(exe, tuple(loaders))
    link16(obj, objects[target], exe, libs)
    return Built(exe)


def needed(roots: tuple[Path, ...], objects: dict[str, Path]) -> list[Path]:
    """The runtime objects the `roots` need, once each."""
    chosen: list[Path] = []
    for root in roots:
        chosen += [one for one in qb32.closure(root, objects) if one not in chosen]
    return chosen


def link16(program: Path, objects: dict[str, Path], exe: Path, libs: tuple[Path, ...] = (), listing: Path | None = None) -> None:
    """The real-mode link; `listing` is where the linker's map goes."""
    command = [str(dosbatch.BIN / "jwlink"), "option", "quiet", *(["option", f"map={listing}"] if listing else []), "format", "dos", "name", str(exe)]
    for one in (program, *libs, *needed((program, *libs), objects)):
        command += ["file", str(one)]
    dosbatch._host(command)


@dataclass(frozen=True)
class Ran:
    """A program's run: how it ended, what it wrote to standard output, the guest instructions the whole run executed (the
    emulator's end event: on virtual time, the same every run) and, for a run asked to profile, the instructions each function of
    its link map ran, itself and what it called, less what ran in code the map does not name (`inclusive`, by upper-cased name; `outside` is what was left out of it; `sized` whether the function has a length in the image's debug info, without which code after it that no symbol names is counted in it)."""

    status: str
    output: bytes
    instructions: int
    inclusive: dict[str, int]
    outside: dict[str, int] | None = None
    sized: dict[str, bool] | None = None


def run(
    built: Built,
    work: Path,
    files: tuple[Path, ...] = (),
    conf: str | None = None,
    budget_ms: int | None = None,
    stdin: bytes | None = None,
    profile: Path | None = None,
) -> Ran:
    """`built` run on its own in `work`.  With `profile`, the program's link map (`listing` of its link), the emulator reads the
    map as the program's symbols and counts by function: nothing the loader or the runtime's start-up ran is in a function's
    count, and nor is what ran in code the map does not name (DOS, the extender: their length is not the function's to own)."""
    import json
    import tempfile

    with tempfile.TemporaryDirectory() as sidecar:
        # the emulator reads NAME.MAP beside NAME.EXE: the job is J
        beside = (Path(shutil.copy(profile, Path(sidecar) / "J.MAP")),) if profile else ()
        job = dosbatch.Job("J", "exe", built.exe, files=(*built.files, *files, *beside), budget_ms=budget_ms, stdin=stdin, profile=bool(profile))
        result = dosbatch.run([job], work, **({"conf": conf} if conf else {}))["J"]
    count, inclusive, outside, sized = 0, {}, {}, {}
    for line in (work / "events.txt").read_text(errors="replace").splitlines():
        if '"ev":"end"' in line:
            count = json.loads(line).get("instructions", 0)
        elif '"ev":"profile"' in line:
            for one in json.loads(line)["functions"]:
                inclusive[one["name"].upper()] = one["inclusive"] - one["inclusive_unknown"]
                outside[one["name"].upper()] = one["inclusive_unknown"]
                sized[one["name"].upper()] = one["sized"]
    return Ran(result.status, qbruntime.raw_output(work, "J"), count, inclusive, outside, sized)
