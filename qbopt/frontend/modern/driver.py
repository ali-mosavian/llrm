"""Invoke the isolated Rust frontend and decode its common-HIR document."""

import os
import subprocess
from pathlib import Path

from qbopt import hir

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / "frontends" / "modern" / "Cargo.toml"


class FrontendError(ValueError):
    """Source parsing or semantic analysis failed above HIR."""


def command() -> tuple[str, ...]:
    configured = os.environ.get("QBOPT_MODERNFRONT")
    if configured:
        return (configured,)
    return ("cargo", "run", "--quiet", "--release", "--manifest-path", str(MANIFEST), "--")


def parsed(source: Path, *, dump: Path | None = None) -> hir.Program:
    try:
        result = subprocess.run(
            (*command(), str(source)),
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise FrontendError(f"could not start modern frontend: {error}") from error
    if result.returncode:
        message = result.stderr.strip() or f"modernfront exited with status {result.returncode}"
        raise FrontendError(message)
    if dump is not None:
        dump.write_text(result.stdout)
    try:
        return hir.decode(result.stdout)
    except (ValueError, TypeError) as error:
        raise FrontendError(f"modernfront emitted invalid HIR: {error}") from error
