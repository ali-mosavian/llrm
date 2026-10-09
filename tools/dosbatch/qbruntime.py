"""Build and inspect the m16 QB runtime archive.

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
import tomllib
from pathlib import Path

import dosbatch


RUNTIME = dosbatch.ROOT / "runtime" / "qb"
PLATFORM = dosbatch.ROOT / "platform" / "qb" / "dos" / "m16"
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


def first_byte_difference(want: bytes, got: bytes) -> str:
    """The first raw-output difference, including whitespace and line endings."""
    for at, (left, right) in enumerate(zip(want, got), 1):
        if left != right:
            return f"byte {at}: want {left} got {right}"
    if len(want) != len(got):
        at = min(len(want), len(got)) + 1
        left = want[at - 1] if at <= len(want) else "<end>"
        right = got[at - 1] if at <= len(got) else "<end>"
        return f"byte {at}: want {left} got {right}"
    return ""


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


def source_groups() -> dict[str, list[Path]]:
    """Runtime sources, kept declarative so a manifest proves the archive inputs."""
    with (RUNTIME / "runtime.toml").open("rb") as text:
        config = tomllib.load(text)
    return {
        "portable": [RUNTIME / name for name in config["portable"]["sources"]],
        "platform": [dosbatch.ROOT / name for name in config["platform"]["sources"]],
    }


def _compile_c(source: Path, obj: Path, include: Path) -> None:
    dosbatch._host([str(dosbatch.BIN / "llrm-c"), str(source), "-m16", "-O2", "-I", str(include), "-o", str(obj)])


def archive(objects: list[Path], output: Path, work: Path) -> None:
    """Use QB45's librarian, not a host archive writer, for a LINK-compatible OMF library."""
    work.mkdir(parents=True, exist_ok=True)
    copied = []
    for at, object_ in enumerate(objects):
        target = work / f"R{at:03d}.OBJ"
        shutil.copyfile(object_, target)
        copied.append(target.name)
    commands = [f"mount c {work}", f"mount v {dosbatch.QB45}", "c:", f"V:\\LIB.EXE C:\\{ARCHIVE_NAME}" + "".join(f"+C:\\{name}" for name in copied) + ";", "."]
    (work / "jobs.txt").write_text("\n".join(commands) + "\n")
    (work / "job.conf").write_text(dosbatch.CONF)
    events = work / "events.txt"
    with (work / "jobs.txt").open() as stdin, events.open("w") as sink:
        subprocess.run(
            [str(dosbatch.DOSBOX), "-nolog", "-conf", str(work / "job.conf")],
            stdin=stdin,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            pass_fds=(sink.fileno(),),
            env={**os.environ, "SDL_VIDEODRIVER": "dummy", "DOSRUN_FD": str(sink.fileno())},
            check=True,
            timeout=300,
        )
    made = work / ARCHIVE_NAME
    if not made.is_file() or made.stat().st_size == 0:
        raise dosbatch.BuildError(f"LIB.EXE made no {ARCHIVE_NAME}: {events.read_text()[-1500:]}")
    shutil.copyfile(made, output)


def build(directory: Path) -> tuple[Path, Path]:
    """Build LLRMQB.LIB and its reproducible input manifest in `directory`."""
    directory.mkdir(parents=True, exist_ok=True)
    include = dosbatch.c_include(dosbatch.REAL_MODE, directory)
    objects: list[Path] = []
    groups = source_groups()
    for at, source in enumerate(groups["portable"]):
        obj = directory / f"C{at:03d}.OBJ"
        _compile_c(source, obj, include)
        objects.append(obj)
    for at, source in enumerate(groups["platform"]):
        obj = directory / f"A{at:03d}.OBJ"
        dosbatch.assemble(source, obj, *dosbatch.os_defines(dosbatch.REAL_MODE, "c"))
        objects.append(obj)
    output = directory / ARCHIVE_NAME
    archive(objects, output, directory / "lib")
    manifest = directory / "LLRMQB.json"
    manifest.write_text(
        json.dumps(
            {
                "archive": output.name,
                "bytes": output.stat().st_size,
                "portable": [str(path.relative_to(dosbatch.ROOT)) for path in groups["portable"]],
                "platform": [str(path.relative_to(dosbatch.ROOT)) for path in groups["platform"]],
                "objects": [path.name for path in objects],
                "exports": [],
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
