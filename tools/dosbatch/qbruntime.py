"""Build and inspect the real-mode QB runtime archive.

The frontend owns the object ABI.  This module owns only the archive selected
at LINK time and the evidence collected while replacing BCOM45.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import cpulimit

import dosbatch


RUNTIME = dosbatch.ROOT / "runtime" / "qb"
ARCHIVE_NAME = "LLRMQB.LIB"
DEMO_NAMES = ("NIBBLES.BAS", "GORILLA.BAS")
MILESTONE_ONE = (
    "bintree", "crc", "fib", "floats", "fpbench", "frames", "grep", "hanoi", "histo", "huge", "lru", "mandel",
    "matmul", "nbody", "nbody_fixed", "nbody_single", "particle", "queens", "quicksort", "ring", "scroll", "shellsort",
    "sieve", "textfill", "tile",
)
UNDEFINED = re.compile(r"(?:Unresolved external(?: symbol)?\s+|error L2029\s*:\s*')(B\$[^'\s:]+)", re.IGNORECASE)


def library(runtime: str, candidate: Path, empty: Path) -> Path | None:
    """The archive a differential side links, or None for the reference."""
    choices = {"bcom45": None, "llrmqb": candidate, "empty": empty}
    try:
        return choices[runtime]
    except KeyError as error:
        raise ValueError(f"unknown QB runtime '{runtime}'") from error


# A run-time error message names the address of the statement that raised it, which depends on the
# code each compiler generated: the two are not compared.
ERROR_ADDRESS = re.compile(rb"(in module \S+ at address )[0-9A-F]{4}:[0-9A-F]{4}")


def first_byte_difference(want: bytes, got: bytes) -> str:
    """The first raw-output difference, including whitespace and line endings, but for the address in a
    run-time error message."""
    want, got = ERROR_ADDRESS.sub(rb"\1....:....", want), ERROR_ADDRESS.sub(rb"\1....:....", got)
    for at, (left, right) in enumerate(zip(want, got), 1):
        if left != right:
            return f"byte {at}: want {left} got {right}"
    if len(want) != len(got):
        at = min(len(want), len(got)) + 1
        left = want[at - 1] if at <= len(want) else "<end>"
        right = got[at - 1] if at <= len(got) else "<end>"
        return f"byte {at}: want {left} got {right}"
    return ""


def screen_samples(work: Path) -> tuple[tuple[str, ...], ...]:
    """DOS text screens sampled at job boundaries."""
    events = work / "events.txt"
    if not events.is_file():
        return ()
    return tuple(
        tuple(json.loads(line)["rows"])
        for line in events.read_text().splitlines()
        if line.startswith('{"ev":"screen"')
    )


def screen_delta(before: tuple[str, ...], after: tuple[str, ...]) -> tuple[str, ...]:
    """Cells the program changed, with unchanged cells masked independently on each side."""
    rows = []
    for row in range(max(len(before), len(after))):
        old = before[row] if row < len(before) else ""
        new = after[row] if row < len(after) else ""
        cells = []
        for column in range(max(len(old), len(new))):
            previous = old[column] if column < len(old) else "\0"
            current = new[column] if column < len(new) else "\uffff"
            cells.append(current if previous != current else "\0")
        rows.append("".join(cells))
    return tuple(rows)


def screen_changes(work: Path, job: int = -1, whole: bool = False) -> tuple[str, ...] | None:
    """The DOS screen a job left, relative to the one before it (job 0 follows the build session), or all of
    it for a program that draws the screen itself and starts by clearing it."""
    screens = screen_samples(work)
    after = job + 1 if job >= 0 else len(screens) - 1
    if not 0 < after < len(screens):
        return None
    if not whole:
        return screen_delta(screens[after - 1], screens[after])
    return screens[after]


def first_screen_difference(want: tuple[str, ...], got: tuple[str, ...]) -> str:
    """The first differing text-mode screen cell, without line normalization."""
    for row, (left, right) in enumerate(zip(want, got), 1):
        for column, (wanted, actual) in enumerate(zip(left, right), 1):
            if wanted != actual:
                return f"cell {row}:{column}: want {wanted!r} got {actual!r}"
        if len(left) != len(right):
            return f"row {row}: want {len(left)} cells got {len(right)}"
    if len(want) != len(got):
        return f"screen: want {len(want)} rows got {len(got)}"
    return ""


def raw_output(work: Path, stem: str) -> bytes:
    """The exact redirected bytes for a completed DOS job."""
    for name in (f"{stem}.TXT", f"{stem.upper()}.TXT", f"{stem.lower()}.txt"):
        path = work / name
        if path.is_file():
            return path.read_bytes()
    return b""


def exe_size(work: Path, stem: str) -> int:
    """The bytes of the EXE a job linked, or 0."""
    exe = work / f"{stem}.EXE"
    return exe.stat().st_size if exe.is_file() else 0


def dos_stem(stem: str) -> str:
    """The one 8.3 basename used by a differential job and its artifacts."""
    if not stem or any(not (character.isascii() and (character.isalnum() or character == "_")) for character in stem):
        raise ValueError(f"DOS stem is not an ASCII basename: {stem!r}")
    return stem[:8]


@dataclass(frozen=True)
class Differential:
    """The two runs of one llrm-qb object and their raw-output difference."""

    reference: dosbatch.Result
    candidate: dosbatch.Result
    difference: str
    screen_difference: str
    # The linked EXEs, BCOM45's and LLRMQB's, in bytes.
    sizes: tuple[int, int] = (0, 0)


# Cells that hold something different on every run: NIBBLES and GORILLA scatter sparkles over their
# introductions at random, so their asterisks are not compared.
RANDOM_CELLS = {"NIBBLES": str.maketrans("*", " "), "GORILLA": str.maketrans("*", " ")}


def draws_screen(name: str) -> bool:
    """Whether a program draws the screen, and so runs with standard output on it: the probes named screen_*
    and the demos."""
    return name.startswith("screen_") or name.upper() in ("NIBBLES", "GORILLA")


# The emulated time a demo is given: it waits for a key once its introduction is drawn.
DEMO_BUDGET_MS = 6000


# Probes that change screen mode many times, or play music, and take longer than the default.
SLOW_PROBES = {"color_forms", "color_pairs", "color_ranges", "color_masks", "play_waits", "play_queue"}
SLOW_BUDGET_MS = 600_000


def budget(name: str) -> int | None:
    """The time a program is given, where it is not the default."""
    if name.upper() in ("NIBBLES", "GORILLA"):
        return DEMO_BUDGET_MS
    return SLOW_BUDGET_MS if name in SLOW_PROBES else None


def typed_input(name: str) -> bytes | None:
    """The keys a program is given: its probe's tests/qbrt/<name>.in, if it has one."""
    path = dosbatch.ROOT / "tests" / "qbrt" / f"{name}.in"
    return path.read_bytes() if path.is_file() else None


