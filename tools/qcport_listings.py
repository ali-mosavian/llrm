"""Create reproducible BCC and qbopt listings for matching QCport C sources.

The linked QCPORT executable has function symbols but no safe source-line to
instruction map.  This tool records the two compiler listings *from one clean
QCport revision* before any loop comparison is attempted:

    uv run python tools/qcport_listings.py /path/to/clean/qcport \
        build/qcport-listings src/render/r_walk.c

The BCC side uses QCport's own `tools/bcc.sh` with `LIST=1`; the qbopt side
uses the same Watcom-front-end flags as the normal QCport wrapper.  The JSON
manifest binds every listing to the source hash, QCport commit, qbopt commit,
CPU profile, and listing hash.  A dirty source tree is rejected deliberately:
matching an old object to edited source is not a measurement.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INCLUDE_DIRS = (
    "src/host",
    "src/render",
    "src/model",
    "src/game",
    "src/sound",
    "src/ui",
    "src/qgl",
)
BCPP_INCLUDE = Path("/Users/alim/work/other/d32x/toolchains/bcpp31/INCLUDE")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(root: Path, *arguments: str) -> str:
    done = subprocess.run(["git", "-C", str(root), *arguments], capture_output=True, text=True)
    if done.returncode:
        raise RuntimeError(done.stderr.strip() or f"git {' '.join(arguments)} failed")
    return done.stdout


def clean_revision(root: Path) -> str:
    """Return the immutable source revision or reject an unprovable input."""
    changed = git(root, "status", "--porcelain=v1")
    if changed:
        raise RuntimeError(
            f"QCport source tree is dirty ({root}); create a clean worktree before collecting listings"
        )
    return git(root, "rev-parse", "HEAD").strip()


def source_path(root: Path, relative: str) -> Path:
    candidate = (root / relative).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError as error:
        raise ValueError(f"source escapes QCport root: {relative}") from error
    if candidate.suffix != ".c" or not candidate.is_file():
        raise ValueError(f"not a QCport C source: {relative}")
    return candidate


def _output_path(root: Path, kind: str, source: Path, suffix: str) -> Path:
    relative = source.relative_to(root).with_suffix(suffix)
    return root / kind / relative


def compile_one(qcport: Path, output: Path, source: Path, cpu: str) -> dict:
    bcc = output / "bcc" / source.relative_to(qcport).with_suffix(".asm")
    qbopt = output / "qbopt" / source.relative_to(qcport).with_suffix(".asm")
    stages = output / "stages" / source.relative_to(qcport).with_suffix("")
    for path in (bcc, qbopt):
        path.parent.mkdir(parents=True, exist_ok=True)
    stages.mkdir(parents=True, exist_ok=True)

    environment = {**os.environ, "LIST": "1"}
    subprocess.run([str(qcport / "tools" / "bcc.sh"), str(source), str(bcc)], check=True, env=environment)
    command = [
        sys.executable,
        "-m",
        "qbopt.cfront",
        "--opt",
        "--cpu",
        cpu,
        "--dump",
        str(stages),
        *(item for directory in INCLUDE_DIRS for item in ("-I", str(qcport / directory))),
        "-I",
        str(BCPP_INCLUDE),
        str(source),
        "-o",
        str(qbopt),
    ]
    subprocess.run(command, check=True, cwd=ROOT)
    return {
        "source": str(source.relative_to(qcport)),
        "source_sha256": digest(source),
        "bcc_listing": str(bcc),
        "bcc_listing_sha256": digest(bcc),
        "qbopt_listing": str(qbopt),
        "qbopt_listing_sha256": digest(qbopt),
        "stage_dump": str(stages),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="qcport-listings", description=__doc__.splitlines()[0])
    parser.add_argument("qcport", type=Path, help="a clean QCport worktree")
    parser.add_argument("output", type=Path, help="directory for paired listings and manifest")
    parser.add_argument("sources", nargs="+", help="C paths relative to the QCport root")
    parser.add_argument("--cpu", default="386", choices=("386", "486", "P5", "P6", "K5", "K6", "K7", "Core"))
    args = parser.parse_args(argv)

    qcport = args.qcport.resolve()
    revision = clean_revision(qcport)
    sources = [source_path(qcport, relative) for relative in args.sources]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    entries = [compile_one(qcport, output, source, args.cpu) for source in sources]
    manifest = {
        "schema": 1,
        "purpose": "paired source-identical QCport BCC/qbopt listing evidence",
        "qcport_revision": revision,
        "qbopt_revision": git(ROOT, "rev-parse", "HEAD").strip(),
        "cpu": args.cpu,
        "bcc_command": "QCport tools/bcc.sh with LIST=1 (-3 -f87 -mm -Ox)",
        "qbopt_command": "qbopt.cfront --opt with the QCport medium-model include set",
        "entries": entries,
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(output / "manifest.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
