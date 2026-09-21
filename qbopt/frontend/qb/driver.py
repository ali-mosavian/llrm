"""Invoke the isolated Rust parser and decode its common-HIR document."""

import os
import json
import subprocess
from pathlib import Path

from qbopt import hir

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "Cargo.toml"
DIALECTS = frozenset(
    one.value
    for one in (
        hir.Dialect.QBASIC11,
        hir.Dialect.QB45,
        hir.Dialect.PDS71,
        hir.Dialect.VBDOS,
    )
)
RUNTIMES = frozenset(
    one.value
    for one in (
        hir.RuntimeProfile.QB45,
        hir.RuntimeProfile.PDS71,
        hir.RuntimeProfile.VBDOS,
    )
)
ARRAY_ORDERS = frozenset(one.value for one in hir.ArrayOrder)


class FrontendError(ValueError):
    """Source parsing or semantic analysis failed above HIR."""


def command() -> tuple[str, ...]:
    """The configured installed producer, or the in-tree Cargo executable."""
    configured = os.environ.get("QBOPT_QBFRONT")
    if configured:
        return (configured,)
    # Do not execute target/release/qbfront directly merely because it exists:
    # that made stage dumps silently use an older semantic frontend after a
    # source edit. Cargo's own dependency check is cheap when the build is
    # current and authoritative when it is not.
    return ("cargo", "run", "--quiet", "--release", "--manifest-path", str(MANIFEST), "--")


def build_release() -> Path:
    try:
        result = subprocess.run(
            (
                "cargo",
                "build",
                "--quiet",
                "--release",
                "--manifest-path",
                str(MANIFEST),
                "--bin",
                "qbfront",
                "--message-format=json",
            ),
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise FrontendError(f"could not start cargo build: {error}") from error
    if result.returncode:
        message = result.stderr.strip() or f"cargo build exited with status {result.returncode}"
        raise FrontendError(message)

    for line in result.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError as error:
            raise FrontendError("cargo build emitted invalid JSON") from error
        target = message.get("target", {})
        if (
            message.get("reason") == "compiler-artifact"
            and target.get("name") == "qbfront"
            and "bin" in target.get("kind", ())
            and (executable := message.get("executable"))
        ):
            return Path(executable)
    raise FrontendError("cargo build did not report the qbfront executable")


def _options(
    source: Path,
    *,
    dialect: str,
    runtime: str,
    include_dirs: tuple[Path, ...],
    array_order: str,
    huge_arrays: bool,
    checked_arrays: bool,
    mbf: bool,
    alternate_math: bool,
) -> tuple[str, ...]:
    if dialect not in DIALECTS:
        raise FrontendError(f"unknown QB dialect {dialect!r}")
    if runtime not in RUNTIMES:
        raise FrontendError(f"unknown QB runtime {runtime!r}")
    if array_order not in ARRAY_ORDERS:
        raise FrontendError(f"unknown QB array order {array_order!r}")
    return (
        "--dialect",
        dialect,
        "--runtime",
        runtime,
        "--array-order",
        array_order,
        *(("--huge-arrays",) if huge_arrays else ()),
        *(("--checked-arrays",) if checked_arrays else ()),
        *(("--mbf",) if mbf else ()),
        *(("--alternate-math",) if alternate_math else ()),
        *(part for directory in include_dirs for part in ("--include", str(directory))),
        str(source),
    )


def syntax_checked(
    source: Path,
    *,
    dialect: str = "vbdos",
    runtime: str = "vbdos",
    include_dirs: tuple[Path, ...] = (),
    array_order: str = "column-major",
    huge_arrays: bool = False,
    checked_arrays: bool = False,
    mbf: bool = False,
    alternate_math: bool = False,
) -> None:
    """Run only source loading and parsing, independently of semantic HIR support."""
    result = subprocess.run(
        (
            *command(),
            "--syntax",
            *_options(
                source,
                dialect=dialect,
                runtime=runtime,
                include_dirs=include_dirs,
                array_order=array_order,
                huge_arrays=huge_arrays,
                checked_arrays=checked_arrays,
                mbf=mbf,
                alternate_math=alternate_math,
            ),
        ),
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode:
        message = result.stderr.strip() or f"qbfront exited with status {result.returncode}"
        raise FrontendError(message)


def parsed(
    source: Path,
    *,
    dialect: str = "vbdos",
    runtime: str = "vbdos",
    dump: Path | None = None,
    include_dirs: tuple[Path, ...] = (),
    array_order: str = "column-major",
    huge_arrays: bool = False,
    checked_arrays: bool = False,
    mbf: bool = False,
    alternate_math: bool = False,
) -> hir.Program:
    result = subprocess.run(
        (
            *command(),
            *_options(
                source,
                dialect=dialect,
                runtime=runtime,
                include_dirs=include_dirs,
                array_order=array_order,
                huge_arrays=huge_arrays,
                checked_arrays=checked_arrays,
                mbf=mbf,
                alternate_math=alternate_math,
            ),
        ),
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode:
        message = result.stderr.strip() or f"qbfront exited with status {result.returncode}"
        raise FrontendError(message)
    if dump is not None:
        dump.parent.mkdir(parents=True, exist_ok=True)
        dump.write_text(result.stdout)
    try:
        return hir.decode(result.stdout)
    except hir.InvalidHIR as error:
        raise FrontendError(f"qbfront produced invalid HIR: {error}") from error