def masked(name: str, screen: tuple[str, ...] | None) -> tuple[str, ...] | None:
    """A screen without the cells that differ on every run."""
    table = RANDOM_CELLS.get(name.upper())
    return screen if table is None or screen is None else tuple(row.translate(table) for row in screen)


def chunked(names: list[str], chunks: int) -> list[list[str]]:
    """`names` in `chunks` runs of neighbours (fewer when there are fewer names)."""
    size = max(1, -(-len(names) // max(1, chunks)))
    return [names[at : at + size] for at in range(0, len(names), size)]


def differential_batch(
    objects: dict[str, tuple[Path, ...]], candidates: dict[str, dict], work: Path, screens: set[str] | None = None, chunks: int = 1
) -> dict[str, dict[str, Differential]]:
    """Each program as BCOM45 builds it against ours at each target.

    `objects[name]`: the object of the source built for the Microsoft runtime (`qb45`, linked with BCOM45 by Microsoft's
    LINK: the oracle), then objects it links besides.  `candidates[target][name]`: a qbbuild.Built.  Raw bytes are compared
    at every target, the screens (what the program drew) at the targets in `screens`: a flat program runs behind DOS/32A's
    banner, which is on the screen.

    The programs go in `chunks` runs of neighbours, the runs at once. A run's sessions (the oracle, then each target) share
    one directory `work/cK/run` and run in the same order, so the mount lines on the DOS screen are the same text on both
    sides of a comparison. Jobs are named J000, J001, ...: a source name is not always an 8.3 basename, or unique in 8
    characters.
    """
    names = {name: f"J{at:03d}" for at, name in enumerate(objects)}
    # V: is mounted when any job of the session has a screen compared, and the mount lines are on that screen: a run's share of the
    # programs must mount it as the whole set would, or its screens differ from the oracle's (which always mounts it) in those lines.
    mounted = {target: any(draws_screen(n) for n in objects if not isinstance(built[n], str)) for target, built in candidates.items()}

    def sessions(group: list[str], run: Path):
        def session(pairs: list[tuple[str, dosbatch.Job]], tools: bool | None = None):
            results = dosbatch.run([job for _, job in pairs], run, mount_tools=tools)
            return {
                name: (results[job.stem], raw_output(run, job.stem), masked(name, screen_changes(run, at, draws_screen(name))), exe_size(run, job.stem))
                for at, (name, job) in enumerate(pairs)
            }

        ref = session([(n, dosbatch.Job(names[n], "obj", objects[n][0], objects=extras(objects[n][1:], False), screen=draws_screen(n), stdin=typed_input(n), budget_ms=budget(n))) for n in group])
        got = {}
        for target, built in candidates.items():
            got[target] = session(
                [
                    (n, dosbatch.Job(names[n], "exe", built[n].exe, files=built[n].files, screen=draws_screen(n), stdin=typed_input(n), budget_ms=budget(n)))
                    for n in group
                    if not isinstance(built[n], str)
                ],
                tools=mounted[target],
            )
        return ref, got

    groups = chunked(list(objects), chunks)
    with ThreadPoolExecutor(max_workers=len(groups)) as pool:
        ran = list(pool.map(lambda at_group: sessions(at_group[1], work / f"c{at_group[0]}" / "run"), enumerate(groups)))
    reference = {n: v for ref, _ in ran for n, v in ref.items()}
    done = {target: {n: v for _, got in ran for n, v in got[target].items()} for target in candidates}
    found: dict[str, dict[str, Differential]] = {name: {} for name in objects}
    for target, built in candidates.items():
        for name in objects:
            if isinstance(built[name], str):  # it did not build: why
                found[name][target] = Differential(reference[name][0], dosbatch.Result("not built", detail=built[name]), "a differential side did not complete", "", (reference[name][3], 0))
                continue
            (want, want_bytes, want_screen, want_size), (got, got_bytes, got_screen, got_size) = reference[name], done[target][name]
            sizes = (want_size, got_size)
            if want.status != "ok" or got.status != "ok":
                found[name][target] = Differential(want, got, "a differential side did not complete", "", sizes)
            elif screens is not None and target not in screens:
                found[name][target] = Differential(want, got, first_byte_difference(want_bytes, got_bytes), "", sizes)
            elif want_screen is None or got_screen is None:
                found[name][target] = Differential(want, got, first_byte_difference(want_bytes, got_bytes), "screen capture unavailable", sizes)
            else:
                found[name][target] = Differential(
                    want, got, first_byte_difference(want_bytes, got_bytes), first_screen_difference(want_screen, got_screen), sizes
                )
    return found


def frontend_flags(source: Path) -> list[str]:
    """The llrm-qb options a source's `' flags:` line asks for (the rest are for other compilers)."""
    for line in source.read_text(encoding="latin-1").splitlines()[:5]:
        if line.startswith("' flags:"):
            return [word for word in line.split()[2:] if word in ("--huge-arrays", "--error-lines", "-fexcess-precision=standard")]
    return []


def linked_objects(source: Path, work: Path, runtime: str = "qb45", flags: tuple[str, ...] = (), tag: str = "") -> tuple[Path, ...]:
    """The objects a source's `' link:` line names, built here: a .nib library by llrm-nib, for the runtime
    (`qb45` or `llrm`) the program is built for, with `flags` (a target) and the file name marked with `tag`."""
    made = []
    for line in source.read_text(encoding="latin-1").splitlines()[:5]:
        if not line.startswith("' link:"):
            continue
        for name in line.split()[2:]:
            library = source.parent / name
            obj = work / f"{library.stem}.{runtime}{tag}.obj"
            done = subprocess.run(
                dosbatch.with_isel(
                    [str(dosbatch.BIN / "llrm-nib"), str(library), "-o", str(obj), "-O2", f"--basic-runtime={runtime}", *flags]
                ),
                capture_output=True,
                text=True,
            )
            if done.returncode != 0:
                raise dosbatch.BuildError(f"{name}: {(done.stderr or done.stdout).strip()[-600:]}")
            made.append(obj)
    return tuple(made)


def linked_pairs(source: Path, work: Path) -> tuple[tuple[Path, Path], ...]:
    """The libraries a source names, each built for BCOM45 and for the llrm runtime: (qb45, llrm) objects."""
    return tuple(zip(linked_objects(source, work, "qb45"), linked_objects(source, work, "llrm")))


def extras(items: tuple, candidate: bool) -> tuple[Path, ...]:
    """The objects a build links besides its program's: an item is one object, or a (reference, candidate)
    pair for a library built for each runtime."""
    return tuple((one[candidate] if isinstance(one, tuple) else one) for one in items)


def compile_basic(source: Path, obj: Path, runtime: str) -> str | None:
    """`source` as llrm-qb compiles it for `runtime` (qb45 or llrm); the reason it did not, or None."""
    done = cpulimit.run(
        [
            str(dosbatch.BIN / "llrm-qb"),
            str(source),
            "--dialect",
            "qb45",
            f"-fqb-runtime={runtime}",
            "-O2",
            *frontend_flags(source),
            "-o",
            str(obj),
        ],
        300,
        capture_output=True,
        text=True,
    )
    return None if done.returncode == 0 and obj.exists() else "compile: " + (done.stderr or done.stdout).strip()[-600:]


def undefined_symbols(link_log: str) -> list[str]:
    """The B$ entries Microsoft LINK actually reports for an empty archive."""
    return sorted({match.group(1) for match in UNDEFINED.finditer(link_log) if match.group(1).upper().startswith("B$")})


def milestone_sources() -> list[Path]:
    """The fixed first milestone corpus, refusing a renamed or missing source."""
    sources = [dosbatch.ROOT / "bench" / name / f"{name}.bas" for name in MILESTONE_ONE]
    missing = [str(source.relative_to(dosbatch.ROOT)) for source in sources if not source.is_file()]
    if missing:
        raise FileNotFoundError("missing milestone-1 BASIC source: " + ", ".join(missing))
    return sources


def demo_sources(directory: Path | None = None) -> tuple[dict[str, Path], str]:
    """The untracked Microsoft demo inputs, or the single loud skip reason."""
    root = directory or Path(os.environ.get("QB45_DEMOS_DIR", ""))
    if not root.is_dir():
        return {}, "QB45_DEMOS_DIR is unset or lacks NIBBLES.BAS and GORILLA.BAS"
    found = {path.name.upper(): path for path in root.rglob("*.BAS") if path.name.upper() in DEMO_NAMES}
    if set(found) != set(DEMO_NAMES):
        return {}, "QB45_DEMOS_DIR is unset or lacks NIBBLES.BAS and GORILLA.BAS"
    return found, ""


def os_directory() -> Path:
    """The OS layer's directory for the real-mode target: runtime/shared/<os>/<mode>."""
    return Path(dosbatch.os_layer(dosbatch.REAL_MODE, "directory", "c"))


def platform_directory() -> Path:
    """The QB runtime's own code for that target: runtime/qb/<os>/<mode>."""
    layer = os_directory()
    return RUNTIME / layer.parent.name / layer.name


def os_group_bits(source: Path) -> dict[str, int]:
    """The groups the OS layer's assembly holds a bit each (its G_<GROUP> equates): the archive has an
    object for each, so a program links what it calls."""
    found = re.findall(r"^G_(\w+)\s+equ\s+(\d+)", source.read_text(), re.M)
    return {name.lower(): int(bit) for name, bit in found}


def device_directory(platform: Path | None = None) -> Path:
    """The QB device interface's code for that target: runtime/qb/dev/<os>/<mode> (the DOS BIOS, VGA, speaker and clock)."""
    platform = platform or platform_directory()
    return RUNTIME / "dev" / platform.parent.name / platform.name


def device_includes(platform: Path | None = None) -> list[str]:
    """The compiler's -I arguments for the device interface: dev.h, the target's devsym.h and device.h, and the code both targets share."""
    where = device_directory(platform)
    return ["-I", str(RUNTIME / "dev"), "-I", str(where), "-I", str(where.parent)]


def shared_sources() -> list[Path]:
    return [os_directory() / dosbatch.os_layer(dosbatch.REAL_MODE, "implementation", "c")]


def portable_sources() -> list[Path]:
    """One object per portable module: a program links only the modules it uses."""
    return sorted(RUNTIME.glob("*.c"))


def platform_sources_of(platform: Path) -> list[Path]:
    """A target's own code: its C and its assembly, and its device interface's."""
    device = device_directory(platform)
    return sorted([*platform.glob("*.c"), *platform.glob("*.asm"), *device.glob("*.c"), *device.glob("*.asm"), *device.parent.glob("*.c")])


def platform_sources() -> list[Path]:
    """The target's own code: its C and its assembly."""
    return platform_sources_of(platform_directory())


# The row the runtime's C is built on; the object cache names it.
RUNTIME_ROW = "x86_16-wat"


def c_flags() -> list[str]:
    # No -I include: a directory's llrm_os.h has a plain `__cdecl` the fork clang ignores (borland.h erases it); the row's own header has the attribute that keeps the OS layer's stack convention.
    # An empty BSSB ahead of each object's _BSS: start.asm zeroes from it (the objects' order is the linker's).
    return ["-mbss-begin", "-I", str(RUNTIME), "-I", str(platform_directory()), *device_includes()]


def c_build_key() -> str:
    """What the runtime's C is compiled with, for the object cache: the row and the command dosbatch.c_compile makes of it, the checkout's own path left out."""
    command, env = dosbatch.c_compile(Path("runtime.c"), Path("runtime.obj"), dosbatch.REAL_MODE, "-Os", None, [], c_flags())  # the command, as the builds make it
    return RUNTIME_ROW + " " + " ".join(command[1:]).replace(str(dosbatch.ROOT), "") + " " + repr(sorted(env.items())) + " " + dosbatch.clang_identity()


def _compile_c(source: Path, obj: Path, include: Path) -> None:
    dosbatch._host(*dosbatch.c_compile(source, obj, dosbatch.REAL_MODE, "-Os", None, [], c_flags()))


def remove_existing_archive(work: Path) -> None:
    """Make LIB construct the archive rather than retaining stale modules."""
    existing = work / ARCHIVE_NAME
    if existing.exists():
        existing.unlink()


def archive(objects: list[Path], output: Path, work: Path, seconds: int = 300) -> None:
    """Use QB45's librarian, not a host archive writer, for a LINK-compatible OMF library; the emulator gets `seconds`
    of CPU time."""
    work.mkdir(parents=True, exist_ok=True)
    remove_existing_archive(work)
    copied = []
    for at, object_ in enumerate(objects):
        target = work / f"R{at:03d}.OBJ"
        shutil.copyfile(object_, target)
        copied.append(target.name)
    (work / "LIB.RSP").write_text("\r\n".join([f"C:\\{ARCHIVE_NAME}", "y", *(f"+C:\\{name} &" for name in copied[:-1]), f"+C:\\{copied[-1]};"]) + "\r\n")
    commands = [f"mount c {work}", f"mount v {dosbatch.QB45}", "c:", r"V:\LIB.EXE @C:\LIB.RSP", "."]
    (work / "jobs.txt").write_text("\n".join(commands) + "\n")
    (work / "job.conf").write_text(dosbatch.CONF)
    events = work / "events.txt"
    with (work / "jobs.txt").open() as stdin, events.open("w") as sink:
        # CPU seconds, not the wall clock: LIB.EXE takes the same CPU time on a host shared by thirty gates, and many times the
        # wall time.
        cpulimit.run(
            [str(dosbatch.DOSBOX), "-nolog", "-conf", str(work / "job.conf")],
            seconds,
            stdin=stdin,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            pass_fds=(sink.fileno(),),
            env={**os.environ, "SDL_VIDEODRIVER": "dummy", "DOSRUN_FD": str(sink.fileno())},
        )
    made = work / ARCHIVE_NAME
    if not made.is_file() or made.stat().st_size == 0:
        raise dosbatch.BuildError(f"LIB.EXE made no {ARCHIVE_NAME}: {events.read_text()[-1500:]}")
    shutil.copyfile(made, output)


def build(directory: Path) -> tuple[Path, Path]:
    """Build LLRMQB.LIB and its manifest in `directory`."""
    directory.mkdir(parents=True, exist_ok=True)
    include = dosbatch.c_include(dosbatch.REAL_MODE, directory)
    objects: list[Path] = []
    for source in portable_sources():
        obj = directory / f"{source.stem}.obj"
        _compile_c(source, obj, include)
        objects.append(obj)
    shared = shared_sources()
    for source in [*platform_sources(), *shared]:
        obj = directory / f"{source.stem}.obj"
        if source.suffix == ".c":
            _compile_c(source, obj, include)
        elif source in shared:
            continue
        else:
            dosbatch.assemble(source, obj)
        objects.append(obj)
    for source in shared:
        for group, bit in os_group_bits(source).items():
            obj = directory / f"{source.stem}_{group}.obj"
            dosbatch.assemble(source, obj, *dosbatch.os_defines(dosbatch.REAL_MODE, "qb"), f"OS_GROUPS={bit}")
            objects.append(obj)
    output = directory / ARCHIVE_NAME
    archive(objects, output, directory / "lib")
    manifest = directory / "LLRMQB.json"
    manifest.write_text(
        json.dumps(
            {
                "archive": output.name,
                "bytes": output.stat().st_size,
                "objects": {obj.name: obj.stat().st_size for obj in objects},
            },
            indent=2,
        )
        + "\n"
    )
    return output, manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, help="directory for LLRMQB.LIB and LLRMQB.json")
    args = parser.parse_args()
    output, manifest = build(args.out or Path(tempfile.mkdtemp(prefix="llrm-qb-runtime-")))
    print(output)
    print(manifest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
